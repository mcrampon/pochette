//! What a search of the catalog found: a row of artists, then a grid of albums that loads
//! more as it scrolls.

use std::rc::Rc;

use qobuz::{Client, artist::ArtistPreview, discover::Albums, models::Album};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, prelude::*},
    typed_view::grid::TypedGridView,
};

use crate::{
    album_tile::{self, AlbumItem},
    artist_page::artist_row,
    covers::VisibleCovers,
    navigation::{Destination, PageOutput},
    playback::CoverActions,
};

/// How many albums load at a time.
const PAGE_SIZE: usize = 50;

/// How many artists show, at most: a row's worth in a wide window.
const ARTISTS: usize = 12;

/// How close to the end of the grid more albums load, in screens.
const LOAD_AHEAD: f64 = 1.5;

pub struct SearchPage {
    client: Option<Client>,
    query: String,
    artists: Vec<ArtistPreview>,
    /// Holds the row of artists, replaced with each search.
    artist_holder: gtk::Box,
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    covers: VisibleCovers,
    scroller: gtk::ScrolledWindow,
    has_more: bool,
    loading_albums: bool,
    loading_artists: bool,
    error: Option<String>,
    actions: CoverActions,
}

#[derive(Debug)]
pub enum SearchPageMsg {
    /// Search for `query`, in place of what was searched before.
    Show {
        client: Client,
        query: String,
    },
    Back,
    LoadMore,
    /// Open the album at this position in the grid.
    OpenAlbum(u32),
    /// Open the artist at this index in the row.
    OpenArtist(usize),
}

#[derive(Debug)]
pub enum SearchPageCommand {
    Artists {
        query: String,
        result: qobuz::Result<Vec<ArtistPreview>>,
    },
    Albums {
        query: String,
        offset: usize,
        result: qobuz::Result<Albums>,
    },
}

#[relm4::component(pub)]
impl Component for SearchPage {
    type Init = CoverActions;
    type Input = SearchPageMsg;
    type Output = PageOutput;
    type CommandOutput = SearchPageCommand;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            add_controller = gtk::GestureClick {
                // The mouse's back button.
                set_button: 8,
                connect_pressed[sender] => move |_, _, _, _| sender.input(SearchPageMsg::Back),
            },
            add_controller = gtk::ShortcutController {
                set_scope: gtk::ShortcutScope::Managed,
                add_shortcut = gtk::Shortcut {
                    set_trigger: gtk::ShortcutTrigger::parse_string("<Alt>Left"),
                    set_action: Some(gtk::CallbackAction::new({
                        let sender = sender.clone();
                        move |page, _| {
                            // Every open page has the shortcut: only the one shown takes it.
                            if !page.is_mapped() {
                                return glib::Propagation::Proceed;
                            }
                            sender.input(SearchPageMsg::Back);
                            glib::Propagation::Stop
                        }
                    })),
                },
            },

            gtk::Box {
                set_spacing: 12,
                set_margin_top: 12,
                set_margin_bottom: 12,
                set_margin_start: 24,
                set_margin_end: 36,

                gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some("Back"),
                    add_css_class: "flat",
                    connect_clicked => SearchPageMsg::Back,
                },
                gtk::Label {
                    #[watch]
                    set_label: &format!("\u{201c}{}\u{201d}", model.query.trim()),
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    add_css_class: "pochette-heading",
                },
            },

            #[local_ref]
            artist_holder -> gtk::Box {
                #[watch]
                set_visible: !model.artists.is_empty(),
                set_margin_bottom: 12,
            },

            #[local_ref]
            scroller -> gtk::ScrolledWindow {
                set_hexpand: true,
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                #[watch]
                set_visible: !model.grid.is_empty(),

                #[local_ref]
                grid_view -> gtk::GridView {
                    set_min_columns: 2,
                    set_max_columns: 16,
                    set_single_click_activate: true,
                    add_css_class: "pochette-album-grid",
                    connect_activate[sender] => move |_, position| {
                        sender.input(SearchPageMsg::OpenAlbum(position));
                    },
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_vexpand: true,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                #[watch]
                set_visible: model.grid.is_empty(),

                gtk::Spinner {
                    #[watch]
                    set_visible: model.loading(),
                    set_spinning: true,
                    set_size_request: (32, 32),
                },
                gtk::Label {
                    #[watch]
                    set_visible: !model.loading(),
                    #[watch]
                    set_label: if model.error.is_some() {
                        "Can't Search Qobuz"
                    } else if model.artists.is_empty() {
                        "Nothing Found"
                    } else {
                        "No Albums Found"
                    },
                    add_css_class: "pochette-heading",
                },
                gtk::Label {
                    #[watch]
                    set_visible: !model.loading() && model.error.is_some(),
                    #[watch]
                    set_label: model.error.as_deref().unwrap_or_default(),
                    add_css_class: "dim-label",
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_justify: gtk::Justification::Center,
                },
            },
        }
    }

    fn init(
        actions: CoverActions,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            client: None,
            query: String::new(),
            artists: Vec::new(),
            artist_holder: gtk::Box::default(),
            grid: TypedGridView::new(),
            covers: VisibleCovers::default(),
            scroller: gtk::ScrolledWindow::new(),
            has_more: false,
            loading_albums: false,
            loading_artists: false,
            error: None,
            actions,
        };
        let artist_holder = &model.artist_holder;
        let scroller = &model.scroller;
        let grid_view = &model.grid.view;
        let widgets = view_output!();

        model.covers.attach(&model.scroller);
        album_tile::fit_columns(&model.grid.view);
        // Load more as the end of the grid comes near, or while the grid doesn't fill the
        // screen.
        let adjustment = model.scroller.vadjustment();
        let input = sender.input_sender().clone();
        adjustment.connect_value_changed(move |_| input.emit(SearchPageMsg::LoadMore));
        let input = sender.input_sender().clone();
        adjustment.connect_changed(move |_| input.emit(SearchPageMsg::LoadMore));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SearchPageMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SearchPageMsg::Show { client, query } => {
                if query.trim() == self.query.trim() && self.client.is_some() {
                    return;
                }
                self.client = Some(client);
                self.query = query;
                self.search(&sender);
            }
            SearchPageMsg::Back => {
                let _ = sender.output(PageOutput::Back);
            }
            SearchPageMsg::LoadMore => {
                if self.needs_more() {
                    self.load_albums(&sender);
                }
            }
            SearchPageMsg::OpenAlbum(position) => {
                if let Some(item) = self.grid.get(position) {
                    let album = Album::clone(&item.borrow().album);
                    let _ = sender.output(PageOutput::Open(Destination::Album(Box::new(album))));
                }
            }
            SearchPageMsg::OpenArtist(index) => {
                if let Some(artist) = self.artists.get(index) {
                    let _ = sender.output(PageOutput::Open(Destination::Artist {
                        id: artist.id,
                        name: artist.name.clone(),
                    }));
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        command: SearchPageCommand,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match command {
            // Results of an earlier search.
            SearchPageCommand::Artists { query, .. } | SearchPageCommand::Albums { query, .. }
                if query != self.query => {}
            SearchPageCommand::Artists { result, .. } => {
                self.loading_artists = false;
                match result {
                    Ok(artists) => self.show_artists(artists, &sender),
                    Err(err) => self.failed(&err, &sender),
                }
            }
            SearchPageCommand::Albums { offset, .. } if offset != self.grid.len() as usize => {}
            SearchPageCommand::Albums { result, .. } => {
                self.loading_albums = false;
                match result {
                    Ok(albums) => {
                        self.has_more = albums.has_more;
                        self.grid
                            .extend_from_iter(albums.albums.into_iter().map(|album| AlbumItem {
                                album: Rc::new(album),
                                caption: None,
                                covers: self.covers.clone(),
                                actions: self.actions.clone(),
                            }));
                    }
                    Err(err) => {
                        // Stop there rather than retry at every scroll.
                        self.has_more = false;
                        self.failed(&err, &sender);
                    }
                }
            }
        }
    }
}

impl SearchPage {
    fn loading(&self) -> bool {
        self.loading_albums || self.loading_artists
    }

    /// Start over with the query.
    fn search(&mut self, sender: &ComponentSender<Self>) {
        self.grid.clear();
        self.scroller.vadjustment().set_value(0.0);
        self.show_artists(Vec::new(), sender);
        self.error = None;
        self.has_more = true;
        self.loading_albums = false;

        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading_artists = true;
        let query = self.query.clone();
        // Commands, which stop with the page.
        sender.oneshot_command(async move {
            let result = client.search_artists(&query, ARTISTS).await;
            SearchPageCommand::Artists { query, result }
        });
        self.load_albums(sender);
    }

    fn show_artists(&mut self, artists: Vec<ArtistPreview>, sender: &ComponentSender<Self>) {
        while let Some(row) = self.artist_holder.first_child() {
            self.artist_holder.remove(&row);
        }
        self.artists = artists;
        if !self.artists.is_empty() {
            let sender = sender.clone();
            let row = artist_row("Artists", &self.artists, move |index| {
                sender.input(SearchPageMsg::OpenArtist(index));
            });
            row.set_hexpand(true);
            self.artist_holder.append(&row);
        }
    }

    fn failed(&mut self, err: &qobuz::Error, sender: &ComponentSender<Self>) {
        if matches!(err, qobuz::Error::Unauthorized { .. }) {
            let _ = sender.output(PageOutput::SessionExpired);
        } else {
            tracing::warn!("cannot search for {:?}: {err}", self.query);
            self.error = Some(err.to_string());
        }
    }

    /// Whether the grid's end is near enough to load more.
    fn needs_more(&self) -> bool {
        let adjustment = self.scroller.vadjustment();
        let page = adjustment.page_size();
        adjustment.upper() - (adjustment.value() + page) < LOAD_AHEAD * page
    }

    fn load_albums(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.loading_albums || !self.has_more || self.query.trim().is_empty() {
            return;
        }
        self.loading_albums = true;
        let (query, offset) = (self.query.clone(), self.grid.len() as usize);
        sender.oneshot_command(async move {
            let result = client.search_albums(&query, offset, PAGE_SIZE).await;
            SearchPageCommand::Albums {
                query,
                offset,
                result,
            }
        });
    }
}
