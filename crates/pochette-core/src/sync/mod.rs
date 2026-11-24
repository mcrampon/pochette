//! Syncing the plays between the user's devices on the same network: each device's key and
//! logs, the encrypted connection between two devices, what they say over it, and the
//! engine that pairs and syncs, which each app runs its own way.

pub mod channel;
pub mod device;
mod engine;
pub mod protocol;

use serde::{Deserialize, Serialize};

pub use engine::{Engine, has_peers};

/// The port devices listen on, unless it's taken. Fixed, for firewalls to let it
/// through.
pub const PORT: u16 = 45617;

/// How syncing is going, as the app shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// This device's name.
    pub name: String,
    pub peers: Vec<PeerStatus>,
    /// Devices nearby, ready to pair, and not paired yet.
    pub nearby: Vec<NearbyStatus>,
    pub pairing: Option<PairingStatus>,
    /// Why the device can't sync, if it can't.
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
    #[must_use]
    pub fn is_ongoing(&self) -> bool {
        matches!(self, Self::Connecting | Self::Confirm | Self::Waiting)
    }
}
