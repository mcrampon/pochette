//! The reports Qobuz didn't get, as when offline: kept in `state_dir/reports.jsonl`, and
//! sent once Qobuz is reachable again, as its own apps do for offline listening, so the
//! artists get paid for it too.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

use qobuz::{Client, Quality, StreamingEnd};
use serde::{Deserialize, Serialize};

/// A report to Qobuz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pending {
    /// A track started.
    Start { track_id: u64, format_id: u32 },
    /// A stretch of a track was heard.
    End {
        track_id: u64,
        format_id: u32,
        /// What Qobuz said of the stream: asked when sending, for a track played from a
        /// download.
        blob: Option<String>,
        /// Ties the stretches of one play together.
        play_id: String,
        /// When the stretch started, as `2026-11-16T19:00:00.000Z`.
        started: String,
        seconds: u64,
        /// Streamed, rather than played from a download.
        online: bool,
    },
}

/// Why a report wasn't sent.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Qobuz couldn't be reached: worth another try.
    #[error("{0}")]
    Unreachable(qobuz::Error),
    /// Qobuz turned it down, or has nothing to report it with: not worth another.
    #[error("{0}")]
    Refused(String),
}

impl From<qobuz::Error> for Error {
    fn from(err: qobuz::Error) -> Self {
        match err {
            qobuz::Error::Http(_) => Self::Unreachable(err),
            err => Self::Refused(err.to_string()),
        }
    }
}

/// Send `report` to Qobuz.
pub async fn send(client: &Client, report: &Pending) -> Result<(), Error> {
    match report {
        Pending::Start {
            track_id,
            format_id,
        } => Ok(client.report_streaming_start(*track_id, *format_id).await?),
        Pending::End {
            track_id,
            format_id,
            blob,
            play_id,
            started,
            seconds,
            online,
        } => {
            let blob = if let Some(blob) = blob {
                blob.clone()
            } else {
                // A download's stream: Qobuz says what to report it with, as for any.
                let quality = Quality::of_format(*format_id).unwrap_or_default();
                client
                    .file_url(*track_id, quality)
                    .await?
                    .blob
                    .ok_or_else(|| {
                        Error::Refused(format!("nothing to report track {track_id} with"))
                    })?
            };
            let event = StreamingEnd {
                blob,
                track_context_uuid: play_id.clone(),
                start_stream: started.clone(),
                online: *online,
                local: false,
                duration: *seconds,
            };
            Ok(client.report_streaming_end(&event).await?)
        }
    }
}

/// Send `report` to Qobuz, with `client` when there's one, or keep it for later if Qobuz
/// can't be reached.
pub async fn send_or_keep(client: Option<Client>, report: Pending) {
    let Some(client) = client else {
        keep(&report);
        return;
    };
    match send(&client, &report).await {
        Ok(()) => {}
        Err(Error::Unreachable(err)) => {
            tracing::info!("cannot reach Qobuz, keeping the report for later: {err}");
            keep(&report);
        }
        Err(Error::Refused(err)) => tracing::warn!("cannot report {report:?}: {err}"),
    }
}

/// Serializes the file's writers.
static FILE: Mutex<()> = Mutex::new(());

fn path() -> PathBuf {
    crate::platform().state_dir.join("reports.jsonl")
}

/// The reports being sent, set aside: what new ones come meanwhile goes to [`path`].
fn sending_path() -> PathBuf {
    crate::platform().state_dir.join("reports-sending.jsonl")
}

/// Keep `report`, to send later.
pub fn keep(report: &Pending) {
    let _file = FILE.lock().unwrap_or_else(PoisonError::into_inner);
    let write = || -> io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut line = serde_json::to_string(report).map_err(io::Error::other)?;
        line.push('\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(line.as_bytes())
    };
    if let Err(err) = write() {
        tracing::warn!("cannot keep the report {report:?}: {err}");
    }
}

/// Send the reports kept, keeping those Qobuz still can't be reached for. Returns how many
/// were sent.
pub async fn send_kept(client: &Client) -> usize {
    let kept = {
        let _file = FILE.lock().unwrap_or_else(PoisonError::into_inner);
        // Those set aside by a send that didn't finish come first.
        let mut kept = fs::read_to_string(sending_path()).unwrap_or_default();
        kept.push_str(&fs::read_to_string(path()).unwrap_or_default());
        if kept.is_empty() {
            return 0;
        }
        if let Err(err) = fs::write(sending_path(), &kept).and_then(|()| fs::remove_file(path()))
            && err.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!("cannot set the kept reports aside: {err}");
            return 0;
        }
        kept
    };
    let mut sent = 0;
    for line in kept.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(report) = serde_json::from_str::<Pending>(line) else {
            tracing::warn!("dropping an unreadable report: {line}");
            continue;
        };
        match send(client, &report).await {
            Ok(()) => sent += 1,
            Err(Error::Unreachable(_)) => keep(&report),
            Err(Error::Refused(err)) => tracing::warn!("cannot report {report:?}: {err}"),
        }
    }
    let _file = FILE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Err(err) = fs::remove_file(sending_path()) {
        tracing::warn!("cannot clear the reports sent: {err}");
    }
    if sent > 0 {
        tracing::info!("sent {sent} reports kept from offline");
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_what_it_keeps() {
        let report = Pending::End {
            track_id: 1,
            format_id: 27,
            blob: None,
            play_id: "p".into(),
            started: "2026-11-16T19:00:00.000Z".into(),
            seconds: 42,
            online: false,
        };
        let line = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<Pending>(&line).unwrap(), report);
        let start = serde_json::to_string(&Pending::Start {
            track_id: 1,
            format_id: 6,
        })
        .unwrap();
        assert_eq!(start, r#"{"kind":"start","track_id":1,"format_id":6}"#);
    }
}
