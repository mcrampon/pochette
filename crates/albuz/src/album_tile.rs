//! An album as a tile: its cover, title and artist, and maybe a caption. In grids, and in
//! the rows of the Discover tab.

use std::rc::Rc;

use qobuz::models::Album;
use relm4::{
    gtk::{self, pango, prelude::*},
    typed_view::grid::RelmGridItem,
};

use crate::covers::VisibleCovers;

/// The side of a cover, in logical pixels.
pub const COVER_SIZE: i32 = 180;

/// An album in a grid.
pub struct AlbumItem {
    pub album: Rc<Album>,
    /// A line under the artist, such as the album's awards.
    pub caption: Option<String>,
    pub covers: VisibleCovers,
}

pub struct AlbumWidgets {
    cover: gtk::Picture,
    title: gtk::Label,
    artist: gtk::Label,
    caption: gtk::Label,
}

/// A tile, empty until [`AlbumWidgets::show`] fills it.
pub fn tile() -> (gtk::Box, AlbumWidgets) {
    relm4::view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,
            set_width_request: COVER_SIZE,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Start,
            set_cursor_from_name: Some("pointer"),

            #[name = "cover"]
            gtk::Picture {
                set_size_request: (COVER_SIZE, COVER_SIZE),
                set_content_fit: gtk::ContentFit::Cover,
                set_overflow: gtk::Overflow::Hidden,
                add_css_class: "albuz-cover",
            },
            #[name = "title"]
            gtk::Label {
                set_xalign: 0.0,
                // Lets the label shrink to the tile instead of widening it.
                set_max_width_chars: 1,
                set_ellipsize: pango::EllipsizeMode::End,
                set_margin_top: 6,
                add_css_class: "albuz-album-title",
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
                add_css_class: "albuz-tile-caption",
            },
        }
    }

    let widgets = AlbumWidgets {
        cover,
        title,
        artist,
        caption,
    };
    (root, widgets)
}

impl AlbumWidgets {
    pub fn show(
        &self,
        root: &gtk::Box,
        album: &Album,
        caption: Option<&str>,
        covers: &VisibleCovers,
    ) {
        let title = album.full_title();
        self.title.set_label(&title);
        self.artist.set_label(&album.artist.name);
        self.caption.set_label(caption.unwrap_or_default());
        self.caption.set_visible(caption.is_some());
        let tooltip = [Some(title.as_str()), Some(&album.artist.name), caption]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n");
        root.set_tooltip_text(Some(&tooltip));

        match cover_url(album, self.cover.scale_factor()) {
            Some(url) => covers.show(&self.cover, url),
            None => covers.hide(&self.cover),
        }
    }

    pub fn hide(&self, covers: &VisibleCovers) {
        covers.hide(&self.cover);
    }
}

impl RelmGridItem for AlbumItem {
    type Root = gtk::Box;
    type Widgets = AlbumWidgets;

    fn setup(_item: &gtk::ListItem) -> (gtk::Box, AlbumWidgets) {
        tile()
    }

    fn bind(&mut self, widgets: &mut AlbumWidgets, root: &mut gtk::Box) {
        widgets.show(root, &self.album, self.caption.as_deref(), &self.covers);
    }

    fn unbind(&mut self, widgets: &mut AlbumWidgets, _root: &mut gtk::Box) {
        widgets.hide(&self.covers);
    }
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
