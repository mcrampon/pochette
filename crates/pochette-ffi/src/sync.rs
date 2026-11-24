//! Syncing the plays with the user's other devices on the same network, with the desktop's
//! engine: while the app is open, and from time to time in the background, as Android lets
//! it ([`Pochette::sync_once`]).

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use pochette_core::sync::{self as core_sync, Engine, Stage};

use crate::{Inner, Pochette, PochetteError, Result, lock, run, runtime};

/// How often the observer hears of changes, at most.
const TICK: Duration = Duration::from_secs(1);

/// What Kotlin hears of, when it changes: see [`Pochette::observe`].
#[uniffi::export(foreign)]
pub trait Observer: Send + Sync {
    fn sync_changed(&self, status: SyncStatus);
    /// The plays changed: what shows them should ask again.
    fn plays_changed(&self);
}

#[derive(Default)]
pub(crate) struct State {
    started: AtomicBool,
    ticking: AtomicBool,
    observer: Mutex<Option<Arc<dyn Observer>>>,
}

/// How syncing is going.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SyncStatus {
    /// This device's name.
    pub name: String,
    pub peers: Vec<Peer>,
    /// Devices nearby, ready to pair, and not paired yet.
    pub nearby: Vec<Nearby>,
    pub pairing: Option<Pairing>,
    /// Why the device can't sync, if it can't.
    pub problem: Option<String>,
}

/// A device it's paired with.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Peer {
    pub id: String,
    pub name: String,
    /// When it last synced, in Unix time.
    pub last_sync: Option<i64>,
    pub syncing: bool,
    /// Why the last sync failed, if it did.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Nearby {
    pub id: String,
    pub name: String,
}

/// A pairing going on, or just over.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Pairing {
    /// The other device's name, or address.
    pub name: String,
    /// The code to check on both devices, once connected.
    pub code: String,
    pub stage: PairingStage,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum PairingStage {
    Connecting,
    /// Waiting for the user to check the code.
    Confirm,
    /// The user confirmed, waiting for the other device's.
    Waiting,
    Paired,
    /// Declined, here or on the other device.
    Declined {
        here: bool,
    },
    Failed {
        reason: String,
    },
}

impl From<core_sync::Status> for SyncStatus {
    fn from(status: core_sync::Status) -> Self {
        Self {
            name: status.name,
            peers: status
                .peers
                .into_iter()
                .map(|peer| Peer {
                    id: peer.id,
                    name: peer.name,
                    last_sync: peer.last_sync,
                    syncing: peer.syncing,
                    error: peer.error,
                })
                .collect(),
            nearby: status
                .nearby
                .into_iter()
                .map(|nearby| Nearby {
                    id: nearby.id,
                    name: nearby.name,
                })
                .collect(),
            pairing: status.pairing.map(|pairing| Pairing {
                name: pairing.name,
                code: pairing.code,
                stage: match pairing.stage {
                    Stage::Connecting => PairingStage::Connecting,
                    Stage::Confirm => PairingStage::Confirm,
                    Stage::Waiting => PairingStage::Waiting,
                    Stage::Paired => PairingStage::Paired,
                    Stage::Declined { here } => PairingStage::Declined { here },
                    Stage::Failed { reason } => PairingStage::Failed { reason },
                },
            }),
            problem: status.problem,
        }
    }
}

#[uniffi::export]
impl Pochette {
    /// Go on the network: be found, find the other devices, and sync with the paired ones,
    /// from now on. Again does nothing.
    pub async fn sync_start(&self) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            inner.start_syncing().await?;
            Ok(())
        })
        .await
    }

    #[must_use]
    pub fn sync_status(&self) -> SyncStatus {
        self.inner.sync_status()
    }

    /// Be ready to pair, or not, as while the app shows its devices.
    pub fn set_pairable(&self, pairable: bool) {
        if let Ok(engine) = &self.inner.engine {
            engine.set_pairable(pairable);
        }
    }

    /// Pair with a device nearby, by its id, or at an address: "192.168.1.20".
    pub fn pair(&self, target: String) {
        if let Ok(engine) = &self.inner.engine {
            engine.pair(target);
        }
    }

    /// Whether the user saw the same code on both devices.
    pub fn answer_pairing(&self, accept: bool) {
        if let Ok(engine) = &self.inner.engine {
            engine.answer_pairing(accept);
        }
    }

    /// Forget how the last pairing went.
    pub fn dismiss_pairing(&self) {
        if let Ok(engine) = &self.inner.engine {
            engine.dismiss_pairing();
        }
    }

    /// Stop syncing with a device.
    pub fn forget(&self, id: &str) {
        if let Ok(engine) = &self.inner.engine {
            engine.forget(id);
        }
    }

    /// Sync with every device now, as after a track was heard.
    pub fn sync_now(&self) {
        if let Ok(engine) = &self.inner.engine {
            engine.sync_now();
        }
    }

    /// Sync with every paired device once, waiting up to `timeout_seconds`: for the
    /// background job. Whether every device synced.
    pub async fn sync_once(&self, timeout_seconds: u32) -> Result<bool> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let engine = inner.start_syncing().await?;
            let since = pochette_core::time::now();
            engine.sync_now();
            let deadline =
                tokio::time::Instant::now() + Duration::from_secs(timeout_seconds.into());
            loop {
                // Done with each device: synced since, or failed.
                let peers = engine.status().peers;
                let synced = peers
                    .iter()
                    .all(|peer| !peer.syncing && peer.last_sync.is_some_and(|at| at >= since));
                let done = peers.iter().all(|peer| {
                    !peer.syncing
                        && (peer.last_sync.is_some_and(|at| at >= since) || peer.error.is_some())
                });
                if done || tokio::time::Instant::now() >= deadline {
                    inner.notice_plays();
                    return Ok(synced);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
    }

    /// Tell `observer` when the sync status or the plays change, until another observer
    /// replaces it, or none.
    pub fn observe(&self, observer: Option<Arc<dyn Observer>>) {
        *lock(&self.inner.sync.observer) = observer;
        if self.inner.sync.ticking.swap(true, Ordering::SeqCst) {
            return;
        }
        let inner = Arc::clone(&self.inner);
        runtime().spawn(async move {
            let mut last = None;
            loop {
                tokio::time::sleep(TICK).await;
                let observer = lock(&inner.sync.observer).clone();
                let Some(observer) = observer else {
                    last = None;
                    continue;
                };
                let status = inner.sync_status();
                if last.as_ref() != Some(&status) {
                    last = Some(status.clone());
                    observer.sync_changed(status);
                }
                inner.notice_plays();
            }
        });
    }
}

impl Inner {
    /// The engine, on the network.
    async fn start_syncing(&self) -> Result<Engine> {
        let engine = self
            .engine
            .clone()
            .map_err(|message| PochetteError::Failed {
                message: format!("Can't sync: {message}"),
            })?;
        if !self.sync.started.swap(true, Ordering::SeqCst) {
            engine.start().await;
        }
        Ok(engine)
    }

    fn sync_status(&self) -> SyncStatus {
        match &self.engine {
            Ok(engine) => engine.status().into(),
            Err(problem) => SyncStatus {
                name: pochette_core::platform().device_name.clone(),
                peers: Vec::new(),
                nearby: Vec::new(),
                pairing: None,
                problem: Some(problem.clone()),
            },
        }
    }

    /// Read the logs again if another device's changed, and say so.
    fn notice_plays(&self) {
        if lock(&self.plays).refresh() {
            self.plays_changed();
        }
    }

    /// Tell the observer the plays changed.
    pub(crate) fn plays_changed(&self) {
        let observer = lock(&self.sync.observer).clone();
        if let Some(observer) = observer {
            observer.plays_changed();
        }
    }
}
