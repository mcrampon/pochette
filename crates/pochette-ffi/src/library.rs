//! The user's albums, and the rest of Qobuz to browse: albums, Discover, artists, search.

use std::sync::Arc;

use pochette_core::library::{self, Entry, Sort as CoreSort, View};
use qobuz::models::Album;

use crate::{
    AlbumCard, AlbumList, AlbumPage, ArtistCard, ArtistPage, Discover, Genre, Pochette,
    ReleaseKind, Result, Section, downloads::with_local_cover, lock, run,
};

/// The orders the albums can go in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum Sort {
    #[default]
    DateAdded,
    Title,
    Artist,
    ReleaseDate,
    LastPlayed,
    PlayCount,
}

impl From<Sort> for CoreSort {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::DateAdded => Self::DateAdded,
            Sort::Title => Self::Title,
            Sort::Artist => Self::Artist,
            Sort::ReleaseDate => Self::ReleaseDate,
            Sort::LastPlayed => Self::LastPlayed,
            Sort::PlayCount => Self::PlayCount,
        }
    }
}

/// Which albums to show, and in what order.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct LibraryView {
    /// Words to look for in the titles, artists and labels.
    pub query: String,
    /// The genres to show, or all of them when empty.
    pub genres: Vec<String>,
    /// The decades to show, or all of them when empty: 1990 for the 1990s.
    pub decades: Vec<u16>,
    pub sort: Sort,
    /// The other way round from the sort's natural order.
    pub reversed: bool,
    /// Only the albums downloaded, as offline: those of the user's albums, and those no
    /// longer among them.
    pub downloaded_only: bool,
}

impl From<LibraryView> for View {
    fn from(view: LibraryView) -> Self {
        Self {
            query: view.query,
            genres: view.genres,
            decades: view.decades,
            sort: view.sort.into(),
            reversed: view.reversed,
        }
    }
}

/// What the sort menu calls `sort`: "Date Added".
#[uniffi::export]
#[must_use]
pub fn sort_label(sort: Sort) -> String {
    CoreSort::from(sort).label()
}

/// How `view`'s order reads: "Newest First", "A to Z"…
#[uniffi::export]
#[must_use]
pub fn order_label(view: LibraryView) -> String {
    View::from(view).order_label()
}

#[uniffi::export]
impl Pochette {
    /// Get the user's favorite albums from Qobuz again: [`Pochette::library`] shows them.
    pub async fn refresh_library(&self) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let albums = inner.client()?.favorite_albums().await?;
            *lock(&inner.favorites) = albums;
            Ok(())
        })
        .await
    }

    /// The user's albums that `view` shows, in its order.
    #[must_use]
    pub fn library(&self, view: LibraryView) -> Vec<AlbumCard> {
        let entries = self.entries(view.downloaded_only);
        let plays = self.inner.plays();
        View::from(view)
            .apply(&entries, plays.history())
            .into_iter()
            .map(|entry| with_local_cover(&self.inner.downloads, AlbumCard::from(&*entry.album)))
            .collect()
    }

    /// The genres of the user's albums, or of those downloaded, in alphabetical order.
    #[must_use]
    pub fn library_genres(&self, downloaded_only: bool) -> Vec<String> {
        library::genres(&self.entries(downloaded_only))
    }

    /// The decades of the user's albums, or of those downloaded, newest first.
    #[must_use]
    pub fn library_decades(&self, downloaded_only: bool) -> Vec<u16> {
        library::decades(&self.entries(downloaded_only))
    }

    #[must_use]
    pub fn is_favorite(&self, album_id: &str) -> bool {
        lock(&self.inner.favorites)
            .iter()
            .any(|album| album.id == album_id)
    }

    /// Add the album to the user's favorites, or take it out.
    pub async fn set_favorite(&self, album_id: String, favorite: bool) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let client = inner.client()?;
            if !favorite {
                client.remove_favorite_album(&album_id).await?;
                lock(&inner.favorites).retain(|album| album.id != album_id);
                return Ok(());
            }
            client.add_favorite_album(&album_id).await?;
            let known = lock(&inner.albums).get(&album_id).cloned();
            let mut album = match known {
                Some(album) => album,
                None => client.album(&album_id).await?,
            };
            album.tracks = None;
            album.favorited_at = Some(pochette_core::time::now());
            let mut favorites = lock(&inner.favorites);
            favorites.retain(|kept| kept.id != album_id);
            favorites.insert(0, album);
            Ok(())
        })
        .await
    }

    /// An album's page, with its tracks.
    pub async fn album(&self, id: String) -> Result<AlbumPage> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            // Downloaded, it's there, offline too.
            let album = match inner.downloads.album(&id) {
                Some(album) => album,
                None => inner.client()?.album(&id).await?,
            };
            let plays = inner.plays().history().album(&id);
            let mut page = AlbumPage::new(&album, plays);
            page.card = with_local_cover(&inner.downloads, page.card);
            lock(&inner.albums).insert(id, album);
            Ok(page)
        })
        .await
    }

    /// The first albums of each of Discover's sections, of the genres `genre_ids`, or of
    /// all when empty.
    pub async fn discover(&self, genre_ids: Vec<u64>) -> Result<Discover> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let discover = inner.client()?.discover(&genre_ids).await?;
            Ok(Discover {
                new_releases: (&discover.new_releases).into(),
                press_awards: (&discover.press_awards).into(),
                ideal_discography: (&discover.ideal_discography).into(),
            })
        })
        .await
    }

    /// `limit` albums of a Discover section from `offset`.
    pub async fn discover_section(
        &self,
        section: Section,
        genre_ids: Vec<u64>,
        offset: u32,
        limit: u32,
    ) -> Result<AlbumList> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let albums = inner
                .client()?
                .discover_section(section.into(), &genre_ids, offset as usize, limit as usize)
                .await?;
            Ok((&albums).into())
        })
        .await
    }

    /// The genres Discover can show.
    pub async fn genres(&self) -> Result<Vec<Genre>> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let genres = inner.client()?.genres().await?;
            Ok(genres
                .into_iter()
                .map(|genre| Genre {
                    id: genre.id,
                    name: genre.name,
                })
                .collect())
        })
        .await
    }

    /// A track, from a link, with the album it's on.
    pub async fn track(&self, id: u64) -> Result<crate::link::LinkedTrack> {
        let inner = Arc::clone(&self.inner);
        run(async move { Ok(inner.client()?.track(id).await?.into()) }).await
    }

    pub async fn artist(&self, id: u64) -> Result<ArtistPage> {
        let inner = Arc::clone(&self.inner);
        run(async move { Ok((&inner.client()?.artist(id).await?).into()) }).await
    }

    /// `limit` releases of the artist of `kind` from `offset`, newest first.
    pub async fn artist_releases(
        &self,
        id: u64,
        kind: ReleaseKind,
        offset: u32,
        limit: u32,
    ) -> Result<AlbumList> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let albums = inner
                .client()?
                .artist_releases(id, kind.into(), offset as usize, limit as usize)
                .await?;
            Ok((&albums).into())
        })
        .await
    }

    /// `limit` albums matching `query` from `offset`, the best matches first.
    pub async fn search_albums(&self, query: String, offset: u32, limit: u32) -> Result<AlbumList> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let albums = inner
                .client()?
                .search_albums(&query, offset as usize, limit as usize)
                .await?;
            Ok((&albums).into())
        })
        .await
    }

    /// The first `limit` artists matching `query`, the best matches first.
    pub async fn search_artists(&self, query: String, limit: u32) -> Result<Vec<ArtistCard>> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let artists = inner
                .client()?
                .search_artists(&query, limit as usize)
                .await?;
            Ok(artists.iter().map(ArtistCard::from).collect())
        })
        .await
    }
}

impl Pochette {
    /// The favorites, as the library sorts and filters them.
    fn entries(&self, downloaded_only: bool) -> Vec<Entry> {
        let favorites: Vec<Album> = lock(&self.inner.favorites).clone();
        if !downloaded_only {
            return favorites.into_iter().map(Entry::new).collect();
        }
        self.inner
            .downloads
            .albums()
            .into_iter()
            .map(|(mut album, downloaded)| {
                // Added when it was to the user's albums, or else when it was downloaded,
                // for the sort.
                album.favorited_at = favorites
                    .iter()
                    .find(|favorite| favorite.id == album.id)
                    .and_then(|favorite| favorite.favorited_at)
                    .or_else(|| {
                        downloaded
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .and_then(|since| i64::try_from(since.as_secs()).ok())
                    });
                Entry::new(album)
            })
            .collect()
    }
}
