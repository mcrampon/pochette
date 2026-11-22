//! The play history, shared by everything that shows plays: the Albums tab sorting by them,
//! album pages counting them, the stats. It follows the player, which writes the log.

use std::{
    cell::{Cell, Ref, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};

use relm4::gtk::{
    gio::{self, prelude::*},
    glib::{self, ControlFlow},
};
use tokio::sync::broadcast::error::RecvError;

use crate::{history::History, player::Player};

/// Cheap to clone: every clone is the same history.
#[derive(Clone)]
pub struct Plays(Rc<Inner>);

/// How long the logs must be still before reading them again: a sync writes many lines.
const SETTLE: Duration = Duration::from_millis(500);

struct Inner {
    history: RefCell<History>,
    listeners: RefCell<Vec<Box<dyn Fn() -> ControlFlow>>>,
    /// Watches the logs for the plays other devices bring.
    monitor: RefCell<Option<gio::FileMonitor>>,
    reload_pending: Cell<Option<glib::SourceId>>,
}

impl Plays {
    /// The history in the logs, kept up with what `player` plays.
    pub fn new(player: &Player) -> Self {
        let plays = Self(Rc::new(Inner {
            history: RefCell::new(History::load()),
            listeners: RefCell::default(),
            monitor: RefCell::default(),
            reload_pending: Cell::default(),
        }));
        plays.follow_other_devices();
        let weak = Rc::downgrade(&plays.0);
        let mut heard = player.heard();
        relm4::spawn_local(async move {
            loop {
                let (album, listen) = match heard.recv().await {
                    Ok(heard) => heard,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                };
                let Some(inner) = weak.upgrade() else {
                    break;
                };
                inner.history.borrow_mut().add(album, listen);
                Self(inner).changed();
            }
        });
        plays
    }

    pub fn history(&self) -> Ref<'_, History> {
        self.0.history.borrow()
    }

    /// Read the logs again, which syncing may have brought plays from other machines to.
    pub fn reload(&self) {
        self.0.history.replace(History::load());
        self.changed();
    }

    /// Read the logs again when another device's changes, as a sync brings plays.
    fn follow_other_devices(&self) {
        let dir = crate::sync::device::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let monitor = match gio::File::for_path(&dir)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            Ok(monitor) => monitor,
            Err(err) => {
                tracing::warn!("cannot watch for plays from other devices: {err}");
                return;
            }
        };
        let own = crate::sync::device::own_log();
        let weak = Rc::downgrade(&self.0);
        monitor.connect_changed(move |_, file, other, _| {
            let is_other_log = |file: &gio::File| {
                file.path().is_some_and(|path| {
                    path != own
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| {
                                name.starts_with("plays-")
                                    && Path::new(name)
                                        .extension()
                                        .is_some_and(|ext| ext == "jsonl")
                            })
                })
            };
            if !(is_other_log(file) || other.is_some_and(is_other_log)) {
                return;
            }
            let Some(inner) = weak.upgrade() else {
                return;
            };
            if let Some(pending) = inner.reload_pending.take() {
                pending.remove();
            }
            let weak = Rc::downgrade(&inner);
            let pending = glib::timeout_add_local_once(SETTLE, move || {
                if let Some(inner) = weak.upgrade() {
                    inner.reload_pending.set(None);
                    tracing::info!("another device's plays changed, reading them again");
                    Self(inner).reload();
                }
            });
            inner.reload_pending.set(Some(pending));
        });
        self.0.monitor.replace(Some(monitor));
    }

    /// Call `f` whenever the plays change, until it breaks.
    pub fn connect_changed(&self, f: impl Fn() -> ControlFlow + 'static) {
        self.0.listeners.borrow_mut().push(Box::new(f));
    }

    fn changed(&self) {
        // Listeners may add listeners.
        let mut listeners = self.0.listeners.take();
        listeners.retain(|listener| listener().is_continue());
        let added = self.0.listeners.replace(listeners);
        self.0.listeners.borrow_mut().extend(added);
    }
}
