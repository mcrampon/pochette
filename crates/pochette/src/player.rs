//! Plays albums: streams their tracks from Qobuz through GStreamer, one after the other
//! without gaps, and tells Qobuz what was listened to, which is what pays the artists.
//!
//! The player runs as a task that takes [`Command`]s and publishes its [`Status`], so any
//! part of the UI can drive it and follow it.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use futures_util::StreamExt;
use gstreamer::{self as gst, prelude::*};
use qobuz::{
    Client, FileUrl, Quality, StreamingEnd,
    models::{Album, Track},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

/// Within this much of a track's start, "previous" goes to the previous track rather than
/// back to the start of this one.
const RESTART_WITHIN: Duration = Duration::from_secs(3);

/// What the player is doing.
#[derive(Debug, Clone, Default)]
pub struct Status {
    pub state: State,
    /// The album being played, or paused.
    pub album_id: Option<String>,
    /// The track being played, or paused.
    pub track_id: Option<u64>,
    /// Why playback stopped, if it failed.
    pub error: Option<String>,
    /// The album being played, without its tracks: they're in `queue`.
    pub album: Option<Arc<Album>>,
    /// The album's tracks that can be streamed, in order.
    pub queue: Arc<[Track]>,
    /// The track being played, as an index in `queue`.
    pub index: Option<usize>,
    /// The quality of the stream being played: "24-bit / 96 kHz".
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Stopped,
    /// Fetching or buffering the stream.
    Loading,
    Playing,
    Paused,
}

impl Status {
    /// Whether `album_id` is the album playing, paused or about to play.
    pub fn is_current(&self, album_id: &str) -> bool {
        self.state != State::Stopped && self.album_id.as_deref() == Some(album_id)
    }

    /// The track being played, or paused.
    pub fn track(&self) -> Option<&Track> {
        self.queue.get(self.index?)
    }

    pub fn has_next(&self) -> bool {
        self.index.is_some_and(|index| index + 1 < self.queue.len())
    }
}

/// Drives the player. Cheap to clone: every clone drives the same player.
#[derive(Clone)]
pub struct Player {
    commands: mpsc::UnboundedSender<Command>,
    status: watch::Receiver<Status>,
    /// To read the position and set the volume, which needs no help from the actor.
    playbin: gst::Element,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("status", &*self.status.borrow())
            .finish_non_exhaustive()
    }
}

enum Command {
    PlayAlbum {
        client: Client,
        album: Box<Album>,
        /// Where to start; the first track when `None`.
        track_id: Option<u64>,
    },
    TogglePause,
    Next,
    /// Back to the start of the track, or to the previous track near its start.
    Previous,
    /// Play the track at this index in the queue.
    PlayIndex(usize),
    Seek(Duration),
    /// Stream in this quality at best, from now on: the track playing too.
    SetQuality(Quality),
    /// Stop, and answer once the last listening is reported.
    Stop(Option<oneshot::Sender<()>>),
}

impl Player {
    pub fn new() -> Result<Self, gst::glib::Error> {
        gst::init()?;
        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .map_err(|err| gst::glib::Error::new(gst::CoreError::MissingPlugin, &err.message))?;
        let (commands, receiver) = mpsc::unbounded_channel();
        let (status_sender, status) = watch::channel(Status::default());
        let actor = Actor::new(playbin.clone(), status_sender);
        relm4::spawn(actor.run(receiver));
        Ok(Self {
            commands,
            status,
            playbin,
        })
    }

    /// Play `album`, from `track_id` or from the start.
    pub fn play_album(&self, client: Client, album: Album, track_id: Option<u64>) {
        self.send(Command::PlayAlbum {
            client,
            album: Box::new(album),
            track_id,
        });
    }

    pub fn toggle_pause(&self) {
        self.send(Command::TogglePause);
    }

    pub fn next(&self) {
        self.send(Command::Next);
    }

    pub fn previous(&self) {
        self.send(Command::Previous);
    }

    /// Play the track at `index` in the queue.
    pub fn play_index(&self, index: usize) {
        self.send(Command::PlayIndex(index));
    }

    pub fn seek(&self, position: Duration) {
        self.send(Command::Seek(position));
    }

    /// Stream in `quality` at best, the track playing included.
    pub fn set_quality(&self, quality: Quality) {
        self.send(Command::SetQuality(quality));
    }

    /// Where playback is in the current track.
    pub fn position(&self) -> Option<Duration> {
        position(&self.playbin)
    }

    /// The volume, from 0 to 1, on the cubic scale people hear, as sliders show it.
    pub fn volume(&self) -> f64 {
        self.playbin.property::<f64>("volume").cbrt()
    }

    pub fn set_volume(&self, volume: f64) {
        self.playbin
            .set_property("volume", volume.clamp(0.0, 1.0).powi(3));
    }

    pub fn stop(&self) {
        self.send(Command::Stop(None));
    }

    /// Stop, and wait for Qobuz to hear about the listening so far, as before quitting.
    pub async fn shut_down(&self) {
        let (done, wait) = oneshot::channel();
        self.send(Command::Stop(Some(done)));
        let _ = wait.await;
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    fn send(&self, command: Command) {
        // The actor only ends with the app.
        let _ = self.commands.send(command);
    }
}

/// The next track's stream, ready for the player to switch to without a gap.
struct Upcoming {
    index: usize,
    file: FileUrl,
}

struct Actor {
    playbin: gst::Element,
    status: watch::Sender<Status>,
    client: Option<Client>,
    /// The album playing, without its tracks.
    album: Option<Arc<Album>>,
    /// The album's tracks that can be streamed, in order.
    queue: Arc<[Track]>,
    /// The quality of the stream playing.
    quality: Option<String>,
    /// The best quality to ask Qobuz for.
    best_quality: Quality,
    /// Where to go in the track starting, once it can seek: where it was before its
    /// stream changed.
    resume_at: Option<Duration>,
    /// The track playing, as an index in the queue.
    current: Option<usize>,
    /// The track asked to play, until its stream starts.
    starting: Option<Upcoming>,
    /// The next track, once its stream is known.
    upcoming: Arc<Mutex<Option<Upcoming>>>,
    /// The next track once GStreamer moved to it, until its stream starts.
    switched: Arc<Mutex<Option<Upcoming>>>,
    prefetch: Option<JoinHandle<()>>,
    /// Whether the user wants sound: buffering pauses the stream, but not the user.
    wants_playing: bool,
    report: Option<Report>,
    /// Reports of tracks no longer playing that may still be on their way to Qobuz.
    sending: Vec<JoinHandle<()>>,
}

impl Actor {
    fn new(playbin: gst::Element, status: watch::Sender<Status>) -> Self {
        let upcoming: Arc<Mutex<Option<Upcoming>>> = Arc::default();
        let switched: Arc<Mutex<Option<Upcoming>>> = Arc::default();

        // Called on GStreamer's streaming thread as the current track nears its end:
        // queuing the next stream now is what makes playback gapless.
        playbin.connect("about-to-finish", false, {
            let upcoming = upcoming.clone();
            let switched = switched.clone();
            move |values| {
                let playbin = values[0].get::<gst::Element>().ok()?;
                let next = upcoming.lock().ok()?.take()?;
                playbin.set_property("uri", &next.file.url);
                *switched.lock().ok()? = Some(next);
                None
            }
        });

        Self {
            playbin,
            status,
            client: None,
            album: None,
            queue: Arc::default(),
            quality: None,
            best_quality: Quality::default(),
            resume_at: None,
            current: None,
            starting: None,
            upcoming,
            switched,
            prefetch: None,
            wants_playing: false,
            report: None,
            sending: Vec::new(),
        }
    }

    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        let mut messages = self.playbin.bus().expect("playbin has a bus").stream();
        loop {
            tokio::select! {
                command = commands.recv() => match command {
                    Some(command) => self.handle_command(command).await,
                    None => break,
                },
                Some(message) = messages.next() => self.handle_message(&message).await,
            }
        }
        self.stop().await;
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::PlayAlbum {
                client,
                mut album,
                track_id,
            } => {
                self.stop().await;
                self.queue = album
                    .tracks
                    .take()
                    .map(|page| page.items)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|track| track.streamable)
                    .collect();
                let start = track_id
                    .and_then(|id| self.queue.iter().position(|track| track.id == id))
                    .unwrap_or(0);
                self.client = Some(client);
                self.album = Some(Arc::new(*album));
                self.play(start).await;
            }
            Command::TogglePause => self.toggle_pause(),
            Command::Next => {
                let next = self.current.map(|index| index + 1);
                if let Some(next) = next.filter(|&next| next < self.queue.len()) {
                    self.skip_to(next).await;
                }
            }
            Command::Previous => match self.current {
                Some(index) if index > 0 && self.position() < RESTART_WITHIN => {
                    self.skip_to(index - 1).await;
                }
                Some(_) => self.seek(Duration::ZERO),
                None => {}
            },
            Command::PlayIndex(index) => {
                if index < self.queue.len() {
                    self.skip_to(index).await;
                }
            }
            Command::Seek(position) => self.seek(position),
            Command::SetQuality(quality) => self.set_quality(quality).await,
            Command::Stop(done) => {
                self.stop().await;
                self.album = None;
                self.queue = Arc::default();
                self.publish(State::Stopped, None);
                if let Some(done) = done {
                    let _ = done.send(());
                }
            }
        }
    }

    /// Start playing the track at `index` in the queue.
    async fn play(&mut self, index: usize) {
        let (Some(client), Some(track)) = (self.client.clone(), self.queue.get(index)) else {
            self.publish(State::Stopped, None);
            return;
        };
        let track_id = track.id;
        self.forget_next();
        self.current = Some(index);
        self.quality = None;
        self.resume_at = None;
        self.wants_playing = true;
        self.publish(State::Loading, None);

        match client.file_url(track_id, self.best_quality).await {
            Ok(file) => {
                let _ = self.playbin.set_state(gst::State::Null);
                self.playbin.set_property("uri", &file.url);
                self.starting = Some(Upcoming { index, file });
                if let Err(err) = self.playbin.set_state(gst::State::Playing) {
                    self.fail(&format!("Can't start playback: {err}"));
                }
            }
            Err(err) => {
                let title = self.queue[index].title.clone();
                self.fail(&format!("Can't stream \u{201c}{title}\u{201d}: {err}"));
            }
        }
    }

    /// Stream in `quality` at best: the track playing starts again in it, where it was,
    /// paused if it was.
    async fn set_quality(&mut self, quality: Quality) {
        if quality == self.best_quality {
            return;
        }
        self.best_quality = quality;
        let state = self.status.borrow().state;
        let Some(index) = self.current.filter(|_| state != State::Stopped) else {
            return;
        };
        let position = self.position();
        self.retire_report(false);
        self.play(index).await;
        self.resume_at = Some(position);
        if state == State::Paused {
            self.wants_playing = false;
            let _ = self.playbin.set_state(gst::State::Paused);
        }
    }

    /// Leave the track playing for the one at `index`.
    async fn skip_to(&mut self, index: usize) {
        self.retire_report(false);
        self.play(index).await;
    }

    fn seek(&mut self, position: Duration) {
        if self.current.is_none() {
            return;
        }
        let from = self.position();
        if let Some(report) = &mut self.report {
            report.stretch_ended(from);
        }
        let to =
            gst::ClockTime::from_nseconds(u64::try_from(position.as_nanos()).unwrap_or(u64::MAX));
        if let Err(err) = self
            .playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, to)
        {
            tracing::warn!("cannot seek: {err}");
        }
        if self.wants_playing
            && let Some(report) = &mut self.report
        {
            report.stretch_started(position);
        }
    }

    /// Done with the current track's report: end it, and keep what it still sends so
    /// quitting can wait for it.
    fn retire_report(&mut self, played_to_the_end: bool) {
        let position = self.position();
        let Some(mut report) = self.report.take() else {
            return;
        };
        if played_to_the_end {
            report.finished();
        } else {
            report.stretch_ended(position);
        }
        self.sending.retain(|sending| !sending.is_finished());
        self.sending.append(&mut report.sending);
    }

    /// Drop the next track's stream: what plays next is changing.
    fn forget_next(&mut self) {
        if let Some(prefetch) = self.prefetch.take() {
            prefetch.abort();
        }
        if let Ok(mut upcoming) = self.upcoming.lock() {
            *upcoming = None;
        }
        if let Ok(mut switched) = self.switched.lock() {
            *switched = None;
        }
    }

    fn toggle_pause(&mut self) {
        let state = self.status.borrow().state;
        let position = self.position();
        match state {
            State::Playing | State::Loading => {
                self.wants_playing = false;
                let _ = self.playbin.set_state(gst::State::Paused);
                if let Some(report) = &mut self.report {
                    report.stretch_ended(position);
                }
                self.publish(State::Paused, None);
            }
            State::Paused => {
                self.wants_playing = true;
                let _ = self.playbin.set_state(gst::State::Playing);
                if let Some(report) = &mut self.report {
                    report.stretch_started(position);
                }
                self.publish(State::Playing, None);
            }
            State::Stopped => {}
        }
    }

    /// Stop the stream, reporting what was listened to.
    async fn stop(&mut self) {
        self.retire_report(false);
        for sending in self.sending.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(5), sending).await;
        }
        self.forget_next();
        let _ = self.playbin.set_state(gst::State::Null);
        self.starting = None;
        self.current = None;
        self.wants_playing = false;
    }

    async fn handle_message(&mut self, message: &gst::Message) {
        use gst::MessageView;

        match message.view() {
            MessageView::StreamStart(_) => {
                let switched = self.switched.lock().ok().and_then(|mut s| s.take());
                if let Some(next) = switched {
                    tracing::debug!("moved on to the next track without a gap");
                    // GStreamer moved on to the next track: the last one played to its end.
                    self.retire_report(true);
                    self.track_started(&next);
                } else if let Some(starting) = self.starting.take() {
                    self.track_started(&starting);
                }
            }
            MessageView::Eos(_) => {
                // The next stream wasn't ready in time, or this was the last track.
                tracing::debug!("the stream ended");
                self.retire_report(true);
                let next = self.current.map(|index| index + 1);
                if let Some(next) = next.filter(|&next| next < self.queue.len()) {
                    self.play(next).await;
                } else {
                    let _ = self.playbin.set_state(gst::State::Null);
                    self.current = None;
                    self.wants_playing = false;
                    self.publish(State::Stopped, None);
                }
            }
            MessageView::Buffering(buffering) => {
                // Hold the stream while the network catches up, then carry on.
                if !self.wants_playing {
                    return;
                }
                if buffering.percent() < 100 {
                    let _ = self.playbin.set_state(gst::State::Paused);
                    self.publish(State::Loading, None);
                } else {
                    let _ = self.playbin.set_state(gst::State::Playing);
                    self.publish(State::Playing, None);
                }
            }
            // The new stream can seek, back to where the track was.
            MessageView::AsyncDone(_) => {
                if let Some(position) = self.resume_at.take() {
                    self.seek(position);
                }
            }
            MessageView::Error(err) => {
                tracing::warn!("playback failed: {} ({:?})", err.error(), err.debug());
                self.fail(&err.error().to_string());
            }
            _ => {}
        }
    }

    /// A track's stream started: report it, show it, and get the next one ready.
    fn track_started(&mut self, started: &Upcoming) {
        let Some(track) = self.queue.get(started.index) else {
            return;
        };
        tracing::info!(
            "playing \u{201c}{}\u{201d}, format {}{}",
            track.title,
            started.file.format_id,
            if started.file.sample {
                ", preview only"
            } else {
                ""
            }
        );
        self.current = Some(started.index);
        self.quality = if started.file.format_id == Quality::Mp3 as u32 {
            Some("MP3 \u{b7} 320 kbps".to_owned())
        } else {
            crate::format::audio_quality(started.file.bit_depth, started.file.sampling_rate)
        };
        if let Some(client) = &self.client {
            self.report = Report::start(client.clone(), track, &started.file);
        }
        // Paused when its stream changed while paused.
        let state = if self.wants_playing {
            State::Playing
        } else {
            State::Paused
        };
        self.publish(state, None);
        self.prefetch_next(started.index + 1);
    }

    fn prefetch_next(&mut self, index: usize) {
        if let Some(prefetch) = self.prefetch.take() {
            prefetch.abort();
        }
        let (Some(client), Some(track)) = (self.client.clone(), self.queue.get(index)) else {
            return;
        };
        let track_id = track.id;
        let (upcoming, quality) = (self.upcoming.clone(), self.best_quality);
        self.prefetch = Some(relm4::spawn(async move {
            match client.file_url(track_id, quality).await {
                Ok(file) => {
                    if let Ok(mut upcoming) = upcoming.lock() {
                        *upcoming = Some(Upcoming { index, file });
                    }
                }
                Err(err) => tracing::warn!("cannot get the next track ready: {err}"),
            }
        }));
    }

    fn fail(&mut self, error: &str) {
        self.retire_report(false);
        let _ = self.playbin.set_state(gst::State::Null);
        self.wants_playing = false;
        self.publish(State::Stopped, Some(error.to_owned()));
    }

    fn publish(&self, state: State, error: Option<String>) {
        let track_id = self
            .current
            .and_then(|index| self.queue.get(index))
            .map(|track| track.id);
        self.status.send_replace(Status {
            state,
            album_id: self.album.as_ref().map(|album| album.id.clone()),
            track_id,
            error,
            album: self.album.clone(),
            queue: self.queue.clone(),
            index: self.current,
            quality: self.quality.clone(),
        });
    }

    fn position(&self) -> Duration {
        position(&self.playbin).unwrap_or_default()
    }
}

fn position(playbin: &gst::Element) -> Option<Duration> {
    let position = playbin.query_position::<gst::ClockTime>()?;
    Some(Duration::from_nanos(position.nseconds()))
}

/// What we tell Qobuz about the listening of one track: that it started, then each stretch
/// listened to without interruption.
struct Report {
    client: Client,
    track_id: u64,
    blob: String,
    /// Ties the stretches of this play together.
    play_id: String,
    duration: Duration,
    /// When and where in the track the current stretch started.
    stretch: Option<(SystemTime, Duration)>,
    /// Reports being sent, so quitting can wait for them.
    sending: Vec<JoinHandle<()>>,
}

impl Report {
    /// Report that `track` started; `None` when Qobuz gave nothing to report it with.
    fn start(client: Client, track: &Track, file: &FileUrl) -> Option<Self> {
        let Some(blob) = file.blob.clone() else {
            tracing::debug!("no blob to report the streaming of track {}", track.id);
            return None;
        };
        let (track_id, format_id) = (track.id, file.format_id);
        let reporting = client.clone();
        let sending = relm4::spawn(async move {
            if let Err(err) = reporting.report_streaming_start(track_id, format_id).await {
                tracing::warn!("cannot report the start of track {track_id}: {err}");
            }
        });
        Some(Self {
            client,
            track_id,
            blob,
            play_id: uuid::Uuid::new_v4().to_string(),
            duration: Duration::from_secs(track.duration.into()),
            stretch: Some((SystemTime::now(), Duration::ZERO)),
            sending: vec![sending],
        })
    }

    fn stretch_started(&mut self, position: Duration) {
        if self.stretch.is_none() {
            self.stretch = Some((SystemTime::now(), position));
        }
    }

    fn stretch_ended(&mut self, position: Duration) {
        let Some((started_at, started_from)) = self.stretch.take() else {
            return;
        };
        let duration = position
            .saturating_sub(started_from)
            .min(self.duration)
            .as_secs();
        if duration == 0 {
            return;
        }
        let event = StreamingEnd {
            blob: self.blob.clone(),
            track_context_uuid: self.play_id.clone(),
            start_stream: humantime::format_rfc3339_millis(started_at).to_string(),
            online: true,
            local: false,
            duration,
        };
        let (client, track_id) = (self.client.clone(), self.track_id);
        self.sending.push(relm4::spawn(async move {
            if let Err(err) = client.report_streaming_end(&event).await {
                tracing::warn!("cannot report the listening of track {track_id}: {err}");
            }
        }));
    }

    /// The track played to its end.
    fn finished(&mut self) {
        self.stretch_ended(self.duration);
    }
}
