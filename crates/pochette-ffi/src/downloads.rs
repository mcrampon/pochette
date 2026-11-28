//! Albums downloaded to listen to offline: see [`pochette_core::downloads`].

use std::sync::Arc;

use pochette_core::downloads::Downloads;
use qobuz::FileUrl;

use crate::{AlbumCard, Pochette, PochetteError, Result, Stream, playback::Quality, run};

/// Hears how a download is going.
#[uniffi::export(foreign)]
pub trait DownloadProgress: Send + Sync {
    /// How much is downloaded, from 0 to 1.
    fn progress(&self, fraction: f32);
}

#[uniffi::export]
impl Pochette {
    /// Download album `id`, its tracks in `quality` at best, telling `progress` how it goes.
    /// Tracks already there are kept: one that stopped picks up where it was. Cancel the call
    /// to stop it.
    pub async fn download_album(
        &self,
        id: String,
        quality: Quality,
        progress: Arc<dyn DownloadProgress>,
    ) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let client = inner.client()?;
            inner
                .downloads
                .download(&client, &inner.http, &id, quality.into(), |fraction| {
                    progress.progress(fraction);
                })
                .await
                .map_err(|err| match err {
                    pochette_core::downloads::Error::Qobuz(err) => err.into(),
                    err => PochetteError::Failed {
                        message: err.to_string(),
                    },
                })
        })
        .await
    }

    /// Whether album `id` is downloaded, all of it.
    #[must_use]
    pub fn is_downloaded(&self, id: &str) -> bool {
        self.inner.downloads.has(id)
    }

    /// The ids of the albums downloaded.
    #[must_use]
    pub fn downloaded_ids(&self) -> Vec<String> {
        self.inner
            .downloads
            .albums()
            .into_iter()
            .map(|(album, _)| album.id)
            .collect()
    }

    /// Remove album `id`'s download, whole or not.
    pub fn remove_download(&self, id: &str) -> Result<()> {
        self.inner
            .downloads
            .remove(id)
            .map_err(|err| PochetteError::Failed {
                message: err.to_string(),
            })
    }

    /// Remove every download.
    pub fn remove_all_downloads(&self) -> Result<()> {
        self.inner
            .downloads
            .remove_all()
            .map_err(|err| PochetteError::Failed {
                message: err.to_string(),
            })
    }

    /// How much room the downloads take, in bytes.
    #[must_use]
    pub fn downloads_size(&self) -> u64 {
        self.inner.downloads.size()
    }

    /// Where track `track_id` plays from when it's downloaded: its file.
    #[must_use]
    pub fn downloaded_stream(&self, track_id: u64) -> Option<Stream> {
        let (path, track) = self.inner.downloads.track(track_id)?;
        let mut stream = Stream::new(
            track_id,
            FileUrl {
                url: format!("file://{}", path.display()),
                format_id: track.format_id,
                mime_type: track.mime_type,
                sampling_rate: track.sampling_rate,
                bit_depth: track.bit_depth,
                blob: None,
                sample: false,
            },
        );
        stream.local = true;
        Some(stream)
    }
}

/// `card`, with its covers from the download when there's one, to show offline.
pub(crate) fn with_local_cover(downloads: &Downloads, mut card: AlbumCard) -> AlbumCard {
    if let Some(cover) = downloads.cover(&card.id) {
        let uri = format!("file://{}", cover.display());
        card.cover = Some(uri.clone());
        card.cover_large = Some(uri);
    }
    card
}
