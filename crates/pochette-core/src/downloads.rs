//! Albums downloaded to listen to offline, each in a folder of its own, by its id, in the
//! folder the app keeps them in:
//!
//! - `album.json`: the album, as Qobuz gave it, with its tracks;
//! - `cover.jpg`: its cover, 600 px;
//! - a file for each track, `<track id>.flac` or `.mp3`, with what Qobuz said of it in
//!   `tracks.json`;
//! - `complete`, once all the tracks are there.
//!
//! A file being downloaded is a `.part` until it's whole.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use qobuz::{Client, FileUrl, Quality, models::Album};
use serde::{Deserialize, Serialize};

/// A track downloaded: its file, and what Qobuz said of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// The file's name, in the album's folder.
    pub file: String,
    /// The quality Qobuz picked, as [`FileUrl::format_id`].
    pub format_id: u32,
    pub mime_type: Option<String>,
    pub bit_depth: Option<u32>,
    /// In kHz.
    pub sampling_rate: Option<f32>,
}

/// Why an album didn't download.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Qobuz(#[from] qobuz::Error),
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Qobuz only gives 30-second previews, as to free accounts.
    #[error("only previews")]
    PreviewsOnly,
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Qobuz(err.into())
    }
}

/// The albums downloaded, in a folder.
#[derive(Debug, Clone)]
pub struct Downloads {
    dir: PathBuf,
}

impl Downloads {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn album_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    /// Download album `id`, its tracks in `quality` at best, and call `progress` with how
    /// much is done, from 0 to 1. Tracks already there are kept: a download that stopped
    /// picks up where it was.
    #[expect(clippy::cast_precision_loss, reason = "a progress needs no precision")]
    pub async fn download(
        &self,
        client: &Client,
        http: &reqwest::Client,
        id: &str,
        quality: Quality,
        progress: impl Fn(f32),
    ) -> Result<(), Error> {
        let album = client.album(id).await?;
        let dir = self.album_dir(id);
        fs::create_dir_all(&dir)?;
        write_whole(
            &dir.join("album.json"),
            &serde_json::to_vec(&album).map_err(io::Error::other)?,
        )?;
        if let Some(url) = &album.image.large
            && !dir.join("cover.jpg").exists()
        {
            let cover = http
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            write_whole(&dir.join("cover.jpg"), &cover)?;
        }

        let mut tracks = read_tracks(&dir);
        let streamable: Vec<_> = album
            .tracks
            .iter()
            .flat_map(|page| &page.items)
            .filter(|track| track.streamable)
            .collect();
        let count = streamable.len().max(1) as f32;
        for (done, track) in streamable.iter().enumerate() {
            let done = done as f32;
            progress(done / count);
            let key = track.id.to_string();
            if tracks
                .get(&key)
                .is_some_and(|kept| dir.join(&kept.file).exists())
            {
                continue;
            }
            let file = client.file_url(track.id, quality).await?;
            if file.sample {
                return Err(Error::PreviewsOnly);
            }
            let name = format!("{}.{}", track.id, extension(&file));
            fetch(http, &file.url, &dir.join(&name), |fraction| {
                progress((done + fraction) / count);
            })
            .await?;
            tracks.insert(
                key,
                Track {
                    file: name,
                    format_id: file.format_id,
                    mime_type: file.mime_type,
                    bit_depth: file.bit_depth,
                    sampling_rate: file.sampling_rate,
                },
            );
            write_whole(
                &dir.join("tracks.json"),
                &serde_json::to_vec(&tracks).map_err(io::Error::other)?,
            )?;
        }
        write_whole(&dir.join("complete"), b"")?;
        progress(1.0);
        Ok(())
    }

    /// Whether album `id` is downloaded, all of it.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.album_dir(id).join("complete").exists()
    }

    /// The albums downloaded, all of them, with when their download finished.
    #[must_use]
    pub fn albums(&self) -> Vec<(Album, SystemTime)> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let id = entry.file_name().to_string_lossy().into_owned();
                let completed = fs::metadata(self.album_dir(&id).join("complete"))
                    .and_then(|meta| meta.modified())
                    .ok()?;
                Some((self.album(&id)?, completed))
            })
            .collect()
    }

    /// Album `id`, as it was downloaded, with its tracks: all of them downloaded or not.
    #[must_use]
    pub fn album(&self, id: &str) -> Option<Album> {
        let bytes = fs::read(self.album_dir(id).join("album.json")).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// The cover of album `id`, if it's downloaded.
    #[must_use]
    pub fn cover(&self, id: &str) -> Option<PathBuf> {
        let path = self.album_dir(id).join("cover.jpg");
        path.exists().then_some(path)
    }

    /// Track `track_id`, downloaded with an album, whole or not yet: its file, and what
    /// Qobuz said of it.
    #[must_use]
    pub fn track(&self, track_id: u64) -> Option<(PathBuf, Track)> {
        let key = track_id.to_string();
        fs::read_dir(&self.dir).ok()?.flatten().find_map(|entry| {
            let dir = entry.path();
            let track = read_tracks(&dir).remove(&key)?;
            let path = dir.join(&track.file);
            path.exists().then_some((path, track))
        })
    }

    /// Remove album `id`'s download, whole or not.
    pub fn remove(&self, id: &str) -> io::Result<()> {
        match fs::remove_dir_all(self.album_dir(id)) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// Remove every download.
    pub fn remove_all(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.dir) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// How much room the downloads take, in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        fn size_of(path: &Path) -> u64 {
            match fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() => fs::read_dir(path).map_or(0, |entries| {
                    entries.flatten().map(|entry| size_of(&entry.path())).sum()
                }),
                Ok(meta) => meta.len(),
                Err(_) => 0,
            }
        }
        size_of(&self.dir)
    }
}

fn read_tracks(dir: &Path) -> BTreeMap<String, Track> {
    fs::read(dir.join("tracks.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The extension of a stream's file, from its type.
fn extension(file: &FileUrl) -> &'static str {
    match file.mime_type.as_deref() {
        Some("audio/mpeg") => "mp3",
        _ if file.format_id == Quality::Mp3 as u32 => "mp3",
        _ => "flac",
    }
}

/// Write `bytes` to `path`, aside first, so a crash never leaves half a file there.
fn write_whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let part = part_of(path);
    fs::write(&part, bytes)?;
    fs::rename(part, path)
}

fn part_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

/// Download `url` to `path`, calling `progress` with how much of it is there, from 0 to 1.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "a progress needs no precision"
)]
async fn fetch(
    http: &reqwest::Client,
    url: &str,
    path: &Path,
    progress: impl Fn(f32),
) -> Result<(), Error> {
    let mut response = http.get(url).send().await?.error_for_status()?;
    let total = response.content_length();
    let part = part_of(path);
    let mut file = io::BufWriter::new(fs::File::create(&part)?);
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
        received += chunk.len() as u64;
        if let Some(total) = total.filter(|total| *total > 0) {
            progress((received as f64 / total as f64).min(1.0) as f32);
        }
    }
    file.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    fs::rename(part, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: &str) -> Album {
        serde_json::from_str(&format!(
            r#"{{"id": "{id}", "title": "T", "artist": {{"id": 1, "name": "A"}}, "image": {{}},
                "tracks": {{"offset": 0, "limit": 50, "total": 1, "items": [
                    {{"id": 7, "title": "One", "track_number": 1, "streamable": true}}]}}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn finds_what_was_downloaded() {
        let dir = std::env::temp_dir().join(format!("pochette-downloads-{}", uuid::Uuid::new_v4()));
        let downloads = Downloads::new(dir.clone());
        let album_dir = dir.join("a1");
        fs::create_dir_all(&album_dir).unwrap();
        fs::write(
            album_dir.join("album.json"),
            serde_json::to_vec(&album("a1")).unwrap(),
        )
        .unwrap();
        fs::write(album_dir.join("7.flac"), b"fLaC").unwrap();
        let mut tracks = BTreeMap::new();
        tracks.insert(
            "7".to_owned(),
            Track {
                file: "7.flac".into(),
                format_id: 6,
                mime_type: Some("audio/flac".into()),
                bit_depth: Some(16),
                sampling_rate: Some(44.1),
            },
        );
        fs::write(
            album_dir.join("tracks.json"),
            serde_json::to_vec(&tracks).unwrap(),
        )
        .unwrap();

        assert!(!downloads.has("a1"), "not complete yet");
        assert!(downloads.albums().is_empty());
        assert_eq!(downloads.track(7).unwrap().0, album_dir.join("7.flac"));
        fs::write(album_dir.join("complete"), b"").unwrap();
        assert!(downloads.has("a1"));
        assert_eq!(downloads.albums()[0].0.id, "a1");
        assert_eq!(
            downloads.album("a1").unwrap().tracks.unwrap().items[0].id,
            7
        );
        assert!(downloads.size() >= 4);

        downloads.remove("a1").unwrap();
        assert!(!downloads.has("a1"));
        assert!(downloads.track(7).is_none());
        downloads.remove_all().unwrap();
        assert!(!dir.exists());
    }
}
