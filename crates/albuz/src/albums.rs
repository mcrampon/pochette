//! The Albums tab: the albums the user saved to their Qobuz favorites.

use qobuz::{Client, models::Album};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, pango, prelude::*},
    typed_view::grid::{RelmGridItem, TypedGridView},
};
use tokio::task::JoinHandle;

use crate::{
    album_page::{AlbumPage, AlbumPageMsg, AlbumPageOutput},
    covers::VisibleCovers,
    player::Player,
};

/// The side of a cover in the grid, in logical pixels.
const COVER_SIZE: i32 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Loading,
    Failed,
    Empty,
    Loaded,
}

pub struct AlbumsPage {
    state: State,
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    client: Option<Client>,
    error: String,
    task: Option<JoinHandle<()>>,
    covers: VisibleCovers,
    album_page: Controller<AlbumPage>,
    /// Whether the album page is shown instead of the grid.
    showing_album: bool,
}

#[derive(Debug)]
pub enum AlbumsMsg {
    /// Show the albums of the user `client` signs in as.
    Show(Client),
    /// Forget everything, as the user signed out.
    Clear,
    Reload,
    Loaded(qobuz::Result<Vec<Album>>),
    /// Open the album at this position in the grid.
    Open(u32),
    CloseAlbum,
    SessionExpired,
}

#[derive(Debug)]
pub enum AlbumsOutput {
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for AlbumsPage {
    type Init = Player;
    type Input = AlbumsMsg;
    type Output = AlbumsOutput;
    type CommandOutput = ();

    view! {
        gtk::Stack {
            set_transition_type: gtk::StackTransitionType::SlideLeftRight,

            add_named[Some("albums")] = &gtk::Box {
            #[transition = "Crossfade"]
            match model.state {
                State::Loading => gtk::Box {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Spinner {
                        set_spinning: true,
                        set_size_request: (32, 32),
                    },
                },

                State::Failed => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_hexpand: true,
                    set_vexpand: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        set_label: "Can't Load Your Albums",
                        add_css_class: "albuz-heading",
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &model.error,
                        add_css_class: "dim-label",
                        set_wrap: true,
                        set_max_width_chars: 50,
                        set_justify: gtk::Justification::Center,
                    },
                    gtk::Button {
                        set_label: "Try Again",
                        set_halign: gtk::Align::Center,
                        connect_clicked => AlbumsMsg::Reload,
                    },
                },

                State::Empty => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_hexpand: true,
                    set_vexpand: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        set_label: "No Saved Albums",
                        add_css_class: "albuz-heading",
                    },
                    gtk::Label {
                        set_label: "Albums you add to your favorites on Qobuz appear here.",
                        add_css_class: "dim-label",
                    },
                },

                State::Loaded => gtk::Box {
                    #[name = "scroller"]
                    gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_hscrollbar_policy: gtk::PolicyType::Never,

                        #[local_ref]
                        grid_view -> gtk::GridView {
                            set_min_columns: 2,
                            set_max_columns: 16,
                            set_single_click_activate: true,
                            add_css_class: "albuz-album-grid",
                            connect_activate[sender] => move |_, position| {
                                sender.input(AlbumsMsg::Open(position));
                            },
                        },
                    },
                },
            }
            },

            add_named: (model.album_page.widget(), Some("album")),

            #[watch]
            set_visible_child_name: if model.showing_album { "album" } else { "albums" },
        }
    }

    fn init(
        player: Player,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            state: State::Loading,
            grid: TypedGridView::new(),
            client: None,
            error: String::new(),
            task: None,
            covers: VisibleCovers::default(),
            album_page: AlbumPage::builder().launch(player).forward(
                sender.input_sender(),
                |output| match output {
                    AlbumPageOutput::Back => AlbumsMsg::CloseAlbum,
                    AlbumPageOutput::SessionExpired => AlbumsMsg::SessionExpired,
                },
            ),
            showing_album: false,
        };
        let grid_view = &model.grid.view;
        let widgets = view_output!();
        model.covers.attach(&widgets.scroller);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AlbumsMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AlbumsMsg::Show(client) => {
                self.client = Some(client);
                self.load(&sender);
            }
            AlbumsMsg::Reload => self.load(&sender),
            AlbumsMsg::Clear => {
                self.stop_task();
                self.client = None;
                self.grid.clear();
                self.state = State::Loading;
                self.showing_album = false;
            }
            AlbumsMsg::Open(position) => {
                let (Some(client), Some(item)) = (self.client.clone(), self.grid.get(position))
                else {
                    return;
                };
                let album = item.borrow().album.clone();
                self.album_page.emit(AlbumPageMsg::Show { client, album });
                self.showing_album = true;
            }
            AlbumsMsg::CloseAlbum => self.showing_album = false,
            AlbumsMsg::SessionExpired => {
                let _ = sender.output(AlbumsOutput::SessionExpired);
            }
            // Albums of a user who signed out meanwhile.
            AlbumsMsg::Loaded(_) if self.client.is_none() => {}
            AlbumsMsg::Loaded(Ok(mut albums)) => {
                self.task = None;
                // Most recently added first; stable, so ties keep Qobuz's order.
                albums.sort_by_key(|album| std::cmp::Reverse(album.favorited_at));
                self.grid.clear();
                self.state = if albums.is_empty() {
                    State::Empty
                } else {
                    State::Loaded
                };
                self.grid
                    .extend_from_iter(albums.into_iter().map(|album| AlbumItem {
                        album,
                        covers: self.covers.clone(),
                    }));
            }
            AlbumsMsg::Loaded(Err(qobuz::Error::Unauthorized { .. })) => {
                self.task = None;
                let _ = sender.output(AlbumsOutput::SessionExpired);
            }
            AlbumsMsg::Loaded(Err(err)) => {
                self.task = None;
                tracing::warn!("cannot load the favorite albums: {err}");
                self.error = err.to_string();
                self.state = State::Failed;
            }
        }
    }
}

impl AlbumsPage {
    fn load(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.stop_task();
        self.state = State::Loading;
        let sender = sender.clone();
        self.task = Some(relm4::spawn(async move {
            sender.input(AlbumsMsg::Loaded(client.favorite_albums().await));
        }));
    }

    fn stop_task(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// An album in the grid.
struct AlbumItem {
    album: Album,
    covers: VisibleCovers,
}

struct AlbumWidgets {
    cover: gtk::Picture,
    title: gtk::Label,
    artist: gtk::Label,
}

impl RelmGridItem for AlbumItem {
    type Root = gtk::Box;
    type Widgets = AlbumWidgets;

    fn setup(_item: &gtk::ListItem) -> (gtk::Box, AlbumWidgets) {
        relm4::view! {
            root = gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 2,
                set_width_request: COVER_SIZE,
                set_halign: gtk::Align::Center,
                set_cursor_from_name: Some("pointer"),

                #[name = "cover"]
                gtk::Picture {
                    set_size_request: (COVER_SIZE, COVER_SIZE),
                    set_content_fit: gtk::ContentFit::Cover,
                    set_overflow: gtk::Overflow::Hidden,
                    add_css_class: "albuz-cover",
                },
                #[name = "title"]
                gtk::Label {
                    set_xalign: 0.0,
                    // Lets the label shrink to the tile instead of widening it.
                    set_max_width_chars: 1,
                    set_ellipsize: pango::EllipsizeMode::End,
                    set_margin_top: 6,
                    add_css_class: "albuz-album-title",
                },
                #[name = "artist"]
                gtk::Label {
                    set_xalign: 0.0,
                    set_max_width_chars: 1,
                    set_ellipsize: pango::EllipsizeMode::End,
                    add_css_class: "dim-label",
                },
            }
        }

        let widgets = AlbumWidgets {
            cover,
            title,
            artist,
        };
        (root, widgets)
    }

    fn bind(&mut self, widgets: &mut AlbumWidgets, root: &mut gtk::Box) {
        let title = self.album.full_title();
        widgets.title.set_label(&title);
        widgets.artist.set_label(&self.album.artist.name);
        root.set_tooltip_text(Some(&format!("{title}\n{}", self.album.artist.name)));

        match cover_url(&self.album, widgets.cover.scale_factor()) {
            Some(url) => self.covers.show(&widgets.cover, url),
            None => self.covers.hide(&widgets.cover),
        }
    }

    fn unbind(&mut self, widgets: &mut AlbumWidgets, _root: &mut gtk::Box) {
        self.covers.hide(&widgets.cover);
    }
}

/// The smallest cover that stays sharp at the screen's scale.
fn cover_url(album: &Album, scale_factor: i32) -> Option<String> {
    let image = &album.image;
    let sizes = if scale_factor > 1 {
        [&image.large, &image.small]
    } else {
        [&image.small, &image.large]
    };
    sizes.into_iter().flatten().next().cloned()
}
