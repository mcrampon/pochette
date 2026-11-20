//! The play history, shared by everything that shows plays: the Albums tab sorting by them,
//! album pages counting them, the stats. It follows the player, which writes the log.

use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use relm4::gtk::glib::ControlFlow;
use tokio::sync::broadcast::error::RecvError;

use crate::{history::History, player::Player};

/// Cheap to clone: every clone is the same history.
#[derive(Clone)]
pub struct Plays(Rc<Inner>);

struct Inner {
    history: RefCell<History>,
    listeners: RefCell<Vec<Box<dyn Fn() -> ControlFlow>>>,
}

impl Plays {
    /// The history in the logs, kept up with what `player` plays.
    pub fn new(player: &Player) -> Self {
        let plays = Self(Rc::new(Inner {
            history: RefCell::new(History::load()),
            listeners: RefCell::default(),
        }));
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
