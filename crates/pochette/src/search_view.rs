//! Search results, beside the tabs rather than in one: the results, then the albums and
//! artists opened from them. Picking a tab leaves them as they are.

use qobuz::Client;
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, prelude::*},
};

use crate::{
    navigation::{Navigator, NavigatorMsg, PageOutput},
    playback::CoverActions,
    player::Player,
};

pub struct SearchView {
    nav: Navigator,
}

#[derive(Debug)]
pub enum SearchViewMsg {
    /// Search for `query`, an empty one closing the results.
    Search(String),
    /// Search as the user `client` signs in as, or close the results as they signed out.
    SetClient(Option<Client>),
    Nav(NavigatorMsg),
}

#[derive(Debug)]
pub enum SearchViewOutput {
    /// The results were closed: back to the tab before.
    Closed,
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for SearchView {
    type Init = (Player, CoverActions);
    type Input = SearchViewMsg;
    type Output = SearchViewOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            #[local_ref]
            nav_stack -> gtk::Stack {
                // Under the results, never shown for long: closing them leaves the view.
                add_named[Some("home")] = &gtk::Box {},
            },
        }
    }

    fn init(
        (player, actions): (Player, CoverActions),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            nav: Navigator::new(player, actions, {
                let input = sender.input_sender().clone();
                move |msg| input.emit(SearchViewMsg::Nav(msg))
            }),
        };
        let nav_stack = model.nav.widget();
        nav_stack.set_hexpand(true);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SearchViewMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SearchViewMsg::Search(query) if query.trim().is_empty() => {
                self.nav.home();
                let _ = sender.output(SearchViewOutput::Closed);
            }
            SearchViewMsg::Search(query) => self.nav.search(query),
            SearchViewMsg::SetClient(client) => self.nav.set_client(client),
            SearchViewMsg::Nav(msg) => {
                let back = matches!(msg, NavigatorMsg::Page(PageOutput::Back));
                if self.nav.update(msg) {
                    let _ = sender.output(SearchViewOutput::SessionExpired);
                } else if back && self.nav.is_home() {
                    // Back from the results.
                    let _ = sender.output(SearchViewOutput::Closed);
                }
            }
        }
    }
}
