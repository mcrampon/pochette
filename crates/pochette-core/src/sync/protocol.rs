//! What two devices say to each other once connected: who they are and what for, then, to
//! sync, what logs each has, and the lines the other lacks.
//!
//! Each log is only ever written by its device, at its end: what a device lacks of
//! another's log is always its end. The one who called sends what it has first; the other
//! answers with what it has and the lines the caller lacks; the caller then sends the lines
//! the other lacks. Only one of them speaks at a time.

use std::{path::Path, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    channel::{Channel, Error, Result},
    device::{self, Log},
};

/// Bumped when devices of different versions can't understand each other.
pub const VERSION: u32 = 1;

/// How long to wait for the other device during a sync.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// The most lines sent at once.
const CHUNK: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Pair,
    Sync,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Message {
    /// The first thing each says: its name, what for, and the port it listens on, to call
    /// it back.
    Hello {
        version: u32,
        name: String,
        purpose: Purpose,
        port: u16,
    },
    /// Not now, and why.
    Refused { reason: String },
    /// Whether the user confirmed the pairing.
    Verdict { accept: bool },
    /// The logs the device has.
    Logs { logs: Vec<Entry> },
    /// Lines to add to the log `name`, which ends at `offset` on the other device.
    Lines {
        name: String,
        offset: u64,
        text: String,
    },
    /// No more lines.
    Done,
}

/// A log a device has: how long it is, and a digest of it, to check that the other
/// device's copy starts the same.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub length: u64,
    pub digest: String,
}

/// Where a sync takes logs from and adds lines to.
pub struct Logs<'a> {
    pub dir: &'a Path,
    /// The tag of this device's own log, which only it writes.
    pub own_tag: &'a str,
    /// The tag of the other device's own log, which it has all of.
    pub their_tag: &'a str,
}

/// How many lines a sync brought.
#[derive(Debug, Default, Clone, Copy)]
pub struct Synced {
    pub received: u64,
    pub sent: u64,
}

fn entries(logs: &[Log]) -> Vec<Entry> {
    logs.iter()
        .map(|log| Entry {
            name: log.name.clone(),
            length: log.text.len() as u64,
            digest: log.digest(),
        })
        .collect()
}

/// Sync with the device that answered, which is `their_tag`'s.
pub async fn sync_as_caller(channel: &mut Channel<Message>, logs: &Logs<'_>) -> Result<Synced> {
    let mut synced = Synced::default();
    channel
        .send(&Message::Logs {
            logs: entries(&device::shared_logs(logs.dir)),
        })
        .await?;
    let Message::Logs { logs: theirs } = channel.recv(REPLY_TIMEOUT).await? else {
        return Err(Error::Protocol("expected their logs".to_owned()));
    };
    synced.received = receive_lines(channel, logs).await?;
    // Now with what just came, which they need not get back.
    synced.sent = send_lines(channel, logs, &theirs).await?;
    Ok(synced)
}

/// Sync with the device that called, which is `their_tag`'s.
pub async fn sync_as_answerer(channel: &mut Channel<Message>, logs: &Logs<'_>) -> Result<Synced> {
    let mut synced = Synced::default();
    let Message::Logs { logs: theirs } = channel.recv(REPLY_TIMEOUT).await? else {
        return Err(Error::Protocol("expected their logs".to_owned()));
    };
    let mine = device::shared_logs(logs.dir);
    channel
        .send(&Message::Logs {
            logs: entries(&mine),
        })
        .await?;
    synced.sent = send_lines(channel, logs, &theirs).await?;
    synced.received = receive_lines(channel, logs).await?;
    Ok(synced)
}

/// Send the lines the other device lacks, given the logs it has, then `Done`.
async fn send_lines(
    channel: &mut Channel<Message>,
    logs: &Logs<'_>,
    theirs: &[Entry],
) -> Result<u64> {
    let mut sent = 0;
    for log in device::shared_logs(logs.dir) {
        // Theirs, which they have all of.
        if log.name.ends_with(logs.their_tag) {
            continue;
        }
        let from = match theirs.iter().find(|entry| entry.name == log.name) {
            None => 0,
            Some(entry) => {
                let Ok(length) = usize::try_from(entry.length) else {
                    continue;
                };
                if length >= log.text.len() {
                    continue;
                }
                match log.text.get(..length) {
                    Some(start) if device::digest(start) == entry.digest => length,
                    _ => {
                        tracing::warn!(
                            "the other device's copy of {} differs, skipping it",
                            log.name
                        );
                        continue;
                    }
                }
            }
        };
        let mut offset = from;
        while offset < log.text.len() {
            let rest = &log.text[offset..];
            let end = if rest.len() <= CHUNK {
                rest.len()
            } else {
                // Whole lines, unless one is longer than a chunk.
                rest[..CHUNK].rfind('\n').map_or_else(
                    || rest.find('\n').map_or(rest.len(), |end| end + 1),
                    |end| end + 1,
                )
            };
            let text = &rest[..end];
            sent += text.lines().count() as u64;
            channel
                .send(&Message::Lines {
                    name: log.name.clone(),
                    offset: offset as u64,
                    text: text.to_owned(),
                })
                .await?;
            offset += end;
        }
    }
    channel.send(&Message::Done).await?;
    Ok(sent)
}

/// Add the lines the other device sends, until `Done`.
async fn receive_lines(channel: &mut Channel<Message>, logs: &Logs<'_>) -> Result<u64> {
    let mut received = 0;
    loop {
        match channel.recv(REPLY_TIMEOUT).await? {
            Message::Lines { name, offset, text } => {
                match device::append_lines(logs.dir, logs.own_tag, &name, offset, &text) {
                    Ok(()) => received += text.lines().count() as u64,
                    // Not worth losing the rest over.
                    Err(err) => tracing::warn!("skipping lines of {name}: {err}"),
                }
            }
            Message::Done => return Ok(received),
            other => return Err(Error::Protocol(format!("expected lines, got {other:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tokio::net::{TcpListener, TcpStream};

    use super::*;
    use crate::sync::device::{Identity, log_tag};

    fn identity() -> Identity {
        let keypair = snow::Builder::new(crate::sync::channel::NOISE_PATTERN.parse().unwrap())
            .generate_keypair()
            .unwrap();
        Identity {
            private: keypair.private,
            public: keypair.public,
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pochette-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn syncs_both_ways() {
        let (a, b) = (identity(), identity());
        let (a_tag, b_tag) = (log_tag(&a.id()), log_tag(&b.id()));
        let (a_dir, b_dir) = (scratch("a"), scratch("b"));
        let a_log = format!("plays-a{a_tag}");
        let b_log = format!("plays-b{b_tag}");
        let c_log = "plays-c-cccccccc.jsonl";
        fs::write(a_dir.join(&a_log), "a1\na2\n").unwrap();
        // b has the start of a's log, and an unfinished line of its own.
        fs::write(b_dir.join(&a_log), "a1\n").unwrap();
        fs::write(b_dir.join(&b_log), "b1\nb2 cut").unwrap();
        // A third device's, which only b has.
        fs::write(b_dir.join(c_log), "c1\n").unwrap();
        // Not shared.
        fs::write(a_dir.join("plays-dummy.jsonl"), "x\n").unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let answerer = {
            let (b, a_tag, b_tag, b_dir) = (b.clone(), a_tag.clone(), b_tag.clone(), b_dir.clone());
            tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut channel = Channel::<Message>::open(stream, &b, false).await.unwrap();
                let logs = Logs {
                    dir: &b_dir,
                    own_tag: &b_tag,
                    their_tag: &a_tag,
                };
                (
                    sync_as_answerer(&mut channel, &logs).await.unwrap(),
                    channel.handshake_hash.clone(),
                )
            })
        };
        let stream = TcpStream::connect(address).await.unwrap();
        let mut channel = Channel::<Message>::open(stream, &a, true).await.unwrap();
        assert_eq!(channel.remote, b.public);
        let logs = Logs {
            dir: &a_dir,
            own_tag: &a_tag,
            their_tag: &b_tag,
        };
        let synced = sync_as_caller(&mut channel, &logs).await.unwrap();
        let (on_b, hash) = answerer.await.unwrap();
        assert_eq!(hash, channel.handshake_hash);
        assert_eq!((synced.received, synced.sent), (2, 1));
        assert_eq!((on_b.received, on_b.sent), (1, 2));

        for dir in [&a_dir, &b_dir] {
            assert_eq!(fs::read_to_string(dir.join(&a_log)).unwrap(), "a1\na2\n");
            assert_eq!(fs::read_to_string(dir.join(c_log)).unwrap(), "c1\n");
        }
        assert_eq!(fs::read_to_string(a_dir.join(&b_log)).unwrap(), "b1\n");
        assert_eq!(
            fs::read_to_string(b_dir.join(&b_log)).unwrap(),
            "b1\nb2 cut"
        );
        assert!(!b_dir.join("plays-dummy.jsonl").exists());
        fs::remove_dir_all(&a_dir).unwrap();
        fs::remove_dir_all(&b_dir).unwrap();
    }
}
