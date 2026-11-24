//! Syncing the plays between the user's devices on the same network.
//!
//! A service, `pochette --sync`, does it, with no window: it keeps going while Pochette is
//! closed, if the user wants it to, started with their session. Devices find each other
//! with mDNS, pair once by checking that both show the same code, then send each other the
//! end of the logs of plays the other lacks, over connections only they can read. The app
//! starts the service and talks to it over D-Bus, from the Devices window.

pub mod autostart;
pub mod client;
mod service;

pub use pochette_core::sync::{PORT, PeerStatus, Stage, Status, device, has_peers};

pub use service::run;

/// The service's name on the session bus, which only one can hold: the app's, and `.Sync`.
pub fn bus_name() -> String {
    format!("{}.Sync", crate::app_id())
}

pub const OBJECT_PATH: &str = "/io/github/mcrampon/Pochette/Sync";
pub const INTERFACE: &str = "io.github.mcrampon.Pochette.Sync1";
