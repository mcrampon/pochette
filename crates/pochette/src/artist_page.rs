//! An artist's page: their portrait, name and biography, their releases by kind as rows of
//! albums, then artists like them.

use std::rc::Rc;

use qobuz::{
    Client,
    artist::{Artist, ArtistPreview, ReleaseKind},
    models::Album,
};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, pango, prelude::*},
};

use crate::{
    album_tile::{self, AlbumWidgets},
    covers::{self, VisibleCovers},
    format,
    i18n::tr,
    navigation::{Destination, PageOutput},
    playback::CoverActions,
    tile_strip::TileStrip,
};

/// The side of the artist's portrait, in logical pixels.
const PORTRAIT_SIZE: i32 = 180;

/// The side of a similar artist's portrait.
const SIMILAR_SIZE: i32 = 120;

/// How long a biography gets before it's folded, in characters.
const SHORT_BIOGRAPHY: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Loading,
    Failed,
    Loaded,
}

pub struct ArtistPage {
    client: Option<Client>,
    id: u64,
    /// Known before the page loads, from where the artist was opened.
    name: String,
    state: State,
    error: String,
    biography: String,
    biography_unfolded: bool,
    portrait: gtk::Picture,
    initial: gtk::Label,
    /// The rows of releases, then of similar artists.
    rows: gtk::Box,
    releases: Vec<(ReleaseKind, Vec<Rc<Album>>)>,
    similar: Vec<ArtistPreview>,
    tiles: Vec<AlbumWidgets>,
    covers: VisibleCovers,
    actions: CoverActions,
}

#[derive(Debug)]
pub enum ArtistPageMsg {
    Show {
        client: Client,
        id: u64,
        name: String,
    },
    Retry,
    Back,
    ToggleBiography,
    /// Open the album at this index in the row of this kind.
    OpenAlbum(ReleaseKind, usize),
    SeeAll(ReleaseKind),
    /// Open the similar artist at this index.
    OpenSimilar(usize),
}

#[derive(Debug)]
pub enum ArtistPageCommand {
    Loaded {
        id: u64,
        result: Box<qobuz::Result<Artist>>,
    },
}

#[relm4::component(pub)]
impl Component for ArtistPage {
    type Init = CoverActions;
    type Input = ArtistPageMsg;
    type Output = PageOutput;
    type CommandOutput = ArtistPageCommand;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,

            add_controller = gtk::GestureClick {
                // The mouse's back button.
                set_button: 8,
                connect_pressed[sender] => move |_, _, _, _| sender.input(ArtistPageMsg::Back),
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
                            sender.input(ArtistPageMsg::Back);
                            glib::Propagation::Stop
                        }
                    })),
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 24,
                set_margin_top: 12,
                set_margin_bottom: 48,

                gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some(&tr!("back")),
                    set_halign: gtk::Align::Start,
                    set_margin_start: 24,
                    add_css_class: "flat",
                    add_css_class: "circular",
                    connect_clicked => ArtistPageMsg::Back,
                },

                // Portrait, name and biography.
                gtk::Box {
                    set_spacing: 32,
                    set_margin_start: 36,
                    set_margin_end: 36,

                    gtk::Overlay {
                        set_valign: gtk::Align::Start,
                        #[local_ref]
                        portrait_frame -> album_tile::CoverFrame {},
                        // Stands in for a missing portrait.
                        add_overlay: initial,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,
                        set_valign: gtk::Align::Center,
                        set_hexpand: true,

                        gtk::Label {
                            set_label: &tr!("artist-kicker").to_uppercase(),
                            set_xalign: 0.0,
                            add_css_class: "pochette-kicker",
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.name,
                            set_xalign: 0.0,
                            set_wrap: true,
                            set_wrap_mode: pango::WrapMode::WordChar,
                            add_css_class: "pochette-album-page-title",
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.shown_biography(),
                            #[watch]
                            set_visible: !model.biography.is_empty(),
                            #[watch]
                            set_lines: if model.folds_biography() { 5 } else { -1 },
                            #[watch]
                            set_ellipsize: if model.folds_biography() {
                                pango::EllipsizeMode::End
                            } else {
                                pango::EllipsizeMode::None
                            },
                            set_xalign: 0.0,
                            set_wrap: true,
                            set_max_width_chars: 90,
                            set_margin_top: 6,
                            add_css_class: "pochette-review",
                        },
                        gtk::Button {
                            #[watch]
                            set_label: &if model.biography_unfolded { tr!("artist-less") } else { tr!("artist-more") },
                            #[watch]
                            set_visible: model.biography_folds(),
                            set_halign: gtk::Align::Start,
                            add_css_class: "flat",
                            connect_clicked => ArtistPageMsg::ToggleBiography,
                        },
                    },
                },

                #[transition = "Crossfade"]
                match model.state {
                    State::Loading => gtk::Box {
                        set_halign: gtk::Align::Center,
                        set_margin_top: 24,

                        gtk::Spinner {
                            set_spinning: true,
                            set_size_request: (32, 32),
                        },
                    },
                    State::Failed => gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,
                        set_margin_top: 24,

                        gtk::Label {
                            #[watch]
                            set_label: &tr!("artist-cannot-load", error = model.error.as_str()),
                            add_css_class: "dim-label",
                            set_wrap: true,
                            set_max_width_chars: 50,
                            set_justify: gtk::Justification::Center,
                        },
                        gtk::Button {
                            set_label: &tr!("try-again"),
                            set_halign: gtk::Align::Center,
                            connect_clicked => ArtistPageMsg::Retry,
                        },
                    },
                    State::Loaded => gtk::Box {
                        #[local_ref]
                        rows -> gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 24,
                            set_hexpand: true,
                        },
                    },
                },
            },
        }
    }

    fn init(
        actions: CoverActions,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            client: None,
            id: 0,
            name: String::new(),
            state: State::Loading,
            error: String::new(),
            biography: String::new(),
            biography_unfolded: false,
            portrait: round_picture(),
            initial: initial_label("pochette-artist-initial"),
            rows: gtk::Box::default(),
            releases: Vec::new(),
            similar: Vec::new(),
            tiles: Vec::new(),
            covers: VisibleCovers::default(),
            actions,
        };
        let portrait_frame = &album_tile::CoverFrame::new(&model.portrait, PORTRAIT_SIZE);
        let initial = &model.initial;
        let rows = &model.rows;
        let widgets = view_output!();
        model.covers.attach(&root);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ArtistPageMsg, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            ArtistPageMsg::Show { client, id, name } => {
                root.vadjustment().set_value(0.0);
                self.initial.set_label(&initial_of(&name));
                self.client = Some(client);
                self.id = id;
                self.name = name;
                self.load(&sender);
            }
            ArtistPageMsg::Retry => self.load(&sender),
            ArtistPageMsg::Back => {
                let _ = sender.output(PageOutput::Back);
            }
            ArtistPageMsg::ToggleBiography => {
                self.biography_unfolded = !self.biography_unfolded;
            }
            ArtistPageMsg::OpenAlbum(kind, index) => {
                let album = self
                    .releases
                    .iter()
                    .find(|(row, _)| *row == kind)
                    .and_then(|(_, albums)| albums.get(index));
                if let Some(album) = album {
                    let album = Album::clone(album);
                    let _ = sender.output(PageOutput::Open(Destination::Album(Box::new(album))));
                }
            }
            ArtistPageMsg::SeeAll(kind) => {
                let _ = sender.output(PageOutput::Open(Destination::Releases {
                    artist_id: self.id,
                    artist_name: self.name.clone(),
                    kind,
                }));
            }
            ArtistPageMsg::OpenSimilar(index) => {
                if let Some(artist) = self.similar.get(index) {
                    let _ = sender.output(PageOutput::Open(Destination::Artist {
                        id: artist.id,
                        name: artist.name.clone(),
                    }));
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        command: ArtistPageCommand,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let ArtistPageCommand::Loaded { id, result } = command;
        if id != self.id {
            return;
        }
        match *result {
            Ok(artist) => self.show(artist, &sender),
            Err(qobuz::Error::Unauthorized { .. }) => {
                let _ = sender.output(PageOutput::SessionExpired);
            }
            Err(err) => {
                tracing::warn!("cannot load artist {id}: {err}");
                self.error = err.to_string();
                self.state = State::Failed;
            }
        }
    }
}

impl ArtistPage {
    fn load(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.state = State::Loading;
        let id = self.id;
        // A command, which stops with the page.
        sender.oneshot_command(async move {
            let result = Box::new(client.artist(id).await);
            ArtistPageCommand::Loaded { id, result }
        });
    }

    /// Whether the biography is long enough to fold: longer than [`SHORT_BIOGRAPHY`], or in
    /// several paragraphs.
    fn biography_folds(&self) -> bool {
        self.biography.chars().count() > SHORT_BIOGRAPHY || self.biography.contains("\n\n")
    }

    fn folds_biography(&self) -> bool {
        !self.biography_unfolded && self.biography_folds()
    }

    /// The biography, or its first paragraph while folded: the label limits each paragraph
    /// to so many lines, not the whole.
    fn shown_biography(&self) -> String {
        if self.folds_biography() {
            let first = self.biography.split("\n\n").next().unwrap_or_default();
            first.to_owned()
        } else {
            self.biography.clone()
        }
    }

    fn show(&mut self, artist: Artist, sender: &ComponentSender<Self>) {
        self.name = artist.name;
        self.initial.set_label(&initial_of(&self.name));
        self.biography = artist
            .biography
            .as_deref()
            .map(format::html_to_text)
            .unwrap_or_default();
        show_portrait(&self.portrait, &self.initial, artist.portrait);

        for tile in self.tiles.drain(..) {
            tile.hide(&self.covers);
        }
        while let Some(row) = self.rows.first_child() {
            self.rows.remove(&row);
        }
        self.releases.clear();
        for (kind, albums) in artist.releases {
            let more_beyond = albums.has_more;
            let albums: Vec<Rc<Album>> = albums.albums.into_iter().map(Rc::new).collect();
            let (open, see_all) = (sender.clone(), sender.clone());
            let (row, tiles) = album_tile::album_row(
                &release_kind_title(kind),
                &albums,
                |_| None,
                (&self.covers, &self.actions),
                album_tile::RowLayout::Strip,
                move |index| open.input(ArtistPageMsg::OpenAlbum(kind, index)),
                Some((
                    Box::new(move || see_all.input(ArtistPageMsg::SeeAll(kind))),
                    more_beyond,
                )),
            );
            self.rows.append(&row);
            self.tiles.extend(tiles);
            self.releases.push((kind, albums));
        }
        if self.releases.is_empty() {
            let none = gtk::Label::builder()
                .label(tr!("artist-no-albums"))
                .css_classes(["dim-label"])
                .margin_start(36)
                .xalign(0.0)
                .build();
            self.rows.append(&none);
        }

        self.similar = artist.similar;
        if !self.similar.is_empty() {
            let sender = sender.clone();
            self.rows.append(&artist_row(
                &tr!("artist-similar"),
                &self.similar,
                move |index| {
                    sender.input(ArtistPageMsg::OpenSimilar(index));
                },
            ));
        }
        self.state = State::Loaded;
    }
}

/// What a kind of releases is called, as a row's title.
pub fn release_kind_title(kind: ReleaseKind) -> String {
    match kind {
        ReleaseKind::Album => tr!("releases-albums"),
        ReleaseKind::EpSingle => tr!("releases-eps-singles"),
        ReleaseKind::Live => tr!("releases-live"),
        ReleaseKind::Compilation => tr!("releases-compilations"),
        ReleaseKind::Other => tr!("releases-other"),
    }
}

/// A row of `artists` under `title`, as many as fit, as round portraits with their names.
/// Clicking one, or pressing Enter on it, calls `open` with its index.
pub fn artist_row(
    title: &str,
    artists: &[ArtistPreview],
    open: impl Fn(usize) + 'static,
) -> gtk::Box {
    let open = Rc::new(open);
    let strip = TileStrip::default();
    strip.set_margin_start(24);
    strip.set_margin_end(24);
    for (index, artist) in artists.iter().enumerate() {
        let picture = round_picture();
        let initial = initial_label("pochette-similar-initial");
        initial.set_label(&initial_of(&artist.name));
        let overlay = gtk::Overlay::builder()
            .child(&album_tile::CoverFrame::new(&picture, SIMILAR_SIZE))
            .build();
        overlay.add_overlay(&initial);
        show_portrait(&picture, &initial, artist.portrait.clone());

        let name = gtk::Label::builder()
            .label(&artist.name)
            .max_width_chars(1)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["pochette-album-title"])
            .build();
        let item = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .width_request(SIMILAR_SIZE)
            .build();
        item.set_cursor_from_name(Some("pointer"));
        item.append(&overlay);
        item.append(&name);
        let open = open.clone();
        strip.append(&album_tile::row_tile(&item, move || open(index)));
    }

    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.append(&album_tile::row_header(title));
    row.append(&strip);
    row
}

/// A round picture, to hold in a [`album_tile::CoverFrame`] of its size.
fn round_picture() -> gtk::Picture {
    gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Cover)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["pochette-portrait"])
        .build()
}

fn initial_label(css_class: &str) -> gtk::Label {
    gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .can_target(false)
        .css_classes(["dim-label", css_class])
        .build()
}

/// The first letter of `name`, to stand in for a missing portrait.
fn initial_of(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// Show the portrait at `url` in `picture` once loaded, or `initial` when there's none.
fn show_portrait(picture: &gtk::Picture, initial: &gtk::Label, url: Option<String>) {
    let cached = url.as_deref().and_then(covers::cached);
    picture.set_paintable(cached.as_ref());
    initial.set_visible(cached.is_none());
    let Some(url) = url.filter(|_| cached.is_none()) else {
        return;
    };
    // The widgets, rather than the page, so that nothing waits on a page that was closed.
    let (picture, initial) = (picture.downgrade(), initial.downgrade());
    relm4::spawn_local(async move {
        let texture = covers::load(url).await;
        if let (Some(texture), Some(picture), Some(initial)) =
            (texture, picture.upgrade(), initial.upgrade())
        {
            picture.set_paintable(Some(&texture));
            initial.set_visible(false);
        }
    });
}
