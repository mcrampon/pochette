//! An album as a tile: its cover, with play and heart buttons on hover, then its title and
//! artist, and maybe a caption. In grids, and in rows like those of the Discover tab.

use std::rc::Rc;

use qobuz::models::Album;
use relm4::{
    gtk::{self, glib, pango, prelude::*},
    typed_view::grid::RelmGridItem,
};

use crate::{
    covers::VisibleCovers,
    favorites::{Heart, with_heart},
    format,
    playback::{CoverActions, PlayButton},
    tile_grid::TileGrid,
    tile_strip::TileStrip,
};

pub use frame::CoverFrame;

/// The side of a cover, in logical pixels.
pub const COVER_SIZE: i32 = 180;

/// The narrowest a cover gets in a row of the Discover tab before the row drops a column,
/// and the narrowest it gets at all, in two columns on a narrow window.
const MIN_COVER_SIZE: i32 = 150;
const SMALLEST_COVER_SIZE: i32 = 100;

/// The widest a cover gets in a row of the Discover tab, on a wide window.
const MAX_COVER_SIZE: i32 = 576;

/// The padding around a tile in a row: see `.pochette-row-tile` in the style sheet.
const ROW_TILE_PADDING: i32 = 2 * 12;

/// An album in a grid.
pub struct AlbumItem {
    pub album: Rc<Album>,
    /// A line under the artist, such as the album's awards.
    pub caption: Option<String>,
    pub covers: VisibleCovers,
    pub actions: CoverActions,
}

pub struct AlbumWidgets {
    cover: gtk::Picture,
    genre: gtk::Label,
    date: gtk::Label,
    heart: Heart,
    play: PlayButton,
    title: gtk::Label,
    artist: gtk::Label,
    caption: gtk::Label,
    /// Whether to show the large cover whatever the screen's scale, for a cover that can
    /// grow past the small one's size.
    large: bool,
}

/// A tile, empty until [`AlbumWidgets::show`] fills it.
pub fn tile() -> (gtk::Box, AlbumWidgets) {
    tile_sized(false)
}

/// A tile whose cover grows and shrinks with the width it's given, if `fluid`, rather than
/// staying [`COVER_SIZE`] wide.
fn tile_sized(fluid: bool) -> (gtk::Box, AlbumWidgets) {
    let size = if fluid { -1 } else { COVER_SIZE };
    let cover = gtk::Picture::builder()
        .width_request(size)
        .height_request(size)
        .content_fit(gtk::ContentFit::Cover)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["pochette-cover"])
        .build();
    let frame = if fluid {
        CoverFrame::fluid(&cover, SMALLEST_COVER_SIZE, COVER_SIZE)
    } else {
        CoverFrame::new(&cover, COVER_SIZE)
    };
    let (shaded, genre, date) = with_shade(&frame);
    let heart = Heart::new();
    let cover_box = with_heart(&shaded, &heart);
    let play = PlayButton::new();
    cover_box.add_overlay(play.widget());

    relm4::view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,
            set_width_request: size,
            set_halign: if fluid { gtk::Align::Fill } else { gtk::Align::Center },
            // Takes the whole width of its column, past its natural width.
            set_hexpand: fluid,
            set_valign: gtk::Align::Start,
            set_cursor_from_name: Some("pointer"),
            // The buttons over the cover show when the pointer is anywhere on the tile.
            add_css_class: "pochette-cover-hover",
            add_css_class: "pochette-album-tile",

            append: &cover_box,
            #[name = "title"]
            gtk::Label {
                set_xalign: 0.0,
                // Lets the label shrink to the tile instead of widening it.
                set_max_width_chars: 1,
                set_ellipsize: pango::EllipsizeMode::End,
                set_margin_top: 6,
                add_css_class: "pochette-album-title",
            },
            #[name = "artist"]
            gtk::Label {
                set_xalign: 0.0,
                set_max_width_chars: 1,
                set_ellipsize: pango::EllipsizeMode::End,
                add_css_class: "dim-label",
            },
            #[name = "caption"]
            gtk::Label {
                set_xalign: 0.0,
                set_max_width_chars: 1,
                set_ellipsize: pango::EllipsizeMode::End,
                add_css_class: "pochette-tile-caption",
            },
        }
    }

    let widgets = AlbumWidgets {
        cover,
        genre,
        date,
        heart,
        play,
        title,
        artist,
        caption,
        large: fluid,
    };
    (root, widgets)
}

impl AlbumWidgets {
    pub fn show(
        &self,
        album: &Rc<Album>,
        caption: Option<&str>,
        covers: &VisibleCovers,
        actions: &CoverActions,
    ) {
        let title = album.full_title();
        self.title.set_label(&title);
        self.artist.set_label(&album.artist.name);
        self.caption.set_label(caption.unwrap_or_default());
        self.caption.set_visible(caption.is_some());
        let genre = album.genre.as_ref().map(|genre| genre.name.as_str());
        self.genre.set_label(genre.unwrap_or_default());
        self.genre.set_visible(genre.is_some());
        let date = album
            .release_date_original
            .as_deref()
            .map(format::release_date);
        self.date.set_label(date.as_deref().unwrap_or_default());
        self.date.set_visible(date.is_some());
        self.heart.show(Some(album.clone()), &actions.favorites);
        self.play.show(Some(album.clone()), &actions.playback);

        match cover_url(album, self.large || self.cover.scale_factor() > 1) {
            Some(url) => covers.show(&self.cover, url),
            None => covers.hide(&self.cover),
        }
    }

    pub fn hide(&self, covers: &VisibleCovers) {
        covers.hide(&self.cover);
        self.heart.clear();
        self.play.clear();
    }
}

impl RelmGridItem for AlbumItem {
    type Root = gtk::Box;
    type Widgets = AlbumWidgets;

    fn setup(_item: &gtk::ListItem) -> (gtk::Box, AlbumWidgets) {
        tile()
    }

    fn bind(&mut self, widgets: &mut AlbumWidgets, _root: &mut gtk::Box) {
        widgets.show(
            &self.album,
            self.caption.as_deref(),
            &self.covers,
            &self.actions,
        );
    }

    fn unbind(&mut self, widgets: &mut AlbumWidgets, _root: &mut gtk::Box) {
        widgets.hide(&self.covers);
    }
}

/// `cover` with a shade over it that shows on hover, with the album's genre and release
/// date in its top left corner: the overlay, then the genre and date labels.
fn with_shade(cover: &impl IsA<gtk::Widget>) -> (gtk::Overlay, gtk::Label, gtk::Label) {
    let label = |class: &str| {
        gtk::Label::builder()
            .xalign(0.0)
            // Lets the label shrink to the cover instead of widening it.
            .max_width_chars(1)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes([class])
            .build()
    };
    let (genre, date) = (label("pochette-cover-genre"), label("pochette-cover-date"));
    let shade = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(["pochette-cover-shade"])
        // Clicks go through to the tile, which opens the album.
        .can_target(false)
        .build();
    shade.append(&genre);
    shade.append(&date);
    let overlay = gtk::Overlay::builder().child(cover).build();
    overlay.add_overlay(&shade);
    (overlay, genre, date)
}

/// Calls a row's handler with whether some of its tiles are hidden.
type ConnectOverflow = Box<dyn FnOnce(Box<dyn Fn(bool)>)>;

/// How a row lays out its albums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowLayout {
    /// One line of covers at their size, as many as fit.
    Strip,
    /// Rows of covers that grow and shrink with the width: two rows of six on a wide
    /// window, down to six rows of two on a narrow one.
    Grid,
}

/// A row of albums under `title`, laid out as `layout`. Given `see_all`, "See All" next to
/// the title calls it, shown when some albums don't fit or Qobuz has more (`more_beyond`).
/// Clicking an album, or pressing Enter on it, calls `open` with its index.
///
/// Returns the row, and the tiles to let go of their covers when the row goes.
pub fn album_row(
    title: &str,
    albums: &[Rc<Album>],
    caption: impl Fn(&Album) -> Option<String>,
    (covers, actions): (&VisibleCovers, &CoverActions),
    layout: RowLayout,
    open: impl Fn(usize) + 'static,
    see_all: Option<(Box<dyn Fn()>, bool)>,
) -> (gtk::Box, Vec<AlbumWidgets>) {
    let open = Rc::new(open);
    let (container, connect_overflow): (gtk::Widget, ConnectOverflow) = match layout {
        RowLayout::Strip => {
            let strip = TileStrip::default();
            (
                strip.clone().upcast(),
                Box::new(move |f| strip.connect_overflow(f)),
            )
        }
        RowLayout::Grid => {
            let grid = TileGrid::new(
                MIN_COVER_SIZE + ROW_TILE_PADDING,
                MAX_COVER_SIZE + ROW_TILE_PADDING,
            );
            (
                grid.clone().upcast(),
                Box::new(move |f| grid.connect_overflow(f)),
            )
        }
    };
    container.set_margin_start(24);
    container.set_margin_end(24);
    let mut tiles = Vec::new();
    for (index, album) in albums.iter().enumerate() {
        let (item, widgets) = tile_sized(layout == RowLayout::Grid);
        widgets.show(album, caption(album).as_deref(), covers, actions);
        let open = open.clone();
        row_tile(&item, move || open(index)).set_parent(&container);
        tiles.push(widgets);
    }

    let header = row_header(title);
    if let Some((see_all, more_beyond)) = see_all {
        let button = gtk::Button::builder()
            .label("See All")
            .css_classes(["flat"])
            .margin_start(6)
            .visible(more_beyond)
            .build();
        button.connect_clicked(move |_| see_all());
        header.append(&button);
        let button = button.downgrade();
        connect_overflow(Box::new(move |hidden| {
            if let Some(button) = button.upgrade() {
                button.set_visible(more_beyond || hidden);
            }
        }));
    }

    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.append(&header);
    row.append(&container);
    (row, tiles)
}

/// The header of a row, lined up with its tiles' covers: its title, then maybe buttons.
pub fn row_header(title: &str) -> gtk::Box {
    let header = gtk::Box::builder()
        .spacing(6)
        .margin_start(36)
        .margin_end(36)
        .build();
    header.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["pochette-heading"])
            .build(),
    );
    header
}

/// `child` as an item of a row, calling `open` on a click or Enter.
///
/// Not a button: a button would take the clicks meant for the heart over a cover.
pub fn row_tile(child: &impl IsA<gtk::Widget>, open: impl Fn() + 'static) -> gtk::Box {
    let item = gtk::Box::builder()
        .css_classes(["pochette-row-tile"])
        .focusable(true)
        .build();
    item.append(child);
    let open = Rc::new(open);

    // The heart claims its clicks, so this one only sees those elsewhere on the tile.
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    let click_open = open.clone();
    click.connect_released(move |gesture, _, x, y| {
        // A click on the heart, or any other button on the tile, isn't one to open.
        let on_button = gesture
            .widget()
            .and_then(|item| item.pick(x, y, gtk::PickFlags::DEFAULT))
            .is_some_and(|target| {
                target.is::<gtk::Button>() || target.ancestor(gtk::Button::static_type()).is_some()
            });
        if !on_button {
            click_open();
        }
    });
    item.add_controller(click);

    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
            open();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    item.add_controller(keys);
    item
}

/// The large cover, or the small one, whichever is wanted and there.
fn cover_url(album: &Album, large: bool) -> Option<String> {
    let image = &album.image;
    let sizes = if large {
        [&image.large, &image.small]
    } else {
        [&image.small, &image.large]
    };
    sizes.into_iter().flatten().next().cloned()
}

/// The awards of `album`, as a caption: "4F de Télérama · Pitchfork: Best New Music".
pub fn awards(album: &Album) -> Option<String> {
    let names: Vec<&str> = album
        .awards
        .iter()
        .map(|award| award.name.as_str())
        .collect();
    (!names.is_empty()).then(|| names.join(" · "))
}

/// The most columns a grid of tiles ever needs.
const MAX_COLUMNS: u32 = 16;

/// How wide a tile is in a grid, with the padding around it, and how much padding the
/// grid adds on its sides: see `.pochette-album-grid` in the style sheet.
const GRID_TILE_WIDTH: i32 = COVER_SIZE + 2 * 12;
const GRID_SIDE_PADDING: i32 = 2 * 24;

/// Keep `grid`'s column count to what fits its width, as the window resizes.
///
/// A grid view keeps tiles for 30 rows of its maximum column count, ready to scroll to,
/// and fills them all whenever its albums change: with a maximum of 16 columns where 5
/// fit, that's three times the tiles to fill, which made searching sluggish.
pub fn fit_columns(grid: &gtk::GridView) {
    let Some(adjustment) = grid.hadjustment() else {
        return;
    };
    let grid = grid.downgrade();
    adjustment.connect_page_size_notify(move |adjustment| {
        #[expect(clippy::cast_possible_truncation, reason = "widths are small")]
        let width = adjustment.page_size() as i32;
        let fit = (width - GRID_SIDE_PADDING) / GRID_TILE_WIDTH;
        // One more, in case the tiles end up narrower than reckoned.
        let columns = u32::try_from(fit + 1).unwrap_or(0).clamp(2, MAX_COLUMNS);
        let Some(grid) = grid.upgrade() else {
            return;
        };
        if grid.max_columns() != columns {
            // Not while the grid is being laid out, which is when the width changes.
            glib::idle_add_local_once(move || grid.set_max_columns(columns));
        }
    });
}

/// Holds a picture at a given size, square, whatever the size of its image: a cover, or
/// an artist's portrait.
///
/// A picture asks for its image's size, often larger than a tile: in a grid it then
/// widened to the column while its row kept it [`COVER_SIZE`] tall, and the cover
/// stretched into a rectangle as the window resized.
mod frame {
    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    glib::wrapper! {
        pub struct CoverFrame(ObjectSubclass<imp::CoverFrame>)
            @extends gtk::Widget,
            @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl CoverFrame {
        /// `picture` at `size` by `size`.
        pub fn new(picture: &gtk::Picture, size: i32) -> Self {
            let frame: Self = glib::Object::new();
            frame.imp().size.set(size);
            picture.set_parent(&frame);
            frame
        }

        /// `picture` as a square as wide as the frame is given, from `minimum` wide, and
        /// `natural` wide when it can be.
        pub fn fluid(picture: &gtk::Picture, minimum: i32, natural: i32) -> Self {
            let frame = Self::new(picture, natural);
            frame.imp().minimum.set(Some(minimum));
            frame
        }
    }

    mod imp {
        use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

        use std::cell::Cell;

        #[derive(Default)]
        pub struct CoverFrame {
            pub size: Cell<i32>,
            /// The narrowest a fluid frame gets; none for one fixed at `size`.
            pub minimum: Cell<Option<i32>>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for CoverFrame {
            const NAME: &'static str = "PochetteCoverFrame";
            type Type = super::CoverFrame;
            type ParentType = gtk::Widget;
        }

        impl ObjectImpl for CoverFrame {
            fn dispose(&self) {
                while let Some(child) = self.obj().first_child() {
                    child.unparent();
                }
            }
        }

        impl WidgetImpl for CoverFrame {
            fn request_mode(&self) -> gtk::SizeRequestMode {
                if self.minimum.get().is_some() {
                    gtk::SizeRequestMode::HeightForWidth
                } else {
                    gtk::SizeRequestMode::ConstantSize
                }
            }

            fn measure(&self, _: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
                let size = self.size.get();
                match self.minimum.get() {
                    None => (size, size, -1, -1),
                    // Square: as tall as it's wide, and as wide as it's tall.
                    Some(_) if for_size >= 0 => (for_size, for_size, -1, -1),
                    Some(minimum) => (minimum, size, -1, -1),
                }
            }

            fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
                let Some(picture) = self.obj().first_child() else {
                    return;
                };
                // Square, centered, even if given more room one way.
                let side = width.min(height);
                // GTK wants a widget measured before it's allocated.
                picture.measure(gtk::Orientation::Horizontal, -1);
                picture.size_allocate(
                    &gtk::Allocation::new((width - side) / 2, (height - side) / 2, side, side),
                    -1,
                );
            }
        }
    }
}
