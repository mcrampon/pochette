//! The main window.

use std::sync::Arc;

use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
    actions::{RelmAction, RelmActionGroup},
    gtk::{self, gio, prelude::*},
};
use tokio::task::JoinHandle;

use crate::{
    account::{self, Account},
    albums::{AlbumsMsg, AlbumsOutput, AlbumsPage},
    session_store::SessionStore,
};

relm4::new_action_group!(WindowActions, "win");
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
    http: reqwest::Client,
    store: Option<Arc<SessionStore>>,
    account: Option<Account>,
    error: Option<String>,
    sign_in_url: Option<String>,
    /// The start-up check or the sign-in in progress.
    task: Option<JoinHandle<()>>,
    albums: Controller<AlbumsPage>,
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
            set_title: Some("Albuz"),
            set_default_size: (1100, 760),

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &gtk::StackSwitcher {
                    set_stack: Some(&library),
                    #[watch]
                    set_visible: model.screen == Screen::SignedIn,
                },

                pack_end = &gtk::MenuButton {
                    set_icon_name: "open-menu-symbolic",
                    set_tooltip_text: Some("Main Menu"),
                    set_menu_model: Some(&main_menu),
                    #[watch]
                    set_visible: model.screen == Screen::SignedIn,
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
                        set_label: "Can't Reach Qobuz",
                        add_css_class: "albuz-heading",
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
                        set_label: "Try Again",
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
                        set_label: "Albuz",
                        add_css_class: "albuz-title",
                    },
                    gtk::Label {
                        set_label: "Your Qobuz library, album by album.",
                        add_css_class: "dim-label",
                    },
                    gtk::Button {
                        set_label: "Sign In with Qobuz",
                        set_halign: gtk::Align::Center,
                        add_css_class: "suggested-action",
                        add_css_class: "albuz-pill",
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
                        set_label: "Signing in opens Qobuz in your browser.\nRequires a Qobuz subscription.",
                        add_css_class: "dim-label",
                        add_css_class: "albuz-caption",
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
                        set_label: "Continue in Your Browser",
                        add_css_class: "albuz-heading",
                    },
                    gtk::Label {
                        set_label: "Sign in to Qobuz in the browser tab that just opened.\nAlbuz continues as soon as you're done.",
                        add_css_class: "dim-label",
                        set_justify: gtk::Justification::Center,
                    },
                    gtk::Box {
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,

                        gtk::Button {
                            set_label: "Open Browser Again",
                            #[watch]
                            set_sensitive: model.sign_in_url.is_some(),
                            connect_clicked => Msg::OpenBrowser,
                        },
                        gtk::Button {
                            set_label: "Copy Link",
                            #[watch]
                            set_sensitive: model.sign_in_url.is_some(),
                            connect_clicked => Msg::CopyLink,
                        },
                    },
                    gtk::Button {
                        set_label: "Cancel",
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
                        set_label: "This account can only play 30-second previews.",
                        add_css_class: "warning",
                        set_margin_all: 6,
                    },

                    #[name = "library"]
                    gtk::Stack {
                        set_vexpand: true,
                        set_transition_type: gtk::StackTransitionType::Crossfade,

                        add_titled[Some("discover"), "Discover"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,

                            gtk::Label {
                                set_label: "Discover",
                                add_css_class: "albuz-heading",
                            },
                            gtk::Label {
                                set_label: "New releases and picks from Qobuz will show up here.",
                                add_css_class: "dim-label",
                            },
                        },
                        add_titled: (model.albums.widget(), Some("albums"), "Albums"),
                        // Discover is empty for now.
                        set_visible_child_name: "albums",
                    },
                },
            }
        }
    }

    menu! {
        main_menu: {
            "Sign Out" => SignOutAction,
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            screen: Screen::Starting,
            http: account::http_client(),
            store: None,
            account: None,
            error: None,
            sign_in_url: None,
            task: None,
            albums: AlbumsPage::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    AlbumsOutput::SessionExpired => Msg::SessionExpired,
                }),
        };
        let widgets = view_output!();

        let mut actions = RelmActionGroup::<WindowActions>::new();
        let input = sender.input_sender().clone();
        actions.add_action(RelmAction::<SignOutAction>::new_stateless(move |_| {
            input.emit(Msg::SignOut);
        }));
        actions.register_for_widget(&root);

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
                self.error = Some("Your session expired. Sign in again.".to_owned());
            }
        }
    }
}

impl App {
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
        self.albums.emit(AlbumsMsg::Show(account.client.clone()));
        self.account = Some(account);
        self.screen = Screen::SignedIn;
    }

    /// Back to the sign-in screen, forgetting the session.
    fn sign_out(&mut self) {
        self.stop_task();
        self.account = None;
        self.albums.emit(AlbumsMsg::Clear);
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
