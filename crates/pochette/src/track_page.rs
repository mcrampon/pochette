//! Where a link to a track leads: Pochette plays albums, whole. The page says so, gently,
//! and offers the album the track is on.

use qobuz::{Client, models::Album};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, prelude::*},
};

use crate::{
    i18n::tr,
    navigation::{Destination, PageOutput},
};

pub struct TrackPage {
    client: Option<Client>,
    id: u64,
    state: State,
}

enum State {
    Loading,
    /// The track's title and its album, if Qobuz says which.
    Loaded {
        track: String,
        album: Option<(String, String)>,
    },
    Failed(String),
}

#[derive(Debug)]
pub enum TrackPageMsg {
    Show { client: Client, id: u64 },
    Back,
    Retry,
    OpenAlbum,
}

#[derive(Debug)]
pub struct Loaded {
    id: u64,
    result: qobuz::Result<qobuz::models::Track>,
}

#[relm4::component(pub)]
impl Component for TrackPage {
    type Init = ();
    type Input = TrackPageMsg;
    type Output = PageOutput;
    type CommandOutput = Loaded;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            add_controller = gtk::GestureClick {
                // The mouse's back button.
                set_button: 8,
                connect_pressed[sender] => move |_, _, _, _| sender.input(TrackPageMsg::Back),
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
                            sender.input(TrackPageMsg::Back);
                            glib::Propagation::Stop
                        }
                    })),
                },
            },

            gtk::Button {
                set_icon_name: "go-previous-symbolic",
                set_tooltip_text: Some(&tr!("back")),
                set_halign: gtk::Align::Start,
                set_margin_top: 12,
                set_margin_start: 24,
                add_css_class: "flat",
                add_css_class: "circular",
                connect_clicked => TrackPageMsg::Back,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,
                set_margin_start: 36,
                set_margin_end: 36,
                set_margin_bottom: 48,

                gtk::Label {
                    set_label: &tr!("track-heading"),
                    set_wrap: true,
                    set_justify: gtk::Justification::Center,
                    add_css_class: "pochette-heading",
                },
                gtk::Label {
                    set_label: &tr!("track-albums-only"),
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_justify: gtk::Justification::Center,
                    add_css_class: "dim-label",
                },
                gtk::Label {
                    #[watch]
                    set_label: &model.detail(),
                    #[watch]
                    set_visible: !model.detail().is_empty(),
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_justify: gtk::Justification::Center,
                    set_margin_top: 12,
                },
                gtk::Spinner {
                    #[watch]
                    set_visible: matches!(model.state, State::Loading),
                    #[watch]
                    set_spinning: matches!(model.state, State::Loading),
                    set_size_request: (32, 32),
                    set_halign: gtk::Align::Center,
                    set_margin_top: 12,
                },
                gtk::Button {
                    set_label: &tr!("track-open-album"),
                    set_halign: gtk::Align::Center,
                    set_margin_top: 12,
                    add_css_class: "suggested-action",
                    #[watch]
                    set_visible: matches!(model.state, State::Loaded { album: Some(_), .. }),
                    connect_clicked => TrackPageMsg::OpenAlbum,
                },
                gtk::Button {
                    set_label: &tr!("try-again"),
                    set_halign: gtk::Align::Center,
                    #[watch]
                    set_visible: matches!(model.state, State::Failed(_)),
                    connect_clicked => TrackPageMsg::Retry,
                },
            },
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            client: None,
            id: 0,
            state: State::Loading,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: TrackPageMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            TrackPageMsg::Show { client, id } => {
                self.client = Some(client);
                self.id = id;
                self.load(&sender);
            }
            TrackPageMsg::Back => {
                let _ = sender.output(PageOutput::Back);
            }
            TrackPageMsg::Retry => self.load(&sender),
            TrackPageMsg::OpenAlbum => {
                if let State::Loaded {
                    album: Some((id, _)),
                    ..
                } = &self.state
                {
                    let album = Album {
                        id: id.clone(),
                        ..Album::default()
                    };
                    let _ = sender.output(PageOutput::Open(Destination::Album(Box::new(album))));
                }
            }
        }
    }

    fn update_cmd(&mut self, loaded: Loaded, sender: ComponentSender<Self>, _root: &Self::Root) {
        if loaded.id != self.id {
            return;
        }
        self.state = match loaded.result {
            Ok(track) => State::Loaded {
                track: track.full_title(),
                album: track.album.map(|album| {
                    let title = album.full_title();
                    (album.id, title)
                }),
            },
            Err(qobuz::Error::Unauthorized { .. }) => {
                let _ = sender.output(PageOutput::SessionExpired);
                return;
            }
            Err(err) => {
                tracing::warn!("cannot load track {}: {err}", loaded.id);
                State::Failed(err.to_string())
            }
        };
    }
}

impl TrackPage {
    fn load(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.state = State::Loading;
        let id = self.id;
        // A command, which stops with the page.
        sender.oneshot_command(async move {
            Loaded {
                id,
                result: client.track(id).await,
            }
        });
    }

    /// Which album the track is on, or why it isn't known.
    fn detail(&self) -> String {
        match &self.state {
            State::Loaded {
                track,
                album: Some((_, album)),
            } => tr!(
                "track-on-album",
                track = track.as_str(),
                album = album.as_str()
            ),
            State::Loading | State::Loaded { album: None, .. } => String::new(),
            State::Failed(error) => tr!("track-cannot-load", error = error.as_str()),
        }
    }
}
