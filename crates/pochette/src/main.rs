//! Pochette, a native Linux player for Qobuz, built around albums.

mod accent;
mod account;
mod album_page;
mod album_tile;
mod albums;
mod app;
mod artist_page;
mod blurred_cover;
mod checklist;
mod choice_picker;
mod clamp;
mod covers;
mod devices;
mod discover;
mod favorites;
mod format;
mod history;
mod icon_label;
mod library;
mod lyrics;
mod lyrics_view;
mod mpris;
mod navigation;
mod now_playing;
mod outputs;
mod path_icon;
mod playback;
mod player;
mod plays;
mod releases_page;
mod search_page;
mod search_view;
mod session_store;
mod settings;
mod sort_picker;
mod split_view;
mod stats_view;
mod sync;
mod tile_grid;
mod tile_strip;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

/// The application id, which names the app to the desktop (D-Bus, `.desktop` file, Flatpak).
const APP_ID: &str = "io.github.mcrampon.Pochette";

/// Give the windows the icon installed under the app id, from `data/icons`. Only when it
/// is installed: title bars that show the icon would show a missing one instead.
fn show_icon() {
    use relm4::gtk::{self, gdk};

    let installed = gdk::Display::default()
        .is_some_and(|display| gtk::IconTheme::for_display(&display).has_icon(APP_ID));
    if installed {
        gtk::Window::set_default_icon_name(APP_ID);
    }
}

fn main() -> ExitCode {
    // Before GTK, which would need a display and reject these options.
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("pochette {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("--help" | "-h") => {
            println!(
                "Usage: pochette [--version | --help | --sync]\n\n\
                 A native Linux player for Qobuz, built around albums.\n\n\
                 --sync  sync the plays with the paired devices, without a window"
            );
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    if std::env::args().nth(1).as_deref() == Some("--sync") {
        return sync::run();
    }

    let relm = relm4::RelmApp::new(APP_ID);
    show_icon();
    relm4::set_global_css(include_str!("style.css"));
    accent::follow();
    covers::prune_cache();
    relm.run::<app::App>(());
    ExitCode::SUCCESS
}
