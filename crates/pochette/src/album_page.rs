//! An album's page: its cover and details, its tracks, then its review.

use std::{cell::RefCell, rc::Rc};

use qobuz::{
    Client,
    models::{Album, Track},
};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, gdk, pango, prelude::*},
};
use tokio::task::JoinHandle;

use crate::{
    clamp::Clamp,
    covers,
    favorites::{Favorites, Heart, with_heart},
    format,
    player::{Player, State, Status},
};

/// The widest the page's content gets, so it stays readable in a wide window.
const MAX_WIDTH: i32 = 960;

/// The side of the cover, in logical pixels.
const COVER_SIZE: i32 = 280;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tracks {
    Loading,
    Failed,
    Loaded,
}

pub struct AlbumPage {
    client: Option<Client>,
    /// What the grid knew of the album at first, then all of it once loaded.
    album: Option<Album>,
    tracks: Tracks,
    error: String,
    task: Option<JoinHandle<()>>,
    cover: gtk::Picture,
    /// Over the cover, to add the album to the favorites or remove it.
    heart: Heart,
    favorites: Favorites,
    track_list: gtk::Box,
    /// The track rows, with the id of their track.
    rows: Vec<(u64, gtk::ListBoxRow)>,
    player: Player,
    playback: Status,
}

#[derive(Debug)]
pub enum AlbumPageMsg {
    /// Show `album`, as much as is known of it, then load the rest.
    Show {
        client: Client,
        album: Album,
    },
    Retry,
    Back,
    Loaded {
        id: String,
        result: qobuz::Result<Album>,
    },
    CoverLoaded {
        url: String,
        texture: Option<gdk::Texture>,
    },
    /// Play the album, or pause or resume it if it's the one playing.
    PlayPressed,
    /// Play the album from this track.
    PlayTrack(u64),
    PlaybackChanged(Status),
}

#[derive(Debug)]
pub enum AlbumPageOutput {
    Back,
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for AlbumPage {
    type Init = (Player, Favorites);
    type Input = AlbumPageMsg;
    type Output = AlbumPageOutput;
    type CommandOutput = ();

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,

            add_controller = gtk::GestureClick {
                // The mouse's back button.
                set_button: 8,
                connect_pressed[sender] => move |_, _, _, _| sender.input(AlbumPageMsg::Back),
            },
            add_controller = gtk::ShortcutController {
                set_scope: gtk::ShortcutScope::Managed,
                add_shortcut = gtk::Shortcut {
                    set_trigger: gtk::ShortcutTrigger::parse_string("<Alt>Left"),
                    set_action: Some(gtk::CallbackAction::new({
                        let sender = sender.clone();
                        move |page, _| {
                            // Each tab has an album page, and the shortcut reaches both.
                            if !page.is_mapped() {
                                return gtk::glib::Propagation::Proceed;
                            }
                            sender.input(AlbumPageMsg::Back);
                            gtk::glib::Propagation::Stop
                        }
                    })),
                },
            },

            Clamp {
                set_max_width: MAX_WIDTH,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 24,
                    set_margin_top: 12,
                    set_margin_bottom: 48,
                    set_margin_start: 24,
                    set_margin_end: 24,

                    gtk::Button {
                        set_icon_name: "go-previous-symbolic",
                        set_tooltip_text: Some("Back to Albums"),
                        set_halign: gtk::Align::Start,
                        add_css_class: "flat",
                        add_css_class: "circular",
                        connect_clicked => AlbumPageMsg::Back,
                    },

                    // Cover and details.
                    gtk::Box {
                        set_spacing: 32,

                        #[local_ref]
                        cover_box -> gtk::Overlay {
                            set_valign: gtk::Align::Start,
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_valign: gtk::Align::End,
                            set_hexpand: true,

                            gtk::Button {
                                set_halign: gtk::Align::Start,
                                set_margin_bottom: 12,
                                add_css_class: "suggested-action",
                                add_css_class: "pochette-play",
                                #[watch]
                                set_sensitive: model.can_play(),
                                connect_clicked => AlbumPageMsg::PlayPressed,

                                gtk::Box {
                                    set_spacing: 8,

                                    gtk::Image {
                                        #[watch]
                                        set_icon_name: Some(if model.is_playing() {
                                            "media-playback-pause-symbolic"
                                        } else {
                                            "media-playback-start-symbolic"
                                        }),
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: if model.is_playing() { "Pause" } else { "Play" },
                                    },
                                },
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.playback_error(),
                                #[watch]
                                set_visible: !model.playback_error().is_empty(),
                                set_xalign: 0.0,
                                set_wrap: true,
                                add_css_class: "error",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.kicker(),
                                set_xalign: 0.0,
                                add_css_class: "pochette-kicker",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.text(Album::full_title),
                                set_xalign: 0.0,
                                set_wrap: true,
                                set_wrap_mode: pango::WrapMode::WordChar,
                                set_selectable: true,
                                add_css_class: "pochette-album-page-title",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.text(|album| album.artist.name.clone()),
                                set_xalign: 0.0,
                                set_wrap: true,
                                add_css_class: "pochette-album-page-artist",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.details(),
                                #[watch]
                                set_visible: !model.details().is_empty(),
                                set_xalign: 0.0,
                                set_wrap: true,
                                set_margin_top: 6,
                                add_css_class: "dim-label",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.length(),
                                set_xalign: 0.0,
                                add_css_class: "dim-label",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.quality(),
                                #[watch]
                                set_visible: !model.quality().is_empty(),
                                set_halign: gtk::Align::Start,
                                set_margin_top: 6,
                                add_css_class: "pochette-badge",
                            },
                        },
                    },

                    // Tracks.
                    gtk::Box {
                        #[transition = "Crossfade"]
                        match model.tracks {
                            Tracks::Loading => gtk::Box {
                                set_hexpand: true,
                                set_halign: gtk::Align::Center,
                                set_margin_top: 24,

                                gtk::Spinner {
                                    set_spinning: true,
                                    set_size_request: (32, 32),
                                },
                            },
                            Tracks::Failed => gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                set_hexpand: true,
                                set_halign: gtk::Align::Center,
                                set_margin_top: 24,

                                gtk::Label {
                                    #[watch]
                                    set_label: &format!("Can't load the tracks: {}", model.error),
                                    add_css_class: "dim-label",
                                    set_wrap: true,
                                    set_justify: gtk::Justification::Center,
                                },
                                gtk::Button {
                                    set_label: "Try Again",
                                    set_halign: gtk::Align::Center,
                                    connect_clicked => AlbumPageMsg::Retry,
                                },
                            },
                            Tracks::Loaded => gtk::Box {
                                #[local_ref]
                                track_list -> gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    set_hexpand: true,
                                },
                            },
                        }
                    },

                    // Review.
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        #[watch]
                        set_visible: !model.review().is_empty(),

                        gtk::Label {
                            set_label: "About This Album",
                            set_xalign: 0.0,
                            add_css_class: "pochette-heading",
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.review(),
                            set_xalign: 0.0,
                            set_wrap: true,
                            set_selectable: true,
                            add_css_class: "pochette-review",
                        },
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &model.copyright(),
                        #[watch]
                        set_visible: !model.copyright().is_empty(),
                        set_xalign: 0.0,
                        set_wrap: true,
                        add_css_class: "dim-label",
                        add_css_class: "pochette-caption",
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
        let model = Self {
            client: None,
            album: None,
            tracks: Tracks::Loading,
            error: String::new(),
            task: None,
            cover: gtk::Picture::builder()
                .width_request(COVER_SIZE)
                .height_request(COVER_SIZE)
                .content_fit(gtk::ContentFit::Cover)
                .overflow(gtk::Overflow::Hidden)
                .css_classes(["pochette-cover", "pochette-cover-large"])
                .build(),
            heart: Heart::new(),
            favorites,
            track_list: gtk::Box::default(),
            rows: Vec::new(),
            playback: player.status().borrow().clone(),
            player,
        };
        let cover_box = &with_heart(&model.cover, &model.heart);
        let track_list = &model.track_list;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AlbumPageMsg, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            AlbumPageMsg::Show { client, album } => {
                root.vadjustment().set_value(0.0);
                self.show_cover(&album, &sender);
                self.heart
                    .show(Some(Rc::new(album.clone())), &self.favorites);
                self.client = Some(client);
                self.album = Some(album);
                self.load(&sender);
            }
            AlbumPageMsg::Retry => self.load(&sender),
            AlbumPageMsg::Back => {
                let _ = sender.output(AlbumPageOutput::Back);
            }
            AlbumPageMsg::Loaded { id, result } => {
                // Another album was opened meanwhile.
                if self.album.as_ref().is_none_or(|album| album.id != id) {
                    return;
                }
                self.task = None;
                match result {
                    Ok(album) => {
                        self.rows = fill_track_list(&self.track_list, &album, &sender);
                        self.highlight_playing_track();
                        self.album = Some(album);
                        self.tracks = Tracks::Loaded;
                    }
                    Err(qobuz::Error::Unauthorized { .. }) => {
                        let _ = sender.output(AlbumPageOutput::SessionExpired);
                    }
                    Err(err) => {
                        tracing::warn!("cannot load album {id}: {err}");
                        self.error = err.to_string();
                        self.tracks = Tracks::Failed;
                    }
                }
            }
            AlbumPageMsg::CoverLoaded { url, texture } => {
                let still_shown = self
                    .album
                    .as_ref()
                    .and_then(|album| album.image.large.as_ref())
                    .is_some_and(|large| *large == url);
                if still_shown && texture.is_some() {
                    self.cover.set_paintable(texture.as_ref());
                }
            }
            AlbumPageMsg::PlayPressed => {
                let is_current = self
                    .album
                    .as_ref()
                    .is_some_and(|album| self.playback.is_current(&album.id));
                if is_current {
                    self.player.toggle_pause();
                } else {
                    self.play(None);
                }
            }
            AlbumPageMsg::PlayTrack(track_id) => self.play(Some(track_id)),
            AlbumPageMsg::PlaybackChanged(status) => {
                self.playback = status;
                self.highlight_playing_track();
            }
        }
    }
}

impl AlbumPage {
    fn play(&self, track_id: Option<u64>) {
        if let (Some(client), Some(album)) = (&self.client, &self.album) {
            self.player
                .play_album(client.clone(), album.clone(), track_id);
        }
    }

    /// Whether the album has tracks to play.
    fn can_play(&self) -> bool {
        self.tracks == Tracks::Loaded
            && self
                .album
                .as_ref()
                .and_then(|album| album.tracks.as_ref())
                .is_some_and(|tracks| tracks.items.iter().any(|track| track.streamable))
    }

    /// Whether this album is playing, or about to.
    fn is_playing(&self) -> bool {
        self.album.as_ref().is_some_and(|album| {
            self.playback.is_current(&album.id)
                && matches!(self.playback.state, State::Playing | State::Loading)
        })
    }

    fn playback_error(&self) -> String {
        let ours =
            self.album.as_ref().map(|album| album.id.as_str()) == self.playback.album_id.as_deref();
        match &self.playback.error {
            Some(error) if ours => error.clone(),
            _ => String::new(),
        }
    }

    fn highlight_playing_track(&self) {
        let playing = self
            .album
            .as_ref()
            .filter(|album| self.playback.is_current(&album.id))
            .and(self.playback.track_id);
        for (track_id, row) in &self.rows {
            if Some(*track_id) == playing {
                row.add_css_class("pochette-playing");
            } else {
                row.remove_css_class("pochette-playing");
            }
        }
    }

    fn load(&mut self, sender: &ComponentSender<Self>) {
        let (Some(client), Some(album)) = (self.client.clone(), self.album.as_ref()) else {
            return;
        };
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.tracks = Tracks::Loading;
        let id = album.id.clone();
        let sender = sender.clone();
        self.task = Some(relm4::spawn(async move {
            let result = client.album(&id).await;
            sender.input(AlbumPageMsg::Loaded { id, result });
        }));
    }

    /// Show the grid's cover right away if it's in memory, then the large one.
    fn show_cover(&self, album: &Album, sender: &ComponentSender<Self>) {
        let image = &album.image;
        let instant = [&image.large, &image.small]
            .into_iter()
            .flatten()
            .find_map(|url| covers::cached(url));
        self.cover.set_paintable(instant.as_ref());

        if let Some(url) = image.large.clone() {
            let sender = sender.clone();
            relm4::spawn_local(async move {
                let texture = covers::load(url.clone()).await;
                sender.input(AlbumPageMsg::CoverLoaded { url, texture });
            });
        }
    }

    fn text(&self, f: impl Fn(&Album) -> String) -> String {
        self.album.as_ref().map(f).unwrap_or_default()
    }

    /// "ALBUM · 2026"
    fn kicker(&self) -> String {
        self.text(|album| {
            let kind = album
                .release_type
                .as_deref()
                .map_or("Album", release_type_name);
            match album.year() {
                Some(year) => format!("{kind} · {year}").to_uppercase(),
                None => kind.to_uppercase(),
            }
        })
    }

    /// "Alternatif et Indé · Rough Trade"
    fn details(&self) -> String {
        self.text(|album| {
            let genre = album.genre.as_ref().map(|genre| genre.name.as_str());
            let label = album.label.as_ref().map(|label| label.name.as_str());
            [genre, label]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ")
        })
    }

    /// "9 tracks · 59 min"
    fn length(&self) -> String {
        self.text(|album| {
            let tracks = format::track_count(album.tracks_count);
            if album.duration > 0 {
                format!("{tracks} · {}", format::album_duration(album.duration))
            } else {
                tracks
            }
        })
    }

    /// "Hi-Res · 24-bit / 96 kHz"
    fn quality(&self) -> String {
        self.text(|album| {
            let quality =
                format::audio_quality(album.maximum_bit_depth, album.maximum_sampling_rate);
            match (album.hires_streamable, quality) {
                (true, Some(quality)) => format!("Hi-Res · {quality}"),
                (true, None) => "Hi-Res".to_owned(),
                (false, Some(quality)) => quality,
                (false, None) => String::new(),
            }
        })
    }

    fn review(&self) -> String {
        self.text(|album| {
            album
                .description
                .as_deref()
                .map(format::html_to_text)
                .unwrap_or_default()
        })
    }

    fn copyright(&self) -> String {
        self.text(|album| album.copyright.clone().unwrap_or_default())
    }
}

/// How a release type reads: "epk" is "EP".
fn release_type_name(release_type: &str) -> &str {
    match release_type {
        "single" => "Single",
        "epk" | "ep" => "EP",
        "live" => "Live",
        "compilation" => "Compilation",
        "boxset" | "box-set" => "Box Set",
        _ => "Album",
    }
}

/// List the tracks, under a heading per disc when there are several, and per work when
/// tracks are movements of a larger piece. Activating a track plays the album from it.
///
/// Returns the rows, with the id of their track.
fn fill_track_list(
    list: &gtk::Box,
    album: &Album,
    sender: &ComponentSender<AlbumPage>,
) -> Vec<(u64, gtk::ListBoxRow)> {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let tracks = album
        .tracks
        .as_ref()
        .map_or(&[][..], |page| page.items.as_slice());
    let several_discs = tracks.iter().any(|track| track.media_number > 1);

    let mut disc = None;
    let mut work = None;
    let mut all_rows = Vec::new();
    // The current list of rows, with the ids of its tracks by position.
    let mut rows: Option<(gtk::ListBox, Rc<RefCell<Vec<u64>>>)> = None;
    for track in tracks {
        if disc != Some(track.media_number) {
            disc = Some(track.media_number);
            work = None;
            if several_discs {
                list.append(&heading(
                    &format!("Disc {}", track.media_number),
                    "pochette-disc",
                ));
            }
            rows = None;
        }
        if track.work.is_some() && track.work != work {
            work.clone_from(&track.work);
            list.append(&heading(
                track.work.as_deref().unwrap_or_default(),
                "pochette-work",
            ));
            rows = None;
        }
        let (rows, ids) = rows.get_or_insert_with(|| {
            let rows = gtk::ListBox::new();
            rows.set_selection_mode(gtk::SelectionMode::None);
            rows.add_css_class("pochette-tracks");
            let ids: Rc<RefCell<Vec<u64>>> = Rc::default();
            let (sender, row_ids) = (sender.clone(), ids.clone());
            rows.connect_row_activated(move |_, row| {
                let id = usize::try_from(row.index())
                    .ok()
                    .and_then(|index| row_ids.borrow().get(index).copied());
                if let Some(id) = id {
                    sender.input(AlbumPageMsg::PlayTrack(id));
                }
            });
            list.append(&rows);
            (rows, ids)
        });
        let row = track_row(track, &album.artist.name);
        rows.append(&row);
        ids.borrow_mut().push(track.id);
        all_rows.push((track.id, row));
    }
    all_rows
}

fn heading(text: &str, css_class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(css_class);
    label
}

fn track_row(track: &Track, album_artist: &str) -> gtk::ListBoxRow {
    // Name the performer only when it isn't the album's artist, as on compilations.
    let performer = track
        .performer
        .as_ref()
        .map(|performer| performer.name.as_str())
        .filter(|name| *name != album_artist);

    relm4::view! {
        row = gtk::ListBoxRow {
            set_activatable: track.streamable,
            set_cursor_from_name: track.streamable.then_some("pointer"),

            gtk::Box {
                set_spacing: 12,
                add_css_class: "pochette-track",

                gtk::Label {
                    set_label: &track.track_number.to_string(),
                    set_width_chars: 3,
                    set_xalign: 1.0,
                    add_css_class: "dim-label",
                    add_css_class: "pochette-numeric",
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        set_label: &track.full_title(),
                        set_xalign: 0.0,
                        set_wrap: true,
                        set_wrap_mode: pango::WrapMode::WordChar,
                    },
                    gtk::Label {
                        set_label: performer.unwrap_or_default(),
                        set_visible: performer.is_some(),
                        set_xalign: 0.0,
                        set_wrap: true,
                        add_css_class: "dim-label",
                        add_css_class: "pochette-caption",
                    },
                },
                gtk::Label {
                    set_label: &format::track_duration(track.duration),
                    add_css_class: "dim-label",
                    add_css_class: "pochette-numeric",
                },
            },
        }
    }

    if !track.streamable {
        row.add_css_class("pochette-unavailable");
        row.set_tooltip_text(Some("Not available for streaming"));
    }
    row
}

/// Pass the player's status changes on to the page.
fn follow_playback(player: &Player, sender: &ComponentSender<AlbumPage>) {
    let mut status = player.status();
    let sender = sender.clone();
    relm4::spawn(async move {
        while status.changed().await.is_ok() {
            let current = status.borrow_and_update().clone();
            sender.input(AlbumPageMsg::PlaybackChanged(current));
        }
    });
}
