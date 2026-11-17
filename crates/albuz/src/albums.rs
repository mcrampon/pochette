//! The Albums tab: the albums the user saved to their Qobuz favorites, to search, filter,
//! sort, and play at random.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::{BuildHasher, RandomState},
    rc::Rc,
};

use qobuz::{Client, models::Album};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, glib, prelude::*},
    typed_view::grid::TypedGridView,
};
use tokio::task::JoinHandle;

use crate::{
    album_page::{AlbumPage, AlbumPageMsg, AlbumPageOutput},
    album_tile::{self, AlbumItem},
    checklist::Checklist,
    covers::VisibleCovers,
    dropdown::dropdown,
    favorites::Favorites,
    library::{self, Entry, Sort, View},
    playback::CoverActions,
    player::Player,
    settings::Settings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Loading,
    Failed,
    Empty,
    Loaded,
}

pub struct AlbumsPage {
    state: State,
    /// Every saved album.
    entries: Vec<Entry>,
    view: View,
    /// The genres and decades the filters offer.
    genres: Vec<String>,
    decades: Vec<u16>,
    /// How many albums the grid shows.
    shown: usize,
    /// Every album, in order; the filter hides those not in `visible`.
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    /// The albums the search and filters let through, by id.
    visible: Rc<RefCell<HashSet<String>>>,
    search: gtk::SearchEntry,
    genre_picker: Checklist,
    decade_picker: Checklist,
    sort_menu: gtk::DropDown,
    scroller: Option<gtk::ScrolledWindow>,
    client: Option<Client>,
    player: Player,
    favorites: Favorites,
    /// What the buttons over the covers do.
    actions: CoverActions,
    error: String,
    task: Option<JoinHandle<()>>,
    /// Fetching the tracks of the album picked at random.
    random: Option<JoinHandle<()>>,
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
    Search(String),
    FocusSearch,
    /// The positions of the genres picked, none meaning all of them.
    Genres(Vec<usize>),
    /// The positions of the decades picked, none meaning all of them.
    Decades(Vec<usize>),
    Sort(u32),
    Reverse,
    ClearFilters,
    /// Play one of the albums shown, at random.
    PlayRandom,
    RandomLoaded(Box<qobuz::Result<Album>>),
    /// An album was added to the favorites, or removed, from anywhere.
    FavoriteChanged {
        album: Box<Album>,
        added: bool,
    },
}

#[derive(Debug)]
pub enum AlbumsOutput {
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for AlbumsPage {
    type Init = (Player, CoverActions);
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
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Box {
                        set_spacing: 6,
                        set_margin_top: 12,
                        set_margin_start: 36,
                        set_margin_end: 36,

                        #[local_ref]
                        search -> gtk::SearchEntry {
                            set_hexpand: true,
                            set_width_request: 120,
                            // Searching is quick, so it needn't wait long for the typing to pause.
                            set_search_delay: 60,
                            set_placeholder_text: Some("Search albums, artists, labels"),
                            connect_search_changed[sender] => move |entry| {
                                sender.input(AlbumsMsg::Search(entry.text().into()));
                            },
                            connect_stop_search => |entry| entry.set_text(""),
                        },
                        #[local_ref]
                        genre_button -> gtk::MenuButton {},
                        #[local_ref]
                        decade_button -> gtk::MenuButton {},
                        #[local_ref]
                        sort_menu -> gtk::DropDown {
                            set_tooltip_text: Some("Sort By"),
                            set_margin_start: 6,
                        },
                        gtk::Button {
                            #[watch]
                            set_icon_name: if model.view.is_descending() {
                                "view-sort-descending-symbolic"
                            } else {
                                "view-sort-ascending-symbolic"
                            },
                            #[watch]
                            set_tooltip_text: Some(model.view.order_label()),
                            add_css_class: "flat",
                            connect_clicked => AlbumsMsg::Reverse,
                        },
                        gtk::Button {
                            set_icon_name: "media-playlist-shuffle-symbolic",
                            #[watch]
                            set_tooltip_text: Some(if model.view.is_filtered() {
                                "Play a Random Album From These"
                            } else {
                                "Play a Random Album"
                            }),
                            set_margin_start: 6,
                            #[watch]
                            set_sensitive: model.shown > 0,
                            connect_clicked => AlbumsMsg::PlayRandom,
                        },
                    },

                    #[name = "scroller"]
                    gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        #[watch]
                        set_visible: model.shown > 0,

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

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_vexpand: true,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        #[watch]
                        set_visible: model.shown == 0,

                        gtk::Label {
                            set_label: "No Matching Albums",
                            add_css_class: "albuz-heading",
                        },
                        gtk::Button {
                            set_label: "Clear Search and Filters",
                            set_halign: gtk::Align::Center,
                            connect_clicked => AlbumsMsg::ClearFilters,
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
        (player, actions): (Player, CoverActions),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let favorites = actions.favorites.clone();
        let sort_labels: Vec<&str> = Sort::ALL.iter().map(|sort| sort.label()).collect();
        let settings = Settings::load();
        let sort_menu = dropdown(&sort_labels);
        let sort_position = Sort::ALL
            .iter()
            .position(|sort| *sort == settings.album_sort);
        sort_menu.set_selected(
            sort_position
                .and_then(|p| u32::try_from(p).ok())
                .unwrap_or(0),
        );
        let mut model = Self {
            state: State::Loading,
            entries: Vec::new(),
            view: View {
                genres: settings.album_genres,
                decades: settings.album_decades,
                sort: settings.album_sort,
                reversed: settings.album_sort_reversed,
                ..View::default()
            },
            genres: Vec::new(),
            decades: Vec::new(),
            shown: 0,
            grid: TypedGridView::new(),
            visible: Rc::default(),
            search: gtk::SearchEntry::new(),
            genre_picker: Checklist::new("Genres"),
            decade_picker: Checklist::new("Decades"),
            sort_menu,
            scroller: None,
            client: None,
            album_page: AlbumPage::builder()
                .launch((player.clone(), favorites.clone()))
                .forward(sender.input_sender(), |output| match output {
                    AlbumPageOutput::Back => AlbumsMsg::CloseAlbum,
                    AlbumPageOutput::SessionExpired => AlbumsMsg::SessionExpired,
                }),
            player,
            favorites,
            actions,
            error: String::new(),
            task: None,
            random: None,
            covers: VisibleCovers::default(),
            showing_album: false,
        };
        let grid_view = &model.grid.view;
        let search = &model.search;
        let genre_button = model.genre_picker.widget();
        let decade_button = model.decade_picker.widget();
        let sort_menu = &model.sort_menu;
        let widgets = view_output!();

        let visible = model.visible.clone();
        model
            .grid
            .add_filter(move |item: &AlbumItem| visible.borrow().contains(&item.album.id));

        let input = sender.input_sender().clone();
        model
            .genre_picker
            .connect_changed(move |picked| input.emit(AlbumsMsg::Genres(picked)));
        let input = sender.input_sender().clone();
        model
            .decade_picker
            .connect_changed(move |picked| input.emit(AlbumsMsg::Decades(picked)));
        let input = sender.input_sender().clone();
        model
            .sort_menu
            .connect_selected_notify(move |menu| input.emit(AlbumsMsg::Sort(menu.selected())));
        // Typing anywhere in the grid searches.
        model.search.set_key_capture_widget(Some(&widgets.scroller));
        add_search_shortcut(&root, &sender);

        album_tile::fit_columns(&model.grid.view);
        let input = sender.input_sender().clone();
        model.favorites.connect_changed(move |album, added| {
            input.emit(AlbumsMsg::FavoriteChanged {
                album: Box::new(album.clone()),
                added,
            });
        });
        model.covers.attach(&widgets.scroller);
        model.scroller = Some(widgets.scroller.clone());
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
                self.entries.clear();
                self.grid.clear();
                self.shown = 0;
                self.state = State::Loading;
                self.showing_album = false;
            }
            AlbumsMsg::Open(position) => {
                let (Some(client), Some(item)) =
                    (self.client.clone(), self.grid.get_visible(position))
                else {
                    return;
                };
                let album = Album::clone(&item.borrow().album);
                self.album_page.emit(AlbumPageMsg::Show { client, album });
                self.showing_album = true;
            }
            AlbumsMsg::CloseAlbum => self.showing_album = false,
            AlbumsMsg::SessionExpired => {
                let _ = sender.output(AlbumsOutput::SessionExpired);
            }
            // Albums of a user who signed out meanwhile.
            AlbumsMsg::Loaded(_) if self.client.is_none() => {}
            AlbumsMsg::Loaded(Ok(albums)) => {
                self.task = None;
                self.entries = albums.into_iter().map(Entry::new).collect();
                self.favorites.reset(
                    self.client.clone(),
                    self.entries.iter().map(|entry| entry.album.id.clone()),
                );
                self.state = if self.entries.is_empty() {
                    State::Empty
                } else {
                    State::Loaded
                };
                self.fill_filters();
                self.fill_grid();
                self.show_albums();
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

            msg @ (AlbumsMsg::Search(_)
            | AlbumsMsg::FocusSearch
            | AlbumsMsg::Genres(_)
            | AlbumsMsg::Decades(_)
            | AlbumsMsg::Sort(_)
            | AlbumsMsg::Reverse
            | AlbumsMsg::ClearFilters) => self.change_view(msg),

            AlbumsMsg::FavoriteChanged { album, added } => {
                let known = self.entries.iter().any(|entry| entry.album.id == album.id);
                if added && !known {
                    let mut album = *album;
                    // Back where it was if Qobuz refused to remove it, at the top if new.
                    album.favorited_at.get_or_insert_with(unix_time);
                    self.entries.push(Entry::new(album));
                    self.state = State::Loaded;
                    self.fill_filters();
                    self.fill_grid();
                    self.refilter();
                } else if !added && known {
                    // Out of the library, so out of sight, wherever it was removed from.
                    self.entries.retain(|entry| entry.album.id != album.id);
                    if self.entries.is_empty() {
                        self.state = State::Empty;
                    }
                    self.fill_filters();
                    self.refilter();
                }
            }
            AlbumsMsg::PlayRandom => self.play_random(&sender),
            AlbumsMsg::RandomLoaded(result) => {
                let result = *result;
                self.random = None;
                match result {
                    Ok(album) => {
                        if let Some(client) = self.client.clone() {
                            self.player.play_album(client, album, None);
                        }
                    }
                    Err(qobuz::Error::Unauthorized { .. }) => {
                        let _ = sender.output(AlbumsOutput::SessionExpired);
                    }
                    Err(err) => tracing::warn!("cannot load the album picked at random: {err}"),
                }
            }
        }
    }
}

impl AlbumsPage {
    /// Search, filter or sort the albums.
    fn change_view(&mut self, msg: AlbumsMsg) {
        match msg {
            AlbumsMsg::Search(query) => {
                if query != self.view.query {
                    self.view.query = query;
                    self.show_albums();
                }
            }
            AlbumsMsg::FocusSearch => {
                if self.state == State::Loaded && !self.showing_album {
                    self.search.grab_focus();
                }
            }
            AlbumsMsg::Genres(picked) => {
                let genres: Vec<String> = picked
                    .into_iter()
                    .filter_map(|index| self.genres.get(index).cloned())
                    .collect();
                if genres != self.view.genres {
                    self.view.genres.clone_from(&genres);
                    Settings::update(|settings| settings.album_genres = genres);
                    self.show_albums();
                }
            }
            AlbumsMsg::Decades(picked) => {
                let decades: Vec<u16> = picked
                    .into_iter()
                    .filter_map(|index| self.decades.get(index).copied())
                    .collect();
                if decades != self.view.decades {
                    self.view.decades = decades;
                    self.save_view();
                    self.show_albums();
                }
            }
            AlbumsMsg::Sort(position) => {
                let sort = usize::try_from(position)
                    .ok()
                    .and_then(|position| Sort::ALL.get(position).copied())
                    .unwrap_or_default();
                if sort != self.view.sort {
                    self.view.sort = sort;
                    // Each sort starts in its natural order.
                    self.view.reversed = false;
                    self.save_view();
                    self.sort_grid();
                    self.show_albums();
                }
            }
            AlbumsMsg::Reverse => {
                self.view.reversed = !self.view.reversed;
                self.save_view();
                self.sort_grid();
                self.show_albums();
            }
            AlbumsMsg::ClearFilters => {
                // Each change comes back as a message, which shows the albums.
                self.search.set_text("");
                self.genre_picker.clear();
                self.decade_picker.clear();
            }
            // Not about the view.
            _ => {}
        }
    }

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
        for task in [self.task.take(), self.random.take()].into_iter().flatten() {
            task.abort();
        }
    }

    /// Offer the genres and decades of the albums, keeping what was picked if it's
    /// still there.
    fn fill_filters(&mut self) {
        self.genres = library::genres(&self.entries);
        self.decades = library::decades(&self.entries);
        self.view.genres.retain(|genre| self.genres.contains(genre));
        let picked: Vec<usize> = self
            .view
            .genres
            .iter()
            .filter_map(|genre| self.genres.iter().position(|g| g == genre))
            .collect();
        self.genre_picker.set_choices(&self.genres, &picked);

        self.view
            .decades
            .retain(|decade| self.decades.contains(decade));
        let labels: Vec<String> = self
            .decades
            .iter()
            .map(|decade| format!("{decade}s"))
            .collect();
        let picked: Vec<usize> = self
            .view
            .decades
            .iter()
            .filter_map(|decade| self.decades.iter().position(|d| d == decade))
            .collect();
        self.decade_picker.set_choices(&labels, &picked);
    }

    /// Remember the decades and order picked, for the next run.
    fn save_view(&self) {
        let (decades, sort, reversed) = (
            self.view.decades.clone(),
            self.view.sort,
            self.view.reversed,
        );
        Settings::update(|settings| {
            settings.album_decades = decades;
            settings.album_sort = sort;
            settings.album_sort_reversed = reversed;
        });
    }

    /// Put every album in the grid, in order. The search and filters then only hide some,
    /// which lets the grid keep the tiles of the albums that stay: refilling it instead
    /// would redraw hundreds of tiles at every key typed.
    fn fill_grid(&mut self) {
        let all = self.view.sorted(&self.entries);
        self.grid.clear();
        self.grid
            .extend_from_iter(all.into_iter().map(|entry| AlbumItem {
                album: entry.album.clone(),
                caption: None,
                covers: self.covers.clone(),
                actions: self.actions.clone(),
            }));
    }

    /// Put the albums of the grid in the order picked.
    fn sort_grid(&self) {
        let ranks: HashMap<&str, usize> = self
            .view
            .sorted(&self.entries)
            .into_iter()
            .enumerate()
            .map(|(rank, entry)| (entry.album.id.as_str(), rank))
            .collect();
        let rank = |item: &AlbumItem| ranks.get(item.album.id.as_str()).copied();
        self.grid.sort(|a, b| rank(a).cmp(&rank(b)));
    }

    /// Show the albums the search and filters let through, from the top.
    fn show_albums(&mut self) {
        self.refilter();
        if let Some(scroller) = &self.scroller {
            scroller.vadjustment().set_value(0.0);
        }
    }

    /// Show the albums the search and filters let through, staying where the grid was.
    fn refilter(&mut self) {
        let shown = self.view.apply(&self.entries);
        self.shown = shown.len();
        *self.visible.borrow_mut() = shown
            .into_iter()
            .map(|entry| entry.album.id.clone())
            .collect();
        self.grid.notify_filter_changed(0);
    }

    fn play_random(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let shown = self.view.apply(&self.entries);
        if shown.is_empty() {
            return;
        }
        let random = RandomState::new().hash_one(std::time::SystemTime::now());
        let count = u64::try_from(shown.len()).unwrap_or(u64::MAX);
        let mut index = usize::try_from(random % count).unwrap_or_default();
        // Something else than what's playing, when there is something else.
        let playing = self.player.status().borrow().album_id.clone();
        if shown.len() > 1 && playing.as_deref() == Some(shown[index].album.id.as_str()) {
            index = (index + 1) % shown.len();
        }
        let id = shown[index].album.id.clone();
        tracing::debug!("playing album {id}, picked at random");

        if let Some(task) = self.random.take() {
            task.abort();
        }
        let sender = sender.clone();
        self.random = Some(relm4::spawn(async move {
            sender.input(AlbumsMsg::RandomLoaded(Box::new(client.album(&id).await)));
        }));
    }
}

/// Ctrl+F searches the albums.
fn add_search_shortcut(root: &gtk::Stack, sender: &ComponentSender<AlbumsPage>) {
    let sender = sender.clone();
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_scope(gtk::ShortcutScope::Managed);
    shortcuts.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control>f"),
        Some(gtk::CallbackAction::new(move |_, _| {
            sender.input(AlbumsMsg::FocusSearch);
            glib::Propagation::Stop
        })),
    ));
    root.add_controller(shortcuts);
}

/// Now, in Unix time.
fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or_default()
}
