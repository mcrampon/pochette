//! This device, as the others it syncs with know it: its key, its name, its log of plays,
//! and the logs of the others it keeps.
//!
//! The key, in `$XDG_STATE_HOME/pochette`, stays on this device: not in the data folder,
//! which people may copy or sync to their other devices, making them the same device.

use std::{
    fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use relm4::gtk::glib;

use super::channel::NOISE_PATTERN;

const KEY_FILE: &str = "sync-key";

/// How a log's name starts and ends.
const LOG_PREFIX: &str = "plays-";
const LOG_SUFFIX: &str = ".jsonl";

/// The device's key, which proves it is the device that was paired.
#[derive(Clone)]
pub struct Identity {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

impl Identity {
    /// This device's key, made the first time it's asked for.
    pub fn load() -> io::Result<Self> {
        let path = state_dir().join(KEY_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::create(&path),
            Err(err) => Err(err),
        }
    }

    fn parse(text: &str) -> io::Result<Self> {
        let mut lines = text.lines().map(|line| BASE64.decode(line.trim()));
        match (lines.next(), lines.next()) {
            (Some(Ok(private)), Some(Ok(public))) if private.len() == 32 && public.len() == 32 => {
                Ok(Self { private, public })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the sync key is damaged",
            )),
        }
    }

    fn create(path: &Path) -> io::Result<Self> {
        let pattern = NOISE_PATTERN.parse().map_err(io::Error::other)?;
        let keypair = snow::Builder::new(pattern)
            .generate_keypair()
            .map_err(io::Error::other)?;
        let identity = Self {
            private: keypair.private,
            public: keypair.public,
        };
        let dir = path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        // Written aside, then linked in place, which fails if the app and the service both
        // made a key at once: the second then takes the first's.
        let aside = dir.join(format!("{KEY_FILE}.{}", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&aside)?;
        writeln!(
            file,
            "{}\n{}",
            BASE64.encode(&identity.private),
            BASE64.encode(&identity.public)
        )?;
        file.sync_all()?;
        let linked = fs::hard_link(&aside, path);
        let _ = fs::remove_file(&aside);
        match linked {
            Ok(()) => Ok(identity),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                Self::parse(&fs::read_to_string(path)?)
            }
            Err(err) => Err(err),
        }
    }

    /// The device's id, which the others know it by.
    pub fn id(&self) -> String {
        hex(&self.public)
    }
}

/// The id of the device whose public key is this.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// Where the key and the paired devices are kept.
pub fn state_dir() -> PathBuf {
    glib::user_state_dir().join("pochette")
}

/// Where the logs of plays are kept.
pub fn data_dir() -> PathBuf {
    glib::user_data_dir().join("pochette")
}

/// What this device is called, to the others.
pub fn name() -> String {
    let name = glib::host_name();
    let name = name.trim();
    if name.is_empty() || name == "localhost" {
        "Pochette".to_owned()
    } else {
        name.to_owned()
    }
}

/// The host name, as it can be in a file name.
fn host_for_file() -> String {
    glib::host_name()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The end of the names of the logs a device writes, from its id: `-` and the id's first 8
/// characters, as in `plays-laptop-3fa29c0e.jsonl`.
pub fn log_tag(id: &str) -> String {
    format!("-{}{LOG_SUFFIX}", id.get(..8).unwrap_or(id))
}

/// Whether `name` is that of a log devices share, which only says where it comes from:
/// `plays-`, letters, digits, `-` or `_`, the device's tag. Others, such as a log copied
/// in by hand, stay on this device.
pub fn is_shared_log(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix(LOG_PREFIX)
        .and_then(|rest| rest.strip_suffix(LOG_SUFFIX))
    else {
        return false;
    };
    let Some((_, tag)) = stem.rsplit_once('-') else {
        return false;
    };
    name.len() <= 128
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && tag.len() == 8
        && tag.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// This device's log, which it alone writes to: the one that ends with its tag, else a new
/// one after its host name. A log from before devices had keys, after the host name alone,
/// becomes it. Without a key, the log is after the host name alone.
pub fn own_log() -> &'static Path {
    static OWN: OnceLock<PathBuf> = OnceLock::new();
    OWN.get_or_init(|| {
        let dir = data_dir();
        let host = host_for_file();
        let legacy = dir.join(format!("{LOG_PREFIX}{host}{LOG_SUFFIX}"));
        let identity = match Identity::load() {
            Ok(identity) => identity,
            Err(err) => {
                tracing::warn!("cannot load this device's sync key, so it won't sync: {err}");
                return legacy;
            }
        };
        let tag = log_tag(&identity.id());
        let existing = fs::read_dir(&dir).ok().and_then(|entries| {
            entries.flatten().map(|entry| entry.path()).find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| is_shared_log(name) && name.ends_with(&tag))
            })
        });
        if let Some(existing) = existing {
            // An older Pochette, which only knows the log after the host name, may have
            // written to it since.
            merge_legacy(&legacy, &existing);
            return existing;
        }
        let own = dir.join(format!("{LOG_PREFIX}{host}{tag}"));
        if legacy.exists() {
            match fs::rename(&legacy, &own) {
                Ok(()) => tracing::info!("named the play log {}", own.display()),
                Err(err) => {
                    tracing::warn!("cannot rename {}: {err}", legacy.display());
                    return legacy;
                }
            }
        }
        own
    })
}

/// Move the lines of `legacy`, if it's there, to the end of `own`, which then shares them.
fn merge_legacy(legacy: &Path, own: &Path) {
    // Taken aside first: only one of the app and the service can, so only one merges it.
    let aside = legacy.with_extension(format!("merging-{}", std::process::id()));
    match fs::rename(legacy, &aside) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            tracing::warn!("cannot merge {}: {err}", legacy.display());
            return;
        }
    }
    let merged = (|| {
        let mut text = fs::read_to_string(&aside)?;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        let mut file = fs::OpenOptions::new().append(true).open(own)?;
        // Its last line may be unfinished, if it was cut short.
        let length = file.metadata()?.len();
        if length > 0 {
            let mut last = [0];
            let mut reader = fs::File::open(own)?;
            std::io::Seek::seek(&mut reader, io::SeekFrom::Start(length - 1))?;
            std::io::Read::read_exact(&mut reader, &mut last)?;
            if last[0] != b'\n' {
                text.insert(0, '\n');
            }
        }
        file.write_all(text.as_bytes())
    })();
    match merged {
        Ok(()) => {
            tracing::info!("merged {} into {}", legacy.display(), own.display());
            let _ = fs::remove_file(&aside);
        }
        Err(err) => {
            tracing::warn!("cannot merge {}: {err}", legacy.display());
            // Back, for the next time.
            let _ = fs::rename(&aside, legacy);
        }
    }
}

/// A shared log, as far as it can be shared: up to its last whole line.
pub struct Log {
    pub name: String,
    pub text: String,
}

impl Log {
    pub fn digest(&self) -> String {
        digest(&self.text)
    }
}

pub fn digest(text: &str) -> String {
    format!("{:x}", md5::compute(text))
}

/// The logs this device shares, its own and those it got from others.
pub fn shared_logs(dir: &Path) -> Vec<Log> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut logs: Vec<Log> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !is_shared_log(&name) {
                return None;
            }
            let mut text = match fs::read_to_string(entry.path()) {
                Ok(text) => text,
                Err(err) => {
                    tracing::warn!("cannot read {name}: {err}");
                    return None;
                }
            };
            // A line being written isn't whole yet.
            text.truncate(text.rfind('\n').map_or(0, |end| end + 1));
            Some(Log { name, text })
        })
        .collect();
    logs.sort_by(|a, b| a.name.cmp(&b.name));
    logs
}

/// How big each shared log is, to tell when they change.
pub fn logs_signature(dir: &Path) -> Vec<(String, u64)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sizes: Vec<(String, u64)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let size = entry.metadata().ok()?.len();
            is_shared_log(&name).then_some((name, size))
        })
        .collect();
    sizes.sort();
    sizes
}

/// Add `text`, whole lines another device sent, to the log `name` at `offset`, as long as
/// the log is another device's, and that's where it ends.
pub fn append_lines(
    dir: &Path,
    own_tag: &str,
    name: &str,
    offset: u64,
    text: &str,
) -> io::Result<()> {
    if !is_shared_log(name) || name.ends_with(own_tag) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a log to take from another device: {name}"),
        ));
    }
    if !text.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lines cut short",
        ));
    }
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let length = file.metadata()?.len();
    if length != offset {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} has {length} bytes here, not {offset}"),
        ));
    }
    file.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_shared_logs() {
        assert!(is_shared_log("plays-laptop-3fa29c0e.jsonl"));
        assert!(is_shared_log("plays-my_pc-0123abcd.jsonl"));
        assert!(!is_shared_log("plays-desktop.jsonl"));
        assert!(!is_shared_log("plays-dummy.jsonl"));
        assert!(!is_shared_log("plays-x-3FA29C0E.jsonl"));
        assert!(!is_shared_log("plays-../x-3fa29c0e.jsonl"));
        assert!(!is_shared_log("plays-a/b-3fa29c0e.jsonl"));
        assert!(!is_shared_log("other-laptop-3fa29c0e.jsonl"));
        assert_eq!(log_tag("3fa29c0e99"), "-3fa29c0e.jsonl");
    }

    #[test]
    fn merges_a_log_written_after_the_host_name_alone() {
        let dir = std::env::temp_dir().join(format!("pochette-merge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("plays-desktop.jsonl");
        let own = dir.join("plays-desktop-3657a131.jsonl");
        fs::write(&own, "a\nb cut").unwrap();
        fs::write(&legacy, "c\nd\n").unwrap();
        merge_legacy(&legacy, &own);
        assert_eq!(fs::read_to_string(&own).unwrap(), "a\nb cut\nc\nd\n");
        assert!(!legacy.exists());
        // Nothing left aside.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        // Nothing to merge.
        merge_legacy(&legacy, &own);
        assert_eq!(fs::read_to_string(&own).unwrap(), "a\nb cut\nc\nd\n");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn appends_only_where_a_log_ends() {
        let dir = std::env::temp_dir().join(format!("pochette-append-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let name = "plays-laptop-3fa29c0e.jsonl";
        let own = "-00000000.jsonl";
        append_lines(&dir, own, name, 0, "a\n").unwrap();
        append_lines(&dir, own, name, 2, "b\n").unwrap();
        assert!(append_lines(&dir, own, name, 2, "c\n").is_err());
        assert!(append_lines(&dir, own, name, 4, "d").is_err());
        assert!(append_lines(&dir, own, "plays-me-00000000.jsonl", 0, "e\n").is_err());
        assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), "a\nb\n");
        let logs = shared_logs(&dir);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].text, "a\nb\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
