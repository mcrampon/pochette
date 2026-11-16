//! Albuz, a native Linux player for Qobuz, built around albums.

mod account;
mod albums;
mod app;
mod covers;
mod session_store;

use tracing_subscriber::EnvFilter;

/// The application id, which names the app to the desktop (D-Bus, `.desktop` file, Flatpak).
const APP_ID: &str = "io.github.albuz.Albuz";

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let relm = relm4::RelmApp::new(APP_ID);
    relm4::set_global_css(include_str!("style.css"));
    relm.run::<app::App>(());
}
