//! What syncs: it listens for the other devices, finds them with mDNS, pairs with them,
//! and syncs with the paired ones whenever either's logs change. Each app runs it its own
//! way: on desktop, a service of its own, `pochette --sync`.

use std::{
    collections::HashMap,
    fs, io,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};

use mdns_sd::{ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Notify, oneshot},
};

use super::{
    NearbyStatus, PORT, PairingStatus, PeerStatus, Stage, Status,
    channel::{self, Channel, Error, Result},
    device::{self, Identity},
    protocol::{self, Logs, Message, Purpose, REPLY_TIMEOUT, VERSION},
};
use crate::history;

const SERVICE_TYPE: &str = "_pochette._tcp.local.";
const PEERS_FILE: &str = "sync-peers.json";

/// How often to look for logs that changed.
const TICK: Duration = Duration::from_secs(15);
/// How often to sync with a device nearby, even if nothing changed here: it may have
/// played something.
const RESYNC: Duration = Duration::from_mins(10);
/// How long to wait before trying a device that failed again, the first time: then twice as
/// long each time, up to `RESYNC`.
const RETRY: Duration = Duration::from_secs(30);
/// How long to wait for a device to answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
/// How long the user has to check the code.
const PAIRING_TIMEOUT: Duration = Duration::from_mins(3);
/// How long a device stays ready to pair, unless the app says otherwise first.
const PAIRABLE_FOR: Duration = Duration::from_mins(30);
/// Syncs the plays with the paired devices, and pairs others: what the app asks of it,
/// on any platform. Cheap to clone: every clone is the same engine.
#[derive(Clone)]
pub struct Engine(Arc<Service>);

impl Engine {
    /// The engine of this device, with its key and the devices it's paired with, not on the
    /// network yet: see [`Engine::start`].
    pub fn new() -> std::result::Result<Self, String> {
        let identity = Identity::load().map_err(|err| format!("no key for this device: {err}"))?;
        let own_log = device::own_log();
        let own_tag = device::log_tag(&identity.id());
        if !own_log.to_string_lossy().ends_with(&own_tag) {
            return Err("this device's log can't be named after its key".to_owned());
        }
        Ok(Self(Arc::new(Service {
            id: identity.id(),
            own_tag,
            identity,
            name: device::name(),
            dir: device::data_dir(),
            port: AtomicU16::new(0),
            mdns: OnceLock::new(),
            wake: Notify::new(),
            state: Mutex::new(State {
                peers: load_peers(),
                ..State::default()
            }),
        })))
    }

    /// Listen for the other devices, look for them, and sync with the paired ones, from
    /// now on. On a Tokio runtime, which it spawns its tasks on.
    pub async fn start(&self) {
        let service = &self.0;
        match listen().await {
            Ok(listener) => {
                let port = listener.local_addr().map_or(0, |address| address.port());
                service.port.store(port, Ordering::Relaxed);
                tokio::spawn(Arc::clone(service).accept(listener));
            }
            Err(err) => {
                service.state().problem = Some(format!("Can't listen for other devices: {err}"));
            }
        }
        match ServiceDaemon::new() {
            Ok(mdns) => match mdns.browse(SERVICE_TYPE) {
                Ok(events) => {
                    let _ = service.mdns.set(mdns);
                    tokio::spawn(Arc::clone(service).discover(events));
                }
                Err(err) => tracing::warn!("cannot look for devices on the network: {err}"),
            },
            Err(err) => tracing::warn!("cannot look for devices on the network: {err}"),
        }
        tracing::info!(
            "syncing as {} on port {}",
            service.name,
            service.port.load(Ordering::Relaxed)
        );
        service.advertise();
        tokio::spawn(Arc::clone(service).schedule());
    }

    /// Leave the network: tell the others this device is leaving.
    pub fn stop(&self) {
        self.0.unadvertise();
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.0.status()
    }

    /// Whether to keep running, given whether the app is open, and whether the user wants
    /// syncing while it's closed.
    #[must_use]
    pub fn wants_to_run(&self, app_open: bool, in_background: bool) -> bool {
        self.0.is_needed(app_open, in_background)
    }

    /// Be ready to pair, or not, as while the app shows its devices.
    pub fn set_pairable(&self, pairable: bool) {
        self.0.set_pairable(pairable);
    }

    /// Pair with a device nearby, by its id, or at an address.
    pub fn pair(&self, target: String) {
        tokio::spawn(Arc::clone(&self.0).pair(target));
    }

    /// Whether the user confirmed the code.
    pub fn answer_pairing(&self, accept: bool) {
        self.0.answer_code(accept);
    }

    /// Forget how the last pairing went.
    pub fn dismiss_pairing(&self) {
        let mut state = self.0.state();
        if state
            .pairing
            .as_ref()
            .is_some_and(|pairing| !pairing.stage.is_ongoing())
        {
            state.pairing = None;
        }
    }

    pub fn forget(&self, id: &str) {
        self.0.forget(id);
    }

    /// Sync with every device now, as after the logs changed.
    pub fn sync_now(&self) {
        for activity in self.0.state().activity.values_mut() {
            activity.last_attempt = None;
            activity.synced = None;
        }
        self.0.wake.notify_one();
    }
}

/// Listen on `PORT`, or any port if it's taken, over IPv6 and IPv4 if the system has
/// IPv6, else IPv4.
async fn listen() -> io::Result<TcpListener> {
    let mut last = None;
    for port in [PORT, 0] {
        for address in [
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
            SocketAddr::from(([0, 0, 0, 0], port)),
        ] {
            match TcpListener::bind(address).await {
                Ok(listener) => return Ok(listener),
                Err(err) => last = Some(err),
            }
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("no address to listen on")))
}

struct Service {
    identity: Identity,
    id: String,
    own_tag: String,
    name: String,
    dir: PathBuf,
    port: AtomicU16,
    mdns: OnceLock<ServiceDaemon>,
    /// Wakes the scheduler, to sync now.
    wake: Notify,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    peers: Vec<Peer>,
    nearby: HashMap<String, Nearby>,
    /// The ids of the devices nearby, by their mDNS name, which their departure gives.
    nearby_ids: HashMap<String, String>,
    pairable_until: Option<Instant>,
    pairing: Option<Pairing>,
    activity: HashMap<String, Activity>,
    problem: Option<String>,
    /// Whether the service is advertised, and as ready to pair or not.
    advertised: Option<bool>,
}

/// A paired device, as kept in `sync-peers.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Peer {
    id: String,
    name: String,
    /// Where it was last reached, to call it there again, as when mDNS can't find it.
    #[serde(default)]
    addresses: Vec<SocketAddr>,
    #[serde(default)]
    last_sync: Option<i64>,
}

struct Nearby {
    name: String,
    addresses: Vec<SocketAddr>,
    pairable: bool,
    seen: Instant,
}

/// How syncing with a paired device is going.
#[derive(Default)]
struct Activity {
    syncing: bool,
    last_attempt: Option<Instant>,
    failures: u32,
    error: Option<String>,
    /// The logs' sizes when it last synced: it needs syncing again when they change.
    synced: Option<Vec<(String, u64)>>,
}

struct Pairing {
    name: String,
    code: String,
    stage: Stage,
    /// Where the user's verdict goes.
    answer: Option<oneshot::Sender<bool>>,
}

impl Service {
    fn port(&self) -> u16 {
        self.port.load(Ordering::Relaxed)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("the sync state is sound")
    }

    fn status(&self) -> Status {
        let state = self.state();
        let mut nearby: Vec<NearbyStatus> = state
            .nearby
            .iter()
            .filter(|(id, nearby)| {
                nearby.pairable && !state.peers.iter().any(|peer| &peer.id == *id)
            })
            .map(|(id, nearby)| NearbyStatus {
                id: id.clone(),
                name: nearby.name.clone(),
            })
            .collect();
        nearby.sort_by(|a, b| a.name.cmp(&b.name));
        Status {
            name: self.name.clone(),
            peers: state
                .peers
                .iter()
                .map(|peer| {
                    let activity = state.activity.get(&peer.id);
                    PeerStatus {
                        id: peer.id.clone(),
                        name: peer.name.clone(),
                        last_sync: peer.last_sync,
                        syncing: activity.is_some_and(|activity| activity.syncing),
                        error: activity.and_then(|activity| activity.error.clone()),
                    }
                })
                .collect(),
            nearby,
            pairing: state.pairing.as_ref().map(|pairing| PairingStatus {
                name: pairing.name.clone(),
                code: pairing.code.clone(),
                stage: pairing.stage.clone(),
            }),
            problem: state.problem.clone(),
        }
    }

    fn is_pairable(state: &State) -> bool {
        state
            .pairable_until
            .is_some_and(|until| Instant::now() < until)
    }

    /// Whether to keep running, given whether the app is open.
    fn is_needed(&self, app_open: bool, in_background: bool) -> bool {
        let mut state = self.state();
        if state
            .pairing
            .as_ref()
            .is_some_and(|pairing| pairing.stage.is_ongoing())
        {
            return true;
        }
        if !app_open {
            state.pairable_until = None;
        }
        let pairable = Self::is_pairable(&state);
        let paired = !state.peers.is_empty();
        if app_open {
            paired || pairable
        } else {
            paired && in_background
        }
    }

    fn set_pairable(&self, pairable: bool) {
        self.state().pairable_until = pairable.then(|| Instant::now() + PAIRABLE_FOR);
        self.advertise();
    }

    /// Advertise this device on the network, as ready to pair or not, while there are
    /// devices to sync with or to pair.
    fn advertise(&self) {
        let Some(mdns) = self.mdns.get() else {
            return;
        };
        let mut state = self.state();
        let pairable = Self::is_pairable(&state);
        let wanted = (pairable || !state.peers.is_empty()).then_some(pairable);
        if wanted == state.advertised || self.port() == 0 {
            return;
        }
        let result = match wanted {
            Some(pairable) => self
                .service_info(pairable)
                .and_then(|info| mdns.register(info)),
            None => mdns.unregister(&self.fullname()).map(drop),
        };
        match result {
            Ok(()) => state.advertised = wanted,
            Err(err) => tracing::warn!("cannot advertise this device: {err}"),
        }
    }

    fn unadvertise(&self) {
        if let Some(mdns) = self.mdns.get() {
            if self.state().advertised.is_some() {
                // Tell the others it's leaving, before shutting down.
                if let Ok(done) = mdns.unregister(&self.fullname()) {
                    let _ = done.recv_timeout(Duration::from_secs(1));
                }
            }
            let _ = mdns.shutdown();
        }
    }

    fn instance(&self) -> String {
        format!("{} {}", self.name, &self.id[..8])
    }

    fn fullname(&self) -> String {
        format!("{}.{SERVICE_TYPE}", self.instance())
    }

    fn service_info(&self, pairable: bool) -> mdns_sd::Result<ServiceInfo> {
        // A host name of its own, not to step on the system's.
        let host = format!("pochette-{}.local.", &self.id[..8]);
        let properties = [
            ("v", VERSION.to_string()),
            ("id", self.id.clone()),
            ("name", self.name.clone()),
            ("pair", u8::from(pairable).to_string()),
        ];
        Ok(ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance(),
            &host,
            "",
            self.port(),
            &properties[..],
        )?
        .enable_addr_auto())
    }

    /// Keep track of the devices on the network.
    async fn discover(self: Arc<Self>, events: mdns_sd::Receiver<ServiceEvent>) {
        while let Ok(event) = events.recv_async().await {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let Some(id) = info.get_property_val_str("id").map(str::to_owned) else {
                        continue;
                    };
                    if id == self.id || id.len() != 64 {
                        continue;
                    }
                    let port = info.get_port();
                    let mut addresses: Vec<SocketAddr> = info
                        .get_addresses()
                        .iter()
                        .map(|ip| socket_address(ip, port))
                        .collect();
                    // IPv4 first: link-local IPv6 addresses are less often reachable.
                    addresses.sort_by_key(SocketAddr::is_ipv6);
                    let name = info
                        .get_property_val_str("name")
                        .map_or_else(|| info.get_fullname().to_owned(), str::to_owned);
                    let pairable = info.get_property_val_str("pair") == Some("1");
                    let paired = {
                        let mut state = self.state();
                        state
                            .nearby_ids
                            .insert(info.get_fullname().to_owned(), id.clone());
                        let returning = state.nearby.contains_key(&id);
                        state.nearby.insert(
                            id.clone(),
                            Nearby {
                                name,
                                addresses,
                                pairable,
                                seen: Instant::now(),
                            },
                        );
                        !returning && state.peers.iter().any(|peer| peer.id == id)
                    };
                    if paired {
                        self.wake.notify_one();
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    let mut state = self.state();
                    if let Some(id) = state.nearby_ids.remove(&fullname) {
                        state.nearby.remove(&id);
                    }
                }
                _ => {}
            }
        }
    }

    /// Sync with each paired device when it's due.
    async fn schedule(self: Arc<Self>) {
        loop {
            tokio::select! {
                () = tokio::time::sleep(TICK) => {}
                () = self.wake.notified() => {}
            }
            let sizes = device::logs_signature(&self.dir);
            let due: Vec<String> = {
                let state = self.state();
                state
                    .peers
                    .iter()
                    .filter(|peer| Self::is_due(&state, peer, &sizes))
                    .map(|peer| peer.id.clone())
                    .collect()
            };
            for id in due {
                tokio::spawn(Arc::clone(&self).sync_with(id));
            }
        }
    }

    fn is_due(state: &State, peer: &Peer, sizes: &[(String, u64)]) -> bool {
        let nearby = state.nearby.get(&peer.id);
        if nearby.is_none() && peer.addresses.is_empty() {
            return false;
        }
        let Some(activity) = state.activity.get(&peer.id) else {
            return true;
        };
        let Some(last_attempt) = activity.last_attempt else {
            return !activity.syncing;
        };
        if activity.syncing {
            return false;
        }
        let since = last_attempt.elapsed();
        if activity.failures > 0 {
            let wait = RETRY
                .saturating_mul(1 << activity.failures.min(5))
                .min(RESYNC);
            // Back on the network: try now.
            let returned = nearby.is_some_and(|nearby| nearby.seen > last_attempt);
            return since >= wait || returned;
        }
        let changed = activity.synced.as_deref() != Some(sizes);
        let returned = nearby.is_some_and(|nearby| nearby.seen > last_attempt);
        changed || returned || since >= RESYNC
    }

    fn activity<'a>(state: &'a mut State, id: &str) -> &'a mut Activity {
        state.activity.entry(id.to_owned()).or_default()
    }

    async fn sync_with(self: Arc<Self>, id: String) {
        let addresses = {
            let mut state = self.state();
            let activity = Self::activity(&mut state, &id);
            if activity.syncing {
                return;
            }
            activity.syncing = true;
            activity.last_attempt = Some(Instant::now());
            Self::addresses_of(&state, &id)
        };
        let result = self.call_to_sync(&id, &addresses).await;
        let sizes = device::logs_signature(&self.dir);
        let mut state = self.state();
        let activity = Self::activity(&mut state, &id);
        activity.syncing = false;
        match result {
            Ok((address, name, synced)) => {
                activity.failures = 0;
                activity.error = None;
                activity.synced = Some(sizes);
                if synced.received + synced.sent > 0 {
                    tracing::info!(
                        "synced with {name}: {} lines in, {} out",
                        synced.received,
                        synced.sent
                    );
                }
                Self::synced(&mut state, &id, &name, Some(address));
            }
            Err(err) => {
                tracing::info!("cannot sync with {id:.8}: {err}");
                activity.failures += 1;
                activity.error = Some(err.to_string());
            }
        }
    }

    /// Where `id` may be: where mDNS found it, then where it was last reached.
    fn addresses_of(state: &State, id: &str) -> Vec<SocketAddr> {
        let mut addresses: Vec<SocketAddr> = state
            .nearby
            .get(id)
            .map(|nearby| nearby.addresses.clone())
            .unwrap_or_default();
        if let Some(peer) = state.peers.iter().find(|peer| peer.id == id) {
            for address in &peer.addresses {
                if !addresses.contains(address) {
                    addresses.push(*address);
                }
            }
        }
        addresses
    }

    /// Remember that `id`, called `name`, synced, and where it was reached.
    fn synced(state: &mut State, id: &str, name: &str, address: Option<SocketAddr>) {
        let Some(peer) = state.peers.iter_mut().find(|peer| peer.id == id) else {
            return;
        };
        peer.last_sync = Some(history::now());
        name.clone_into(&mut peer.name);
        if let Some(address) = address {
            // Also those kept twice, as an IPv4 address mapped in IPv6 and as itself.
            let canonical =
                |known: &SocketAddr| SocketAddr::new(known.ip().to_canonical(), known.port());
            peer.addresses = peer.addresses.iter().map(canonical).collect();
            let mut seen = Vec::new();
            peer.addresses.retain(|known| {
                let first = !seen.contains(known);
                seen.push(*known);
                first
            });
            peer.addresses.retain(|known| *known != address);
            peer.addresses.insert(0, address);
            peer.addresses.truncate(4);
        }
        save_peers(&state.peers);
    }

    fn hello(&self, purpose: Purpose) -> Message {
        Message::Hello {
            version: VERSION,
            name: self.name.clone(),
            purpose,
            port: self.port(),
        }
    }

    /// Call `id` at one of `addresses` and sync with it. Gives where it listens, and its
    /// name.
    async fn call_to_sync(
        &self,
        id: &str,
        addresses: &[SocketAddr],
    ) -> Result<(SocketAddr, String, protocol::Synced)> {
        let (stream, address) = connect(addresses).await?;
        let mut channel = Channel::open(stream, &self.identity, true).await?;
        if device::hex(&channel.remote) != id {
            return Err(Error::Protocol(format!(
                "another device answered at {address}"
            )));
        }
        channel.send(&self.hello(Purpose::Sync)).await?;
        let (name, port) = expect_hello(&mut channel).await?;
        let logs = Logs {
            dir: &self.dir,
            own_tag: &self.own_tag,
            their_tag: &device::log_tag(id),
        };
        let synced = protocol::sync_as_caller(&mut channel, &logs).await?;
        Ok((
            SocketAddr::new(address.ip().to_canonical(), port),
            name,
            synced,
        ))
    }

    async fn accept(self: Arc<Self>, listener: TcpListener) {
        loop {
            match listener.accept().await {
                Ok((stream, from)) => {
                    let service = Arc::clone(&self);
                    tokio::spawn(async move {
                        if let Err(err) = service.answer(stream, from).await {
                            tracing::info!("call from {from} failed: {err}");
                        }
                    });
                }
                Err(err) => {
                    tracing::warn!("cannot take calls from other devices: {err}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Answer a device that called.
    async fn answer(self: Arc<Self>, stream: TcpStream, from: SocketAddr) -> Result<()> {
        let mut channel = Channel::open(stream, &self.identity, false).await?;
        let Message::Hello {
            version,
            name,
            purpose,
            port,
        } = channel.recv(REPLY_TIMEOUT).await?
        else {
            return Err(Error::Protocol("expected a hello".to_owned()));
        };
        if version != VERSION {
            return refuse(
                &mut channel,
                "Pochette is a different version on each device: update both",
            )
            .await;
        }
        let id = device::hex(&channel.remote);
        let address = SocketAddr::new(from.ip().to_canonical(), port);
        match purpose {
            Purpose::Sync => self.answer_sync(channel, &id, &name, address).await,
            Purpose::Pair => self.answer_pairing(channel, id, name, address).await,
        }
    }

    async fn answer_sync(
        &self,
        mut channel: Channel<Message>,
        id: &str,
        name: &str,
        address: SocketAddr,
    ) -> Result<()> {
        let paired = {
            let mut state = self.state();
            let paired = state.peers.iter().any(|peer| peer.id == id);
            if paired {
                Self::activity(&mut state, id).syncing = true;
            }
            paired
        };
        if !paired {
            let reason = format!("{} isn't paired with this device", self.name);
            return refuse(&mut channel, &reason).await;
        }
        channel.send(&self.hello(Purpose::Sync)).await?;
        let logs = Logs {
            dir: &self.dir,
            own_tag: &self.own_tag,
            their_tag: &device::log_tag(id),
        };
        let result = protocol::sync_as_answerer(&mut channel, &logs).await;
        let sizes = device::logs_signature(&self.dir);
        let mut state = self.state();
        let activity = Self::activity(&mut state, id);
        activity.syncing = false;
        let synced = result?;
        activity.last_attempt = Some(Instant::now());
        activity.failures = 0;
        activity.error = None;
        activity.synced = Some(sizes);
        if synced.received + synced.sent > 0 {
            tracing::info!(
                "{name} synced: {} lines in, {} out",
                synced.received,
                synced.sent
            );
        }
        // Where it listens, if it does.
        let address = (address.port() != 0).then_some(address);
        Self::synced(&mut state, id, name, address);
        Ok(())
    }

    /// Pair with `target`: a device nearby, by its id, or an address.
    async fn pair(self: Arc<Self>, target: String) {
        let (name, addresses) = {
            let mut state = self.state();
            if state
                .pairing
                .as_ref()
                .is_some_and(|pairing| pairing.stage.is_ongoing())
            {
                return;
            }
            let found = state
                .nearby
                .get(&target)
                .map(|nearby| (nearby.name.clone(), nearby.addresses.clone()));
            let (name, addresses) = found.unwrap_or_else(|| (target.clone(), Vec::new()));
            state.pairing = Some(Pairing {
                name: name.clone(),
                code: String::new(),
                stage: Stage::Connecting,
                answer: None,
            });
            (name, addresses)
        };
        let result = async {
            let addresses = if addresses.is_empty() {
                resolve(&target).await?
            } else {
                addresses
            };
            let (stream, address) = connect(&addresses).await?;
            let mut channel = Channel::open(stream, &self.identity, true).await?;
            let id = device::hex(&channel.remote);
            if id == self.id {
                return Err(Error::Protocol("That's this device".to_owned()));
            }
            channel.send(&self.hello(Purpose::Pair)).await?;
            let (their_name, port) = expect_hello(&mut channel).await?;
            let address = SocketAddr::new(address.ip().to_canonical(), port);
            self.confirm(channel, id, their_name, address, true).await
        }
        .await;
        if let Err(err) = result {
            tracing::info!("cannot pair with {name}: {err}");
            self.set_stage(Stage::Failed {
                reason: err.to_string(),
            });
        }
    }

    async fn answer_pairing(
        &self,
        mut channel: Channel<Message>,
        id: String,
        name: String,
        address: SocketAddr,
    ) -> Result<()> {
        let refusal = {
            let mut state = self.state();
            if !Self::is_pairable(&state) {
                Some(format!(
                    "{} isn't ready to pair: open Devices in Pochette on it",
                    self.name
                ))
            } else if state
                .pairing
                .as_ref()
                .is_some_and(|pairing| pairing.stage.is_ongoing())
            {
                Some(format!("{} is pairing with another device", self.name))
            } else {
                state.pairing = Some(Pairing {
                    name: name.clone(),
                    code: String::new(),
                    stage: Stage::Connecting,
                    answer: None,
                });
                None
            }
        };
        if let Some(reason) = refusal {
            return refuse(&mut channel, &reason).await;
        }
        channel.send(&self.hello(Purpose::Pair)).await?;
        let result = self.confirm(channel, id, name, address, false).await;
        if let Err(err) = &result {
            self.set_stage(Stage::Failed {
                reason: err.to_string(),
            });
        }
        result
    }

    /// Have the user check the code, and wait for both devices' verdicts. The device that
    /// called then syncs first.
    async fn confirm(
        &self,
        mut channel: Channel<Message>,
        id: String,
        name: String,
        address: SocketAddr,
        caller: bool,
    ) -> Result<()> {
        let (answer_sender, mut answer) = oneshot::channel();
        self.state().pairing = Some(Pairing {
            name: name.clone(),
            code: channel::pairing_code(&channel.handshake_hash),
            stage: Stage::Confirm,
            answer: Some(answer_sender),
        });
        let deadline = tokio::time::Instant::now() + PAIRING_TIMEOUT;
        let mut mine = None;
        let mut theirs = None;
        while mine.is_none() || theirs.is_none() {
            let within = deadline.saturating_duration_since(tokio::time::Instant::now());
            tokio::select! {
                accept = &mut answer, if mine.is_none() => {
                    let accept = accept.unwrap_or(false);
                    mine = Some(accept);
                    channel.send(&Message::Verdict { accept }).await?;
                    if !accept {
                        self.set_stage(Stage::Declined { here: true });
                        return Ok(());
                    }
                    self.set_stage(Stage::Waiting);
                }
                message = channel.recv(within), if theirs.is_none() => {
                    match message? {
                        Message::Verdict { accept: true } => theirs = Some(true),
                        Message::Verdict { accept: false } => {
                            self.set_stage(Stage::Declined { here: false });
                            return Ok(());
                        }
                        other => return Err(Error::Protocol(format!("expected a verdict, got {other:?}"))),
                    }
                }
                () = tokio::time::sleep_until(deadline) => return Err(Error::Timeout),
            }
        }
        {
            let mut state = self.state();
            state.peers.retain(|peer| peer.id != id);
            state.peers.push(Peer {
                id: id.clone(),
                name: name.clone(),
                addresses: if address.port() == 0 {
                    Vec::new()
                } else {
                    vec![address]
                },
                last_sync: None,
            });
            state.activity.remove(&id);
            save_peers(&state.peers);
        }
        tracing::info!("paired with {name}");
        self.set_stage(Stage::Paired);
        self.advertise();
        if caller {
            self.wake.notify_one();
        }
        Ok(())
    }

    fn set_stage(&self, stage: Stage) {
        if let Some(pairing) = &mut self.state().pairing {
            pairing.stage = stage;
        }
    }

    fn answer_code(&self, accept: bool) {
        let answer = self
            .state()
            .pairing
            .as_mut()
            .and_then(|pairing| pairing.answer.take());
        if let Some(answer) = answer {
            let _ = answer.send(accept);
        }
    }

    fn forget(&self, id: &str) {
        let mut state = self.state();
        state.peers.retain(|peer| peer.id != id);
        state.activity.remove(id);
        save_peers(&state.peers);
        drop(state);
        self.advertise();
    }
}

/// The address of a service mDNS found, with the interface a link-local IPv6 address is on.
fn socket_address(ip: &ScopedIp, port: u16) -> SocketAddr {
    match ip {
        ScopedIp::V6(v6) => {
            let scope = if v6.addr().segments()[0] & 0xffc0 == 0xfe80 {
                v6.scope_id().index
            } else {
                0
            };
            SocketAddr::V6(SocketAddrV6::new(*v6.addr(), port, 0, scope))
        }
        other => SocketAddr::new(other.to_ip_addr(), port),
    }
}

/// The addresses `target` stands for: an IP address or a host name, with or without a port.
async fn resolve(target: &str) -> Result<Vec<SocketAddr>> {
    let target = target.trim();
    if let Ok(address) = target.parse::<SocketAddr>() {
        return Ok(vec![address]);
    }
    if let Ok(ip) = target.trim_matches(['[', ']']).parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, PORT)]);
    }
    let with_port = if target
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
    {
        target.to_owned()
    } else {
        format!("{target}:{PORT}")
    };
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host(with_port)
        .await
        .map_err(|_| Error::Protocol(format!("Can't find {target}")))?
        .collect();
    if addresses.is_empty() {
        return Err(Error::Protocol(format!("Can't find {target}")));
    }
    Ok(addresses)
}

/// Connect to the first of `addresses` that answers.
async fn connect(addresses: &[SocketAddr]) -> Result<(TcpStream, SocketAddr)> {
    let mut last = Error::Protocol("Not found on this network".to_owned());
    for address in addresses {
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok((stream, *address)),
            Ok(Err(err)) => last = Error::Io(err),
            Err(_) => last = Error::Protocol("Not answering".to_owned()),
        }
    }
    Err(last)
}

async fn expect_hello(channel: &mut Channel<Message>) -> Result<(String, u16)> {
    match channel.recv(REPLY_TIMEOUT).await? {
        Message::Hello {
            version,
            name,
            port,
            ..
        } if version == VERSION => Ok((name, port)),
        Message::Hello { .. } => Err(Error::Protocol(
            "Pochette is a different version on each device: update both".to_owned(),
        )),
        Message::Refused { reason } => Err(Error::Protocol(reason)),
        other => Err(Error::Protocol(format!("expected a hello, got {other:?}"))),
    }
}

async fn refuse(channel: &mut Channel<Message>, reason: &str) -> Result<()> {
    channel
        .send(&Message::Refused {
            reason: reason.to_owned(),
        })
        .await
}

fn peers_path() -> PathBuf {
    device::state_dir().join(PEERS_FILE)
}

fn load_peers() -> Vec<Peer> {
    match fs::read_to_string(peers_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
            tracing::warn!("cannot read the paired devices: {err}");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// Whether this device is paired with any, for the app to start the service if it is.
#[must_use]
pub fn has_peers() -> bool {
    !load_peers().is_empty()
}

fn save_peers(peers: &[Peer]) {
    let path = peers_path();
    let result = (|| {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let aside = path.with_extension("json.new");
        fs::write(&aside, serde_json::to_vec_pretty(peers)?)?;
        fs::rename(&aside, &path)
    })();
    if let Err(err) = result {
        tracing::warn!("cannot save the paired devices: {err}");
    }
}
