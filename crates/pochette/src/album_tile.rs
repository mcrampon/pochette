//! An album as a tile: its cover, with play and heart buttons on hover, then its title and
//! artist, and maybe a caption. In grids, and in the rows of the Discover tab.

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
};

use frame::CoverFrame;

/// The side of a cover, in logical pixels.
pub const COVER_SIZE: i32 = 180;

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
}

/// A tile, empty until [`AlbumWidgets::show`] fills it.
pub fn tile() -> (gtk::Box, AlbumWidgets) {
    let cover = gtk::Picture::builder()
        .width_request(COVER_SIZE)
        .height_request(COVER_SIZE)
        .content_fit(gtk::ContentFit::Cover)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["pochette-cover"])
        .build();
    let (shaded, genre, date) = with_shade(&CoverFrame::new(&cover));
    let heart = Heart::new();
    let cover_box = with_heart(&shaded, &heart);
    let play = PlayButton::new();
    cover_box.add_overlay(play.widget());

    relm4::view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,
            set_width_request: COVER_SIZE,
            set_halign: gtk::Align::Center,
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

        match cover_url(album, self.cover.scale_factor()) {
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

/// Holds a cover at [`COVER_SIZE`], square, whatever the size of its image.
///
/// A picture asks for its image's size, often larger than a tile: in a grid it then
/// widened to the column while its row kept it [`COVER_SIZE`] tall, and the cover
/// stretched into a rectangle as the window resized.
mod frame {
    use relm4::gtk::{self, glib, prelude::*};

    glib::wrapper! {
        pub struct CoverFrame(ObjectSubclass<imp::CoverFrame>)
            @extends gtk::Widget,
            @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl CoverFrame {
        pub fn new(picture: &gtk::Picture) -> Self {
            let frame: Self = glib::Object::new();
            picture.set_parent(&frame);
            frame
        }
    }

    mod imp {
        use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

        use super::super::COVER_SIZE;

        #[derive(Default)]
        pub struct CoverFrame;

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
            fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
                (COVER_SIZE, COVER_SIZE, -1, -1)
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
