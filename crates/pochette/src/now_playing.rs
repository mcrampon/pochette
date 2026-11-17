//! The now playing pane: the controls first, then the album's tracks.

use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use qobuz::models::Track;
use relm4::{
    Component, ComponentParts, ComponentSender, RelmWidgetExt,
    gtk::{self, gdk, glib, pango, prelude::*},
};

use crate::{
    clamp::Clamp,
    covers,
    favorites::{Favorites, Heart, with_heart},
    format,
    player::{Player, State, Status},
    settings::Settings,
};

/// The side of the cover, in logical pixels.
const COVER_SIZE: i32 = 280;

/// How often the progress bar moves.
const TICK: Duration = Duration::from_millis(250);

/// How long after the user lets go of the progress bar before playback seeks there.
const SCRUB_DELAY: Duration = Duration::from_millis(200);

/// How long the progress bar trusts the user over the player after a seek, which takes
/// a moment to land.
const SEEK_SETTLE: Duration = Duration::from_millis(800);

/// How much one notch of the mouse wheel changes the volume.
const VOLUME_STEP: f64 = 0.05;

/// How long the volume must stay put before it is saved, so dragging the slider doesn't
/// write the settings at every step.
const VOLUME_SAVE_DELAY: Duration = Duration::from_millis(500);

pub struct NowPlaying {
    player: Player,
    status: Status,
    cover: gtk::Picture,
    /// Over the cover, to add the album to the favorites or remove it.
    heart: Heart,
    favorites: Favorites,
    cover_url: Option<String>,
    progress: gtk::Scale,
    elapsed: gtk::Label,
    remaining: gtk::Label,
    queue_list: gtk::ListBox,
    /// The queue the list shows, to rebuild it only when the queue changes.
    listed: Arc<[Track]>,
    /// Where the user moved the progress bar to, until playback seeks there.
    scrub: Option<f64>,
    /// Tells the latest scrub from the earlier ones.
    scrubs: u64,
    /// Tells the latest volume change from the earlier ones.
    volume_changes: u64,
    seeked_at: Option<Instant>,
}

#[derive(Debug)]
pub enum NowPlayingMsg {
    PlaybackChanged(Status),
    CoverLoaded {
        url: String,
        texture: Option<gdk::Texture>,
    },
    TogglePause,
    Previous,
    Next,
    PlayIndex(usize),
    /// The user moved the progress bar.
    Scrub(f64),
    /// Seek to where the user moved the progress bar, unless they moved it again since.
    Seek(u64),
    Tick,
    Volume(f64),
    /// Save the volume, unless it changed again since.
    SaveVolume(u64),
}

#[relm4::component(pub)]
impl Component for NowPlaying {
    type Init = (Player, Favorites);
    type Input = NowPlayingMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::ScrolledWindow {
            add_css_class: "pochette-now-playing",
            set_hscrollbar_policy: gtk::PolicyType::Never,

            Clamp {
                set_max_width: 480,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 24,

                    // The controls.
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        #[local_ref]
                        cover_box -> gtk::Overlay {
                            set_halign: gtk::Align::Center,
                            set_margin_bottom: 12,
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &model.status.track().map(Track::full_title).unwrap_or_default(),
                            add_css_class: "pochette-now-playing-title",
                            set_wrap: true,
                            set_justify: gtk::Justification::Center,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.artist(),
                            set_wrap: true,
                            set_justify: gtk::Justification::Center,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.status.album.as_ref().map(|album| album.full_title()).unwrap_or_default(),
                            add_css_class: "dim-label",
                            set_wrap: true,
                            set_justify: gtk::Justification::Center,
                        },
                        gtk::Label {
                            #[watch]
                            set_visible: model.status.quality.is_some(),
                            #[watch]
                            set_label: model.status.quality.as_deref().unwrap_or_default(),
                            add_css_class: "pochette-badge",
                            set_halign: gtk::Align::Center,
                            set_margin_top: 6,
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_top: 12,

                            #[local_ref]
                            progress -> gtk::Scale {
                                set_draw_value: false,
                                set_tooltip_text: Some("Position"),
                                connect_change_value[sender] => move |_, _, value| {
                                    sender.input(NowPlayingMsg::Scrub(value));
                                    glib::Propagation::Proceed
                                },
                            },
                            gtk::Box {
                                set_margin_start: 10,
                                set_margin_end: 10,

                                #[local_ref]
                                elapsed -> gtk::Label {
                                    set_hexpand: true,
                                    set_xalign: 0.0,
                                    add_css_class: "dim-label",
                                    add_css_class: "pochette-caption",
                                    add_css_class: "pochette-numeric",
                                },
                                #[local_ref]
                                remaining -> gtk::Label {
                                    set_xalign: 1.0,
                                    add_css_class: "dim-label",
                                    add_css_class: "pochette-caption",
                                    add_css_class: "pochette-numeric",
                                },
                            },
                        },

                        gtk::Box {
                            set_halign: gtk::Align::Center,
                            set_spacing: 18,

                            gtk::Button {
                                set_icon_name: "media-skip-backward-symbolic",
                                set_tooltip_text: Some("Previous"),
                                set_valign: gtk::Align::Center,
                                add_css_class: "circular",
                                add_css_class: "flat",
                                add_css_class: "pochette-skip",
                                connect_clicked => NowPlayingMsg::Previous,
                            },
                            gtk::Button {
                                #[watch]
                                set_icon_name: if model.is_playing() {
                                    "media-playback-pause-symbolic"
                                } else {
                                    "media-playback-start-symbolic"
                                },
                                #[watch]
                                set_tooltip_text: Some(if model.is_playing() { "Pause" } else { "Play" }),
                                add_css_class: "circular",
                                add_css_class: "suggested-action",
                                add_css_class: "pochette-play-pause",
                                connect_clicked => NowPlayingMsg::TogglePause,
                            },
                            gtk::Button {
                                set_icon_name: "media-skip-forward-symbolic",
                                set_tooltip_text: Some("Next"),
                                set_valign: gtk::Align::Center,
                                add_css_class: "circular",
                                add_css_class: "flat",
                                add_css_class: "pochette-skip",
                                #[watch]
                                set_sensitive: model.status.has_next(),
                                connect_clicked => NowPlayingMsg::Next,
                            },
                        },

                        gtk::Box {
                            set_spacing: 6,
                            set_margin_top: 6,
                            set_margin_start: 24,
                            set_margin_end: 24,

                            gtk::Image {
                                set_icon_name: Some("audio-volume-low-symbolic"),
                                add_css_class: "dim-label",
                            },
                            gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05) {
                                set_hexpand: true,
                                set_draw_value: false,
                                set_tooltip_text: Some("Volume"),
                                set_value: model.player.volume(),
                                connect_value_changed[sender] => move |scale| {
                                    sender.input(NowPlayingMsg::Volume(scale.value()));
                                },
                                // GTK's own step is far too big on so short a range.
                                add_controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL) {
                                    set_propagation_phase: gtk::PropagationPhase::Capture,
                                    connect_scroll => move |controller, _, dy| {
                                        if let Some(scale) = controller.widget().and_downcast::<gtk::Scale>() {
                                            scale.set_value((scale.value() - dy * VOLUME_STEP).clamp(0.0, 1.0));
                                        }
                                        glib::Propagation::Stop
                                    },
                                },
                            },
                            gtk::Image {
                                set_icon_name: Some("audio-volume-high-symbolic"),
                                add_css_class: "dim-label",
                            },
                        },
                    },

                    // The album's tracks.
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        gtk::Label {
                            set_label: "Tracks",
                            set_xalign: 0.0,
                            add_css_class: "pochette-kicker",
                        },
                        #[local_ref]
                        queue_list -> gtk::ListBox {
                            add_css_class: "pochette-tracks",
                            set_selection_mode: gtk::SelectionMode::None,
                            connect_row_activated[sender] => move |_, row| {
                                if let Ok(index) = usize::try_from(row.index()) {
                                    sender.input(NowPlayingMsg::PlayIndex(index));
                                }
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        (player, favorites): (Player, Favorites),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        follow_playback(&player, &sender);
        tick(&sender);
        let model = Self {
            status: player.status().borrow().clone(),
            player,
            cover: gtk::Picture::builder()
                .width_request(COVER_SIZE)
                .height_request(COVER_SIZE)
                .content_fit(gtk::ContentFit::Cover)
                .overflow(gtk::Overflow::Hidden)
                .css_classes(["pochette-cover", "pochette-cover-large"])
                .build(),
            heart: Heart::new(),
            favorites,
            cover_url: None,
            progress: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0),
            elapsed: gtk::Label::new(None),
            remaining: gtk::Label::new(None),
            queue_list: gtk::ListBox::new(),
            listed: Arc::default(),
            scrub: None,
            scrubs: 0,
            volume_changes: 0,
            seeked_at: None,
        };
        let cover_box = &with_heart(&model.cover, &model.heart);
        let progress = &model.progress;
        let elapsed = &model.elapsed;
        let remaining = &model.remaining;
        let queue_list = &model.queue_list;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: NowPlayingMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            NowPlayingMsg::PlaybackChanged(status) => {
                let track_changed = status.track_id != self.status.track_id;
                self.status = status;
                if track_changed {
                    self.scrub = None;
                    self.seeked_at = None;
                }
                self.show_cover(&sender);
                self.show_queue();
                self.show_progress();
            }
            NowPlayingMsg::CoverLoaded { url, texture } => {
                if self.cover_url.as_deref() == Some(&url) && texture.is_some() {
                    self.cover.set_paintable(texture.as_ref());
                }
            }
            NowPlayingMsg::TogglePause => self.player.toggle_pause(),
            NowPlayingMsg::Previous => self.player.previous(),
            NowPlayingMsg::Next => self.player.next(),
            NowPlayingMsg::PlayIndex(index) => self.player.play_index(index),
            NowPlayingMsg::Scrub(value) => {
                self.scrub = Some(value);
                self.scrubs += 1;
                self.show_progress();
                let (scrubs, sender) = (self.scrubs, sender.clone());
                glib::timeout_add_local_once(SCRUB_DELAY, move || {
                    sender.input(NowPlayingMsg::Seek(scrubs));
                });
            }
            NowPlayingMsg::Seek(scrubs) => {
                if scrubs == self.scrubs
                    && let Some(value) = self.scrub.take()
                {
                    self.player.seek(Duration::from_secs_f64(value.max(0.0)));
                    self.progress.set_value(value);
                    self.seeked_at = Some(Instant::now());
                }
            }
            NowPlayingMsg::Tick => {
                if self.status.state != State::Stopped {
                    self.show_progress();
                }
            }
            NowPlayingMsg::Volume(volume) => {
                self.player.set_volume(volume);
                self.volume_changes += 1;
                let (changes, sender) = (self.volume_changes, sender.clone());
                glib::timeout_add_local_once(VOLUME_SAVE_DELAY, move || {
                    sender.input(NowPlayingMsg::SaveVolume(changes));
                });
            }
            NowPlayingMsg::SaveVolume(changes) => {
                if changes == self.volume_changes {
                    let volume = self.player.volume();
                    Settings::update(|settings| settings.volume = volume);
                }
            }
        }
    }
}

impl NowPlaying {
    fn is_playing(&self) -> bool {
        matches!(self.status.state, State::Playing | State::Loading)
    }

    /// The track's performer, or the album's artist.
    fn artist(&self) -> String {
        self.status
            .track()
            .and_then(|track| track.performer.as_ref())
            .map(|performer| performer.name.clone())
            .or_else(|| {
                let album = self.status.album.as_ref()?;
                Some(album.artist.name.clone())
            })
            .unwrap_or_default()
    }

    fn show_cover(&mut self, sender: &ComponentSender<Self>) {
        let url = self.status.album.as_ref().and_then(|album| {
            let image = &album.image;
            image.large.clone().or_else(|| image.small.clone())
        });
        if url == self.cover_url {
            return;
        }
        self.cover_url.clone_from(&url);
        let album = self.status.album.as_deref().cloned().map(Rc::new);
        self.heart.show(album, &self.favorites);
        let Some(url) = url else {
            self.cover.set_paintable(None::<&gdk::Paintable>);
            return;
        };
        let cached = covers::cached(&url);
        self.cover.set_paintable(cached.as_ref());
        if cached.is_none() {
            let sender = sender.clone();
            relm4::spawn_local(async move {
                let texture = covers::load(url.clone()).await;
                sender.input(NowPlayingMsg::CoverLoaded { url, texture });
            });
        }
    }

    /// List the queue's tracks, the current one stressed.
    fn show_queue(&mut self) {
        if !Arc::ptr_eq(&self.listed, &self.status.queue) {
            self.listed = self.status.queue.clone();
            self.queue_list.remove_all();
            for track in self.listed.iter() {
                self.queue_list.append(&queue_row(track));
            }
        }
        let mut index = 0;
        while let Some(row) = self.queue_list.row_at_index(index) {
            let current = usize::try_from(index).ok() == self.status.index;
            if current {
                row.add_css_class("pochette-playing");
            } else {
                row.remove_css_class("pochette-playing");
            }
            index += 1;
        }
    }

    fn show_progress(&self) {
        let duration = self.status.track().map_or(0, |track| track.duration);
        self.progress.set_range(0.0, f64::from(duration.max(1)));

        let settling = self
            .seeked_at
            .is_some_and(|seeked_at| seeked_at.elapsed() < SEEK_SETTLE);
        let position = match self.scrub {
            Some(value) => value,
            None if settling => self.progress.value(),
            None => {
                let position = self.player.position().unwrap_or_default().as_secs_f64();
                self.progress.set_value(position);
                position
            }
        };
        let elapsed = seconds(position).min(duration);
        self.elapsed.set_label(&format::track_duration(elapsed));
        self.remaining.set_label(&format!(
            "\u{2212}{}",
            format::track_duration(duration - elapsed)
        ));
    }
}

fn queue_row(track: &Track) -> gtk::ListBoxRow {
    let number = gtk::Label::builder()
        .label(track.track_number.to_string())
        .width_chars(2)
        .xalign(1.0)
        .css_classes(["dim-label", "pochette-numeric"])
        .build();
    let title = gtk::Label::builder()
        .label(track.full_title())
        .hexpand(true)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .tooltip_text(track.full_title())
        .build();
    let duration = gtk::Label::builder()
        .label(format::track_duration(track.duration))
        .css_classes(["dim-label", "pochette-numeric"])
        .build();
    let content = gtk::Box::builder()
        .spacing(12)
        .css_classes(["pochette-track"])
        .build();
    content.append(&number);
    content.append(&title);
    content.append(&duration);
    let row = gtk::ListBoxRow::builder().child(&content).build();
    row.set_cursor_from_name(Some("pointer"));
    row
}

/// Whole seconds in `value`.
fn seconds(value: f64) -> u32 {
    u32::try_from(Duration::from_secs_f64(value.max(0.0)).as_secs()).unwrap_or(u32::MAX)
}

fn follow_playback(player: &Player, sender: &ComponentSender<NowPlaying>) {
    let mut status = player.status();
    let sender = sender.clone();
    relm4::spawn(async move {
        while status.changed().await.is_ok() {
            let current = status.borrow_and_update().clone();
            sender.input(NowPlayingMsg::PlaybackChanged(current));
        }
    });
}

/// Move the progress bar along, for as long as the pane exists.
fn tick(sender: &ComponentSender<NowPlaying>) {
    let input = sender.input_sender().clone();
    glib::timeout_add_local(TICK, move || {
        if input.send(NowPlayingMsg::Tick).is_ok() {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
}
