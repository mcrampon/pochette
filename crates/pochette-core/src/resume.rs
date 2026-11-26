//! Where the listening was, to take it up again when the app opens: the album, its track,
//! and how far into it. Not the page the app showed.
//!
//! The players save it as they go: as a track starts, on pause, on seek, from time to time
//! while playing, and on quitting. Played to its end, or stopped, the album leaves nothing
//! to take up.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// Where the listening was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePoint {
    pub album_id: String,
    pub track_id: u64,
    /// How far into the track, in milliseconds.
    pub position_ms: u64,
}

/// Where the listening was when the app last saved it, if it's to be taken up.
#[must_use]
pub fn load() -> Option<ResumePoint> {
    load_from(&path())
}

/// Save where the listening is.
pub fn save(point: &ResumePoint) {
    if let Err(err) = save_to(&path(), point) {
        tracing::warn!("cannot save where the listening is: {err}");
    }
}

/// Nothing to take up: the album was played to its end, or stopped.
pub fn forget() {
    forget_at(&path());
}

fn path() -> PathBuf {
    crate::platform().state_dir.join("resume.json")
}

fn load_from(path: &Path) -> Option<ResumePoint> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes)
        .inspect_err(|err| tracing::warn!("cannot read where the listening was: {err}"))
        .ok()
}

fn save_to(path: &Path, point: &ResumePoint) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // Written aside, then moved in place: never half a file, if the app is killed.
    let aside = path.with_extension("json.new");
    fs::write(&aside, serde_json::to_vec(point)?)?;
    fs::rename(&aside, path)
}

fn forget_at(path: &Path) {
    match fs::remove_file(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!("cannot forget where the listening was: {err}");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_takes_up_and_forgets() {
        let dir = std::env::temp_dir().join(format!("pochette-resume-{}", uuid::Uuid::new_v4()));
        let path = dir.join("resume.json");
        assert_eq!(load_from(&path), None);

        let point = ResumePoint {
            album_id: "0060253780545".into(),
            track_id: 12_345,
            position_ms: 83_250,
        };
        save_to(&path, &point).unwrap();
        assert_eq!(load_from(&path), Some(point.clone()));

        let later = ResumePoint {
            position_ms: 93_250,
            ..point
        };
        save_to(&path, &later).unwrap();
        assert_eq!(load_from(&path), Some(later));

        forget_at(&path);
        assert_eq!(load_from(&path), None);
        // Forgetting twice is fine.
        forget_at(&path);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_broken_file_is_nothing_to_take_up() {
        let dir = std::env::temp_dir().join(format!("pochette-resume-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("resume.json");
        fs::write(&path, b"{\"album_id\": ").unwrap();
        assert_eq!(load_from(&path), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
