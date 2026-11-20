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

    /// The album, as tiles and pages take it: what's missing, they load.
    pub fn to_album(&self) -> Option<Album> {
        let json = serde_json::json!({
            "id": self.id,
            "title": self.title,
            "version": self.version,
            "artist": {"id": self.artist.id, "name": self.artist.name},
            "image": {"small": self.cover, "large": self.cover_large},
            "genre": self.genre.as_ref().map(|name| serde_json::json!({"id": 0, "name": name})),
            "release_date_original": self.released,
            "duration": self.length,
            "streamable": true,
        });
        serde_json::from_value(json).ok()
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

/// Now, in Unix time.
pub fn now() -> i64 {
    glib::real_time() / 1_000_000
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

/// A time span to sum up: all time, a year, or a month of a year, from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    All,
    Year(i32),
    Month(i32, i32),
}

impl Period {
    fn contains(self, at: i64) -> bool {
        let (year, month, _) = local(at);
        match self {
            Self::All => true,
            Self::Year(y) => year == y,
            Self::Month(y, m) => (year, month) == (y, m),
        }
    }

    /// Where `at` goes in the timeline: its year for all time, its month for a year, its
    /// day for a month.
    fn bucket(self, at: i64) -> i32 {
        let (year, month, day) = local(at);
        match self {
            Self::All => year,
            Self::Year(_) => month,
            Self::Month(..) => day,
        }
    }
}

/// The year, month and day of `at`, in the local time zone.
fn local(at: i64) -> (i32, i32, i32) {
    glib::DateTime::from_unix_local(at)
        .map(|date| (date.year(), date.month(), date.day_of_month()))
        .unwrap_or_default()
}

/// How many days `month` of `year` has.
fn days_in_month(year: i32, month: i32) -> i32 {
    glib::DateTime::from_local(year, month, 1, 12, 0, 0.0)
        .and_then(|first| first.add_months(1))
        .and_then(|next| next.add_days(-1))
        .map_or(31, |last| last.day_of_month())
}

/// What a period's listening adds up to, the most played first.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// The seconds heard.
    pub time: u64,
    pub album_plays: u32,
    /// Albums with their plays and the seconds heard.
    pub albums: Vec<(AlbumRecord, u32, u64)>,
    /// Artists with the seconds heard.
    pub artists: Vec<(ArtistRecord, u64)>,
    /// Genres with the seconds heard.
    pub genres: Vec<(String, u64)>,
    /// The seconds heard by year for all time, by month for a year, by day for a month.
    pub timeline: Vec<(i32, u64)>,
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

    pub fn is_empty(&self) -> bool {
        self.listens.is_empty()
    }

    /// The years with listening, the latest first.
    pub fn years(&self) -> Vec<i32> {
        let mut years: Vec<i32> = self.listens.iter().map(|l| local(l.at).0).collect();
        years.sort_unstable_by(|a, b| b.cmp(a));
        years.dedup();
        years
    }

    /// The months of `year` with listening, from January.
    pub fn months(&self, year: i32) -> Vec<i32> {
        let mut months: Vec<i32> = self
            .listens
            .iter()
            .map(|listen| local(listen.at))
            .filter(|(y, _, _)| *y == year)
            .map(|(_, month, _)| month)
            .collect();
        months.sort_unstable();
        months.dedup();
        months
    }

    pub fn summary(&self, period: Period) -> Summary {
        let mut summary = Summary::default();

        let mut album_plays: HashMap<&str, u32> = HashMap::new();
        for (album, _) in self
            .album_plays
            .iter()
            .filter(|(_, at)| period.contains(*at))
        {
            *album_plays.entry(album).or_default() += 1;
            summary.album_plays += 1;
        }

        let mut album_time: HashMap<&str, u64> = HashMap::new();
        let mut artist_time: HashMap<&ArtistRecord, u64> = HashMap::new();
        let mut genre_time: HashMap<&str, u64> = HashMap::new();
        let mut timeline: HashMap<i32, u64> = HashMap::new();
        for listen in self.listens.iter().filter(|l| period.contains(l.at)) {
            let heard = u64::from(listen.heard);
            summary.time += heard;
            *album_time.entry(&listen.album).or_default() += heard;
            if let Some(album) = self.albums.get(&listen.album) {
                *artist_time.entry(&album.artist).or_default() += heard;
                if let Some(genre) = &album.genre {
                    *genre_time.entry(genre).or_default() += heard;
                }
            }
            *timeline.entry(period.bucket(listen.at)).or_default() += heard;
        }

        summary.albums = album_time
            .iter()
            .filter_map(|(id, time)| {
                let album = self.albums.get(*id)?.clone();
                Some((
                    album,
                    album_plays.get(id).copied().unwrap_or_default(),
                    *time,
                ))
            })
            .collect();
        summary.albums.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(b.2.cmp(&a.2))
                .then(a.0.title.cmp(&b.0.title))
        });
        summary.artists = artist_time
            .into_iter()
            .map(|(artist, time)| (artist.clone(), time))
            .collect();
        summary
            .artists
            .sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));
        summary.genres = genre_time
            .into_iter()
            .map(|(genre, time)| (genre.to_owned(), time))
            .collect();
        summary
            .genres
            .sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        summary.timeline = self.timeline(period, &timeline);
        summary
    }

    /// The seconds heard in each part of `period`, from those heard in the parts with
    /// some.
    fn timeline(&self, period: Period, heard: &HashMap<i32, u64>) -> Vec<(i32, u64)> {
        let buckets = match period {
            Period::All => {
                let years = self.years();
                match (years.last(), years.first()) {
                    (Some(&first), Some(&last)) => first..=last,
                    _ => return Vec::new(),
                }
            }
            Period::Year(_) => 1..=12,
            Period::Month(year, month) => 1..=days_in_month(year, month),
        };
        buckets
            .map(|bucket| (bucket, heard.get(&bucket).copied().unwrap_or_default()))
            .collect()
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
    fn sums_up_a_year_and_a_month() {
        let mut history = History::default();
        sit(&mut history, "old", 5, day(1, 0) - 365 * 24 * 3600, 300);
        sit(&mut history, "a", 5, day(3, 0), 300);
        sit(&mut history, "old", 5, day(3, 4), 300);
        sit(&mut history, "a", 5, day(5, 0), 300);
        let mut record = album("b", "Bill Evans", 1500);
        record.genre = Some("Bebop".into());
        history.add(record, listen("b", day(5, 4), 30));

        assert_eq!(history.years(), [2026, 2025]);
        assert_eq!(history.months(2026), [3, 5]);
        let year = history.summary(Period::Year(2026));
        assert_eq!(year.album_plays, 3);
        assert_eq!(year.time, 15 * 300 + 30);
        let albums: Vec<(&str, u32)> = year
            .albums
            .iter()
            .map(|(album, plays, _)| (album.id.as_str(), *plays))
            .collect();
        assert_eq!(albums, [("a", 2), ("old", 1), ("b", 0)]);
        assert_eq!(year.artists[0].0.name, "Miles Davis");
        assert_eq!(year.genres[1], ("Bebop".to_owned(), 30));
        assert_eq!(year.timeline.len(), 12);
        assert_eq!(year.timeline[2], (3, 3000));
        assert_eq!(year.timeline[4], (5, 1530));

        let may = history.summary(Period::Month(2026, 5));
        assert_eq!(may.album_plays, 1);
        assert_eq!(may.timeline.len(), 31);
        assert_eq!(may.timeline[14], (15, 1530));

        let all = history.summary(Period::All);
        assert_eq!(all.album_plays, 4);
        assert_eq!(all.timeline, [(2025, 1500), (2026, 4530)]);
    }

    #[test]
    fn knows_the_length_of_months() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2026, 12), 31);
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
        assert!(history.albums["a"].to_album().is_some());
    }
}
