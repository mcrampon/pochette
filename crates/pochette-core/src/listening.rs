//! What players do with what's heard, whatever plays it (GStreamer on desktop, ExoPlayer on
//! Android): tell Qobuz, which is what pays the artists, and keep it in the play history.

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime},
};

use qobuz::{
    Client, FileUrl,
    models::{Album, Track},
};

use crate::{
    history::{AlbumRecord, Listen},
    reports::{self, Pending},
};

/// A report to Qobuz, for the app to send on its runtime.
pub type Sending = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// What we tell Qobuz about the listening of one track: that it started, then each stretch
/// listened to without interruption. The reports pile up for the app to send: see
/// [`Report::take_sending`]. Those Qobuz can't get, offline, are kept for later: see
/// [`reports`].
pub struct Report {
    /// None offline.
    client: Option<Client>,
    track_id: u64,
    format_id: u32,
    /// What Qobuz needs to hear about it: asked when sending, for a download.
    blob: Option<String>,
    /// Whether Qobuz is told at all: not of a stream it gave nothing to report with.
    told: bool,
    /// Streamed, rather than played from a download.
    online: bool,
    /// Ties the stretches of this play together.
    play_id: String,
    duration: Duration,
    /// How long it was heard, over all its stretches.
    heard: Duration,
    /// When and where in the track the current stretch started.
    stretch: Option<(SystemTime, Duration)>,
    to_send: Vec<Sending>,
}

impl Report {
    /// Report that `track` started, if Qobuz gave something to report it with.
    pub fn start(client: Client, track: &Track, file: &FileUrl) -> Self {
        if file.blob.is_none() {
            tracing::debug!("no blob to report the streaming of track {}", track.id);
        }
        Self::new(
            Some(client),
            track,
            file.format_id,
            file.blob.clone(),
            file.blob.is_some(),
            true,
        )
    }

    /// Report that `track` started, played from a download in the quality of `format_id`,
    /// with `client`, or later if there's none, offline.
    #[must_use]
    pub fn start_downloaded(client: Option<Client>, track: &Track, format_id: u32) -> Self {
        Self::new(client, track, format_id, None, true, false)
    }

    fn new(
        client: Option<Client>,
        track: &Track,
        format_id: u32,
        blob: Option<String>,
        told: bool,
        online: bool,
    ) -> Self {
        let mut report = Self {
            client,
            track_id: track.id,
            format_id,
            blob,
            told,
            online,
            play_id: uuid::Uuid::new_v4().to_string(),
            duration: Duration::from_secs(track.duration.into()),
            heard: Duration::ZERO,
            stretch: Some((SystemTime::now(), Duration::ZERO)),
            to_send: Vec::new(),
        };
        report.tell(Pending::Start {
            track_id: report.track_id,
            format_id,
        });
        report
    }

    /// Have `pending` sent, if Qobuz is told of this track.
    fn tell(&mut self, pending: Pending) {
        if self.told {
            self.to_send.push(Box::pin(reports::send_or_keep(
                self.client.clone(),
                pending,
            )));
        }
    }

    /// Playing again, from `position`.
    pub fn stretch_started(&mut self, position: Duration) {
        if self.stretch.is_none() {
            self.stretch = Some((SystemTime::now(), position));
        }
    }

    /// Stopped playing, at `position`, as when paused or seeking.
    pub fn stretch_ended(&mut self, position: Duration) {
        let Some((started_at, started_from)) = self.stretch.take() else {
            return;
        };
        let stretch = position.saturating_sub(started_from).min(self.duration);
        self.heard += stretch;
        let seconds = stretch.as_secs();
        if seconds == 0 {
            return;
        }
        self.tell(Pending::End {
            track_id: self.track_id,
            format_id: self.format_id,
            blob: self.blob.clone(),
            play_id: self.play_id.clone(),
            started: humantime::format_rfc3339_millis(started_at).to_string(),
            seconds,
            online: self.online,
        });
    }

    /// The track played to its end.
    pub fn finished(&mut self) {
        self.stretch_ended(self.duration);
    }

    /// How long the track was heard, over the stretches that ended.
    #[must_use]
    pub fn heard(&self) -> Duration {
        self.heard
    }

    /// The reports to send now.
    pub fn take_sending(&mut self) -> Vec<Sending> {
        std::mem::take(&mut self.to_send)
    }
}

/// What the play history keeps of a track heard: the track at `index` of `tracks`, of
/// `album`, from `at`, for `heard`. Nothing for a preview, or a track not heard at all.
#[must_use]
pub fn listen(
    album: &Album,
    tracks: &[Track],
    index: usize,
    at: SystemTime,
    heard: Duration,
    sample: bool,
) -> Option<(AlbumRecord, Listen)> {
    let track = tracks.get(index)?;
    let heard = u32::try_from(heard.as_secs()).unwrap_or(u32::MAX);
    if sample || heard == 0 {
        return None;
    }
    let length = tracks.iter().map(|track| track.duration).sum();
    let at = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_secs()).ok())
        .unwrap_or_default();
    Some((
        AlbumRecord::new(album, length),
        Listen {
            at,
            album: album.id.clone(),
            heard: heard.min(track.duration.max(1)),
        },
    ))
}
