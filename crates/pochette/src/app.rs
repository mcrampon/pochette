//! The main window.

use std::{sync::Arc, time::Duration};

use pochette_core::link::{self, Link};
use qobuz::models::Album;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
    actions::{RelmAction, RelmActionGroup},
    gtk::{self, gio, prelude::*},
};
use tokio::task::JoinHandle;

use crate::{
    account::{self, Account},
    albums::{AlbumsMsg, AlbumsOutput, AlbumsPage},
    discover::{DiscoverMsg, DiscoverOutput, DiscoverPage},
    favorites::Favorites,
    i18n::tr,
    icon_label::{HeaderFit, IconLabel},
    mpris,
    navigation::{self, Destination},
    now_playing::{NowPlaying, NowPlayingOutput},
    playback::{CoverActions, Playback},
    player::{Player, State},
    plays::Plays,
    search_view::{SearchView, SearchViewMsg, SearchViewOutput},
    session_store::SessionStore,
    settings::Settings,
    settings_page::{SettingsMsg, SettingsPage},
    split_view::SplitView,
    stats_view::{StatsView, StatsViewMsg, StatsViewOutput},
};

relm4::new_action_group!(WindowActions, "win");
relm4::new_stateless_action!(StatsAction, WindowActions, "stats");
relm4::new_stateless_action!(SettingsAction, WindowActions, "settings");
relm4::new_stateless_action!(SignOutAction, WindowActions, "sign-out");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Checking the stored session.
    Starting,
    /// The stored session couldn't be checked, most likely because we're offline.
    CannotConnect,
    SignedOut,
    /// The sign-in page is open in the browser.
    WaitingForBrowser,
    SignedIn,
}

pub struct App {
    screen: Screen,
    /// A Qobuz link opened before signing in, to open once signed in.
    pending_link: Option<Link>,
    http: reqwest::Client,
    store: Option<Arc<SessionStore>>,
    account: Option<Account>,
    error: Option<String>,
    sign_in_url: Option<String>,
    /// The start-up check or the sign-in in progress.
    task: Option<JoinHandle<()>>,
    albums: Controller<AlbumsPage>,
    discover: Controller<DiscoverPage>,
    now_playing: Controller<NowPlaying>,
    /// Search results, beside the tabs.
    search_view: Controller<SearchView>,
    /// The listening stats, beside the tabs too.
    stats: Controller<StatsView>,
    /// The settings, beside the tabs too.
    settings: Controller<SettingsPage>,
    /// The tabs and the search results.
    library: Option<gtk::Stack>,
    /// The tab to go back to when the search results close.
    last_tab: String,
    /// What the library shows: a tab, the search results, the stats or the settings.
    shown: String,
    player: Player,
    /// The albums in the user's library, which hearts add and remove.
    favorites: Favorites,
    /// Plays albums from the buttons over their covers.
    playback: Playback,
    /// Whether something is playing or paused.
    playing: bool,
    /// Whether the user wants the now playing pane, while something plays.
    pane_open: bool,
    /// Whether the window is too narrow for the pane next to the library, which it
    /// then replaces.
    collapsed: bool,
}

#[derive(Debug)]
pub enum Msg {
    Start,
    SignIn,
    OpenBrowser,
    CopyLink,
    CancelSignIn,
    SignOut,
    SessionExpired,
    PlayingChanged(bool),
    ShowPane(bool),
    Collapsed(bool),
    /// The user picked a tab.
    LibraryShown,
    /// Search the catalog, from the top bar.
    Search(String),
    /// The search results were closed.
    SearchClosed,
    /// Show the listening stats, from the main menu, rather than what's open over them.
    ShowStats,
    /// Show the settings, from the main menu.
    ShowSettings,
    /// The tab shown was clicked: back to it, from what's open over it.
    TabClickedAgain,
    /// Open the page of an album in the library, as from Now Playing.
    OpenAlbum(Box<Album>),
    /// Bring the window to the front, as the desktop's media controls ask.
    Raise,
    /// Open what a Qobuz link points to, as the desktop hands it to Pochette: once signed
    /// in, if need be.
    OpenLink(String),
    // Results of background tasks.
    Started(Arc<SessionStore>, Result<Option<Account>, account::Error>),
    SignInPageReady(String),
    SignInFinished(Result<Account, account::Error>),
}

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some("Pochette"),
            set_default_size: (1100, 760),
            // Lets the Now Playing button's label go when the header bar is short of room.
            #[wrap(Some)]
            #[name = "header_fit"]
            set_titlebar = &HeaderFit {
                #[wrap(Some)]
                set_child = &gtk::HeaderBar {
                    pack_start = &gtk::SearchEntry {
                        set_placeholder_text: Some(&tr!("search-qobuz")),
                        set_width_request: 260,
                        // Each search is a request: wait for the typing to pause.
                        set_search_delay: 300,
                        #[watch]
                        set_visible: model.screen == Screen::SignedIn,
                        connect_search_changed[sender] => move |entry| {
                            sender.input(Msg::Search(entry.text().into()));
                        },
                        // Enter shows the results again, after picking a tab.
                        connect_activate[sender] => move |entry| {
                            sender.input(Msg::Search(entry.text().into()));
                        },
                        connect_stop_search => |entry| entry.set_text(""),
                    },

                    #[wrap(Some)]
                    #[name = "switcher"]
                    set_title_widget = &gtk::StackSwitcher {
                        set_stack: Some(&library),
                        #[watch]
                        set_visible: model.screen == Screen::SignedIn,
                    },

                    pack_end = &gtk::MenuButton {
                        set_icon_name: "open-menu-symbolic",
                        set_tooltip_text: Some(&tr!("main-menu")),
                        set_menu_model: Some(&main_menu),
                        #[watch]
                        set_visible: model.screen == Screen::SignedIn,
                    },

                    pack_end = &gtk::ToggleButton {
                        // Its label goes when the header bar is short of room.
                        set_child: Some(&now_playing_label),
                        // Its padding is an icon's, which the label balances.
                        add_css_class: "image-button",
                        set_tooltip_text: Some(&tr!("now-playing")),
                        #[watch]
                        set_visible: model.screen == Screen::SignedIn && model.playing,
                        #[watch]
                        #[block_signal(pane_toggled)]
                        set_active: model.pane_open,
                        connect_toggled[sender] => move |button| {
                            sender.input(Msg::ShowPane(button.is_active()));
                        } @pane_toggled,
                    },
                },
            },

            #[transition = "Crossfade"]
            match model.screen {
                Screen::Starting => gtk::Box {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Spinner {
                        set_spinning: true,
                        set_size_request: (32, 32),
                    },
                },

                Screen::CannotConnect => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        set_label: &tr!("unreachable"),
                        add_css_class: "pochette-heading",
                    },
                    gtk::Label {
                        #[watch]
                        set_label: model.error.as_deref().unwrap_or_default(),
                        add_css_class: "dim-label",
                        set_wrap: true,
                        set_max_width_chars: 50,
                        set_justify: gtk::Justification::Center,
                    },
                    gtk::Button {
                        set_label: &tr!("try-again"),
                        set_halign: gtk::Align::Center,
                        connect_clicked => Msg::Start,
                    },
                },

                Screen::SignedOut => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 18,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        set_label: "Pochette",
                        add_css_class: "pochette-title",
                    },
                    gtk::Label {
                        set_label: &tr!("sign-in-tagline"),
                        add_css_class: "dim-label",
                    },
                    gtk::Button {
                        set_label: &tr!("sign-in"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "suggested-action",
                        add_css_class: "pochette-pill",
                        connect_clicked => Msg::SignIn,
                    },
                    gtk::Label {
                        #[watch]
                        set_visible: model.error.is_some(),
                        #[watch]
                        set_label: model.error.as_deref().unwrap_or_default(),
                        add_css_class: "error",
                        set_wrap: true,
                        set_max_width_chars: 50,
                        set_justify: gtk::Justification::Center,
                    },
                    gtk::Label {
                        set_label: &tr!("sign-in-hint"),
                        add_css_class: "dim-label",
                        add_css_class: "pochette-caption",
                        set_justify: gtk::Justification::Center,
                    },
                },

                Screen::WaitingForBrowser => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 18,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,

                    gtk::Spinner {
                        set_spinning: true,
                        set_size_request: (32, 32),
                    },
                    gtk::Label {
                        set_label: &tr!("sign-in-continue"),
                        add_css_class: "pochette-heading",
                    },
                    gtk::Label {
                        set_label: &tr!("sign-in-continue-hint"),
                        add_css_class: "dim-label",
                        set_justify: gtk::Justification::Center,
                    },
                    gtk::Box {
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,

                        gtk::Button {
                            set_label: &tr!("sign-in-open-again"),
                            #[watch]
                            set_sensitive: model.sign_in_url.is_some(),
                            connect_clicked => Msg::OpenBrowser,
                        },
                        gtk::Button {
                            set_label: &tr!("sign-in-copy-link"),
                            #[watch]
                            set_sensitive: model.sign_in_url.is_some(),
                            connect_clicked => Msg::CopyLink,
                        },
                    },
                    gtk::Button {
                        set_label: &tr!("sign-in-cancel"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "flat",
                        connect_clicked => Msg::CancelSignIn,
                    },
                },

                Screen::SignedIn => gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Label {
                        #[watch]
                        set_visible: model.account.as_ref().is_some_and(|a| !a.user.can_stream()),
                        set_label: &tr!("previews-only"),
                        add_css_class: "warning",
                        set_margin_all: 6,
                    },

                    #[name = "split"]
                    SplitView {
                        set_vexpand: true,
                        set_pane: model.now_playing.widget(),
                        #[watch]
                        set_show_pane: model.playing && model.pane_open,

                        #[name = "library"]
                        set_content = &gtk::Stack {
                        set_transition_type: gtk::StackTransitionType::Crossfade,

                        add_titled: (model.discover.widget(), Some("discover"), &tr!("tab-discover")),
                        add_titled: (model.albums.widget(), Some("albums"), &tr!("tab-albums")),
                        // Without a title, so without a button: the search bar shows it.
                        add_named: (model.search_view.widget(), Some("search")),
                        add_named: (model.stats.widget(), Some("stats")),
                        add_named: (model.settings.widget(), Some("settings")),
                        // The library first: Discover is for when there's time to explore.
                        set_visible_child_name: "albums",
                        connect_visible_child_notify => Msg::LibraryShown,
                        },
                    },
                },
            }
        }
    }

    menu! {
        main_menu: {
            &tr!("menu-stats") => StatsAction,
            &tr!("settings") => SettingsAction,
            &tr!("menu-sign-out") => SignOutAction,
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let player = Player::new().expect("GStreamer and its playbin are installed");
        let favorites = Favorites::default();
        let playback = Playback::new(player.clone());
        let actions = CoverActions {
            favorites: favorites.clone(),
            playback: playback.clone(),
            plays: Plays::new(&player),
        };
        let settings = Settings::load();
        player.set_volume(settings.volume);
        player.set_quality(settings.quality);
        finish_playback_before_closing(&root, &player);
        play_pause_with_space(&root, &player);
        leave_search_bars_on_click(&root);
        let raise = sender.input_sender().clone();
        mpris::start(player.clone(), move || raise.emit(Msg::Raise));
        let mut model = Self {
            screen: Screen::Starting,
            pending_link: None,
            http: account::http_client(),
            store: None,
            account: None,
            error: None,
            sign_in_url: None,
            task: None,
            albums: AlbumsPage::builder()
                .launch((player.clone(), actions.clone()))
                .forward(sender.input_sender(), |output| match output {
                    AlbumsOutput::SessionExpired => Msg::SessionExpired,
                }),
            discover: DiscoverPage::builder()
                .launch((player.clone(), actions.clone()))
                .forward(sender.input_sender(), |output| match output {
                    DiscoverOutput::SessionExpired => Msg::SessionExpired,
                }),
            now_playing: NowPlaying::builder()
                .launch((player.clone(), favorites.clone()))
                .forward(sender.input_sender(), |output| match output {
                    NowPlayingOutput::OpenAlbum(album) => Msg::OpenAlbum(Box::new(album)),
                }),
            search_view: SearchView::builder()
                .launch((player.clone(), actions.clone()))
                .forward(sender.input_sender(), |output| match output {
                    SearchViewOutput::Closed => Msg::SearchClosed,
                    SearchViewOutput::SessionExpired => Msg::SessionExpired,
                }),
            stats: StatsView::builder()
                .launch((player.clone(), actions.clone()))
                .forward(sender.input_sender(), |output| match output {
                    StatsViewOutput::SessionExpired => Msg::SessionExpired,
                }),
            settings: SettingsPage::builder().launch(()).detach(),
            library: None,
            last_tab: "albums".to_owned(),
            shown: "albums".to_owned(),
            player,
            favorites,
            playback,
            playing: false,
            pane_open: false,
            collapsed: false,
        };
        let now_playing_label = IconLabel::new("audio-x-generic-symbolic", &tr!("now-playing"));
        let widgets = view_output!();
        widgets.header_fit.set_label(&now_playing_label);
        model.library = Some(widgets.library.clone());
        let input = sender.input_sender().clone();
        widgets
            .split
            .connect_collapsed(move |collapsed| input.emit(Msg::Collapsed(collapsed)));
        widgets.split.set_pane_width(settings.pane_width);
        widgets.split.connect_pane_resized(|width| {
            Settings::update(|settings| settings.pane_width = width);
        });
        follow_playback(&model.player, &sender);
        follow_clicks_on_tab_shown(&widgets.switcher, &sender);

        let mut actions = RelmActionGroup::<WindowActions>::new();
        let input = sender.input_sender().clone();
        actions.add_action(RelmAction::<StatsAction>::new_stateless(move |_| {
            input.emit(Msg::ShowStats);
        }));
        let input = sender.input_sender().clone();
        actions.add_action(RelmAction::<SettingsAction>::new_stateless(move |_| {
            input.emit(Msg::ShowSettings);
        }));
        relm4::main_application().set_accels_for_action("win.settings", &["<Control>comma"]);
        let input = sender.input_sender().clone();
        actions.add_action(RelmAction::<SignOutAction>::new_stateless(move |_| {
            input.emit(Msg::SignOut);
        }));
        actions.register_for_widget(&root);

        crate::sync::client::start_if_paired();
        sender.input(Msg::Start);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Msg, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Msg::Start => {
                self.screen = Screen::Starting;
                self.error = None;
                let store = self.store.clone();
                let http = self.http.clone();
                self.run(async move {
                    let store = match store {
                        Some(store) => store,
                        None => Arc::new(SessionStore::open().await),
                    };
                    let result = account::resume(&store, &http).await;
                    sender.input(Msg::Started(store, result));
                });
            }
            Msg::Started(store, result) => {
                self.store = Some(store);
                match result {
                    Ok(Some(account)) => self.signed_in(account),
                    Ok(None) => self.screen = Screen::SignedOut,
                    Err(err) => {
                        tracing::warn!("cannot check the stored session: {err}");
                        self.error = Some(err.to_string());
                        self.screen = Screen::CannotConnect;
                    }
                }
            }

            Msg::SignIn => {
                let Some(store) = self.store.clone() else {
                    return;
                };
                self.screen = Screen::WaitingForBrowser;
                self.error = None;
                self.sign_in_url = None;
                let http = self.http.clone();
                self.run(async move {
                    let page_ready = sender.input_sender().clone();
                    let result = account::sign_in(&store, &http, move |url| {
                        page_ready.emit(Msg::SignInPageReady(url));
                    })
                    .await;
                    sender.input(Msg::SignInFinished(result));
                });
            }
            Msg::SignInPageReady(url) if self.screen == Screen::WaitingForBrowser => {
                open_in_browser(&url, root);
                self.sign_in_url = Some(url);
            }
            Msg::OpenBrowser => {
                if let Some(url) = &self.sign_in_url {
                    open_in_browser(url, root);
                }
            }
            Msg::CopyLink => {
                if let Some(url) = &self.sign_in_url {
                    root.clipboard().set_text(url);
                }
            }
            Msg::CancelSignIn => {
                self.stop_task();
                self.sign_in_url = None;
                self.screen = Screen::SignedOut;
            }
            Msg::SignInFinished(result) if self.screen == Screen::WaitingForBrowser => {
                self.task = None;
                self.sign_in_url = None;
                match result {
                    Ok(account) => self.signed_in(account),
                    Err(err) => {
                        tracing::warn!("sign-in failed: {err}");
                        self.error = Some(err.to_string());
                        self.screen = Screen::SignedOut;
                    }
                }
            }
            // A sign-in that was cancelled meanwhile.
            Msg::SignInPageReady(_) | Msg::SignInFinished(_) => {}

            Msg::SignOut => self.sign_out(),
            Msg::SessionExpired => {
                tracing::info!("Qobuz no longer accepts the session");
                self.sign_out();
                self.error = Some(tr!("session-expired"));
            }

            Msg::PlayingChanged(playing) => {
                if playing && !self.playing {
                    // Open the pane when playback starts, unless it would hide what the
                    // user is looking at.
                    self.pane_open = !self.collapsed;
                }
                self.playing = playing;
            }
            Msg::ShowPane(show) => self.pane_open = show,
            Msg::Collapsed(collapsed) => self.collapsed = collapsed,
            Msg::Raise => root.present(),
            Msg::Search(query) => self.search(query),
            Msg::SearchClosed => self.show(&self.last_tab.clone()),
            Msg::ShowStats => self.show_stats(),
            Msg::ShowSettings => self.show("settings"),
            Msg::TabClickedAgain => self.home(),
            Msg::LibraryShown => self.library_shown(),
            Msg::OpenAlbum(album) => self.open_album(*album),
            Msg::OpenLink(uri) => self.open_uri(&uri),
        }
    }
}

/// A click on the button of the tab shown, which does nothing to the switcher, takes the
/// tab back to its own page.
///
/// A click on any of them first takes the focus from the page shown, if it has it: the
/// stack would hand it to the first widget of the page it shows next, often a search bar.
fn follow_clicks_on_tab_shown(switcher: &gtk::StackSwitcher, sender: &ComponentSender<App>) {
    let click = gtk::GestureClick::new();
    // Before the button, which still gets the click.
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sender = sender.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        let Some(switcher) = gesture.widget() else {
            return;
        };
        if let Some(stack) = switcher
            .downcast_ref::<gtk::StackSwitcher>()
            .and_then(gtk::StackSwitcher::stack)
        {
            navigation::drop_focus_within(&stack);
        }
        let button = switcher
            .pick(x, y, gtk::PickFlags::DEFAULT)
            .and_then(|target| {
                if target.is::<gtk::ToggleButton>() {
                    Some(target)
                } else {
                    target.ancestor(gtk::ToggleButton::static_type())
                }
            })
            .and_then(|button| button.downcast::<gtk::ToggleButton>().ok());
        // Active before the click: the tab shown.
        if button.is_some_and(|button| button.is_active()) {
            sender.input(Msg::TabClickedAgain);
        }
    });
    switcher.add_controller(click);
}

fn follow_playback(player: &Player, sender: &ComponentSender<App>) {
    let mut status = player.status();
    let sender = sender.clone();
    relm4::spawn(async move {
        let mut playing = false;
        while status.changed().await.is_ok() {
            let now = status.borrow_and_update().state != State::Stopped;
            if now != playing {
                playing = now;
                sender.input(Msg::PlayingChanged(playing));
            }
        }
    });
}

impl App {
    /// Search the catalog in the tab shown.
    /// Open the page of `album` in what the library shows: a tab, or the search results.
    fn open_album(&mut self, album: Album) {
        self.open(Destination::Album(Box::new(album)));
    }

    /// Open what `uri`, a Qobuz link, points to, now or once signed in.
    fn open_uri(&mut self, uri: &str) {
        tracing::info!("asked to open {uri}");
        match link::parse(uri) {
            Some(link) if self.account.is_some() => self.open_link(link),
            Some(link) => self.pending_link = Some(link),
            None => tracing::warn!("cannot open {uri}: not a Qobuz album, artist or track"),
        }
    }

    /// Open what `link` points to, as an album from the pane is.
    fn open_link(&mut self, link: Link) {
        self.open(match link {
            // The pages fill in the rest as they load.
            Link::Album(id) => Destination::Album(Box::new(Album {
                id,
                ..Album::default()
            })),
            Link::Artist(id) => Destination::Artist {
                id,
                name: String::new(),
            },
            Link::Track(id) => Destination::Track(id),
        });
    }

    /// Open `destination` over the tab shown, or over Albums when what's shown has no
    /// pages.
    fn open(&mut self, destination: Destination) {
        let shown = self
            .library
            .as_ref()
            .and_then(gtk::Stack::visible_child_name);
        match shown.as_deref() {
            Some("search") => self.search_view.emit(SearchViewMsg::OpenPage(destination)),
            Some("discover") => self.discover.emit(DiscoverMsg::OpenPage(destination)),
            Some("stats") => self.stats.emit(StatsViewMsg::OpenPage(destination)),
            // The settings have no pages over them: it opens in Albums.
            Some("settings") => {
                self.show("albums");
                self.albums.emit(AlbumsMsg::OpenPage(destination));
            }
            _ => self.albums.emit(AlbumsMsg::OpenPage(destination)),
        }
        // Over the library, the pane would hide the page.
        if self.collapsed {
            self.pane_open = false;
        }
    }

    /// Show the listening stats, rather than what was opened over them.
    fn show_stats(&self) {
        self.stats.emit(StatsViewMsg::Home);
        self.show("stats");
    }

    /// Back to the tab shown, closing what's open over it.
    fn home(&self) {
        self.reset(&self.shown);
    }

    /// Take the tab named `name` back to its own page, closing what's open over it.
    fn reset(&self, name: &str) {
        match name {
            "albums" => self.albums.emit(AlbumsMsg::Home),
            "discover" => self.discover.emit(DiscoverMsg::Home),
            "stats" => self.stats.emit(StatsViewMsg::Home),
            _ => {}
        }
    }

    /// Show the tab, or what's beside the tabs, named `name`.
    fn show(&self, name: &str) {
        if let Some(library) = &self.library {
            // Or the stack hands it to the first widget of the page shown: a search bar.
            navigation::drop_focus_within(library);
            library.set_visible_child_name(name);
        }
    }

    /// A tab, or the search results, was shown.
    fn library_shown(&mut self) {
        let shown = self
            .library
            .as_ref()
            .and_then(gtk::Stack::visible_child_name);
        // Read the plays again each time, which syncing may have changed.
        self.stats.emit(if shown.as_deref() == Some("stats") {
            StatsViewMsg::Show
        } else {
            StatsViewMsg::Hidden
        });
        // Ready to pair while the settings show.
        self.settings.emit(if shown.as_deref() == Some("settings") {
            SettingsMsg::Show
        } else {
            SettingsMsg::Hidden
        });
        // The tab left starts over from its own page when it shows again.
        let shown = shown.map(|name| name.to_string()).unwrap_or_default();
        let left = std::mem::replace(&mut self.shown, shown.clone());
        if left != shown {
            self.reset(&left);
        }
        if shown != "search" {
            self.last_tab = shown;
        }
        // Over the library, the pane hides the tab the user just picked.
        if self.collapsed {
            self.pane_open = false;
        }
    }

    /// Show what `query` finds, beside the tabs; or close the results if it's empty.
    fn search(&self, query: String) {
        let close = query.trim().is_empty();
        self.search_view.emit(SearchViewMsg::Search(query));
        if let Some(library) = self.library.as_ref().filter(|_| !close) {
            library.set_visible_child_name("search");
        }
    }

    /// Run `task` in the background, in place of the current one.
    fn run(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        self.stop_task();
        self.task = Some(relm4::spawn(task));
    }

    fn stop_task(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }

    fn signed_in(&mut self, account: Account) {
        tracing::info!("signed in as user {}", account.user.id);
        self.favorites.reset(Some(account.client.clone()), []);
        self.playback.set_client(Some(account.client.clone()));
        self.albums.emit(AlbumsMsg::Show(account.client.clone()));
        self.discover
            .emit(DiscoverMsg::Show(account.client.clone()));
        self.search_view
            .emit(SearchViewMsg::SetClient(Some(account.client.clone())));
        self.stats
            .emit(StatsViewMsg::SetClient(Some(account.client.clone())));
        self.take_up_listening(account.client.clone());
        self.account = Some(account);
        self.screen = Screen::SignedIn;
        if let Some(link) = self.pending_link.take() {
            self.open_link(link);
        }
    }

    /// Get the album that was playing when the app last quit ready to play again, paused
    /// where it was.
    fn take_up_listening(&self, client: qobuz::Client) {
        let Some(point) = pochette_core::resume::load() else {
            return;
        };
        let player = self.player.clone();
        relm4::spawn(async move {
            match client.album(&point.album_id).await {
                Ok(album) => player.cue(
                    client,
                    album,
                    point.track_id,
                    Duration::from_millis(point.position_ms),
                ),
                Err(err) => tracing::warn!(
                    "cannot load album {} to take up the listening: {err}",
                    point.album_id
                ),
            }
        });
    }

    /// Back to the sign-in screen, forgetting the session.
    fn sign_out(&mut self) {
        self.stop_task();
        self.account = None;
        self.player.stop();
        self.favorites.reset(None, []);
        self.playback.set_client(None);
        self.albums.emit(AlbumsMsg::Clear);
        self.discover.emit(DiscoverMsg::Clear);
        self.search_view.emit(SearchViewMsg::SetClient(None));
        self.stats.emit(StatsViewMsg::SetClient(None));
        self.screen = Screen::SignedOut;
        if let Some(store) = self.store.clone() {
            relm4::spawn(async move {
                if let Err(err) = store.clear().await {
                    tracing::error!("cannot forget the session: {err}");
                }
            });
        }
    }
}

/// Open `url` in the default browser, through the portal when sandboxed.
fn open_in_browser(url: &str, window: &gtk::ApplicationWindow) {
    gtk::UriLauncher::new(url).launch(Some(window), gio::Cancellable::NONE, |result| {
        if let Err(err) = result {
            tracing::warn!("cannot open the browser: {err}");
        }
    });
}

/// When the window closes during playback, stop first and let Qobuz hear about the
/// listening so far, then close: quitting right away would drop those reports.
fn finish_playback_before_closing(window: &gtk::ApplicationWindow, player: &Player) {
    let player = player.clone();
    window.connect_close_request(move |window| {
        if player.status().borrow().state == crate::player::State::Stopped {
            return gtk::glib::Propagation::Proceed;
        }
        let (window, player) = (window.clone(), player.clone());
        relm4::spawn_local(async move {
            player.shut_down().await;
            window.close();
        });
        gtk::glib::Propagation::Stop
    });
}

/// Space plays and pauses, wherever the focus is, except in text fields where it types
/// a space. It does nothing else: not clicking the focused button, not playing the
/// focused track.
/// A click anywhere outside the search bar being typed in takes the focus from it: the top
/// bar's, or the Albums tab's. Clicks on what takes no focus would leave it there.
fn leave_search_bars_on_click(window: &gtk::ApplicationWindow) {
    let click = gtk::GestureClick::new();
    // Any button, seen before the widget clicked, which still gets the click.
    click.set_button(0);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let window_ref = window.downgrade();
    click.connect_pressed(move |_, _, x, y| {
        let Some(window) = window_ref.upgrade() else {
            return;
        };
        let Some(search) = GtkWindowExt::focus(&window)
            .and_then(|focus| focus.ancestor(gtk::SearchEntry::static_type()))
        else {
            return;
        };
        let in_search = window
            .pick(x, y, gtk::PickFlags::DEFAULT)
            .is_some_and(|target| target == search || target.is_ancestor(&search));
        if !in_search {
            GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
        }
    });
    window.add_controller(click);
}

fn play_pause_with_space(window: &gtk::ApplicationWindow, player: &Player) {
    let keys = gtk::EventControllerKey::new();
    // Before the focused widget gets the key.
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    // Held down, the key repeats: only the first press counts.
    let held = std::rc::Rc::new(std::cell::Cell::new(false));

    let (player, window_ref, pressed) = (player.clone(), window.downgrade(), held.clone());
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let modifiers = modifiers
            & (gtk::gdk::ModifierType::CONTROL_MASK
                | gtk::gdk::ModifierType::ALT_MASK
                | gtk::gdk::ModifierType::SUPER_MASK);
        if key != gtk::gdk::Key::space || !modifiers.is_empty() {
            return gtk::glib::Propagation::Proceed;
        }
        let typing = window_ref
            .upgrade()
            .and_then(|window| GtkWindowExt::focus(&window))
            .is_some_and(|focus| focus.is::<gtk::Editable>() || focus.is::<gtk::TextView>());
        if typing {
            return gtk::glib::Propagation::Proceed;
        }
        if !pressed.replace(true) && player.status().borrow().state != State::Stopped {
            player.toggle_pause();
        }
        gtk::glib::Propagation::Stop
    });
    keys.connect_key_released(move |_, key, _, _| {
        if key == gtk::gdk::Key::space {
            held.set(false);
        }
    });
    window.add_controller(keys);
}
