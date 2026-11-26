//! The Devices section of the settings: the devices this one syncs its plays with, pairing
//! others, and whether to keep syncing while Pochette is closed.
//!
//! While the settings show, this device is ready to pair, and the service tells it what's
//! going on every second. A pairing going on takes the section's place until it's over.

use std::time::Duration;

use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, prelude::*},
};

use crate::{
    format, history,
    sync::{
        self, PeerStatus, Stage, Status, autostart,
        client::{Client, Error},
    },
};

/// How often to ask the service what's going on.
const POLL_EVERY: Duration = Duration::from_secs(1);

pub struct Devices {
    client: Option<Client>,
    status: Status,
    /// Why the service can't be reached, if it can't.
    error: Option<String>,
    /// How the last pairing went.
    notice: Option<String>,
    background: bool,
    open: bool,
    /// Counts the times the settings showed, so the polls of a previous time stop.
    session: u32,
    peers: gtk::Box,
    nearby: gtk::Box,
    address: gtk::Entry,
}

#[derive(Debug)]
pub enum DevicesMsg {
    Open,
    Closed,
    Pair(String),
    PairAddress,
    Answer(bool),
    Forget(String),
    SetBackground(bool),
}

#[derive(Debug)]
pub enum DevicesCommand {
    Connected {
        session: u32,
        result: Result<Client, String>,
    },
    Status {
        session: u32,
        result: Result<Status, String>,
    },
    /// A request went through, or why not.
    Done(Result<(), String>),
}

#[relm4::component(pub)]
impl Component for Devices {
    type Init = ();
    type Input = DevicesMsg;
    type Output = ();
    type CommandOutput = DevicesCommand;

    view! {
        gtk::Stack {
            set_transition_type: gtk::StackTransitionType::Crossfade,
            // As big as the page shown: the pages' wrapping text doesn't size well together.
            set_hhomogeneous: false,
            set_vhomogeneous: false,

            add_named[Some("devices")] = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 18,

                gtk::Label {
                    set_label: "Paired devices share what you play with each other, for the \
                                stats and play counts, whenever they're on the same network.",
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_xalign: 0.0,
                    add_css_class: "dim-label",
                },

                gtk::Label {
                    #[watch]
                    set_label: model.problem().unwrap_or_default(),
                    #[watch]
                    set_visible: model.problem().is_some(),
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_xalign: 0.0,
                    add_css_class: "error",
                },

                gtk::Label {
                    #[watch]
                    set_label: model.notice.as_deref().unwrap_or_default(),
                    #[watch]
                    set_visible: model.notice.is_some(),
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_xalign: 0.0,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    #[watch]
                    set_visible: !model.status.peers.is_empty(),

                    gtk::Label {
                        set_label: "Paired Devices",
                        set_xalign: 0.0,
                        add_css_class: "heading",
                    },
                    #[local_ref]
                    peers -> gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,
                    },
                    gtk::CheckButton {
                        set_label: Some("Keep syncing while Pochette is closed"),
                        set_margin_top: 6,
                        #[watch]
                        #[block_signal(background_toggled)]
                        set_active: model.background,
                        connect_toggled[sender] => move |check| {
                            sender.input(DevicesMsg::SetBackground(check.is_active()));
                        } @background_toggled,
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,

                    gtk::Label {
                        set_label: "Pair a Device",
                        set_xalign: 0.0,
                        add_css_class: "heading",
                    },
                    gtk::Label {
                        set_label: "Open Settings in Pochette on the other device as well, \
                                    and it shows up here.",
                        set_wrap: true,
                        set_max_width_chars: 50,
                        set_xalign: 0.0,
                        add_css_class: "dim-label",
                    },
                    #[local_ref]
                    nearby -> gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,
                    },
                    gtk::Box {
                        set_spacing: 12,
                        add_css_class: "pochette-card",
                        #[watch]
                        set_visible: model.status.nearby.is_empty(),

                        gtk::Spinner {
                            #[watch]
                            set_spinning: model.open && model.status.nearby.is_empty(),
                        },
                        gtk::Label {
                            set_label: "Looking for devices on this network…",
                            set_xalign: 0.0,
                            add_css_class: "dim-label",
                        },
                    },
                    gtk::Expander {
                        set_label: Some("Pair by Address"),
                        set_margin_top: 6,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            set_margin_top: 8,

                            gtk::Box {
                                set_spacing: 6,

                                #[local_ref]
                                address -> gtk::Entry {
                                    set_hexpand: true,
                                    set_placeholder_text: Some("Its address, such as 192.168.1.20"),
                                    connect_activate => DevicesMsg::PairAddress,
                                },
                                gtk::Button {
                                    set_label: "Pair",
                                    connect_clicked => DevicesMsg::PairAddress,
                                },
                            },
                            gtk::Label {
                                set_label: &format!(
                                    "If the devices can't find each other, the network or a \
                                     firewall may be in the way. Pochette needs TCP port {} to \
                                     sync, and UDP port 5353 to find devices.",
                                    sync::PORT,
                                ),
                                set_wrap: true,
                                set_max_width_chars: 50,
                                set_xalign: 0.0,
                                add_css_class: "dim-label",
                                add_css_class: "pochette-caption",
                            },
                        },
                    },
                },
            },

            // A pairing going on: connecting, the code to check on both devices, then waiting
            // for the other's answer.
            add_named[Some("pairing")] = &gtk::Box {
                add_css_class: "pochette-card",
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 18,
                set_valign: gtk::Align::Center,

                gtk::Label {
                    #[watch]
                    set_label: &format!("Pair With {}", model.pairing_name()),
                    add_css_class: "pochette-heading",
                    set_wrap: true,
                    set_justify: gtk::Justification::Center,
                },
                gtk::Label {
                    #[watch]
                    set_label: &model.pairing_hint(),
                    #[watch]
                    set_visible: !model.pairing_hint().is_empty(),
                    set_wrap: true,
                    set_max_width_chars: 40,
                    set_justify: gtk::Justification::Center,
                    add_css_class: "dim-label",
                },
                gtk::Label {
                    #[watch]
                    set_label: model.pairing_code(),
                    #[watch]
                    set_visible: model.pairing_stage() == Some(&Stage::Confirm),
                    add_css_class: "pochette-title",
                    add_css_class: "pochette-numeric",
                },
                gtk::Spinner {
                    #[watch]
                    set_visible: model.pairing_stage() != Some(&Stage::Confirm),
                    #[watch]
                    set_spinning: model.pairing_stage().is_some_and(|stage| stage != &Stage::Confirm),
                    set_size_request: (32, 32),
                    set_halign: gtk::Align::Center,
                },
                gtk::Box {
                    set_spacing: 12,
                    set_halign: gtk::Align::Center,
                    #[watch]
                    set_visible: model.pairing_stage() == Some(&Stage::Confirm),

                    gtk::Button {
                        set_label: "Cancel",
                        connect_clicked => DevicesMsg::Answer(false),
                    },
                    gtk::Button {
                        set_label: "Pair",
                        add_css_class: "suggested-action",
                        connect_clicked => DevicesMsg::Answer(true),
                    },
                },
            },

            // Once the pages are there.
            #[watch]
            set_visible_child_name: if model.pairing_stage().is_some() { "pairing" } else { "devices" },
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            client: None,
            status: Status::default(),
            error: None,
            notice: None,
            background: autostart::is_enabled(),
            open: false,
            session: 0,
            peers: gtk::Box::default(),
            nearby: gtk::Box::default(),
            address: gtk::Entry::default(),
        };
        let peers = &model.peers;
        let nearby = &model.nearby;
        let address = &model.address;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: DevicesMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            DevicesMsg::Open => {
                if self.open {
                    return;
                }
                self.open = true;
                self.session += 1;
                self.notice = None;
                self.background = autostart::is_enabled();
                let session = self.session;
                sender.oneshot_command(async move {
                    let result = async {
                        let client = Client::connect().await?;
                        client.start().await?;
                        client.set_pairable(true).await?;
                        // To show where the devices are at.
                        client.sync_now().await?;
                        Ok::<_, Error>(client)
                    }
                    .await
                    .map_err(|err| err.to_string());
                    DevicesCommand::Connected { session, result }
                });
            }
            DevicesMsg::Closed => {
                if !self.open {
                    return;
                }
                self.open = false;
                // A pairing left unanswered is declined.
                if self.pairing_stage() == Some(&Stage::Confirm) {
                    self.request(&sender, |client| async move {
                        client.answer_pairing(false).await
                    });
                }
                self.request(
                    &sender,
                    |client| async move { client.set_pairable(false).await },
                );
            }
            DevicesMsg::Pair(id) => {
                self.notice = None;
                self.request(&sender, |client| async move { client.pair(&id).await });
            }
            DevicesMsg::PairAddress => {
                let address = self.address.text().trim().to_owned();
                if address.is_empty() {
                    return;
                }
                self.notice = None;
                self.request(&sender, |client| async move { client.pair(&address).await });
            }
            DevicesMsg::Answer(accept) => {
                self.request(&sender, move |client| async move {
                    client.answer_pairing(accept).await
                });
            }
            DevicesMsg::Forget(id) => {
                self.request(&sender, |client| async move { client.forget(&id).await });
            }
            DevicesMsg::SetBackground(background) => self.set_background(background),
        }
    }

    fn update_cmd(
        &mut self,
        command: DevicesCommand,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match command {
            DevicesCommand::Connected { session, result } => {
                if session != self.session {
                    return;
                }
                match result {
                    Ok(client) => {
                        self.error = None;
                        self.client = Some(client);
                        self.poll(&sender, Duration::ZERO);
                    }
                    Err(err) => {
                        tracing::warn!("{err}");
                        self.error = Some(err);
                    }
                }
            }
            DevicesCommand::Status { session, result } => {
                if session != self.session || !self.open {
                    return;
                }
                match result {
                    Ok(status) => {
                        self.error = None;
                        self.show(status, &sender);
                    }
                    Err(err) => self.error = Some(err),
                }
                self.poll(&sender, POLL_EVERY);
            }
            DevicesCommand::Done(result) => {
                if let Err(err) = result {
                    tracing::warn!("{err}");
                    self.error = Some(err);
                }
            }
        }
    }
}

impl Devices {
    fn poll(&self, sender: &ComponentSender<Self>, after: Duration) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let session = self.session;
        sender.oneshot_command(async move {
            tokio::time::sleep(after).await;
            // The service may have stopped, as when the session ended: start it again.
            let result = async {
                if let Ok(status) = client.status().await {
                    Ok(status)
                } else {
                    client.start().await?;
                    client.set_pairable(true).await?;
                    client.status().await
                }
            }
            .await
            .map_err(|err| err.to_string());
            DevicesCommand::Status { session, result }
        });
    }

    /// Ask the service something, through `call`.
    fn request<F>(
        &self,
        sender: &ComponentSender<Self>,
        call: impl FnOnce(Client) -> F + Send + 'static,
    ) where
        F: Future<Output = Result<(), Error>> + Send,
    {
        let Some(client) = self.client.clone() else {
            return;
        };
        sender.oneshot_command(async move {
            DevicesCommand::Done(call(client).await.map_err(|err| err.to_string()))
        });
    }

    fn show(&mut self, status: Status, sender: &ComponentSender<Self>) {
        if let Some(pairing) = &status.pairing {
            let name = &pairing.name;
            let notice = match &pairing.stage {
                Stage::Paired => Some(format!("Paired with {name}.")),
                Stage::Declined { here: true } => Some(format!("Not paired with {name}.")),
                Stage::Declined { here: false } => Some(format!("{name} declined to pair.")),
                Stage::Failed { reason } => Some(format!("Can't pair with {name}: {reason}")),
                _ => None,
            };
            if let Some(notice) = notice {
                // Syncing is what pairing is for: in the background too, unless turned off.
                if pairing.stage == Stage::Paired && self.status.peers.is_empty() {
                    self.set_background(true);
                }
                self.notice = Some(notice);
                self.request(
                    sender,
                    |client| async move { client.dismiss_pairing().await },
                );
            }
        }
        if status.peers != self.status.peers {
            self.show_peers(&status.peers, sender);
        }
        if status.nearby != self.status.nearby {
            self.show_nearby(&status, sender);
        }
        self.status = status;
    }

    fn show_peers(&self, peers: &[PeerStatus], sender: &ComponentSender<Self>) {
        while let Some(child) = self.peers.first_child() {
            self.peers.remove(&child);
        }
        let now = history::now();
        for peer in peers {
            let detail = if peer.syncing {
                "Syncing…".to_owned()
            } else {
                match (peer.last_sync, &peer.error) {
                    (_, Some(error)) if peer.last_sync.is_none_or(|at| now - at > 15 * 60) => {
                        peer.last_sync.map_or_else(
                            || error.clone(),
                            |at| format!("Synced {}. {error}", format::since(at, now)),
                        )
                    }
                    (Some(at), _) => format!("Synced {}", format::since(at, now)),
                    (None, _) => "Not synced yet".to_owned(),
                }
            };
            let id = peer.id.clone();
            let sender = sender.clone();
            let forget = gtk::Button::builder()
                .label("Forget")
                .valign(gtk::Align::Center)
                .tooltip_text("Stop syncing with this device")
                .build();
            forget.connect_clicked(move |_| sender.input(DevicesMsg::Forget(id.clone())));
            self.peers.append(&row(&peer.name, Some(&detail), &forget));
        }
    }

    fn show_nearby(&self, status: &Status, sender: &ComponentSender<Self>) {
        while let Some(child) = self.nearby.first_child() {
            self.nearby.remove(&child);
        }
        for device in &status.nearby {
            let id = device.id.clone();
            let sender = sender.clone();
            let pair = gtk::Button::builder()
                .label("Pair")
                .valign(gtk::Align::Center)
                .build();
            pair.connect_clicked(move |_| sender.input(DevicesMsg::Pair(id.clone())));
            self.nearby.append(&row(&device.name, None, &pair));
        }
    }

    fn set_background(&mut self, background: bool) {
        match autostart::set_enabled(background) {
            Ok(()) => self.background = background,
            Err(err) => {
                tracing::warn!("cannot change whether syncing starts with the session: {err}");
                self.error = Some(format!("Can't change it: {err}"));
            }
        }
    }

    fn problem(&self) -> Option<&str> {
        self.error.as_deref().or(self.status.problem.as_deref())
    }

    fn pairing_stage(&self) -> Option<&Stage> {
        self.status
            .pairing
            .as_ref()
            .map(|pairing| &pairing.stage)
            .filter(|stage| stage.is_ongoing())
    }

    fn pairing_name(&self) -> &str {
        self.status
            .pairing
            .as_ref()
            .map_or("", |pairing| pairing.name.as_str())
    }

    fn pairing_code(&self) -> &str {
        self.status
            .pairing
            .as_ref()
            .map_or("", |pairing| pairing.code.as_str())
    }

    fn pairing_hint(&self) -> String {
        let name = self.pairing_name();
        match self.pairing_stage() {
            Some(Stage::Connecting) => "Connecting…".to_owned(),
            Some(Stage::Confirm) => {
                format!("Check that {name} shows the same code, then pair on both.")
            }
            Some(Stage::Waiting) => format!("Waiting for {name} to confirm…"),
            _ => String::new(),
        }
    }
}

/// A device in a list: its name, what's going on with it, and a button.
fn row(name: &str, detail: Option<&str>, button: &gtk::Button) -> gtk::Box {
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(
        &gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["pochette-device-name"])
            .build(),
    );
    if let Some(detail) = detail {
        text.append(
            &gtk::Label::builder()
                .label(detail)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label", "pochette-caption"])
                .build(),
        );
    }
    let row = gtk::Box::builder()
        .spacing(12)
        .css_classes(["pochette-card", "pochette-device"])
        .build();
    row.append(&text);
    row.append(button);
    row
}
