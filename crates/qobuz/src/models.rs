//! What the API returns.
//!
//! Fields are optional unless Qobuz always sends them: its responses vary between accounts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub id: u64,
    pub login: Option<String>,
    pub display_name: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub avatar: Option<String>,
    pub country_code: Option<String>,
    pub credential: Option<Credential>,
}

impl User {
    /// The name to greet the user with.
    #[must_use]
    pub fn name(&self) -> &str {
        [&self.display_name, &self.firstname, &self.login]
            .into_iter()
            .flatten()
            .find(|name| !name.is_empty())
            .map_or("Qobuz user", String::as_str)
    }

    /// Whether the subscription allows streaming full tracks.
    ///
    /// Free accounts have no credential parameters and only get 30 second samples.
    #[must_use]
    pub fn can_stream(&self) -> bool {
        self.credential
            .as_ref()
            .is_some_and(|c| c.parameters.is_some())
    }
}

/// The user's subscription.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Credential {
    /// The plan's name, such as "Studio".
    pub label: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<CredentialParameters>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CredentialParameters {
    #[serde(default)]
    pub lossy_streaming: bool,
    #[serde(default)]
    pub lossless_streaming: bool,
    #[serde(default)]
    pub hires_streaming: bool,
    pub short_label: Option<String>,
}

/// One page of a longer list.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page<T> {
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    /// Sets editions apart, such as "Deluxe Edition" or "Remastered".
    pub version: Option<String>,
    pub artist: ArtistSummary,
    pub image: AlbumImage,
    /// The original release date, as `YYYY-MM-DD`.
    pub release_date_original: Option<String>,
    #[serde(default)]
    pub tracks_count: u32,
    /// In seconds.
    #[serde(default)]
    pub duration: u32,
    pub genre: Option<Genre>,
    pub label: Option<Label>,
    #[serde(default)]
    pub streamable: bool,
    #[serde(default)]
    pub hires_streamable: bool,
    pub maximum_bit_depth: Option<u32>,
    /// In kHz.
    pub maximum_sampling_rate: Option<f32>,
    #[serde(default)]
    pub parental_warning: bool,
    /// When the user added it to their favorites, in Unix time. Only in favorites.
    pub favorited_at: Option<i64>,
    /// "album", "single", "epk", "live"… Only from `album/get`.
    pub release_type: Option<String>,
    /// How many discs.
    pub media_count: Option<u32>,
    /// The Qobuz review, in HTML. Only from `album/get`, and often empty.
    pub description: Option<String>,
    pub copyright: Option<String>,
    /// Prizes from Qobuz and the press: "Qobuzissime", "4F de Télérama"…
    #[serde(default)]
    pub awards: Vec<Award>,
    /// Only from `album/get`.
    pub tracks: Option<Page<Track>>,
}

impl Album {
    /// The title, with the version when there is one: "Abbey Road (Remastered)".
    #[must_use]
    pub fn full_title(&self) -> String {
        with_version(&self.title, self.version.as_deref())
    }

    /// The year of the original release.
    #[must_use]
    pub fn year(&self) -> Option<&str> {
        self.release_date_original.as_deref()?.get(..4)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ArtistSummary {
    pub id: u64,
    pub name: String,
}

/// The cover, in several sizes.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AlbumImage {
    /// 50 px.
    pub thumbnail: Option<String>,
    /// 230 px.
    pub small: Option<String>,
    /// 600 px.
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Genre {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Award {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Label {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Track {
    pub id: u64,
    pub title: String,
    pub version: Option<String>,
    /// The disc it's on, from 1.
    #[serde(default = "first")]
    pub media_number: u32,
    /// Its position on the disc, from 1.
    pub track_number: u32,
    /// In seconds.
    #[serde(default)]
    pub duration: u32,
    pub performer: Option<ArtistSummary>,
    pub composer: Option<ArtistSummary>,
    /// The piece it's part of, mostly in classical music: "Symphony No. 5 in C minor, Op. 67".
    pub work: Option<String>,
    /// Everyone who took part, with their roles, as one string.
    pub performers: Option<String>,
    #[serde(default)]
    pub streamable: bool,
    #[serde(default)]
    pub hires_streamable: bool,
    pub maximum_bit_depth: Option<u32>,
    /// In kHz.
    pub maximum_sampling_rate: Option<f32>,
    #[serde(default)]
    pub parental_warning: bool,
    /// The album it's on. Only from `track/get`.
    pub album: Option<TrackAlbum>,
}

/// The album a track is on.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackAlbum {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
}

impl TrackAlbum {
    /// The title, with the version when there is one: "Abbey Road (Remastered)".
    #[must_use]
    pub fn full_title(&self) -> String {
        with_version(&self.title, self.version.as_deref())
    }
}

const fn first() -> u32 {
    1
}

impl Track {
    /// The title, with the version when there is one: "Help! (Remastered 2009)".
    #[must_use]
    pub fn full_title(&self) -> String {
        with_version(&self.title, self.version.as_deref())
    }
}

fn with_version(title: &str, version: Option<&str>) -> String {
    match version.map(str::trim) {
        Some(version) if !version.is_empty() => format!("{title} ({version})"),
        _ => title.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real `favorite/getUserFavorites?type=albums` response.
    const FAVORITE_ALBUMS: &str = r#"{
        "albums": {"offset": 0, "limit": 1, "total": 666, "items": [{
            "maximum_bit_depth": 24,
            "image": {
                "small": "https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_230.jpg",
                "thumbnail": "https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_50.jpg",
                "large": "https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_600.jpg",
                "back": null
            },
            "artist": {"image": null, "name": "Radie Peat", "id": 3333061, "albums_count": 16},
            "label": {"name": "Rough Trade", "id": 9293, "albums_count": 1401},
            "title": "No Human Tongue Can Tell",
            "version": null,
            "duration": 3558,
            "parental_warning": false,
            "tracks_count": 9,
            "genre": {"path": [112, 119, 113], "name": "Alternatif et Indé", "id": 113},
            "id": "dpqdxzmrwt2a3",
            "maximum_sampling_rate": 44.1,
            "release_date_original": "2026-11-13",
            "streamable": true,
            "hires_streamable": true,
            "favorited_at": 1794576133
        }]},
        "user": {"id": 1, "login": "someone"}
    }"#;

    #[test]
    fn reads_favorite_albums() {
        #[derive(Deserialize)]
        struct Response {
            albums: Page<Album>,
        }
        let page = serde_json::from_str::<Response>(FAVORITE_ALBUMS)
            .unwrap()
            .albums;
        assert_eq!(page.total, 666);
        let album = &page.items[0];
        assert_eq!(album.full_title(), "No Human Tongue Can Tell");
        assert_eq!(album.artist.name, "Radie Peat");
        assert_eq!(album.year(), Some("2026"));
        assert_eq!(album.favorited_at, Some(1_794_576_133));
    }

    /// Trimmed from a real `album/get` response.
    const ALBUM: &str = r#"{
        "id": "dpqdxzmrwt2a3",
        "title": "No Human Tongue Can Tell",
        "version": null,
        "artist": {"name": "Radie Peat", "id": 3333061, "albums_count": 16},
        "image": {"small": "https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_230.jpg"},
        "release_type": "album",
        "media_count": 1,
        "description": "",
        "copyright": "2026 Radie Peat under exclusive license to Rough Trade Records Ltd",
        "tracks": {"offset": 0, "limit": 500, "total": 9, "items": [{
            "maximum_bit_depth": 24,
            "performers": "Radie Peat, MainArtist, AssociatedPerformer",
            "performer": {"name": "Radie Peat", "id": 3333061},
            "work": null,
            "composer": {"name": "Radie Peat", "id": 3333061},
            "title": "Hares on The Mountain",
            "version": null,
            "duration": 213,
            "parental_warning": false,
            "track_number": 1,
            "id": 446392172,
            "media_number": 1,
            "maximum_sampling_rate": 44.1,
            "streamable": true,
            "hires_streamable": true
        }]}
    }"#;

    #[test]
    fn reads_an_album_with_its_tracks() {
        let album: Album = serde_json::from_str(ALBUM).unwrap();
        assert_eq!(album.release_type.as_deref(), Some("album"));
        let tracks = album.tracks.unwrap();
        assert_eq!(tracks.total, 9);
        let track = &tracks.items[0];
        assert_eq!(track.full_title(), "Hares on The Mountain");
        assert_eq!((track.media_number, track.track_number), (1, 1));
        assert_eq!(track.duration, 213);
        assert_eq!(track.performer.as_ref().unwrap().name, "Radie Peat");
        assert_eq!(track.work, None);
    }

    #[test]
    fn the_full_title_names_the_version() {
        let mut album: Album = serde_json::from_str(
            r#"{"id": "x", "title": "Abbey Road", "version": "Remastered",
                "artist": {"id": 1, "name": "The Beatles"}, "image": {}}"#,
        )
        .unwrap();
        assert_eq!(album.full_title(), "Abbey Road (Remastered)");
        album.version = Some(" ".into());
        assert_eq!(album.full_title(), "Abbey Road");
    }
}
