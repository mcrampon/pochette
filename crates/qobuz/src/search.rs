//! Searching the catalog for albums and artists.

use serde::Deserialize;

use crate::{
    Client, Result,
    artist::ArtistPreview,
    client::read_json,
    discover::Albums,
    models::{Album, Page},
};

impl Client {
    /// `limit` albums matching `query` from `offset`, the best matches first.
    pub async fn search_albums(&self, query: &str, offset: usize, limit: usize) -> Result<Albums> {
        #[derive(Deserialize)]
        struct Response {
            albums: Page<Album>,
        }

        let response = self
            .get("album/search")
            .query(&[("query", query)])
            .query(&[("offset", offset), ("limit", limit)])
            .send()
            .await?;
        let page = read_json::<Response>(response).await?.albums;
        Ok(Albums {
            has_more: !page.items.is_empty() && page.offset + page.items.len() < page.total,
            albums: page.items,
        })
    }

    /// The first `limit` artists matching `query`, the best matches first.
    pub async fn search_artists(&self, query: &str, limit: usize) -> Result<Vec<ArtistPreview>> {
        #[derive(Deserialize)]
        struct Response {
            artists: Page<FoundArtist>,
        }

        let response = self
            .get("artist/search")
            .query(&[("query", query)])
            .query(&[("limit", limit)])
            .send()
            .await?;
        let page = read_json::<Response>(response).await?.artists;
        Ok(page.items.into_iter().map(ArtistPreview::from).collect())
    }
}

/// An artist as the search lists them.
#[derive(Deserialize)]
struct FoundArtist {
    id: u64,
    name: String,
    image: Option<Portraits>,
}

#[derive(Deserialize)]
struct Portraits {
    medium: Option<String>,
}

impl From<FoundArtist> for ArtistPreview {
    fn from(artist: FoundArtist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            portrait: artist.image.and_then(|image| image.medium),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_found_artist() {
        let json = serde_json::json!({
            "picture": "https://static.qobuz.com/images/artists/covers/small/8051.jpg",
            "image": {
                "small": "https://static.qobuz.com/images/artists/covers/small/8051.jpg",
                "medium": "https://static.qobuz.com/images/artists/covers/medium/8051.jpg"
            },
            "name": "Gorillaz",
            "slug": "gorillaz",
            "albums_count": 191,
            "id": 45975
        });
        let artist = ArtistPreview::from(serde_json::from_value::<FoundArtist>(json).unwrap());
        assert_eq!(artist.name, "Gorillaz");
        assert_eq!(
            artist.portrait.as_deref(),
            Some("https://static.qobuz.com/images/artists/covers/medium/8051.jpg")
        );

        let without = serde_json::json!({"name": "Gorilla Zoe", "id": 1, "image": null});
        let artist = ArtistPreview::from(serde_json::from_value::<FoundArtist>(without).unwrap());
        assert_eq!(artist.portrait, None);
    }
}
