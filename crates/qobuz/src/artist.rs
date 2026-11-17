//! An artist's page: their portrait and biography, their releases by kind, and artists
//! like them.

use serde::Deserialize;

use crate::{
    Client, Result,
    client::read_json,
    discover::{Albums, Items},
};

/// The kinds of releases an artist page groups, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseKind {
    Album,
    EpSingle,
    Live,
    Compilation,
    /// Mostly others' takes on the artist's songs: covers, tributes.
    Other,
}

impl ReleaseKind {
    pub const ALL: [Self; 5] = [
        Self::Album,
        Self::EpSingle,
        Self::Live,
        Self::Compilation,
        Self::Other,
    ];

    /// The name the API knows it by.
    fn api_name(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::EpSingle => "epSingle",
            Self::Live => "live",
            Self::Compilation => "compilation",
            Self::Other => "other",
        }
    }

    fn from_api_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.api_name() == name)
    }
}

#[derive(Debug, Clone)]
pub struct Artist {
    pub id: u64,
    pub name: String,
    /// A square portrait, 360 px, when Qobuz has one.
    pub portrait: Option<String>,
    /// In HTML, as Qobuz writes it. Often missing.
    pub biography: Option<String>,
    /// The first releases of each kind that has some, in the order of [`ReleaseKind::ALL`].
    pub releases: Vec<(ReleaseKind, Albums)>,
    pub similar: Vec<SimilarArtist>,
}

#[derive(Debug, Clone)]
pub struct SimilarArtist {
    pub id: u64,
    pub name: String,
    /// A square portrait, 360 px, when Qobuz has one.
    pub portrait: Option<String>,
}

impl Client {
    /// The page of the artist `id`, with their first releases of each kind.
    pub async fn artist(&self, id: u64) -> Result<Artist> {
        let response = self
            .get("artist/page")
            .query(&[("artist_id", id)])
            .send()
            .await?;
        Ok(read_json::<ArtistPage>(response).await?.into())
    }

    /// `limit` releases of the artist `id` of `kind` from `offset`, newest first.
    pub async fn artist_releases(
        &self,
        id: u64,
        kind: ReleaseKind,
        offset: usize,
        limit: usize,
    ) -> Result<Albums> {
        let response = self
            .get("artist/getReleasesList")
            .query(&[
                ("artist_id", id.to_string()),
                ("offset", offset.to_string()),
            ])
            .query(&[
                ("limit", limit.to_string().as_str()),
                ("release_type", kind.api_name()),
                ("sort", "release_date"),
                ("order", "desc"),
            ])
            .send()
            .await?;
        Ok(read_json::<Items>(response).await?.into())
    }
}

#[derive(Deserialize)]
struct ArtistPage {
    id: u64,
    name: Name,
    images: Option<Images>,
    biography: Option<Biography>,
    #[serde(default)]
    releases: Vec<Releases>,
    similar_artists: Option<Similar>,
}

#[derive(Deserialize)]
struct Name {
    display: String,
}

#[derive(Deserialize)]
struct Images {
    portrait: Option<Portrait>,
}

#[derive(Deserialize)]
struct Portrait {
    hash: String,
    format: String,
}

impl Portrait {
    fn url(&self) -> String {
        format!(
            "https://static.qobuz.com/images/artists/covers/medium/{}.{}",
            self.hash, self.format
        )
    }
}

#[derive(Deserialize)]
struct Biography {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Releases {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    items: Items,
}

#[derive(Deserialize)]
struct Similar {
    #[serde(default)]
    items: Vec<SimilarItem>,
}

#[derive(Deserialize)]
struct SimilarItem {
    id: u64,
    name: Name,
    images: Option<Images>,
}

fn portrait(images: Option<&Images>) -> Option<String> {
    images?.portrait.as_ref().map(Portrait::url)
}

impl From<ArtistPage> for Artist {
    fn from(page: ArtistPage) -> Self {
        // The page lists awarded releases and a few others besides the kinds; and it may list
        // a kind twice.
        let mut releases: Vec<(ReleaseKind, Albums)> = Vec::new();
        for group in page.releases {
            let Some(kind) = ReleaseKind::from_api_name(&group.kind) else {
                continue;
            };
            let albums = Albums::from(group.items);
            if !albums.albums.is_empty() && releases.iter().all(|(known, _)| *known != kind) {
                releases.push((kind, albums));
            }
        }
        releases.sort_by_key(|(kind, _)| ReleaseKind::ALL.iter().position(|k| k == kind));

        Self {
            id: page.id,
            name: page.name.display,
            portrait: portrait(page.images.as_ref()),
            biography: page
                .biography
                .and_then(|biography| biography.content)
                .filter(|content| !content.trim().is_empty()),
            releases,
            similar: page
                .similar_artists
                .map(|similar| similar.items)
                .unwrap_or_default()
                .into_iter()
                .map(|artist| SimilarArtist {
                    portrait: portrait(artist.images.as_ref()),
                    id: artist.id,
                    name: artist.name.display,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": "Hail to the Thief",
            "artists": [{"id": 43840, "name": "Radiohead", "roles": ["main-artist"]}],
            "image": {"small": format!("https://static.qobuz.com/images/covers/{id}_230.jpg")},
            "dates": {"original": "2003-06-09"},
            "rights": {"streamable": true}
        })
    }

    #[test]
    fn reads_an_artist_page() {
        let json = serde_json::json!({
            "id": 43840,
            "name": {"display": "Radiohead"},
            "artist_category": "performer",
            "images": {"portrait": {"hash": "ae36", "format": "jpg"}},
            "biography": {"content": "<p>Formé en 1991 à Oxford…</p>", "language": "fr"},
            "releases": [
                {"type": "album", "has_more": false, "items": [album("a1"), album("a2")]},
                {"type": "live", "has_more": false, "items": [album("l1")]},
                {"type": "compilation", "has_more": false, "items": []},
                {"type": "epSingle", "has_more": true, "items": [album("e1")]},
                {"type": "download", "has_more": false, "items": [album("d1")]},
                {"type": "awardedRelease", "has_more": false, "items": [album("a1")]},
                {"type": "awardedRelease", "has_more": false, "items": [album("a1")]}
            ],
            "similar_artists": {"has_more": false, "items": [
                {"id": 27940, "name": {"display": "Björk"}, "images": {"portrait": null}}
            ]},
            "top_tracks": []
        });
        let artist = Artist::from(serde_json::from_value::<ArtistPage>(json).unwrap());

        assert_eq!(artist.name, "Radiohead");
        assert_eq!(
            artist.portrait.as_deref(),
            Some("https://static.qobuz.com/images/artists/covers/medium/ae36.jpg")
        );
        assert!(artist.biography.is_some());
        let kinds: Vec<ReleaseKind> = artist.releases.iter().map(|(kind, _)| *kind).collect();
        // In the page's order, without empty kinds or those it doesn't group by.
        assert_eq!(
            kinds,
            [ReleaseKind::Album, ReleaseKind::EpSingle, ReleaseKind::Live]
        );
        assert_eq!(artist.releases[0].1.albums.len(), 2);
        assert!(artist.releases[1].1.has_more);
        assert_eq!(artist.releases[0].1.albums[0].artist.name, "Radiohead");
        assert_eq!(artist.similar[0].name, "Björk");
        assert_eq!(artist.similar[0].portrait, None);
    }

    #[test]
    fn reads_an_artist_without_biography_or_portrait() {
        let json = serde_json::json!({
            "id": 3_333_061,
            "name": {"display": "Radie Peat"},
            "images": {"portrait": null},
            "biography": null,
            "releases": [],
            "similar_artists": null
        });
        let artist = Artist::from(serde_json::from_value::<ArtistPage>(json).unwrap());
        assert_eq!(artist.portrait, None);
        assert_eq!(artist.biography, None);
        assert!(artist.releases.is_empty());
        assert!(artist.similar.is_empty());
    }
}
