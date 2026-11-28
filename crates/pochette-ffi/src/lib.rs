//! Pochette's core, for the Android app: what its Kotlin code calls, through the bindings
//! UniFFI generates from this library.
//!
//! One [`Pochette`] object holds the session, the library, the play history and the sync
//! engine. Its `async` methods become Kotlin `suspend` functions, which run on this
//! library's own Tokio runtime.

mod downloads;
mod error;
mod library;
mod link;
mod playback;
mod records;
mod session;
mod stats;
mod sync;
mod text;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use pochette_core::{history, sync::Engine};
use qobuz::{Client, models::Album};

pub use downloads::DownloadProgress;
pub use error::PochetteError;
pub use library::LibraryView;
pub use playback::{Listening, Stream};
pub use records::*;
pub use session::SignIn;
pub use stats::{Period, Stats};
pub use sync::{Observer, SyncStatus};

uniffi::setup_scaffolding!();

type Result<T, E = PochetteError> = std::result::Result<T, E>;

/// Where the app keeps things, and what the device is called.
#[derive(Debug, uniffi::Record)]
pub struct Config {
    /// The play logs: backed up with the app's data.
    pub data_dir: String,
    /// What stays on this device: its sync key, the devices it's paired with.
    pub state_dir: String,
    /// What the other devices call this one: "Pixel 9".
    pub device_name: String,
    /// The app's version, which has its own: "1.0.0".
    pub app_version: String,
    /// Where downloaded albums go: room that isn't backed up.
    pub download_dir: String,
}

/// Pochette, as the Android app sees it. Create one, for the whole app.
#[derive(uniffi::Object)]
pub struct Pochette {
    inner: Arc<Inner>,
}

/// What the calls share, and the tasks they leave running.
pub(crate) struct Inner {
    http: reqwest::Client,
    account: Mutex<Option<session::SignedIn>>,
    /// The user's favorite albums, as Qobuz last listed them.
    favorites: Mutex<Vec<Album>>,
    /// Albums with their tracks, as their pages loaded them: what's played comes from here.
    albums: Mutex<HashMap<String, Album>>,
    plays: Mutex<stats::Plays>,
    log: Mutex<history::Log>,
    /// The sync engine, or why there's none.
    engine: Result<Engine, String>,
    sync: sync::State,
    downloads: pochette_core::downloads::Downloads,
}

#[uniffi::export]
impl Pochette {
    /// Pochette, keeping its things where `config` says. Call it once.
    #[uniffi::constructor]
    pub fn new(config: Config) -> Result<Arc<Self>> {
        #[cfg(target_os = "android")]
        log_to_logcat();
        pochette_core::init(pochette_core::Platform {
            data_dir: PathBuf::from(&config.data_dir),
            state_dir: PathBuf::from(&config.state_dir),
            file_name: pochette_core::file_name(&config.device_name),
            device_name: config.device_name,
        });
        let http = http_client(&config.app_version)?;
        let engine = Engine::new();
        if let Err(err) = &engine {
            tracing::warn!("cannot sync: {err}");
        }
        Ok(Arc::new(Self {
            inner: Arc::new(Inner {
                http,
                account: Mutex::default(),
                favorites: Mutex::default(),
                albums: Mutex::default(),
                plays: Mutex::new(stats::Plays::load()),
                log: Mutex::default(),
                engine,
                sync: sync::State::default(),
                downloads: pochette_core::downloads::Downloads::new(PathBuf::from(
                    &config.download_dir,
                )),
            }),
        }))
    }
}

/// Send the core's logs to logcat, tagged "Pochette".
#[cfg(target_os = "android")]
fn log_to_logcat() {
    use tracing_subscriber::{EnvFilter, prelude::*};

    let _ = tracing_subscriber::registry()
        .with(paranoid_android::layer("Pochette").with_filter(EnvFilter::new("info,mdns_sd=warn")))
        .try_init();
}

fn http_client(app_version: &str) -> Result<reqwest::Client> {
    let builder =
        reqwest::Client::builder().user_agent(format!("Pochette/{app_version} (Android)"));
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );
    builder.build().map_err(|err| PochetteError::Failed {
        message: format!("Can't set up connections: {err}"),
    })
}

/// The runtime every call runs on, for the whole process: reqwest, the sync engine and the
/// sign-in's server need Tokio, whatever polls the call from Kotlin.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("pochette")
            .enable_all()
            .build()
            .expect("a runtime starts")
    })
}

/// Run `future` on the runtime, and wait for it. Cancelled with the call, as when Kotlin
/// cancels the coroutine waiting for it.
async fn run<T: Send + 'static>(
    future: impl Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    /// Aborts the task when the call is dropped before it's done.
    struct Abort(tokio::task::AbortHandle);
    impl Drop for Abort {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    let task = runtime().spawn(future);
    let _abort = Abort(task.abort_handle());
    task.await.map_err(|err| PochetteError::Failed {
        message: format!("The call stopped: {err}"),
    })?
}

/// `mutex`, even if a thread panicked holding it: what it guards stays usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Inner {
    /// The signed-in user's client.
    fn client(&self) -> Result<Client> {
        lock(&self.account)
            .as_ref()
            .map(|signed_in| signed_in.client.clone())
            .ok_or_else(|| PochetteError::SignedOut {
                message: "Not signed in".to_owned(),
            })
    }
}
