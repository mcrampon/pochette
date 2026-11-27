//! The Qobuz links the app is handed: what they point to.

use pochette_core::link::{self, Link};

/// What a Qobuz link points to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum QobuzLink {
    Album { id: String },
    Artist { id: u64 },
    Track { id: u64 },
}

impl From<Link> for QobuzLink {
    fn from(link: Link) -> Self {
        match link {
            Link::Album(id) => Self::Album { id },
            Link::Artist(id) => Self::Artist { id },
            Link::Track(id) => Self::Track { id },
        }
    }
}

/// What the first Qobuz link in `text` points to, if it's to an album, an artist or a track:
/// a link alone, or a text shared with one in it.
#[uniffi::export]
#[must_use]
pub fn find_link(text: &str) -> Option<QobuzLink> {
    link::find(text).map(QobuzLink::from)
}

/// A track a link points to, and the album it's on, which Pochette shows instead.
#[derive(Debug, Clone, uniffi::Record)]
pub struct LinkedTrack {
    /// With its version.
    pub title: String,
    pub album_id: Option<String>,
    /// With its version.
    pub album_title: Option<String>,
}

impl From<qobuz::models::Track> for LinkedTrack {
    fn from(track: qobuz::models::Track) -> Self {
        Self {
            title: track.full_title(),
            album_title: track
                .album
                .as_ref()
                .map(qobuz::models::TrackAlbum::full_title),
            album_id: track.album.map(|album| album.id),
        }
    }
}
