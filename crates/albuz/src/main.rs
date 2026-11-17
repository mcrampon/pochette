//! Albuz, a native Linux player for Qobuz, built around albums.

mod account;
mod album_page;
mod album_tile;
mod albums;
mod app;
mod checklist;
mod clamp;
mod covers;
mod discover;
mod dropdown;
mod favorites;
mod format;

mod library;
mod mpris;
mod now_playing;
mod playback;
mod player;
mod session_store;
mod settings;
mod split_view;

use tracing_subscriber::EnvFilter;

/// The application id, which names the app to the desktop (D-Bus, `.desktop` file, Flatpak).
const APP_ID: &str = "io.github.mcrampon.Albuz";

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let relm = relm4::RelmApp::new(APP_ID);
    relm4::set_global_css(include_str!("style.css"));
    covers::prune_cache();
    relm.run::<app::App>(());
}
