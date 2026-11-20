//! What the user listened to, and when: a log of every track heard, from which the play
//! counts, the last plays and the stats are worked out. No UI here, so it can be tested.
//!
//! Each machine appends to its own file, `$XDG_DATA_HOME/pochette/plays-<host>.jsonl`, and
//! reads them all: syncing that folder between machines adds up their plays, without two
//! machines ever writing to the same file.

use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use qobuz::models::Album;
use relm4::gtk::glib;
use serde::{Deserialize, Serialize};

/// Within a sitting, listens of the same album no further apart than this: a longer
/// silence starts another sitting.
const SITTING_GAP: i64 = 3 * 60 * 60;

/// An album, as the log remembers it: enough to show it without asking Qobuz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumRecord {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub artist: ArtistRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// The cover's URLs, small and large.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_large: Option<String>,
    /// The original release date, as `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    /// How long its tracks that can be played last, in seconds.
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtistRecord {
    pub id: u64,
    pub name: String,
}

impl AlbumRecord {
    /// `album`, whose tracks that can be played last `length` seconds.
    pub fn new(album: &Album, length: u32) -> Self {
        Self {
            id: album.id.clone(),
            title: album.title.clone(),
            version: album.version.clone(),
            artist: ArtistRecord {
                id: album.artist.id,
                name: album.artist.name.clone(),
            },
            genre: album.genre.as_ref().map(|genre| genre.name.clone()),
            cover: album.image.small.clone(),
            cover_large: album.image.large.clone(),
            released: album.release_date_original.clone(),
            length,
        }
    }
}

/// Some of an album heard: a track, from its start until the player left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listen {
    /// When it started, in Unix time.
    pub at: i64,
    pub album: String,
    /// How much was heard, in seconds.
    pub heard: u32,
}

/// A line of the log.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Line {
    /// What the listens after it, of that album, refer to.
    Album(AlbumRecord),
    Listen(Listen),
}

fn dir() -> PathBuf {
    glib::user_data_dir().join("pochette")
}

/// This machine's log, which it alone writes to.
fn own_file() -> PathBuf {
    let host: String = glib::host_name()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir().join(format!("plays-{host}.jsonl"))
}

/// Writes this machine's log.
#[derive(Debug, Default)]
pub struct Log {
    /// The album last written, which the listens after it need not repeat.
    album: Option<AlbumRecord>,
}

impl Log {
    /// Add `listen`, of `album`, to the log.
    pub fn append(&mut self, album: &AlbumRecord, listen: &Listen) -> io::Result<()> {
        let mut text = String::new();
        if self.album.as_ref() != Some(album) {
            text.push_str(&serde_json::to_string(&Line::Album(album.clone()))?);
            text.push('\n');
        }
        text.push_str(&serde_json::to_string(&Line::Listen(listen.clone()))?);
        text.push('\n');
        let path = own_file();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(text.as_bytes())?;
        self.album = Some(album.clone());
        Ok(())
    }
}

/// The plays of an album.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AlbumPlays {
    pub count: u32,
    /// When it was last played, in Unix time.
    pub last: Option<i64>,
}

/// Everything listened to, on every machine, and the plays it adds up to.
#[derive(Debug, Default)]
pub struct History {
    albums: HashMap<String, AlbumRecord>,
    /// In the order they started.
    listens: Vec<Listen>,
    /// Each album play, as the album and when its sitting started.
    album_plays: Vec<(String, i64)>,
    by_album: HashMap<String, AlbumPlays>,
}

impl History {
    /// The logs of every machine.
    pub fn load() -> Self {
        let mut history = Self::default();
        let entries = match fs::read_dir(dir()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return history,
            Err(err) => {
                tracing::warn!("cannot read the play history: {err}");
                return history;
            }
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("plays-") && name.ends_with(".jsonl")) {
                continue;
            }
            match fs::read_to_string(entry.path()) {
                Ok(text) => history.read(&text),
                Err(err) => tracing::warn!("cannot read {name}: {err}"),
            }
        }
        history.settle();
        history
    }

    /// Take in the lines of a log, skipping those it can't read, as a line cut short.
    fn read(&mut self, text: &str) {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str(line) {
                Ok(Line::Album(album)) => {
                    self.albums.insert(album.id.clone(), album);
                }
                Ok(Line::Listen(listen)) => self.listens.push(listen),
                Err(err) => tracing::debug!("skipping a line of the play history: {err}"),
            }
        }
    }

    /// Add what was just heard.
    pub fn add(&mut self, album: AlbumRecord, listen: Listen) {
        self.albums.insert(album.id.clone(), album);
        self.listens.push(listen);
        self.settle();
    }

    /// Work out the plays again from the listens.
    fn settle(&mut self) {
        self.listens.sort_by_key(|listen| listen.at);
        self.album_plays = self.count_album_plays();
        self.by_album.clear();
        for (album, at) in &self.album_plays {
            let plays = self.by_album.entry(album.clone()).or_default();
            plays.count += 1;
            plays.last = plays.last.max(Some(*at));
        }
    }

    /// An album is played once per half of it heard in one sitting, rounded: half of it
    /// is a play, and so is all of it, but twice in a row is two.
    fn count_album_plays(&self) -> Vec<(String, i64)> {
        struct Sitting<'a> {
            album: &'a str,
            start: i64,
            end: i64,
            heard: u64,
        }
        let mut plays = Vec::new();
        let mut close = |sitting: &Sitting| {
            let length = self
                .albums
                .get(sitting.album)
                .map_or(0, |album| album.length);
            if length > 0 {
                let count = (sitting.heard * 2 + u64::from(length)) / (2 * u64::from(length));
                for _ in 0..count {
                    plays.push((sitting.album.to_owned(), sitting.start));
                }
            }
        };
        let mut sitting: Option<Sitting> = None;
        for listen in &self.listens {
            let end = listen.at + i64::from(listen.heard);
            match &mut sitting {
                Some(current)
                    if current.album == listen.album && listen.at <= current.end + SITTING_GAP =>
                {
                    current.end = current.end.max(end);
                    current.heard += u64::from(listen.heard);
                }
                _ => {
                    if let Some(done) = sitting.take() {
                        close(&done);
                    }
                    sitting = Some(Sitting {
                        album: &listen.album,
                        start: listen.at,
                        end,
                        heard: listen.heard.into(),
                    });
                }
            }
        }
        if let Some(done) = sitting {
            close(&done);
        }
        plays
    }

    pub fn album(&self, id: &str) -> AlbumPlays {
        self.by_album.get(id).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noon on the 15th of `month` 2026, local time.
    fn day(month: i32, hour_offset: i64) -> i64 {
        glib::DateTime::from_local(2026, month, 15, 12, 0, 0.0)
            .unwrap()
            .to_unix()
            + hour_offset * 3600
    }

    fn album(id: &str, artist: &str, length: u32) -> AlbumRecord {
        AlbumRecord {
            id: id.into(),
            title: id.into(),
            version: None,
            artist: ArtistRecord {
                id: artist.len() as u64,
                name: artist.into(),
            },
            genre: Some("Jazz".into()),
            cover: None,
            cover_large: None,
            released: None,
            length,
        }
    }

    fn listen(album: &str, at: i64, heard: u32) -> Listen {
        Listen {
            at,
            album: album.into(),
            heard,
        }
    }

    /// An album of `tracks` tracks of 300 seconds, heard `heard` seconds each from `at`.
    fn sit(history: &mut History, id: &str, tracks: i64, at: i64, heard: u32) {
        let record = album(id, "Miles Davis", 1500);
        for track in 0..tracks {
            history.add(record.clone(), listen(id, at + track * 300, heard));
        }
    }

    #[test]
    fn counts_an_album_once_half_of_it_is_heard() {
        let mut history = History::default();
        // Two tracks of five: not half.
        sit(&mut history, "a", 2, day(1, 0), 300);
        assert_eq!(history.album("a"), AlbumPlays::default());
        // Then three more, in the same sitting: the whole album, still one play.
        sit(&mut history, "a", 3, day(1, 0) + 600, 300);
        assert_eq!(
            history.album("a"),
            AlbumPlays {
                count: 1,
                last: Some(day(1, 0)),
            }
        );
    }

    #[test]
    fn counts_an_album_again_in_another_sitting() {
        let mut history = History::default();
        sit(&mut history, "a", 3, day(1, 0), 300);
        // Another album between: another sitting.
        sit(&mut history, "b", 3, day(1, 2), 300);
        sit(&mut history, "a", 3, day(1, 4), 300);
        // Hours later: another sitting too.
        sit(&mut history, "a", 3, day(1, 12), 300);
        assert_eq!(history.album("a").count, 3);
        assert_eq!(history.album("a").last, Some(day(1, 12)));
        assert_eq!(history.album("b").count, 1);
    }

    #[test]
    fn counts_an_album_heard_twice_in_a_row_twice() {
        let mut history = History::default();
        sit(&mut history, "a", 5, day(1, 0), 300);
        sit(&mut history, "a", 5, day(1, 0) + 1500, 300);
        assert_eq!(history.album("a").count, 2);
    }

    #[test]
    fn reads_logs_skipping_broken_lines_and_older_fields() {
        let mut history = History::default();
        let album = serde_json::to_string(&Line::Album(album("a", "Zaz", 600))).unwrap();
        // Listens used to name their track.
        let heard = r#"{"listen":{"at":0,"album":"a","track":1,"title":"T","length":600,"heard":600,"ended":true}}"#;
        history.read(&format!("{album}\n{heard}\n{{\"listen\": {{\"at\"\n"));
        history.settle();
        assert_eq!(history.album("a").count, 1);
    }
}
