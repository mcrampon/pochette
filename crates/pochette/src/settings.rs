//! What Pochette remembers between runs, in `$XDG_CONFIG_HOME/pochette/settings.json`.

use std::{io, path::PathBuf};

use qobuz::Quality;
use relm4::gtk::glib;
use serde::{Deserialize, Serialize};

use crate::library::Sort;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// From 0 to 1, as the volume slider shows it.
    pub volume: f64,
    /// The now playing pane's width next to the library, in logical pixels.
    pub pane_width: i32,
    /// The genres picked in the Albums tab, by name: they come from the albums.
    pub album_genres: Vec<String>,
    /// The decades picked in the Albums tab.
    pub album_decades: Vec<u16>,
    /// The one decade the Albums tab used to pick, read to carry it over.
    #[serde(skip_serializing)]
    pub album_decade: Option<u16>,
    /// How the Albums tab sorts, and whether the other way round.
    pub album_sort: Sort,
    pub album_sort_reversed: bool,
    /// The genres picked in the Discover tab, by Qobuz id.
    pub discover_genres: Vec<u64>,
    /// The best quality to stream in.
    pub quality: Quality,
    /// The audio output to play through, by its id, or the system's default when `None`.
    pub output: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pane_width: 360,
            album_genres: Vec::new(),
            album_decades: Vec::new(),
            album_decade: None,
            album_sort: Sort::default(),
            album_sort_reversed: false,
            discover_genres: Vec::new(),
            quality: Quality::default(),
            output: None,
        }
    }
}

impl Settings {
    /// The saved settings, or the defaults when there are none or they can't be read.
    pub fn load() -> Self {
        let read =
            std::fs::read(path()).and_then(|bytes| Self::parse(&bytes).map_err(io::Error::other));
        match read {
            Ok(settings) => settings,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                tracing::warn!("cannot read the settings, using the defaults: {err}");
                Self::default()
            }
        }
    }

    fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        let mut settings: Self = serde_json::from_slice(bytes)?;
        // Older settings had one decade.
        if let Some(decade) = settings.album_decade.take()
            && settings.album_decades.is_empty()
        {
            settings.album_decades.push(decade);
        }
        Ok(settings)
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
    glib::user_config_dir()
        .join("pochette")
        .join("settings.json")
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
    fn carries_the_one_decade_of_older_settings_over() {
        let settings = Settings::parse(br#"{"album_decade": 1970}"#).unwrap();
        assert_eq!(settings.album_decades, [1970]);
        assert!(
            !serde_json::to_string(&settings)
                .unwrap()
                .contains("album_decade\"")
        );
    }

    #[test]
    fn reads_the_volume() {
        let settings: Settings = serde_json::from_str(r#"{"volume": 0.4}"#).unwrap();
        assert!((settings.volume - 0.4).abs() < f64::EPSILON);
    }
}
