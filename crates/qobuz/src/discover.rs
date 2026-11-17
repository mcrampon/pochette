//! Qobuz's editorial sections: new releases, albums the press awarded, the ideal
//! discography.

use serde::Deserialize;

use crate::{
    Client, Result,
    client::read_json,
    models::{Album, AlbumImage, ArtistSummary, Award, Genre, Label},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    NewReleases,
    /// Albums the press awarded: Télérama, Pitchfork, Diapason…
    PressAwards,
    /// Qobuz's essential albums.
    IdealDiscography,
}

impl Section {
    fn endpoint(self) -> &'static str {
        match self {
            Self::NewReleases => "discover/newReleases",
            Self::PressAwards => "discover/pressAward",
            Self::IdealDiscography => "discover/idealDiscography",
        }
    }
}

/// Albums from a section, and whether it has more.
#[derive(Debug, Clone, Default)]
pub struct Albums {
    pub albums: Vec<Album>,
    pub has_more: bool,
}

/// The first albums of each section, from one call.
#[derive(Debug, Clone, Default)]
pub struct Discover {
    pub new_releases: Albums,
    pub press_awards: Albums,
    pub ideal_discography: Albums,
}

impl Discover {
    #[must_use]
    pub fn section(&self, section: Section) -> &Albums {
        match section {
            Section::NewReleases => &self.new_releases,
            Section::PressAwards => &self.press_awards,
            Section::IdealDiscography => &self.ideal_discography,
        }
    }
}

impl Client {
    /// The first albums of each section, of genre `genre_id` or of all genres.
    pub async fn discover(&self, genre_id: Option<u64>) -> Result<Discover> {
        #[derive(Deserialize)]
        struct Index {
            containers: Containers,
        }
        #[derive(Deserialize)]
        struct Containers {
            new_releases: Option<Container>,
            press_awards: Option<Container>,
            ideal_discography: Option<Container>,
        }
        #[derive(Deserialize)]
        struct Container {
            data: Items,
        }

        let mut request = self.get("discover/index");
        if let Some(genre_id) = genre_id {
            request = request.query(&[("genre_ids", genre_id)]);
        }
        let index: Index = read_json(request.send().await?).await?;
        let albums = |container: Option<Container>| {
            container
                .map(|container| container.data.into())
                .unwrap_or_default()
        };
        let containers = index.containers;
        Ok(Discover {
            new_releases: albums(containers.new_releases),
            press_awards: albums(containers.press_awards),
            ideal_discography: albums(containers.ideal_discography),
        })
    }

    /// `limit` albums of `section` from `offset`, of genre `genre_id` or of all genres.
    pub async fn discover_section(
        &self,
        section: Section,
        genre_id: Option<u64>,
        offset: usize,
        limit: usize,
    ) -> Result<Albums> {
        let mut request = self
            .get(section.endpoint())
            .query(&[("offset", offset), ("limit", limit)]);
        if let Some(genre_id) = genre_id {
            request = request.query(&[("genre_ids", genre_id)]);
        }
        let items: Items = read_json(request.send().await?).await?;
        Ok(items.into())
    }

    /// The main genres, as the sections can be filtered by.
    pub async fn genres(&self) -> Result<Vec<Genre>> {
        #[derive(Deserialize)]
        struct Response {
            genres: Genres,
        }
        #[derive(Deserialize)]
        struct Genres {
            items: Vec<Genre>,
        }

        let response = self.get("genre/list").send().await?;
        Ok(read_json::<Response>(response).await?.genres.items)
    }
}

#[derive(Deserialize)]
struct Items {
    #[serde(default)]
    items: Vec<DiscoverAlbum>,
    #[serde(default)]
    has_more: bool,
}

impl From<Items> for Albums {
    fn from(items: Items) -> Self {
        Self {
            albums: items.items.into_iter().map(Album::from).collect(),
            has_more: items.has_more,
        }
    }
}

/// An album as the sections list it: the same album as elsewhere, in another shape.
#[derive(Deserialize)]
struct DiscoverAlbum {
    id: String,
    title: String,
    version: Option<String>,
    #[serde(default)]
    artists: Vec<DiscoverArtist>,
    image: AlbumImage,
    #[serde(default)]
    track_count: u32,
    #[serde(default)]
    duration: u32,
    genre: Option<Genre>,
    label: Option<Label>,
    #[serde(default)]
    parental_warning: bool,
    dates: Option<Dates>,
    #[serde(default)]
    awards: Vec<Award>,
    audio_info: Option<AudioInfo>,
    rights: Option<Rights>,
}

#[derive(Deserialize)]
struct DiscoverArtist {
    id: u64,
    name: String,
    #[serde(default)]
    roles: Vec<String>,
}

#[derive(Deserialize)]
struct Dates {
    original: Option<String>,
}

#[derive(Deserialize)]
struct AudioInfo {
    maximum_bit_depth: Option<u32>,
    maximum_sampling_rate: Option<f32>,
}

#[derive(Deserialize)]
struct Rights {
    #[serde(default)]
    streamable: bool,
    #[serde(default)]
    hires_streamable: bool,
}

impl From<DiscoverAlbum> for Album {
    fn from(album: DiscoverAlbum) -> Self {
        let rights = album.rights.as_ref();
        let audio = album.audio_info.as_ref();
        Self {
            artist: main_artist(&album.artists),
            id: album.id,
            title: album.title,
            version: album.version,
            image: album.image,
            release_date_original: album.dates.and_then(|dates| dates.original),
            tracks_count: album.track_count,
            duration: album.duration,
            genre: album.genre,
            label: album.label,
            streamable: rights.is_some_and(|rights| rights.streamable),
            hires_streamable: rights.is_some_and(|rights| rights.hires_streamable),
            maximum_bit_depth: audio.and_then(|audio| audio.maximum_bit_depth),
            maximum_sampling_rate: audio.and_then(|audio| audio.maximum_sampling_rate),
            parental_warning: album.parental_warning,
            awards: album.awards,
            favorited_at: None,
            release_type: None,
            media_count: None,
            description: None,
            copyright: None,
            tracks: None,
        }
    }
}

/// The main artists as one: "Armand Hammer, billy woods & E L U C I D".
fn main_artist(artists: &[DiscoverArtist]) -> ArtistSummary {
    let main: Vec<&DiscoverArtist> = artists
        .iter()
        .filter(|artist| artist.roles.iter().any(|role| role == "main-artist"))
        .collect();
    let main = if main.is_empty() {
        artists.iter().collect()
    } else {
        main
    };
    let names: Vec<&str> = main.iter().map(|artist| artist.name.as_str()).collect();
    let name = match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} & {last}", rest.join(", ")),
    };
    ArtistSummary {
        id: main.first().map_or(0, |artist| artist.id),
        name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_section_album() {
        let json = serde_json::json!({
            "id": "tcrx83ajnxyrb",
            "title": "Snakes & Ladders",
            "version": null,
            "track_count": 16,
            "duration": 2600,
            "parental_warning": false,
            "image": {"large": "https://static.qobuz.com/images/covers/rb/xy/tcrx83ajnxyrb_600.jpg"},
            "artists": [
                {"id": 3_322_343, "name": "Armand Hammer", "roles": ["main-artist"]},
                {"id": 1_624_644, "name": "billy woods", "roles": ["main-artist"]},
                {"id": 7_910_387, "name": "E L U C I D", "roles": ["main-artist"]},
                {"id": 1, "name": "A Producer", "roles": ["producer"]}
            ],
            "label": {"id": 8_374_024, "name": "Backwoodz Studioz LLC"},
            "genre": {"id": 133, "name": "Hip-Hop/Rap", "path": [133]},
            "dates": {"original": "2026-11-11", "stream": "2026-11-11"},
            "awards": [{"id": 74, "name": "Pitchfork: Best New Music", "awarded_at": "2026-11-11"}],
            "audio_info": {"maximum_sampling_rate": 48, "maximum_bit_depth": 24},
            "rights": {"streamable": true, "hires_streamable": true}
        });
        let album = Album::from(serde_json::from_value::<DiscoverAlbum>(json).unwrap());
        assert_eq!(
            album.artist.name,
            "Armand Hammer, billy woods & E L U C I D"
        );
        assert_eq!(album.artist.id, 3_322_343);
        assert_eq!(album.year(), Some("2026"));
        assert_eq!(album.awards[0].name, "Pitchfork: Best New Music");
        assert_eq!(album.maximum_bit_depth, Some(24));
        assert!(album.streamable);
    }

    #[test]
    fn names_a_single_artist_alone() {
        let artists = [DiscoverArtist {
            id: 1,
            name: "Ragga Twins".into(),
            roles: vec!["main-artist".into()],
        }];
        assert_eq!(main_artist(&artists).name, "Ragga Twins");
    }
}
