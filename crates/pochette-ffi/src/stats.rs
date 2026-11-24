//! The play history, from every device's log, and the stats it adds up to.

use std::sync::MutexGuard;

use pochette_core::{
    history::{self, History},
    sync::device,
};

use crate::{AlbumCard, Inner, Plays as AlbumPlays, Pochette, lock};

/// The history, and the logs it was read from, to read them again once they change.
pub(crate) struct Plays {
    history: History,
    signature: Vec<(String, u64)>,
}

impl Plays {
    pub fn load() -> Self {
        Self {
            signature: signature(),
            history: History::load(),
        }
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    /// Read the logs again if they changed, as when another device synced: whether they did.
    pub fn refresh(&mut self) -> bool {
        let signature = signature();
        if signature == self.signature {
            return false;
        }
        *self = Self {
            signature,
            history: History::load(),
        };
        true
    }

    /// Take in what this device just heard, which its log already has.
    pub fn add(&mut self, album: history::AlbumRecord, listen: history::Listen) {
        self.history.add(album, listen);
        self.signature = signature();
    }
}

fn signature() -> Vec<(String, u64)> {
    device::logs_signature(&device::data_dir())
}

impl Inner {
    /// The history, up to date with the logs.
    pub(crate) fn plays(&self) -> MutexGuard<'_, Plays> {
        let mut plays = lock(&self.plays);
        plays.refresh();
        plays
    }
}

/// A time span to sum up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Period {
    All,
    Year {
        year: i32,
    },
    /// `month` from 1.
    Month {
        year: i32,
        month: i32,
    },
}

impl From<Period> for history::Period {
    fn from(period: Period) -> Self {
        match period {
            Period::All => Self::All,
            Period::Year { year } => Self::Year(year),
            Period::Month { year, month } => Self::Month(year, month),
        }
    }
}

/// What a period's listening adds up to, the most played first.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Stats {
    /// The seconds heard.
    pub time: u64,
    pub album_plays: u32,
    pub albums: Vec<AlbumStat>,
    pub artists: Vec<ArtistStat>,
    pub genres: Vec<GenreStat>,
    /// The seconds heard by year for all time, by month for a year, by day for a month.
    pub timeline: Vec<TimelinePoint>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AlbumStat {
    pub album: AlbumCard,
    pub plays: u32,
    /// The seconds heard.
    pub time: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArtistStat {
    pub id: u64,
    pub name: String,
    pub time: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GenreStat {
    pub name: String,
    pub time: u64,
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct TimelinePoint {
    /// The year, the month from 1, or the day from 1.
    pub at: i32,
    pub time: u64,
}

#[uniffi::export]
impl Pochette {
    /// How often the album was played, on every device.
    #[must_use]
    pub fn album_plays(&self, album_id: &str) -> AlbumPlays {
        self.inner.plays().history().album(album_id).into()
    }

    #[must_use]
    pub fn stats(&self, period: Period) -> Stats {
        let summary = self.inner.plays().history().summary(period.into());
        Stats {
            time: summary.time,
            album_plays: summary.album_plays,
            albums: summary
                .albums
                .iter()
                .filter_map(|(record, plays, time)| {
                    Some(AlbumStat {
                        album: AlbumCard::from(&record.to_album()?),
                        plays: *plays,
                        time: *time,
                    })
                })
                .collect(),
            artists: summary
                .artists
                .into_iter()
                .map(|(artist, time)| ArtistStat {
                    id: artist.id,
                    name: artist.name,
                    time,
                })
                .collect(),
            genres: summary
                .genres
                .into_iter()
                .map(|(name, time)| GenreStat { name, time })
                .collect(),
            timeline: summary
                .timeline
                .into_iter()
                .map(|(at, time)| TimelinePoint { at, time })
                .collect(),
        }
    }

    /// The years with plays, for the stats' periods.
    #[must_use]
    pub fn stats_years(&self) -> Vec<i32> {
        self.inner.plays().history().years()
    }

    /// The months of `year` with plays, from 1.
    #[must_use]
    pub fn stats_months(&self, year: i32) -> Vec<i32> {
        self.inner.plays().history().months(year)
    }
}
