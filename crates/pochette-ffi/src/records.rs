//! What the screens show, as records Kotlin gets: made from Qobuz's models, with what the
//! app would otherwise work out itself (full titles, years, qualities) already there.

use pochette_core::{format, history::AlbumPlays};
use qobuz::{
    artist::{Artist, ArtistPreview},
    discover::Albums,
    models::{Album, Track, User},
};

/// The signed-in user.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Account {
    pub id: u64,
    /// The name to greet them with.
    pub name: String,
    pub avatar: Option<String>,
    /// Their subscription: "Studio".
    pub plan: Option<String>,
    /// Whether they can stream whole tracks; free accounts only get previews.
    pub can_stream: bool,
}

impl From<&User> for Account {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.name().to_owned(),
            avatar: user.avatar.clone(),
            plan: user
                .credential
                .as_ref()
                .and_then(|credential| credential.label.clone()),
            can_stream: user.can_stream(),
        }
    }
}

/// An album, as tiles and lists show it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AlbumCard {
    pub id: String,
    /// With its version: "Abbey Road (Remastered)".
    pub title: String,
    pub artist: String,
    pub artist_id: u64,
    /// 230 px.
    pub cover: Option<String>,
    /// 600 px.
    pub cover_large: Option<String>,
    /// Of the original release.
    pub year: Option<String>,
    pub genre: Option<String>,
    /// "24-bit / 96 kHz", when it's better than CD.
    pub hires: Option<String>,
    pub explicit: bool,
    pub streamable: bool,
}

impl From<&Album> for AlbumCard {
    fn from(album: &Album) -> Self {
        Self {
            id: album.id.clone(),
            title: album.full_title(),
            artist: album.artist.name.clone(),
            artist_id: album.artist.id,
            cover: album.image.small.clone(),
            cover_large: album.image.large.clone(),
            year: album.year().map(str::to_owned),
            genre: album.genre.as_ref().map(|genre| genre.name.clone()),
            hires: album
                .hires_streamable
                .then(|| {
                    format::audio_quality(album.maximum_bit_depth, album.maximum_sampling_rate)
                })
                .flatten(),
            explicit: album.parental_warning,
            streamable: album.streamable,
        }
    }
}

/// An album's page.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AlbumPage {
    pub card: AlbumCard,
    /// "Album", "Single", "Live"…
    pub kind: Option<String>,
    pub label: Option<String>,
    /// "16 October 2026".
    pub released: Option<String>,
    /// "9 tracks".
    pub track_count: String,
    /// "59 min".
    pub length: String,
    /// The best quality it streams in: "24-bit / 96 kHz", "16-bit / 44.1 kHz".
    pub quality: Option<String>,
    /// Qobuz's review, as text.
    pub description: Option<String>,
    pub copyright: Option<String>,
    pub awards: Vec<String>,
    /// More than one disc: the tracks say which they're on.
    pub discs: u32,
    pub tracks: Vec<TrackRow>,
    pub plays: Plays,
}

impl AlbumPage {
    pub(crate) fn new(album: &Album, plays: AlbumPlays) -> Self {
        let tracks: Vec<TrackRow> = album
            .tracks
            .iter()
            .flat_map(|page| &page.items)
            .map(TrackRow::from)
            .collect();
        let discs = tracks.iter().map(|track| track.disc).max().unwrap_or(1);
        Self {
            card: AlbumCard::from(album),
            kind: album.release_type.as_deref().map(capitalized),
            label: album.label.as_ref().map(|label| label.name.clone()),
            released: album
                .release_date_original
                .as_deref()
                .map(format::release_date),
            track_count: format::track_count(album.tracks_count),
            length: format::album_duration(album.duration),
            quality: format::audio_quality(album.maximum_bit_depth, album.maximum_sampling_rate),
            description: album
                .description
                .as_deref()
                .map(format::html_to_text)
                .filter(|text| !text.trim().is_empty()),
            copyright: album.copyright.clone(),
            awards: album
                .awards
                .iter()
                .map(|award| award.name.clone())
                .collect(),
            discs,
            tracks,
            plays: plays.into(),
        }
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A track, on its album's page.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TrackRow {
    pub id: u64,
    /// With its version: "Help! (Remastered 2009)".
    pub title: String,
    /// On its disc.
    pub number: u32,
    pub disc: u32,
    /// In seconds.
    pub duration: u32,
    pub performer: Option<String>,
    pub composer: Option<String>,
    /// The piece it's part of: "Symphony No. 5 in C minor, Op. 67".
    pub work: Option<String>,
    pub streamable: bool,
    pub explicit: bool,
}

impl From<&Track> for TrackRow {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id,
            title: track.full_title(),
            number: track.track_number,
            disc: track.media_number,
            duration: track.duration,
            performer: track.performer.as_ref().map(|artist| artist.name.clone()),
            composer: track.composer.as_ref().map(|artist| artist.name.clone()),
            work: track.work.clone(),
            streamable: track.streamable,
            explicit: track.parental_warning,
        }
    }
}

/// How often an album was played, on every device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Plays {
    pub count: u32,
    /// When it was last played, in Unix time.
    pub last: Option<i64>,
}

impl From<AlbumPlays> for Plays {
    fn from(plays: AlbumPlays) -> Self {
        Self {
            count: plays.count,
            last: plays.last,
        }
    }
}

/// Some albums of a longer list, and whether there are more.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AlbumList {
    pub albums: Vec<AlbumCard>,
    pub has_more: bool,
}

impl From<&Albums> for AlbumList {
    fn from(albums: &Albums) -> Self {
        Self {
            albums: albums.albums.iter().map(AlbumCard::from).collect(),
            has_more: albums.has_more,
        }
    }
}

/// The Discover tab's sections, with their first albums.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Discover {
    pub new_releases: AlbumList,
    /// Albums the press awarded.
    pub press_awards: AlbumList,
    /// Qobuz's essential albums.
    pub ideal_discography: AlbumList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Section {
    NewReleases,
    PressAwards,
    IdealDiscography,
}

impl From<Section> for qobuz::discover::Section {
    fn from(section: Section) -> Self {
        match section {
            Section::NewReleases => Self::NewReleases,
            Section::PressAwards => Self::PressAwards,
            Section::IdealDiscography => Self::IdealDiscography,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Genre {
    pub id: u64,
    pub name: String,
}

/// An artist, as lists show them.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArtistCard {
    pub id: u64,
    pub name: String,
    /// Square, 360 px.
    pub portrait: Option<String>,
}

impl From<&ArtistPreview> for ArtistCard {
    fn from(artist: &ArtistPreview) -> Self {
        Self {
            id: artist.id,
            name: artist.name.clone(),
            portrait: artist.portrait.clone(),
        }
    }
}

/// An artist's page.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ArtistPage {
    pub id: u64,
    pub name: String,
    pub portrait: Option<String>,
    /// As text.
    pub biography: Option<String>,
    /// The first releases of each kind that has some.
    pub releases: Vec<Releases>,
    pub similar: Vec<ArtistCard>,
}

impl From<&Artist> for ArtistPage {
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name.clone(),
            portrait: artist.portrait.clone(),
            biography: artist
                .biography
                .as_deref()
                .map(format::html_to_text)
                .filter(|text| !text.trim().is_empty()),
            releases: artist
                .releases
                .iter()
                .map(|(kind, albums)| Releases {
                    kind: (*kind).into(),
                    albums: albums.into(),
                })
                .collect(),
            similar: artist.similar.iter().map(ArtistCard::from).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Releases {
    pub kind: ReleaseKind,
    pub albums: AlbumList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReleaseKind {
    Album,
    EpSingle,
    Live,
    Compilation,
    /// Mostly others' takes on the artist's songs: covers, tributes.
    Other,
}

impl From<qobuz::artist::ReleaseKind> for ReleaseKind {
    fn from(kind: qobuz::artist::ReleaseKind) -> Self {
        use qobuz::artist::ReleaseKind as Kind;
        match kind {
            Kind::Album => Self::Album,
            Kind::EpSingle => Self::EpSingle,
            Kind::Live => Self::Live,
            Kind::Compilation => Self::Compilation,
            Kind::Other => Self::Other,
        }
    }
}

impl From<ReleaseKind> for qobuz::artist::ReleaseKind {
    fn from(kind: ReleaseKind) -> Self {
        match kind {
            ReleaseKind::Album => Self::Album,
            ReleaseKind::EpSingle => Self::EpSingle,
            ReleaseKind::Live => Self::Live,
            ReleaseKind::Compilation => Self::Compilation,
            ReleaseKind::Other => Self::Other,
        }
    }
}
