//! What the player needs: where tracks stream from, and what to do with what's heard, as
//! on desktop: tell Qobuz, which is what pays the artists, and keep it in the play history.
//! ExoPlayer plays; the app tells a [`Listening`] when it starts and stops.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use pochette_core::{
    format,
    listening::{self, Report},
    lyrics,
};
use qobuz::{
    FileUrl,
    models::{Album, Track},
};

use crate::{Inner, Pochette, PochetteError, Result, lock, run, runtime};

/// The best quality to ask Qobuz for: it answers with the best the track and the
/// subscription allow, up to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum Quality {
    Mp3,
    /// FLAC, 16-bit / 44.1 kHz.
    Cd,
    /// FLAC, up to 24-bit / 96 kHz.
    HiRes96,
    /// FLAC, up to 24-bit / 192 kHz.
    #[default]
    HiRes192,
}

impl From<Quality> for qobuz::Quality {
    fn from(quality: Quality) -> Self {
        match quality {
            Quality::Mp3 => Self::Mp3,
            Quality::Cd => Self::Cd,
            Quality::HiRes96 => Self::HiRes96,
            Quality::HiRes192 => Self::HiRes192,
        }
    }
}

/// Where a track streams from. Its address expires after a while (an hour, in 2026): ask
/// again for a track that's been waiting.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Stream {
    pub track_id: u64,
    pub url: String,
    /// The quality Qobuz picked.
    pub format_id: u32,
    pub mime_type: Option<String>,
    pub bit_depth: Option<u32>,
    /// In kHz.
    pub sampling_rate: Option<f32>,
    /// What reports this stream to Qobuz.
    pub blob: Option<String>,
    /// Only a 30-second preview, as for free accounts: not heard as the album.
    pub sample: bool,
    /// "24-bit / 96 kHz", "MP3 · 320 kbps".
    pub quality: Option<String>,
}

impl Stream {
    fn new(track_id: u64, file: FileUrl) -> Self {
        let quality = if file.format_id == qobuz::Quality::Mp3 as u32 {
            Some("MP3 \u{b7} 320 kbps".to_owned())
        } else {
            format::audio_quality(file.bit_depth, file.sampling_rate)
        };
        Self {
            track_id,
            url: file.url,
            format_id: file.format_id,
            mime_type: file.mime_type,
            bit_depth: file.bit_depth,
            sampling_rate: file.sampling_rate,
            blob: file.blob,
            sample: file.sample,
            quality,
        }
    }

    fn file(&self) -> FileUrl {
        FileUrl {
            url: self.url.clone(),
            format_id: self.format_id,
            mime_type: self.mime_type.clone(),
            sampling_rate: self.sampling_rate,
            bit_depth: self.bit_depth,
            blob: self.blob.clone(),
            sample: self.sample,
        }
    }
}

/// A song's lyrics.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Lyrics {
    /// Lines timed to the song, in order.
    Synced {
        lines: Vec<LyricLine>,
    },
    /// Lines without timing.
    Plain {
        text: String,
    },
    Instrumental,
    /// No one wrote them down.
    Missing,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LyricLine {
    /// When it's sung, in milliseconds.
    pub at_ms: u64,
    /// Empty between verses, where the music plays alone.
    pub text: String,
}

/// The line of `lines` sung at `position_ms`, if one is.
#[uniffi::export]
#[must_use]
pub fn current_line(lines: Vec<LyricLine>, position_ms: u64) -> Option<u32> {
    let lines: Vec<lyrics::Line> = lines
        .into_iter()
        .map(|line| lyrics::Line {
            at: Duration::from_millis(line.at_ms),
            text: line.text,
        })
        .collect();
    lyrics::current_line(&lines, Duration::from_millis(position_ms))
        .and_then(|index| u32::try_from(index).ok())
}

#[uniffi::export]
impl Pochette {
    /// Where the track streams from, in `quality` at best.
    pub async fn stream(&self, track_id: u64, quality: Quality) -> Result<Stream> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let file = inner.client()?.file_url(track_id, quality.into()).await?;
            Ok(Stream::new(track_id, file))
        })
        .await
    }

    /// The track started playing from `stream`: Qobuz hears of it. The album is one whose
    /// page was loaded, by [`Pochette::album`].
    pub fn start_listening(&self, album_id: &str, stream: &Stream) -> Result<Arc<Listening>> {
        let client = self.inner.client()?;
        let album = lock(&self.inner.albums)
            .get(album_id)
            .cloned()
            .ok_or_else(|| PochetteError::Failed {
                message: format!("Album {album_id} isn't loaded"),
            })?;
        // As on desktop: the album's tracks that can be played, which its length adds up.
        let tracks: Vec<Track> = album
            .tracks
            .iter()
            .flat_map(|page| &page.items)
            .filter(|track| track.streamable)
            .cloned()
            .collect();
        let index = tracks
            .iter()
            .position(|track| track.id == stream.track_id)
            .ok_or_else(|| PochetteError::Failed {
                message: format!("Track {} isn't on album {album_id}", stream.track_id),
            })?;
        let mut report = Report::start(client, &tracks[index], &stream.file());
        send(&mut report);
        Ok(Arc::new(Listening {
            inner: Arc::clone(&self.inner),
            state: Mutex::new(Some(Heard {
                report,
                album,
                tracks,
                index,
                at: SystemTime::now(),
                sample: stream.sample,
            })),
        }))
    }

    /// The song's lyrics, from LRCLIB: `duration` in seconds.
    pub async fn lyrics(
        &self,
        title: String,
        artist: String,
        album: String,
        duration: u32,
    ) -> Result<Lyrics> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let song = lyrics::Song {
                title,
                artist,
                album,
                duration,
            };
            Ok(match lyrics::find(&inner.http, &song).await? {
                lyrics::Lyrics::Synced(lines) => Lyrics::Synced {
                    lines: lines
                        .into_iter()
                        .map(|line| LyricLine {
                            at_ms: u64::try_from(line.at.as_millis()).unwrap_or(u64::MAX),
                            text: line.text,
                        })
                        .collect(),
                },
                lyrics::Lyrics::Plain(text) => Lyrics::Plain { text },
                lyrics::Lyrics::Instrumental => Lyrics::Instrumental,
                lyrics::Lyrics::Missing => Lyrics::Missing,
            })
        })
        .await
    }
}

/// A track being heard, from its start until the player leaves it: tell it each stretch
/// played without interruption, then [`Listening::end`] it.
#[derive(uniffi::Object)]
pub struct Listening {
    inner: Arc<Inner>,
    /// Until it ends.
    state: Mutex<Option<Heard>>,
}

struct Heard {
    report: Report,
    album: Album,
    tracks: Vec<Track>,
    index: usize,
    at: SystemTime,
    sample: bool,
}

#[uniffi::export]
impl Listening {
    /// The sound started, or started again, from `position_ms`.
    pub fn stretch_started(&self, position_ms: u64) {
        self.with_report(|report| report.stretch_started(Duration::from_millis(position_ms)));
    }

    /// The sound stopped at `position_ms`: paused, or before a seek.
    pub fn stretch_ended(&self, position_ms: u64) {
        self.with_report(|report| report.stretch_ended(Duration::from_millis(position_ms)));
    }

    /// The track played to its end.
    pub fn finished(&self) {
        self.with_report(Report::finished);
    }

    /// How long it was heard so far, in seconds, over all its stretches.
    #[must_use]
    pub fn heard_seconds(&self) -> u64 {
        lock(&self.state)
            .as_ref()
            .map_or(0, |heard| heard.report.heard().as_secs())
    }

    /// The player left the track: what was heard goes into the play history. Whether it
    /// did: not a preview, nor a track not heard at all. End a stretch still going first.
    pub fn end(&self) -> Result<bool> {
        let Some(heard) = lock(&self.state).take() else {
            return Ok(false);
        };
        let Some((record, listen)) = listening::listen(
            &heard.album,
            &heard.tracks,
            heard.index,
            heard.at,
            heard.report.heard(),
            heard.sample,
        ) else {
            return Ok(false);
        };
        lock(&self.inner.log)
            .append(&record, &listen)
            .map_err(|err| PochetteError::Failed {
                message: format!("Can't add to the play history: {err}"),
            })?;
        lock(&self.inner.plays).add(record, listen);
        self.inner.plays_changed();
        // The other devices hear of it now, not at the next round.
        if let Ok(engine) = &self.inner.engine {
            engine.sync_now();
        }
        Ok(true)
    }
}

impl Listening {
    fn with_report(&self, change: impl FnOnce(&mut Report)) {
        if let Some(heard) = lock(&self.state).as_mut() {
            change(&mut heard.report);
            send(&mut heard.report);
        }
    }
}

/// Send what `report` has for Qobuz, in the background.
fn send(report: &mut Report) {
    for sending in report.take_sending() {
        runtime().spawn(sending);
    }
}
