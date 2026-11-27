//! The app's side of syncing: starting the service, and asking it things over D-Bus.

use std::{
    io,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    time::Duration,
};

use super::{INTERFACE, OBJECT_PATH, Status, bus_name};
use crate::i18n::tr;

/// How long the service may take to start.
const START_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", tr!("sync-no-bus", error = .0.to_string()))]
    Bus(#[from] zbus::Error),
    #[error("{}", tr!("sync-cannot-start", error = .0))]
    Start(String),
}

/// A link to the service, which starts it if it isn't running.
#[derive(Clone, Debug)]
pub struct Client {
    bus: zbus::Connection,
}

impl Client {
    pub async fn connect() -> Result<Self, Error> {
        Ok(Self {
            bus: zbus::Connection::session().await?,
        })
    }

    async fn is_running(&self) -> Result<bool, Error> {
        let dbus = zbus::fdo::DBusProxy::new(&self.bus).await?;
        Ok(dbus
            .name_has_owner(
                zbus::names::BusName::try_from(bus_name())
                    .expect("the service's name is a bus name"),
            )
            .await
            .map_err(zbus::Error::from)?)
    }

    /// Start the service, unless it's running.
    pub async fn start(&self) -> Result<(), Error> {
        if self.is_running().await? {
            return Ok(());
        }
        spawn().map_err(|err| Error::Start(err.to_string()))?;
        let started = tokio::time::Instant::now();
        while started.elapsed() < START_TIMEOUT {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.is_running().await? {
                return Ok(());
            }
        }
        Err(Error::Start(tr!("sync-no-answer")))
    }

    async fn call<R>(
        &self,
        method: &str,
        body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
    ) -> Result<R, Error>
    where
        R: serde::de::DeserializeOwned + zbus::zvariant::Type,
    {
        let reply = self
            .bus
            .call_method(Some(bus_name()), OBJECT_PATH, Some(INTERFACE), method, body)
            .await?;
        Ok(reply.body().deserialize()?)
    }

    pub async fn status(&self) -> Result<Status, Error> {
        let json: String = self.call("Status", &()).await?;
        serde_json::from_str(&json)
            .map_err(|err| Error::Start(format!("the service said something odd: {err}")))
    }

    pub async fn set_pairable(&self, pairable: bool) -> Result<(), Error> {
        self.call("SetPairable", &(pairable,)).await
    }

    pub async fn pair(&self, target: &str) -> Result<(), Error> {
        self.call("Pair", &(target,)).await
    }

    pub async fn answer_pairing(&self, accept: bool) -> Result<(), Error> {
        self.call("AnswerPairing", &(accept,)).await
    }

    pub async fn dismiss_pairing(&self) -> Result<(), Error> {
        self.call("DismissPairing", &()).await
    }

    pub async fn forget(&self, id: &str) -> Result<(), Error> {
        self.call("Forget", &(id,)).await
    }

    pub async fn sync_now(&self) -> Result<(), Error> {
        self.call("SyncNow", &()).await
    }
}

/// Start the service, apart from the app: in its own process group, so that it stays when
/// the app's terminal closes, as it may run while the app is closed.
fn spawn() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .arg("--sync")
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()?;
    // Reaped once it ends, as it may before the app does.
    std::thread::Builder::new()
        .name("sync-service".to_owned())
        .spawn(move || {
            let _ = child.wait();
        })?;
    Ok(())
}

/// Start the service if this device is paired with others, to sync while the app is open.
pub fn start_if_paired() {
    if !super::has_peers() {
        return;
    }
    relm4::spawn(async {
        let started = async { Client::connect().await?.start().await }.await;
        if let Err(err) = started {
            tracing::warn!("{err}");
        }
    });
}
