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
mod i18n;
mod icon_label;
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
mod settings_page;
mod sort_picker;
mod split_view;
mod stats_view;
mod sync;
mod tile_grid;
mod tile_strip;
mod track_page;

use std::{process::ExitCode, sync::OnceLock};

pub use pochette_core::{format, history, library, lyrics};

use tracing_subscriber::EnvFilter;

/// The application id, which names the app to the desktop (D-Bus, `.desktop` file, Flatpak).
const APP_ID: &str = "io.github.mcrampon.Pochette";

/// Set to a name, as `Test`, it runs a separate instance of Pochette, for development: with
/// an app id, MPRIS name and sync service of its own, beside the Pochette installed, which
/// would otherwise take its place. Its folders are still the user's: point `XDG_*` elsewhere.
const INSTANCE_VARIABLE: &str = "POCHETTE_INSTANCE";

/// This run's instance, if it's a separate one: letters, digits and `_`, a letter first,
/// as a part of a D-Bus name must be.
fn instance() -> Option<&'static str> {
    static INSTANCE: OnceLock<Option<String>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            let instance = std::env::var(INSTANCE_VARIABLE).ok()?;
            let valid = instance.starts_with(|c: char| c.is_ascii_alphabetic())
                && instance
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                eprintln!("{INSTANCE_VARIABLE} must be letters, digits and _, a letter first");
                return None;
            }
            Some(instance)
        })
        .as_deref()
}

/// The app id this run goes by on the session bus: [`APP_ID`], and its instance if it's a
/// separate one.
fn app_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| match instance() {
        Some(instance) => format!("{APP_ID}.{instance}"),
        None => APP_ID.to_owned(),
    })
}

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

/// Where the app keeps things, as XDG has it, and the host name, for the core.
fn tell_core_the_platform() {
    use relm4::gtk::glib;

    let host = glib::host_name();
    let host = host.trim();
    let device_name = if host.is_empty() || host == "localhost" {
        "Pochette"
    } else {
        host
    };
    pochette_core::init(pochette_core::Platform {
        data_dir: glib::user_data_dir().join("pochette"),
        state_dir: glib::user_state_dir().join("pochette"),
        device_name: device_name.to_owned(),
        // What logs were named after before devices had keys: keep it, to find them.
        file_name: pochette_core::file_name(&glib::host_name()),
    });
}

/// Where the Qobuz links Pochette is asked to open go, to the main window.
static LINKS: relm4::MessageBroker<app::Msg> = relm4::MessageBroker::new();

/// The application, which also opens the Qobuz links the desktop hands it, as
/// `qobuzapp://album/<id>`, which Qobuz's pages open, or given on the command line. A second
/// `pochette` hands them to the one running.
fn application() -> relm4::gtk::Application {
    use relm4::gtk::{self, gio, prelude::*};

    // As RelmApp::new does, for what's set up before the app runs.
    gtk::init().expect("GTK starts, which needs a display");
    let app = gtk::Application::builder()
        .application_id(app_id())
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_open(|app, files, _| {
        // Opened with links, the app isn't activated, which shows the window.
        app.activate();
        for file in files {
            LINKS.send(app::Msg::OpenLink(file.uri().into()));
        }
    });
    app
}

/// Whether Pochette runs already, under this app id, on the session bus.
fn is_running() -> bool {
    use relm4::gtk::{gio, glib, prelude::*};

    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
        return false;
    };
    bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&(app_id(),).to_variant()),
        Some(glib::VariantTy::new("(b)").expect("a valid type")),
        gio::DBusCallFlags::NONE,
        1000,
        None::<&gio::Cancellable>,
    )
    .ok()
    .and_then(|reply| reply.get::<(bool,)>())
    .is_some_and(|(owned,)| owned)
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
                "Usage: pochette [--version | --help | --sync | <Qobuz link>]\n\n\
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

    tell_core_the_platform();
    if std::env::args().nth(1).as_deref() == Some("--sync") {
        return sync::run();
    }

    i18n::init(settings::Settings::load().language.as_deref());
    if let Some(instance) = instance() {
        tracing::info!("running as the separate instance {instance}");
    }
    let app = application();
    // Pochette runs already: this one only hands it the links, or has it show its window,
    // and goes, without Relm4, which would then wait for events that never come.
    if is_running() {
        use relm4::gtk::prelude::*;
        return if app.run() == relm4::gtk::glib::ExitCode::SUCCESS {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    let relm = relm4::RelmApp::from_app(app).with_broker(&LINKS);
    show_icon();
    relm4::set_global_css(include_str!("style.css"));
    accent::follow();
    covers::prune_cache();
    relm.run::<app::App>(());
    ExitCode::SUCCESS
}
