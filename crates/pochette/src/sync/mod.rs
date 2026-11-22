//! Syncing the plays between the user's devices on the same network.
//!
//! A service, `pochette --sync`, does it, with no window: it keeps going while Pochette is
//! closed, if the user wants it to, started with their session. Devices find each other
//! with mDNS, pair once by checking that both show the same code, then send each other the
//! end of the logs of plays the other lacks, over connections only they can read. The app
//! starts the service and talks to it over D-Bus, from the Devices window.

pub mod autostart;
mod channel;
pub mod client;
pub mod device;
mod protocol;
mod service;

use serde::{Deserialize, Serialize};

pub use service::run;

/// The service's name on the session bus, which only one can hold.
pub const BUS_NAME: &str = "io.github.mcrampon.Pochette.Sync";
pub const OBJECT_PATH: &str = "/io/github/mcrampon/Pochette/Sync";
pub const INTERFACE: &str = "io.github.mcrampon.Pochette.Sync1";

/// The port the service listens on, unless it's taken. Fixed, for firewalls to let it
/// through.
pub const PORT: u16 = 45617;

/// What the service tells the app.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// This device's name.
    pub name: String,
    pub peers: Vec<PeerStatus>,
    /// Devices nearby, ready to pair, and not paired yet.
    pub nearby: Vec<NearbyStatus>,
    pub pairing: Option<PairingStatus>,
    /// Why the service can't sync, if it can't.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerStatus {
    pub id: String,
    pub name: String,
    /// When it last synced, in Unix time.
    pub last_sync: Option<i64>,
    pub syncing: bool,
    /// Why the last sync failed, if it did.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearbyStatus {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairingStatus {
    /// The other device's name, or address.
    pub name: String,
    /// The code to check, once connected.
    pub code: String,
    pub stage: Stage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Connecting,
    /// Waiting for the user to check the code.
    Confirm,
    /// The user confirmed, waiting for the other device's.
    Waiting,
    Paired,
    /// Declined, here (`true`) or on the other device.
    Declined {
        here: bool,
    },
    Failed {
        reason: String,
    },
}

impl Stage {
    /// Whether the pairing is still going on.
    pub fn is_ongoing(&self) -> bool {
        matches!(self, Self::Connecting | Self::Confirm | Self::Waiting)
    }
}
