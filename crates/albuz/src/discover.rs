//! The Discover tab: Qobuz's new releases, the albums the press awarded and the ideal
//! discography, each as a row of covers, filtered by genre. "See All" opens a section as
//! a grid that loads more as it scrolls.

use std::rc::Rc;

use qobuz::{
    Client,
    discover::{Albums, Discover, Section},
    models::{Album, Genre},
};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, glib, prelude::*},
    typed_view::grid::TypedGridView,
};
use tokio::task::JoinHandle;

use crate::{
    album_page::{AlbumPage, AlbumPageMsg, AlbumPageOutput},
    album_tile::{self, AlbumItem, AlbumWidgets},
    covers::VisibleCovers,
    genre_picker::GenrePicker,
    player::Player,
};

/// The sections, in the order they're shown.
const SECTIONS: [Section; 3] = [
    Section::NewReleases,
    Section::PressAwards,
    Section::IdealDiscography,
];

/// How many albums a section's grid loads at a time.
const PAGE_SIZE: usize = 50;

/// How close to the end of a section's grid more albums load, in screens.
const LOAD_AHEAD: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Home {
    Loading,
    Failed,
    Loaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Section,
    Album,
}

pub struct DiscoverPage {
    client: Option<Client>,
    home: Home,
    page: Page,
    error: String,
    genres: Vec<Genre>,
    genre_ids: Vec<u64>,
    genre_picker: GenrePicker,
    /// The rows, refilled when the genre changes.
    rows: gtk::Box,
    /// The albums of each row, in order.
    row_albums: Vec<(Section, Vec<Rc<Album>>)>,
    /// The tiles of the rows, to let go of their covers.
    row_tiles: Vec<AlbumWidgets>,
    row_covers: VisibleCovers,
    /// The section shown as a grid.
    section: Option<Section>,
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    grid_covers: VisibleCovers,
    grid_scroller: Option<gtk::ScrolledWindow>,
    has_more: bool,
    loading_more: bool,
    section_error: Option<String>,
    home_task: Option<JoinHandle<()>>,
    section_task: Option<JoinHandle<()>>,
    album_page: Controller<AlbumPage>,
}

#[derive(Debug)]
pub enum DiscoverMsg {
    /// Show the sections to the user `client` signs in as.
    Show(Client),
    /// Forget everything, as the user signed out.
    Clear,
    Reload,
    GenresLoaded(qobuz::Result<Vec<Genre>>),
    Loaded {
        genre_ids: Vec<u64>,
        result: Box<qobuz::Result<Discover>>,
    },
    /// The positions of the genres picked, none meaning all of them.
    Genres(Vec<usize>),
    /// Open the album at this index in a row.
    OpenRowAlbum(Section, usize),
    SeeAll(Section),
    CloseSection,
    LoadMore,
    SectionLoaded {
        section: Section,
        genre_ids: Vec<u64>,
        offset: usize,
        result: qobuz::Result<Albums>,
    },
    /// Open the album at this position in the section's grid.
    OpenGridAlbum(u32),
    CloseAlbum,
    SessionExpired,
}

#[derive(Debug)]
pub enum DiscoverOutput {
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for DiscoverPage {
    type Init = Player;
    type Input = DiscoverMsg;
    type Output = DiscoverOutput;
    type CommandOutput = ();

    view! {
        gtk::Stack {
            set_transition_type: gtk::StackTransitionType::SlideLeftRight,

            add_named[Some("home")] = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_margin_top: 12,
                    set_margin_start: 36,
                    set_margin_end: 36,

                    #[local_ref]
                    genre_button -> gtk::MenuButton {},
                },

                #[transition = "Crossfade"]
                match model.home {
                    Home::Loading => gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,

                        gtk::Spinner {
                            set_spinning: true,
                            set_size_request: (32, 32),
                        },
                    },

                    Home::Failed => gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,

                        gtk::Label {
                            set_label: "Can't Load Discover",
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
                            connect_clicked => DiscoverMsg::Reload,
                        },
                    },

                    Home::Loaded => gtk::Box {
                        #[name = "home_scroller"]
                        gtk::ScrolledWindow {
                            set_hexpand: true,
                            set_vexpand: true,
                            set_hscrollbar_policy: gtk::PolicyType::Never,

                            #[local_ref]
                            rows -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 24,
                                set_margin_top: 12,
                                set_margin_bottom: 24,
                            },
                        },
                    },
                },
            },

            add_named[Some("section")] = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                add_controller = gtk::GestureClick {
                    // The mouse's back button.
                    set_button: 8,
                    connect_pressed[sender] => move |_, _, _, _| sender.input(DiscoverMsg::CloseSection),
                },
                add_controller = gtk::ShortcutController {
                    set_scope: gtk::ShortcutScope::Managed,
                    add_shortcut = gtk::Shortcut {
                        set_trigger: gtk::ShortcutTrigger::parse_string("<Alt>Left"),
                        set_action: Some(gtk::CallbackAction::new({
                            let sender = sender.clone();
                            move |page, _| {
                                if !page.is_mapped() {
                                    return glib::Propagation::Proceed;
                                }
                                sender.input(DiscoverMsg::CloseSection);
                                glib::Propagation::Stop
                            }
                        })),
                    },
                },

                gtk::Box {
                    set_spacing: 12,
                    set_margin_top: 12,
                    set_margin_start: 24,
                    set_margin_end: 36,

                    gtk::Button {
                        set_icon_name: "go-previous-symbolic",
                        set_tooltip_text: Some("Back"),
                        add_css_class: "flat",
                        connect_clicked => DiscoverMsg::CloseSection,
                    },
                    gtk::Label {
                        #[watch]
                        set_label: model.section.map_or("", title),
                        add_css_class: "albuz-heading",
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &model.genre_names(),
                        add_css_class: "dim-label",
                    },
                },

                #[name = "grid_scroller"]
                gtk::ScrolledWindow {
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
                        add_css_class: "albuz-album-grid",
                        connect_activate[sender] => move |_, position| {
                            sender.input(DiscoverMsg::OpenGridAlbum(position));
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
                        set_visible: model.loading_more,
                        set_spinning: true,
                        set_size_request: (32, 32),
                    },
                    gtk::Label {
                        #[watch]
                        set_visible: !model.loading_more,
                        #[watch]
                        set_label: if model.section_error.is_some() {
                            "Can't Load These Albums"
                        } else {
                            "No Albums Here"
                        },
                        add_css_class: "albuz-heading",
                    },
                    gtk::Label {
                        #[watch]
                        set_visible: !model.loading_more && model.section_error.is_some(),
                        #[watch]
                        set_label: model.section_error.as_deref().unwrap_or_default(),
                        add_css_class: "dim-label",
                        set_wrap: true,
                        set_max_width_chars: 50,
                        set_justify: gtk::Justification::Center,
                    },
                },
            },

            add_named: (model.album_page.widget(), Some("album")),

            #[watch]
            set_visible_child_name: match model.page {
                Page::Home => "home",
                Page::Section => "section",
                Page::Album => "album",
            },
        }
    }

    fn init(
        player: Player,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = Self {
            client: None,
            home: Home::Loading,
            page: Page::Home,
            error: String::new(),
            genres: Vec::new(),
            genre_ids: Vec::new(),
            genre_picker: GenrePicker::new(),
            rows: gtk::Box::default(),
            row_albums: Vec::new(),
            row_tiles: Vec::new(),
            row_covers: VisibleCovers::default(),
            section: None,
            grid: TypedGridView::new(),
            grid_covers: VisibleCovers::default(),
            grid_scroller: None,
            has_more: false,
            loading_more: false,
            section_error: None,
            home_task: None,
            section_task: None,
            album_page: AlbumPage::builder().launch(player).forward(
                sender.input_sender(),
                |output| match output {
                    AlbumPageOutput::Back => DiscoverMsg::CloseAlbum,
                    AlbumPageOutput::SessionExpired => DiscoverMsg::SessionExpired,
                },
            ),
        };
        let genre_button = model.genre_picker.widget();
        let rows = &model.rows;
        let grid_view = &model.grid.view;
        let widgets = view_output!();

        let input = sender.input_sender().clone();
        model
            .genre_picker
            .connect_changed(move |picked| input.emit(DiscoverMsg::Genres(picked)));

        model.row_covers.attach(&widgets.home_scroller);
        model.grid_covers.attach(&widgets.grid_scroller);
        // Load more as the end of the grid comes near, or while the grid doesn't fill the
        // screen.
        let adjustment = widgets.grid_scroller.vadjustment();
        let input = sender.input_sender().clone();
        adjustment.connect_value_changed(move |_| input.emit(DiscoverMsg::LoadMore));
        let input = sender.input_sender().clone();
        adjustment.connect_changed(move |_| input.emit(DiscoverMsg::LoadMore));
        model.grid_scroller = Some(widgets.grid_scroller.clone());

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: DiscoverMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            DiscoverMsg::Show(client) => {
                self.client = Some(client.clone());
                let genres_sender = sender.clone();
                relm4::spawn(async move {
                    genres_sender.input(DiscoverMsg::GenresLoaded(client.genres().await));
                });
                self.load_home(&sender);
            }
            DiscoverMsg::Clear => {
                for task in [self.home_task.take(), self.section_task.take()]
                    .into_iter()
                    .flatten()
                {
                    task.abort();
                }
                self.client = None;
                self.genre_ids.clear();
                self.fill_rows(&sender, &Discover::default());
                self.home = Home::Loading;
                self.close_section();
                self.page = Page::Home;
            }
            DiscoverMsg::Reload => self.load_home(&sender),
            DiscoverMsg::GenresLoaded(Ok(genres)) => {
                self.genres = genres;
                let names: Vec<String> =
                    self.genres.iter().map(|genre| genre.name.clone()).collect();
                self.genre_picker.set_choices(&names, &[]);
            }
            DiscoverMsg::GenresLoaded(Err(err)) => Self::failed(&sender, "the genres", &err),
            // Sections of another genre, or of a user who signed out meanwhile.
            DiscoverMsg::Loaded { genre_ids, .. }
                if genre_ids != self.genre_ids || self.client.is_none() => {}
            DiscoverMsg::Loaded { result, .. } => {
                self.home_task = None;
                match *result {
                    Ok(discover) => {
                        self.fill_rows(&sender, &discover);
                        self.home = Home::Loaded;
                    }
                    Err(err) => {
                        Self::failed(&sender, "Discover", &err);
                        self.error = err.to_string();
                        self.home = Home::Failed;
                    }
                }
            }
            DiscoverMsg::Genres(picked) => {
                let genre_ids: Vec<u64> = picked
                    .into_iter()
                    .filter_map(|index| self.genres.get(index))
                    .map(|genre| genre.id)
                    .collect();
                if genre_ids != self.genre_ids {
                    self.genre_ids = genre_ids;
                    self.load_home(&sender);
                    if let Some(section) = self.section {
                        self.open_section(section);
                        self.load_more(&sender);
                    }
                }
            }
            DiscoverMsg::OpenRowAlbum(section, index) => {
                let album = self
                    .row_albums
                    .iter()
                    .find(|(row, _)| *row == section)
                    .and_then(|(_, albums)| albums.get(index))
                    .cloned();
                if let Some(album) = album {
                    self.open_album(&album);
                }
            }
            msg @ (DiscoverMsg::SeeAll(_)
            | DiscoverMsg::CloseSection
            | DiscoverMsg::LoadMore
            | DiscoverMsg::SectionLoaded { .. }) => self.update_section(msg, &sender),
            DiscoverMsg::OpenGridAlbum(position) => {
                if let Some(item) = self.grid.get(position) {
                    let album = item.borrow().album.clone();
                    self.open_album(&album);
                }
            }
            DiscoverMsg::CloseAlbum => {
                if self.page == Page::Album {
                    self.page = if self.section.is_some() {
                        Page::Section
                    } else {
                        Page::Home
                    };
                }
            }
            DiscoverMsg::SessionExpired => {
                let _ = sender.output(DiscoverOutput::SessionExpired);
            }
        }
    }
}

impl DiscoverPage {
    /// Open, close and fill the grid of a section.
    fn update_section(&mut self, msg: DiscoverMsg, sender: &ComponentSender<Self>) {
        match msg {
            DiscoverMsg::SeeAll(section) => {
                self.open_section(section);
                self.page = Page::Section;
                self.load_more(sender);
            }
            DiscoverMsg::CloseSection => {
                if self.page == Page::Section {
                    self.close_section();
                    self.page = Page::Home;
                }
            }
            DiscoverMsg::LoadMore => {
                if self.page == Page::Section && self.needs_more() {
                    self.load_more(sender);
                }
            }
            DiscoverMsg::SectionLoaded {
                section,
                genre_ids,
                offset,
                result,
            } => {
                // A page of another section, genre or scroll than the one shown.
                if Some(section) != self.section
                    || genre_ids != self.genre_ids
                    || offset != self.grid.len() as usize
                {
                    return;
                }
                self.section_task = None;
                self.loading_more = false;
                match result {
                    Ok(albums) => {
                        self.has_more = albums.has_more && !albums.albums.is_empty();
                        self.grid
                            .extend_from_iter(albums.albums.into_iter().map(|album| AlbumItem {
                                caption: caption(section, &album),
                                album: Rc::new(album),
                                covers: self.grid_covers.clone(),
                            }));
                    }
                    Err(err) => {
                        Self::failed(sender, "more albums", &err);
                        // Stop there rather than retry at every scroll.
                        self.has_more = false;
                        self.section_error = Some(err.to_string());
                    }
                }
            }
            // Not about the section.
            _ => {}
        }
    }

    /// The genres picked: "Jazz · Classique".
    fn genre_names(&self) -> String {
        self.genres
            .iter()
            .filter(|genre| self.genre_ids.contains(&genre.id))
            .map(|genre| genre.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    fn load_home(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if let Some(task) = self.home_task.take() {
            task.abort();
        }
        self.home = Home::Loading;
        let (genre_ids, sender) = (self.genre_ids.clone(), sender.clone());
        self.home_task = Some(relm4::spawn(async move {
            let result = Box::new(client.discover(&genre_ids).await);
            sender.input(DiscoverMsg::Loaded { genre_ids, result });
        }));
    }

    /// Log a failure, and sign out if it's because the session expired.
    fn failed(sender: &ComponentSender<Self>, what: &str, err: &qobuz::Error) {
        if matches!(err, qobuz::Error::Unauthorized { .. }) {
            let _ = sender.output(DiscoverOutput::SessionExpired);
        } else {
            tracing::warn!("cannot load {what}: {err}");
        }
    }

    /// Replace the rows with the sections of `discover`.
    fn fill_rows(&mut self, sender: &ComponentSender<Self>, discover: &Discover) {
        for tile in self.row_tiles.drain(..) {
            tile.hide(&self.row_covers);
        }
        while let Some(row) = self.rows.first_child() {
            self.rows.remove(&row);
        }
        self.row_albums.clear();

        for section in SECTIONS {
            let albums: Vec<Rc<Album>> = discover
                .section(section)
                .albums
                .iter()
                .cloned()
                .map(Rc::new)
                .collect();
            if albums.is_empty() {
                continue;
            }
            let (row, tiles) = row(section, &albums, &self.row_covers, sender);
            self.rows.append(&row);
            self.row_tiles.extend(tiles);
            self.row_albums.push((section, albums));
        }
    }

    /// Show `section` as an empty grid, ready to load.
    fn open_section(&mut self, section: Section) {
        if let Some(task) = self.section_task.take() {
            task.abort();
        }
        self.section = Some(section);
        self.grid.clear();
        self.has_more = true;
        self.loading_more = false;
        self.section_error = None;
        if let Some(scroller) = &self.grid_scroller {
            scroller.vadjustment().set_value(0.0);
        }
    }

    fn close_section(&mut self) {
        if let Some(task) = self.section_task.take() {
            task.abort();
        }
        self.section = None;
        self.grid.clear();
        self.loading_more = false;
    }

    /// Whether the grid's end is near enough to load more.
    fn needs_more(&self) -> bool {
        let Some(scroller) = &self.grid_scroller else {
            return false;
        };
        let adjustment = scroller.vadjustment();
        let page = adjustment.page_size();
        adjustment.upper() - (adjustment.value() + page) < LOAD_AHEAD * page
    }

    fn load_more(&mut self, sender: &ComponentSender<Self>) {
        let (Some(client), Some(section)) = (self.client.clone(), self.section) else {
            return;
        };
        if self.loading_more || !self.has_more {
            return;
        }
        self.loading_more = true;
        let (genre_ids, offset, sender) = (
            self.genre_ids.clone(),
            self.grid.len() as usize,
            sender.clone(),
        );
        self.section_task = Some(relm4::spawn(async move {
            let result = client
                .discover_section(section, &genre_ids, offset, PAGE_SIZE)
                .await;
            sender.input(DiscoverMsg::SectionLoaded {
                section,
                genre_ids,
                offset,
                result,
            });
        }));
    }

    fn open_album(&mut self, album: &Album) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.album_page.emit(AlbumPageMsg::Show {
            client,
            album: album.clone(),
        });
        self.page = Page::Album;
    }
}

fn title(section: Section) -> &'static str {
    match section {
        Section::NewReleases => "New Releases",
        Section::PressAwards => "Press Awards",
        Section::IdealDiscography => "Ideal Discography",
    }
}

/// What a tile says under the artist: in the Press Awards, which awards.
fn caption(section: Section, album: &Album) -> Option<String> {
    (section == Section::PressAwards)
        .then(|| album_tile::awards(album))
        .flatten()
}

/// A section's row: its title and buttons, then its albums side by side.
fn row(
    section: Section,
    albums: &[Rc<Album>],
    covers: &VisibleCovers,
    sender: &ComponentSender<DiscoverPage>,
) -> (gtk::Box, Vec<AlbumWidgets>) {
    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    strip.set_margin_start(24);
    strip.set_margin_end(24);
    let mut tiles = Vec::new();
    for (index, album) in albums.iter().enumerate() {
        let (tile, widgets) = album_tile::tile();
        widgets.show(&tile, album, caption(section, album).as_deref(), covers);
        let button = gtk::Button::builder()
            .child(&tile)
            .css_classes(["flat", "albuz-row-tile"])
            .build();
        let sender = sender.clone();
        button.connect_clicked(move |_| sender.input(DiscoverMsg::OpenRowAlbum(section, index)));
        strip.append(&button);
        tiles.push(widgets);
    }
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&strip)
        .build();

    let heading = gtk::Label::builder()
        .label(title(section))
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["albuz-heading"])
        .build();
    let scroll_button = |icon: &str, tooltip: &str, direction: f64| {
        let button = gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tooltip)
            .css_classes(["flat", "circular"])
            .build();
        let adjustment = scroller.hadjustment();
        button.connect_clicked(move |_| {
            let step = adjustment.page_size() * 0.8;
            adjustment.set_value(adjustment.value() + direction * step);
        });
        button
    };
    let see_all = gtk::Button::builder()
        .label("See All")
        .css_classes(["flat"])
        .margin_start(6)
        .build();
    let see_all_sender = sender.clone();
    see_all.connect_clicked(move |_| see_all_sender.input(DiscoverMsg::SeeAll(section)));

    let header = gtk::Box::builder()
        .spacing(6)
        .margin_start(36)
        .margin_end(36)
        .build();
    header.append(&heading);
    header.append(&scroll_button("go-previous-symbolic", "Scroll Back", -1.0));
    header.append(&scroll_button("go-next-symbolic", "Scroll On", 1.0));
    header.append(&see_all);

    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.append(&header);
    row.append(&scroller);
    (row, tiles)
}
