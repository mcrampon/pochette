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

/// The best quality to ask Qobuz for.
const QUALITY: Quality = Quality::HiRes192;

/// What the player is doing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    pub state: State,
    /// The album being played, or paused.
    pub album_id: Option<String>,
    /// The track being played, or paused.
    pub track_id: Option<u64>,
    /// Why playback stopped, if it failed.
    pub error: Option<String>,
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
}

/// Drives the player. Cheap to clone: every clone drives the same player.
#[derive(Clone)]
pub struct Player {
    commands: mpsc::UnboundedSender<Command>,
    status: watch::Receiver<Status>,
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
        let actor = Actor::new(playbin, status_sender);
        relm4::spawn(actor.run(receiver));
        Ok(Self { commands, status })
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
    album_id: Option<String>,
    /// The album's tracks that can be streamed, in order.
    queue: Vec<Track>,
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
            album_id: None,
            queue: Vec::new(),
            current: None,
            starting: None,
            upcoming,
            switched,
            prefetch: None,
            wants_playing: false,
            report: None,
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
                album,
                track_id,
            } => {
                self.stop().await;
                self.queue = album
                    .tracks
                    .map(|page| page.items)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|track| track.streamable)
                    .collect();
                let start = track_id
                    .and_then(|id| self.queue.iter().position(|track| track.id == id))
                    .unwrap_or(0);
                self.client = Some(client);
                self.album_id = Some(album.id);
                self.play(start).await;
            }
            Command::TogglePause => self.toggle_pause(),
            Command::Stop(done) => {
                self.stop().await;
                self.album_id = None;
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
        self.current = Some(index);
        self.wants_playing = true;
        self.publish(State::Loading, None);

        match client.file_url(track.id, QUALITY).await {
            Ok(file) => {
                let _ = self.playbin.set_state(gst::State::Null);
                self.playbin.set_property("uri", &file.url);
                self.starting = Some(Upcoming { index, file });
                if let Err(err) = self.playbin.set_state(gst::State::Playing) {
                    self.fail(&format!("Can't start playback: {err}"));
                }
            }
            Err(err) => self.fail(&format!(
                "Can't stream \u{201c}{}\u{201d}: {err}",
                track.title
            )),
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
        let position = self.position();
        if let Some(mut report) = self.report.take() {
            report.stretch_ended(position);
            report.flush().await;
        }
        if let Some(prefetch) = self.prefetch.take() {
            prefetch.abort();
        }
        if let Ok(mut upcoming) = self.upcoming.lock() {
            *upcoming = None;
        }
        if let Ok(mut switched) = self.switched.lock() {
            *switched = None;
        }
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
                    if let Some(mut report) = self.report.take() {
                        report.finished();
                    }
                    self.track_started(&next);
                } else if let Some(starting) = self.starting.take() {
                    self.track_started(&starting);
                }
            }
            MessageView::Eos(_) => {
                // The next stream wasn't ready in time, or this was the last track.
                tracing::debug!("the stream ended");
                if let Some(mut report) = self.report.take() {
                    report.finished();
                }
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
        if let Some(client) = &self.client {
            self.report = Report::start(client.clone(), track, &started.file);
        }
        self.publish(State::Playing, None);
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
        let upcoming = self.upcoming.clone();
        self.prefetch = Some(relm4::spawn(async move {
            match client.file_url(track_id, QUALITY).await {
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
        let position = self.position();
        if let Some(mut report) = self.report.take() {
            report.stretch_ended(position);
        }
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
            album_id: self.album_id.clone(),
            track_id,
            error,
        });
    }

    fn position(&self) -> Duration {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map_or(Duration::ZERO, |position| {
                Duration::from_nanos(position.nseconds())
            })
    }
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

    /// Wait for the reports to reach Qobuz.
    async fn flush(&mut self) {
        for sending in self.sending.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(5), sending).await;
        }
    }
}
