//! A tab's pages, one over the other: the tab's own, then the albums, artists and lists
//! opened from it. Back slides to the page under, as it was left.

use std::rc::Rc;

use qobuz::{Client, artist::ReleaseKind, models::Album};
use relm4::{
    Component, ComponentController, Controller,
    gtk::{self, prelude::*},
};

use crate::{
    album_page::{AlbumPage, AlbumPageMsg},
    artist_page::{ArtistPage, ArtistPageMsg},
    playback::CoverActions,
    player::Player,
    releases_page::{ReleasesPage, ReleasesPageMsg},
    search_page::{SearchPage, SearchPageMsg},
};

/// A page to open.
#[derive(Debug, Clone)]
pub enum Destination {
    Album(Box<Album>),
    Artist {
        id: u64,
        name: String,
    },
    /// All the releases of an artist of one kind.
    Releases {
        artist_id: u64,
        artist_name: String,
        kind: ReleaseKind,
    },
    /// What a search of the catalog finds.
    Search(String),
}

/// What a page asks of the navigator.
#[derive(Debug)]
pub enum PageOutput {
    Back,
    Open(Destination),
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[derive(Debug)]
pub enum NavigatorMsg {
    Page(PageOutput),
    /// A slide between pages ended.
    Settled,
}

enum Page {
    Album(Controller<AlbumPage>),
    Artist(Controller<ArtistPage>),
    Releases(Controller<ReleasesPage>),
    Search(Controller<SearchPage>),
}

/// What a page shows, to tell when what's opened is already shown.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shows {
    Album(String),
    Artist(u64),
    Releases(u64, ReleaseKind),
    Search,
}

impl Destination {
    fn shows(&self) -> Shows {
        match self {
            Self::Album(album) => Shows::Album(album.id.clone()),
            Self::Artist { id, .. } => Shows::Artist(*id),
            Self::Releases {
                artist_id, kind, ..
            } => Shows::Releases(*artist_id, *kind),
            Self::Search(_) => Shows::Search,
        }
    }
}

impl Page {
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Album(page) => page.widget().clone().upcast(),
            Self::Artist(page) => page.widget().clone().upcast(),
            Self::Releases(page) => page.widget().clone().upcast(),
            Self::Search(page) => page.widget().clone().upcast(),
        }
    }
}

pub struct Navigator {
    /// The tab's own page, named "home", then the pages opened.
    stack: gtk::Stack,
    pages: Vec<(Shows, Page)>,
    /// Pages gone back from, still in the stack until the slide away from them ends.
    leaving: Vec<Page>,
    client: Option<Client>,
    player: Player,
    actions: CoverActions,
    forward: Rc<dyn Fn(NavigatorMsg)>,
}

/// Pass what a page asks on to the navigator.
fn to_navigator<M>(
    forward: &Rc<dyn Fn(NavigatorMsg)>,
) -> impl FnMut(&mut relm4::Sender<M>, PageOutput) + 'static {
    let forward = forward.clone();
    move |_, output| forward(NavigatorMsg::Page(output))
}

impl Navigator {
    /// A navigator passing its messages to `forward`, for the tab to hand back to
    /// [`Navigator::update`]. The tab adds its own page to [`Navigator::widget`], named
    /// "home".
    pub fn new(
        player: Player,
        actions: CoverActions,
        forward: impl Fn(NavigatorMsg) + 'static,
    ) -> Self {
        let forward: Rc<dyn Fn(NavigatorMsg)> = Rc::new(forward);
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeftRight)
            .build();
        let settled = forward.clone();
        stack.connect_transition_running_notify(move |stack| {
            if !stack.is_transition_running() {
                settled(NavigatorMsg::Settled);
            }
        });
        Self {
            stack,
            pages: Vec::new(),
            leaving: Vec::new(),
            client: None,
            player,
            actions,
            forward,
        }
    }

    pub fn widget(&self) -> &gtk::Stack {
        &self.stack
    }

    /// Open pages as the user `client` signs in as, or close them all as they signed out.
    pub fn set_client(&mut self, client: Option<Client>) {
        if client.is_none() {
            self.home();
        }
        self.client = client;
    }

    /// Back to the tab's own page, closing all the others.
    pub fn home(&mut self) {
        self.leaving
            .extend(self.pages.drain(..).map(|(_, page)| page));
        self.stack.set_visible_child_name("home");
        self.settle_unless_sliding();
    }

    /// Handle a message passed to `forward`. Returns whether Qobuz no longer accepts the
    /// session.
    pub fn update(&mut self, msg: NavigatorMsg) -> bool {
        match msg {
            NavigatorMsg::Page(PageOutput::Back) => self.back(),
            NavigatorMsg::Page(PageOutput::Open(destination)) => self.open(destination),
            NavigatorMsg::Page(PageOutput::SessionExpired) => return true,
            NavigatorMsg::Settled => self.settle(),
        }
        false
    }

    pub fn open(&mut self, destination: Destination) {
        let Some(client) = self.client.clone() else {
            return;
        };
        // Clicked again, or twice: it's shown already.
        let shows = destination.shows();
        if self.pages.last().is_some_and(|(shown, _)| *shown == shows) {
            return;
        }
        let page = match destination {
            Destination::Album(album) => {
                let page = AlbumPage::builder()
                    .launch((self.player.clone(), self.actions.favorites.clone()))
                    .connect_receiver(to_navigator(&self.forward));
                page.emit(AlbumPageMsg::Show {
                    client,
                    album: *album,
                });
                Page::Album(page)
            }
            Destination::Artist { id, name } => {
                let page = ArtistPage::builder()
                    .launch(self.actions.clone())
                    .connect_receiver(to_navigator(&self.forward));
                page.emit(ArtistPageMsg::Show { client, id, name });
                Page::Artist(page)
            }
            Destination::Releases {
                artist_id,
                artist_name,
                kind,
            } => {
                let page = ReleasesPage::builder()
                    .launch(self.actions.clone())
                    .connect_receiver(to_navigator(&self.forward));
                page.emit(ReleasesPageMsg::Show {
                    client,
                    artist_id,
                    artist_name,
                    kind,
                });
                Page::Releases(page)
            }
            Destination::Search(query) => {
                let page = SearchPage::builder()
                    .launch(self.actions.clone())
                    .connect_receiver(to_navigator(&self.forward));
                page.emit(SearchPageMsg::Show { client, query });
                Page::Search(page)
            }
        };
        let widget = page.widget();
        self.stack.add_child(&widget);
        self.stack.set_visible_child(&widget);
        self.pages.push((shows, page));
    }

    /// Search the catalog for `query`, in results at the bottom of the pages: back to them
    /// if they're there, from whatever was opened from them, or opened.
    pub fn search(&mut self, query: String) {
        let results = match self.pages.first() {
            Some((_, Page::Search(page))) => Some(page.sender().clone()),
            _ => None,
        };
        match (results, self.client.clone()) {
            (Some(results), Some(client)) => {
                results.emit(SearchPageMsg::Show { client, query });
                if self.pages.len() > 1 {
                    self.leaving
                        .extend(self.pages.drain(1..).map(|(_, page)| page));
                    self.stack.set_visible_child(&self.pages[0].1.widget());
                    self.settle_unless_sliding();
                }
            }
            (None, _) => self.open(Destination::Search(query)),
            (Some(_), None) => {}
        }
    }

    /// Whether no page is open over the home page.
    pub fn is_home(&self) -> bool {
        self.pages.is_empty()
    }

    /// Back to the page under the one shown.
    fn back(&mut self) {
        let Some((_, page)) = self.pages.pop() else {
            return;
        };
        self.leaving.push(page);
        match self.pages.last() {
            Some((_, under)) => self.stack.set_visible_child(&under.widget()),
            None => self.stack.set_visible_child_name("home"),
        }
        self.settle_unless_sliding();
    }

    fn settle_unless_sliding(&mut self) {
        // Without animations, nothing slides and nothing tells when it's over.
        if !self.stack.is_transition_running() {
            self.settle();
        }
    }

    /// Let go of the pages gone back from, now out of sight.
    fn settle(&mut self) {
        for page in self.leaving.drain(..) {
            self.stack.remove(&page.widget());
        }
    }
}
