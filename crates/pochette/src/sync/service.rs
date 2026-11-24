//! The service that syncs on desktop, `pochette --sync`: the core's sync engine, in a
//! process of its own, which the app talks to over D-Bus. It runs while Pochette is open,
//! and, if the user wants, while it's closed, started with their session.

use std::{process::ExitCode, time::Duration};

use pochette_core::sync::Engine;

use super::{BUS_NAME, OBJECT_PATH, autostart};

/// How often to check whether the service is still needed.
const LIFECYCLE_EVERY: Duration = Duration::from_secs(5);

/// Run the service until it's no longer needed.
pub fn run() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!("cannot start syncing: {err}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("cannot sync: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), String> {
    let engine = Engine::new()?;
    // The name first: if another service holds it, this one leaves before going on the
    // network.
    let bus = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, Bus(engine.clone()))
        .build()
        .await;
    let bus = match bus {
        Ok(bus) => bus,
        Err(zbus::Error::NameTaken) => {
            tracing::info!("already syncing");
            return Ok(());
        }
        Err(err) => return Err(format!("cannot connect to the session bus: {err}")),
    };
    engine.start().await;

    let dbus = zbus::fdo::DBusProxy::new(&bus)
        .await
        .map_err(|err| err.to_string())?;
    let app_id = zbus::names::BusName::try_from(crate::APP_ID).expect("the app id is a bus name");
    loop {
        tokio::time::sleep(LIFECYCLE_EVERY).await;
        // No answer: the session bus is gone, as when the user logged out.
        let asked =
            tokio::time::timeout(LIFECYCLE_EVERY, dbus.name_has_owner(app_id.clone())).await;
        let Ok(Ok(app_open)) = asked else {
            tracing::info!("the session ended");
            break;
        };
        if !engine.wants_to_run(app_open, autostart::is_enabled()) {
            tracing::info!("no longer needed");
            break;
        }
    }
    tracing::info!("stopping");
    engine.stop();
    Ok(())
}

/// The service, to the app, on the session bus, as `INTERFACE`.
struct Bus(Engine);

#[zbus::interface(name = "io.github.mcrampon.Pochette.Sync1")]
impl Bus {
    /// What's going on, as JSON: see `Status`.
    fn status(&self) -> String {
        serde_json::to_string(&self.0.status()).unwrap_or_default()
    }

    fn set_pairable(&self, pairable: bool) {
        self.0.set_pairable(pairable);
    }

    /// Pair with a device nearby, by its id, or at an address.
    fn pair(&self, target: String) {
        self.0.pair(target);
    }

    /// Whether the user confirmed the code.
    fn answer_pairing(&self, accept: bool) {
        self.0.answer_pairing(accept);
    }

    /// Forget how the last pairing went.
    fn dismiss_pairing(&self) {
        self.0.dismiss_pairing();
    }

    fn forget(&self, id: &str) {
        self.0.forget(id);
    }

    /// Sync with every device now, as after the logs changed.
    fn sync_now(&self) {
        self.0.sync_now();
    }
}
