//! The Albums tab: the albums the user saved to their Qobuz favorites, to search, filter,
//! sort, and play at random.

use std::{
    hash::{BuildHasher, RandomState},
    rc::Rc,
};

use qobuz::{Client, models::Album};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, glib, pango, prelude::*},
    typed_view::grid::{RelmGridItem, TypedGridView},
};
use tokio::task::JoinHandle;

use crate::{
    album_page::{AlbumPage, AlbumPageMsg, AlbumPageOutput},
    covers::VisibleCovers,
    library::{self, Entry, Sort, View},
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
    /// Every saved album.
    entries: Vec<Entry>,
    view: View,
    /// The genres and decades the filters offer, after "All".
    genres: Vec<String>,
    decades: Vec<u16>,
    /// How many albums the grid shows.
    shown: usize,
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    search: gtk::SearchEntry,
    genre_filter: gtk::DropDown,
    decade_filter: gtk::DropDown,
    sort_menu: gtk::DropDown,
    scroller: Option<gtk::ScrolledWindow>,
    client: Option<Client>,
    player: Player,
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
    /// The position picked in the genre filter, 0 being all of them.
    Genre(u32),
    /// The position picked in the decade filter, 0 being all of them.
    Decade(u32),
    Sort(u32),
    Reverse,
    ClearFilters,
    /// Play one of the albums shown, at random.
    PlayRandom,
    RandomLoaded(Box<qobuz::Result<Album>>),
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
                            set_placeholder_text: Some("Search albums, artists, labels"),
                            connect_search_changed[sender] => move |entry| {
                                sender.input(AlbumsMsg::Search(entry.text().into()));
                            },
                            connect_stop_search => |entry| entry.set_text(""),
                        },
                        #[local_ref]
                        genre_filter -> gtk::DropDown {
                            set_tooltip_text: Some("Genre"),
                        },
                        #[local_ref]
                        decade_filter -> gtk::DropDown {
                            set_tooltip_text: Some("Decade"),
                        },
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
        player: Player,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sort_labels: Vec<&str> = Sort::ALL.iter().map(|sort| sort.label()).collect();
        let mut model = Self {
            state: State::Loading,
            entries: Vec::new(),
            view: View::default(),
            genres: Vec::new(),
            decades: Vec::new(),
            shown: 0,
            grid: TypedGridView::new(),
            search: gtk::SearchEntry::new(),
            genre_filter: dropdown(&["All Genres"]),
            decade_filter: dropdown(&["All Decades"]),
            sort_menu: dropdown(&sort_labels),
            scroller: None,
            client: None,
            album_page: AlbumPage::builder().launch(player.clone()).forward(
                sender.input_sender(),
                |output| match output {
                    AlbumPageOutput::Back => AlbumsMsg::CloseAlbum,
                    AlbumPageOutput::SessionExpired => AlbumsMsg::SessionExpired,
                },
            ),
            player,
            error: String::new(),
            task: None,
            random: None,
            covers: VisibleCovers::default(),
            showing_album: false,
        };
        let grid_view = &model.grid.view;
        let search = &model.search;
        let genre_filter = &model.genre_filter;
        let decade_filter = &model.decade_filter;
        let sort_menu = &model.sort_menu;
        let widgets = view_output!();

        for (dropdown, msg) in [
            (
                &model.genre_filter,
                AlbumsMsg::Genre as fn(u32) -> AlbumsMsg,
            ),
            (&model.decade_filter, AlbumsMsg::Decade),
            (&model.sort_menu, AlbumsMsg::Sort),
        ] {
            let sender = sender.clone();
            dropdown
                .connect_selected_notify(move |dropdown| sender.input(msg(dropdown.selected())));
        }
        // Typing anywhere in the grid searches.
        model.search.set_key_capture_widget(Some(&widgets.scroller));
        add_search_shortcut(&root, &sender);

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
                let (Some(client), Some(item)) = (self.client.clone(), self.grid.get(position))
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
                self.state = if self.entries.is_empty() {
                    State::Empty
                } else {
                    State::Loaded
                };
                self.fill_filters();
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
            | AlbumsMsg::Genre(_)
            | AlbumsMsg::Decade(_)
            | AlbumsMsg::Sort(_)
            | AlbumsMsg::Reverse
            | AlbumsMsg::ClearFilters) => self.change_view(msg),

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
            AlbumsMsg::Genre(position) => {
                let genre = pick(&self.genres, position);
                if genre != self.view.genre {
                    self.view.genre = genre;
                    self.show_albums();
                }
            }
            AlbumsMsg::Decade(position) => {
                let decade = pick(&self.decades, position);
                if decade != self.view.decade {
                    self.view.decade = decade;
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
                    self.show_albums();
                }
            }
            AlbumsMsg::Reverse => {
                self.view.reversed = !self.view.reversed;
                self.show_albums();
            }
            AlbumsMsg::ClearFilters => {
                // Each change comes back as a message, which shows the albums.
                self.search.set_text("");
                self.genre_filter.set_selected(0);
                self.decade_filter.set_selected(0);
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
        let genre_labels: Vec<String> = std::iter::once("All Genres".to_owned())
            .chain(self.genres.iter().cloned())
            .collect();
        let decade_labels: Vec<String> = std::iter::once("All Decades".to_owned())
            .chain(self.decades.iter().map(|decade| format!("{decade}s")))
            .collect();
        let genre = position(&self.genres, self.view.genre.as_ref());
        let decade = position(&self.decades, self.view.decade.as_ref());
        replace_choices(&self.genre_filter, &genre_labels, genre);
        replace_choices(&self.decade_filter, &decade_labels, decade);
        // The selection changes above come back as messages after this one, and find
        // the view already up to date.
        self.view.genre = pick(&self.genres, genre);
        self.view.decade = pick(&self.decades, decade);
    }

    /// Fill the grid with the albums the search and filters let through, in order.
    fn show_albums(&mut self) {
        let shown = self.view.apply(&self.entries);
        self.shown = shown.len();
        self.grid.clear();
        self.grid
            .extend_from_iter(shown.into_iter().map(|entry| AlbumItem {
                album: entry.album.clone(),
                covers: self.covers.clone(),
            }));
        if let Some(scroller) = &self.scroller {
            scroller.vadjustment().set_value(0.0);
        }
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

/// The choice at `position` in a filter, where 0 is "All".
fn pick<T: Clone>(choices: &[T], position: u32) -> Option<T> {
    let index = usize::try_from(position).ok()?.checked_sub(1)?;
    choices.get(index).cloned()
}

/// Where `choice` is in a filter, where 0 is "All".
fn position<T: PartialEq>(choices: &[T], choice: Option<&T>) -> u32 {
    choice
        .and_then(|choice| choices.iter().position(|c| c == choice))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(0)
}

/// A drop-down whose button shortens long choices, so it doesn't widen the toolbar.
fn dropdown(choices: &[&str]) -> gtk::DropDown {
    let dropdown = gtk::DropDown::new(Some(gtk::StringList::new(choices)), gtk::Expression::NONE);
    dropdown.set_factory(Some(&label_factory(true)));
    dropdown.set_list_factory(Some(&label_factory(false)));
    dropdown
}

fn label_factory(shorten: bool) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        if shorten {
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.set_max_width_chars(14);
        }
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let text = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .map(|choice| choice.string());
        if let Some(label) = item.child().and_downcast::<gtk::Label>() {
            label.set_label(text.as_deref().unwrap_or_default());
        }
    });
    factory
}

fn replace_choices(dropdown: &gtk::DropDown, labels: &[String], selected: u32) {
    let Some(list) = dropdown.model().and_downcast::<gtk::StringList>() else {
        return;
    };
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    list.splice(0, list.n_items(), &labels);
    dropdown.set_selected(selected);
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

/// An album in the grid.
struct AlbumItem {
    album: Rc<Album>,
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
