//! What players do with what's heard, whatever plays it (GStreamer on desktop, ExoPlayer on
//! Android): tell Qobuz, which is what pays the artists, and keep it in the play history.

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime},
};

use qobuz::{
    Client, FileUrl, StreamingEnd,
    models::{Album, Track},
};

use crate::history::{AlbumRecord, Listen};

/// A report to Qobuz, for the app to send on its runtime.
pub type Sending = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// What we tell Qobuz about the listening of one track: that it started, then each stretch
/// listened to without interruption. The reports pile up for the app to send: see
/// [`Report::take_sending`].
pub struct Report {
    client: Client,
    track_id: u64,
    /// What Qobuz needs to hear about it; without it, nothing is told.
    blob: Option<String>,
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
        let (track_id, format_id) = (track.id, file.format_id);
        let blob = file.blob.clone();
        let mut to_send: Vec<Sending> = Vec::new();
        if blob.is_some() {
            let reporting = client.clone();
            to_send.push(Box::pin(async move {
                if let Err(err) = reporting.report_streaming_start(track_id, format_id).await {
                    tracing::warn!("cannot report the start of track {track_id}: {err}");
                }
            }));
        } else {
            tracing::debug!("no blob to report the streaming of track {track_id}");
        }
        Self {
            client,
            track_id,
            blob,
            play_id: uuid::Uuid::new_v4().to_string(),
            duration: Duration::from_secs(track.duration.into()),
            heard: Duration::ZERO,
            stretch: Some((SystemTime::now(), Duration::ZERO)),
            to_send,
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
        let duration = stretch.as_secs();
        let Some(blob) = self.blob.clone().filter(|_| duration > 0) else {
            return;
        };
        let event = StreamingEnd {
            blob,
            track_context_uuid: self.play_id.clone(),
            start_stream: humantime::format_rfc3339_millis(started_at).to_string(),
            online: true,
            local: false,
            duration,
        };
        let (client, track_id) = (self.client.clone(), self.track_id);
        self.to_send.push(Box::pin(async move {
            if let Err(err) = client.report_streaming_end(&event).await {
                tracing::warn!("cannot report the listening of track {track_id}: {err}");
            }
        }));
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
