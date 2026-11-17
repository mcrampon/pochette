//! All the releases of an artist of one kind, as a grid that loads more as it scrolls.

use std::rc::Rc;

use qobuz::{Client, artist::ReleaseKind, discover::Albums, models::Album};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, prelude::*},
    typed_view::grid::TypedGridView,
};

use crate::{
    album_tile::{self, AlbumItem},
    artist_page::release_kind_title,
    covers::VisibleCovers,
    navigation::{Destination, PageOutput},
    playback::CoverActions,
};

/// How many albums load at a time.
const PAGE_SIZE: usize = 50;

/// How close to the end of the grid more albums load, in screens.
const LOAD_AHEAD: f64 = 1.5;

pub struct ReleasesPage {
    client: Option<Client>,
    artist_id: u64,
    artist_name: String,
    kind: ReleaseKind,
    grid: TypedGridView<AlbumItem, gtk::NoSelection>,
    covers: VisibleCovers,
    scroller: gtk::ScrolledWindow,
    has_more: bool,
    loading: bool,
    error: Option<String>,
    actions: CoverActions,
}

#[derive(Debug)]
pub enum ReleasesPageMsg {
    Show {
        client: Client,
        artist_id: u64,
        artist_name: String,
        kind: ReleaseKind,
    },
    Back,
    LoadMore,
    /// Open the album at this position in the grid.
    Open(u32),
}

#[derive(Debug)]
pub enum ReleasesPageCommand {
    Loaded {
        offset: usize,
        result: qobuz::Result<Albums>,
    },
}

#[relm4::component(pub)]
impl Component for ReleasesPage {
    type Init = CoverActions;
    type Input = ReleasesPageMsg;
    type Output = PageOutput;
    type CommandOutput = ReleasesPageCommand;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            add_controller = gtk::GestureClick {
                // The mouse's back button.
                set_button: 8,
                connect_pressed[sender] => move |_, _, _, _| sender.input(ReleasesPageMsg::Back),
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
                            sender.input(ReleasesPageMsg::Back);
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
                    connect_clicked => ReleasesPageMsg::Back,
                },
                gtk::Label {
                    #[watch]
                    set_label: release_kind_title(model.kind),
                    add_css_class: "pochette-heading",
                },
                gtk::Label {
                    #[watch]
                    set_label: &model.artist_name,
                    add_css_class: "dim-label",
                },
            },

            #[local_ref]
            scroller -> gtk::ScrolledWindow {
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
                    add_css_class: "pochette-album-grid",
                    connect_activate[sender] => move |_, position| {
                        sender.input(ReleasesPageMsg::Open(position));
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
                    set_visible: model.loading,
                    set_spinning: true,
                    set_size_request: (32, 32),
                },
                gtk::Label {
                    #[watch]
                    set_visible: !model.loading,
                    #[watch]
                    set_label: if model.error.is_some() {
                        "Can't Load These Albums"
                    } else {
                        "No Albums Here"
                    },
                    add_css_class: "pochette-heading",
                },
                gtk::Label {
                    #[watch]
                    set_visible: !model.loading && model.error.is_some(),
                    #[watch]
                    set_label: model.error.as_deref().unwrap_or_default(),
                    add_css_class: "dim-label",
                    set_wrap: true,
                    set_max_width_chars: 50,
                    set_justify: gtk::Justification::Center,
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
            artist_id: 0,
            artist_name: String::new(),
            kind: ReleaseKind::Album,
            grid: TypedGridView::new(),
            covers: VisibleCovers::default(),
            scroller: gtk::ScrolledWindow::new(),
            has_more: true,
            loading: false,
            error: None,
            actions,
        };
        let scroller = &model.scroller;
        let grid_view = &model.grid.view;
        let widgets = view_output!();

        model.covers.attach(&model.scroller);
        album_tile::fit_columns(&model.grid.view);
        // Load more as the end of the grid comes near, or while the grid doesn't fill the
        // screen.
        let adjustment = model.scroller.vadjustment();
        let input = sender.input_sender().clone();
        adjustment.connect_value_changed(move |_| input.emit(ReleasesPageMsg::LoadMore));
        let input = sender.input_sender().clone();
        adjustment.connect_changed(move |_| input.emit(ReleasesPageMsg::LoadMore));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ReleasesPageMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ReleasesPageMsg::Show {
                client,
                artist_id,
                artist_name,
                kind,
            } => {
                self.client = Some(client);
                self.artist_id = artist_id;
                self.artist_name = artist_name;
                self.kind = kind;
                self.load_more(&sender);
            }
            ReleasesPageMsg::Back => {
                let _ = sender.output(PageOutput::Back);
            }
            ReleasesPageMsg::LoadMore => {
                if self.needs_more() {
                    self.load_more(&sender);
                }
            }
            ReleasesPageMsg::Open(position) => {
                if let Some(item) = self.grid.get(position) {
                    let album = Album::clone(&item.borrow().album);
                    let _ = sender.output(PageOutput::Open(Destination::Album(Box::new(album))));
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        command: ReleasesPageCommand,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let ReleasesPageCommand::Loaded { offset, result } = command;
        if offset != self.grid.len() as usize {
            return;
        }
        self.loading = false;
        match result {
            Ok(albums) => {
                self.has_more = albums.has_more && !albums.albums.is_empty();
                self.grid
                    .extend_from_iter(albums.albums.into_iter().map(|album| AlbumItem {
                        album: Rc::new(album),
                        caption: None,
                        covers: self.covers.clone(),
                        actions: self.actions.clone(),
                    }));
            }
            Err(qobuz::Error::Unauthorized { .. }) => {
                let _ = sender.output(PageOutput::SessionExpired);
            }
            Err(err) => {
                tracing::warn!("cannot load more releases: {err}");
                // Stop there rather than retry at every scroll.
                self.has_more = false;
                self.error = Some(err.to_string());
            }
        }
    }
}

impl ReleasesPage {
    /// Whether the grid's end is near enough to load more.
    fn needs_more(&self) -> bool {
        let adjustment = self.scroller.vadjustment();
        let page = adjustment.page_size();
        adjustment.upper() - (adjustment.value() + page) < LOAD_AHEAD * page
    }

    fn load_more(&mut self, sender: &ComponentSender<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.loading || !self.has_more {
            return;
        }
        self.loading = true;
        let (id, kind, offset) = (self.artist_id, self.kind, self.grid.len() as usize);
        // A command, which stops with the page.
        sender.oneshot_command(async move {
            let result = client.artist_releases(id, kind, offset, PAGE_SIZE).await;
            ReleasesPageCommand::Loaded { offset, result }
        });
    }
}
