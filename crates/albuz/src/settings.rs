//! What Albuz remembers between runs, in `$XDG_CONFIG_HOME/albuz/settings.json`.

use std::{io, path::PathBuf};

use relm4::gtk::glib;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// From 0 to 1, as the volume slider shows it.
    pub volume: f64,
    /// The now playing pane's width next to the library, in logical pixels.
    pub pane_width: i32,
    /// The genres picked in the Albums tab, by name: they come from the albums.
    pub album_genres: Vec<String>,
    /// The genres picked in the Discover tab, by Qobuz id.
    pub discover_genres: Vec<u64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pane_width: 360,
            album_genres: Vec::new(),
            discover_genres: Vec::new(),
        }
    }
}

impl Settings {
    /// The saved settings, or the defaults when there are none or they can't be read.
    pub fn load() -> Self {
        match std::fs::read(path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                tracing::warn!("cannot read the settings, using the defaults: {err}");
                Self::default()
            }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                tracing::warn!("cannot read the settings, using the defaults: {err}");
                Self::default()
            }
        }
    }

    /// Change the saved settings with `change`.
    pub fn update(change: impl FnOnce(&mut Self)) {
        let mut settings = Self::load();
        change(&mut settings);
        if let Err(err) = settings.save() {
            tracing::warn!("cannot save the settings: {err}");
        }
    }

    fn save(&self) -> io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Written aside then moved in place, so a crash never leaves half a file.
        let part = path.with_extension("json.part");
        std::fs::write(
            &part,
            serde_json::to_vec_pretty(self).map_err(io::Error::other)?,
        )?;
        std::fs::rename(part, path)
    }
}

fn path() -> PathBuf {
    glib::user_config_dir().join("albuz").join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_older_files_with_missing_settings() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn reads_the_volume() {
        let settings: Settings = serde_json::from_str(r#"{"volume": 0.4}"#).unwrap();
        assert!((settings.volume - 0.4).abs() < f64::EPSILON);
    }
}
