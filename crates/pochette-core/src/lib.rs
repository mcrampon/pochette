//! What Pochette's apps share, on the desktop and on Android: the play history and the
//! stats worked out from it, searching and sorting the saved albums, syncing the plays
//! between devices, lyrics, and how values read. No UI here.
//!
//! Each app calls [`init`] first, with where it keeps things and what the device is called.

pub mod format;
pub mod history;
pub mod library;
pub mod listening;
pub mod lyrics;
pub mod sync;
pub mod time;

use std::{path::PathBuf, sync::OnceLock};

/// Where the app keeps things, and what the device is called, which each platform knows.
#[derive(Debug, Clone)]
pub struct Platform {
    /// The logs of plays, which people may back up or copy to their other devices.
    pub data_dir: PathBuf,
    /// What stays on this device: its sync key, the devices it's paired with.
    pub state_dir: PathBuf,
    /// What the device is called, to the others.
    pub device_name: String,
    /// The device's name, as it can be in a file name: letters, digits, `-` and `_`.
    pub file_name: String,
}

static PLATFORM: OnceLock<Platform> = OnceLock::new();

/// Tell the core about the platform, once, before anything else.
pub fn init(platform: Platform) {
    if PLATFORM.set(platform).is_err() {
        tracing::warn!("the platform was already set");
    }
}

/// The platform, as [`init`] told it.
pub fn platform() -> &'static Platform {
    PLATFORM
        .get()
        .expect("pochette_core::init is called before anything else")
}

/// `name`, as it can be in a file name.
#[must_use]
pub fn file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
