//! Searching, filtering and sorting the saved albums. No UI here, so it can be tested.

use std::{cmp::Ordering, rc::Rc};

use qobuz::models::Album;
use serde::{Deserialize, Serialize};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::history::History;

/// `text` in lower case and without accents, so "Beyoncé" matches "beyonce".
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// An album, with what searching and sorting it needs worked out once.
pub struct Entry {
    pub album: Rc<Album>,
    /// Everything a search looks through, folded.
    search: String,
    title: String,
    artist: String,
    pub decade: Option<u16>,
}

impl Entry {
    pub fn new(album: Album) -> Self {
        let label = album.label.as_ref().map(|label| label.name.as_str());
        let search = fold(
            &[
                album.title.as_str(),
                album.version.as_deref().unwrap_or_default(),
                album.artist.name.as_str(),
                label.unwrap_or_default(),
            ]
            .join(" "),
        );
        Self {
            title: fold(&album.full_title()),
            artist: fold(&album.artist.name),
            decade: decade(&album),
            search,
            album: Rc::new(album),
        }
    }

    fn genre(&self) -> Option<&str> {
        Some(self.album.genre.as_ref()?.name.as_str())
    }
}

/// The decade of the original release: 1990 for 1997.
fn decade(album: &Album) -> Option<u16> {
    let year: u16 = album.year()?.parse().ok()?;
    Some(year / 10 * 10)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    DateAdded,
    Title,
    Artist,
    ReleaseDate,
    LastPlayed,
    PlayCount,
}

impl Sort {
    /// In the order the sort menu lists them.
    pub const ALL: [Self; 6] = [
        Self::DateAdded,
        Self::Title,
        Self::Artist,
        Self::ReleaseDate,
        Self::LastPlayed,
        Self::PlayCount,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DateAdded => "Date Added",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::ReleaseDate => "Release Date",
            Self::LastPlayed => "Last Played",
            Self::PlayCount => "Play Count",
        }
    }

    /// Dates go newest first, and counts highest first; names from A to Z.
    fn newest_first(self) -> bool {
        !matches!(self, Self::Title | Self::Artist)
    }

    /// Whether the order depends on the plays.
    pub fn uses_plays(self) -> bool {
        matches!(self, Self::LastPlayed | Self::PlayCount)
    }

    fn compare(self, a: &Entry, b: &Entry, history: &History) -> Ordering {
        let release = |entry: &Entry| entry.album.release_date_original.clone();
        let added = || b.album.favorited_at.cmp(&a.album.favorited_at);
        let (plays_a, plays_b) = (history.album(&a.album.id), history.album(&b.album.id));
        match self {
            Self::DateAdded => added(),
            // Albums never played last.
            Self::LastPlayed => plays_b.last.cmp(&plays_a.last).then_with(added),
            Self::PlayCount => plays_b
                .count
                .cmp(&plays_a.count)
                .then_with(|| plays_b.last.cmp(&plays_a.last))
                .then_with(added),
            Self::Title => a.title.cmp(&b.title).then_with(|| a.artist.cmp(&b.artist)),
            // An artist's albums in the order they came out.
            Self::Artist => a
                .artist
                .cmp(&b.artist)
                .then_with(|| release(a).cmp(&release(b)))
                .then_with(|| a.title.cmp(&b.title)),
            // Albums without a date last.
            Self::ReleaseDate => release(b).cmp(&release(a)),
        }
    }
}

/// Which albums to show, and in what order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    pub query: String,
    /// The genres to show, or all of them when empty.
    pub genres: Vec<String>,
    /// The decades to show, or all of them when empty.
    pub decades: Vec<u16>,
    pub sort: Sort,
    /// The other way round from the sort's natural order.
    pub reversed: bool,
}

impl View {
    pub fn apply<'a>(&self, entries: &'a [Entry], history: &History) -> Vec<&'a Entry> {
        let words: Vec<String> = fold(&self.query)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let mut shown: Vec<&Entry> = entries
            .iter()
            .filter(|entry| words.iter().all(|word| entry.search.contains(word)))
            .filter(|entry| {
                self.genres.is_empty()
                    || entry
                        .genre()
                        .is_some_and(|genre| self.genres.iter().any(|picked| picked == genre))
            })
            .filter(|entry| {
                self.decades.is_empty()
                    || entry
                        .decade
                        .is_some_and(|decade| self.decades.contains(&decade))
            })
            .collect();
        self.sort(&mut shown, history);
        shown
    }

    /// All of `entries`, whatever the search and filters, in order.
    pub fn sorted<'a>(&self, entries: &'a [Entry], history: &History) -> Vec<&'a Entry> {
        let mut all: Vec<&Entry> = entries.iter().collect();
        self.sort(&mut all, history);
        all
    }

    fn sort(&self, entries: &mut [&Entry], history: &History) {
        // Stable, so ties keep the order Qobuz gave.
        entries.sort_by(|a, b| self.sort.compare(a, b, history));
        if self.reversed {
            entries.reverse();
        }
    }

    /// Whether anything hides some of the albums.
    pub fn is_filtered(&self) -> bool {
        !self.query.trim().is_empty() || !self.genres.is_empty() || !self.decades.is_empty()
    }

    /// How the order reads: "Newest First", "A to Z"…
    pub fn order_label(&self) -> &'static str {
        match (self.sort, self.reversed) {
            (Sort::LastPlayed, false) => return "Most Recent First",
            (Sort::LastPlayed, true) => return "Least Recent First",
            (Sort::PlayCount, false) => return "Most Played First",
            (Sort::PlayCount, true) => return "Least Played First",
            _ => {}
        }
        match (self.sort.newest_first(), self.reversed) {
            (true, false) => "Newest First",
            (true, true) => "Oldest First",
            (false, false) => "A to Z",
            (false, true) => "Z to A",
        }
    }

    /// Whether the list goes from high to low: newest first, or Z to A.
    pub fn is_descending(&self) -> bool {
        self.sort.newest_first() != self.reversed
    }
}

/// The genres of `entries`, in alphabetical order.
pub fn genres(entries: &[Entry]) -> Vec<String> {
    let mut genres: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.genre().map(str::to_owned))
        .collect();
    genres.sort_by_cached_key(|genre| fold(genre));
    genres.dedup();
    genres
}

/// The decades of `entries`, newest first.
pub fn decades(entries: &[Entry]) -> Vec<u16> {
    let mut decades: Vec<u16> = entries.iter().filter_map(|entry| entry.decade).collect();
    decades.sort_unstable_by(|a, b| b.cmp(a));
    decades.dedup();
    decades
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(title: &str, artist: &str, genre: &str, released: &str, added: i64) -> Entry {
        let json = serde_json::json!({
            "id": title,
            "title": title,
            "artist": {"id": 1, "name": artist},
            "image": {},
            "genre": {"id": 1, "name": genre},
            "release_date_original": released,
            "favorited_at": added,
        });
        Entry::new(serde_json::from_value(json).unwrap())
    }

    fn library() -> Vec<Entry> {
        vec![
            album("Lemonade", "Beyoncé", "Pop", "2016-04-23", 3),
            album("Kind of Blue", "Miles Davis", "Jazz", "1959-08-17", 1),
            album("Bitches Brew", "Miles Davis", "Jazz", "1970-03-30", 2),
            album("Écoute", "Zaz", "Chanson française", "2010-05-10", 4),
        ]
    }

    fn titles(shown: &[&Entry]) -> Vec<String> {
        shown
            .iter()
            .map(|entry| entry.album.title.clone())
            .collect()
    }

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Beyoncé ÉCOUTE"), "beyonce ecoute");
    }

    #[test]
    fn searches_titles_and_artists_whatever_the_accents() {
        let library = library();
        let view = View {
            query: "beyonce".into(),
            ..View::default()
        };
        assert_eq!(
            titles(&view.apply(&library, &History::default())),
            ["Lemonade"]
        );

        let view = View {
            query: "  davis   brew ".into(),
            ..View::default()
        };
        assert_eq!(
            titles(&view.apply(&library, &History::default())),
            ["Bitches Brew"]
        );
    }

    #[test]
    fn filters_by_genre_and_decade() {
        let library = library();
        let view = View {
            genres: vec!["Jazz".into()],
            decades: vec![1950],
            ..View::default()
        };
        assert_eq!(
            titles(&view.apply(&library, &History::default())),
            ["Kind of Blue"]
        );
    }

    #[test]
    fn filters_by_several_decades() {
        let library = library();
        let view = View {
            decades: vec![1950, 1970],
            ..View::default()
        };
        assert_eq!(
            titles(&view.apply(&library, &History::default())),
            ["Bitches Brew", "Kind of Blue"]
        );
    }
    #[test]
    fn filters_by_several_genres() {
        let library = library();
        let view = View {
            genres: vec!["Pop".into(), "Chanson française".into()],
            ..View::default()
        };
        assert_eq!(
            titles(&view.apply(&library, &History::default())),
            ["Écoute", "Lemonade"]
        );
    }

    #[test]
    fn sorts() {
        let library = library();
        let sorted = |sort, reversed| {
            titles(
                &View {
                    sort,
                    reversed,
                    ..View::default()
                }
                .apply(&library, &History::default()),
            )
        };
        assert_eq!(
            sorted(Sort::DateAdded, false),
            ["Écoute", "Lemonade", "Bitches Brew", "Kind of Blue"]
        );
        assert_eq!(
            sorted(Sort::Title, false),
            ["Bitches Brew", "Écoute", "Kind of Blue", "Lemonade"]
        );
        assert_eq!(
            sorted(Sort::Artist, false),
            ["Lemonade", "Kind of Blue", "Bitches Brew", "Écoute"]
        );
        assert_eq!(
            sorted(Sort::ReleaseDate, true),
            ["Kind of Blue", "Bitches Brew", "Écoute", "Lemonade"]
        );
    }

    #[test]
    fn sorts_by_plays() {
        use crate::history::{AlbumRecord, ArtistRecord, Listen};

        let library = library();
        let mut history = History::default();
        // Lemonade played twice, long ago; Kind of Blue once, lately; a third of Écoute, which
        // isn't a play.
        for (album, at, heard) in [
            ("Lemonade", 0, 600),
            ("Lemonade", 100_000, 600),
            ("Kind of Blue", 200_000, 600),
            ("Écoute", 300_000, 200),
        ] {
            let record = AlbumRecord {
                id: album.into(),
                title: album.into(),
                version: None,
                artist: ArtistRecord {
                    id: 1,
                    name: "Someone".into(),
                },
                genre: None,
                cover: None,
                cover_large: None,
                released: None,
                length: 600,
            };
            let listen = Listen {
                at,
                album: album.into(),
                heard,
            };
            history.add(record, listen);
        }
        let sorted = |sort, reversed| {
            let view = View {
                sort,
                reversed,
                ..View::default()
            };
            titles(&view.apply(&library, &history))
        };
        assert_eq!(
            sorted(Sort::PlayCount, false),
            ["Lemonade", "Kind of Blue", "Écoute", "Bitches Brew"]
        );
        assert_eq!(
            sorted(Sort::LastPlayed, false),
            ["Kind of Blue", "Lemonade", "Écoute", "Bitches Brew"]
        );
        // Never played first: the least recently played.
        assert_eq!(
            sorted(Sort::LastPlayed, true),
            ["Bitches Brew", "Écoute", "Lemonade", "Kind of Blue"]
        );
    }

    #[test]
    fn sorts_everything_whatever_the_filters() {
        let library = library();
        let view = View {
            query: "miles".into(),
            sort: Sort::Title,
            ..View::default()
        };
        let all: Vec<&str> = view
            .sorted(&library, &History::default())
            .iter()
            .map(|entry| entry.album.title.as_str())
            .collect();
        assert_eq!(all, ["Bitches Brew", "Écoute", "Kind of Blue", "Lemonade"]);
    }
    #[test]
    fn lists_genres_and_decades() {
        let library = library();
        assert_eq!(genres(&library), ["Chanson française", "Jazz", "Pop"]);
        assert_eq!(decades(&library), [2010, 1970, 1950]);
    }
}
