//! The albums the user saved to their favorites, which is their library, and the heart
//! that adds or removes one wherever its cover shows.

use std::{
    cell::RefCell,
    collections::HashSet,
    rc::{Rc, Weak},
};

use qobuz::{Client, models::Album};
use relm4::gtk::{self, prelude::*};

use crate::i18n::tr;

/// Which albums are favorites. Cheap to clone: every clone is the same.
#[derive(Clone, Default)]
pub struct Favorites(Rc<Inner>);

#[derive(Default)]
struct Inner {
    client: RefCell<Option<Client>>,
    ids: RefCell<HashSet<String>>,
    /// The hearts to update when an album is added or removed.
    hearts: RefCell<Vec<Weak<HeartInner>>>,
    #[expect(clippy::type_complexity)]
    listeners: RefCell<Vec<Box<dyn Fn(&Album, bool)>>>,
}

impl Favorites {
    /// Start over with the favorites of the user `client` signs in as, or none.
    pub fn reset(&self, client: Option<Client>, ids: impl IntoIterator<Item = String>) {
        self.0.client.replace(client);
        self.0.ids.replace(ids.into_iter().collect());
        self.refresh_hearts();
    }

    pub fn contains(&self, album_id: &str) -> bool {
        self.0.ids.borrow().contains(album_id)
    }

    /// Call `f` with each album added (`true`) or removed (`false`).
    pub fn connect_changed(&self, f: impl Fn(&Album, bool) + 'static) {
        self.0.listeners.borrow_mut().push(Box::new(f));
    }

    /// Add `album` to the favorites, or remove it, and tell Qobuz. Takes it back if Qobuz
    /// refuses.
    pub fn toggle(&self, album: &Rc<Album>) {
        let Some(client) = self.0.client.borrow().clone() else {
            return;
        };
        let add = !self.contains(&album.id);
        self.set(album, add);

        let (favorites, album) = (self.clone(), album.clone());
        relm4::spawn_local(async move {
            let id = album.id.clone();
            let saved = relm4::spawn(async move {
                if add {
                    client.add_favorite_album(&id).await
                } else {
                    client.remove_favorite_album(&id).await
                }
            })
            .await;
            match saved {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::warn!("cannot update the favorites: {err}");
                    favorites.set(&album, !add);
                }
                Err(err) => tracing::warn!("cannot update the favorites: {err}"),
            }
        });
    }

    fn set(&self, album: &Album, favorite: bool) {
        {
            let mut ids = self.0.ids.borrow_mut();
            if favorite {
                ids.insert(album.id.clone());
            } else {
                ids.remove(&album.id);
            }
        }
        self.refresh_hearts();
        for listener in self.0.listeners.borrow().iter() {
            listener(album, favorite);
        }
    }

    fn register(&self, heart: &Rc<HeartInner>) {
        let mut hearts = self.0.hearts.borrow_mut();
        hearts.retain(|heart| heart.strong_count() > 0);
        hearts.push(Rc::downgrade(heart));
    }

    fn refresh_hearts(&self) {
        let hearts: Vec<Rc<HeartInner>> = self
            .0
            .hearts
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for heart in hearts {
            Heart(heart).refresh();
        }
    }
}

/// A round button over a cover, that shows on hover: a full heart for a favorite, an
/// empty one otherwise. Clicking it adds or removes the album.
#[derive(Clone)]
pub struct Heart(Rc<HeartInner>);

struct HeartInner {
    button: gtk::Button,
    icon: HeartIcon,
    album: RefCell<Option<Rc<Album>>>,
    favorites: RefCell<Option<Favorites>>,
}

impl Heart {
    pub fn new() -> Self {
        let icon = HeartIcon::default();
        icon.set_size_request(20, 20);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        let button = gtk::Button::builder()
            .child(&icon)
            .css_classes(["pochette-heart", "circular"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::End)
            .margin_end(8)
            .margin_bottom(8)
            .visible(false)
            .build();
        let heart = Self(Rc::new(HeartInner {
            button,
            icon,
            album: RefCell::default(),
            favorites: RefCell::default(),
        }));
        let weak = Rc::downgrade(&heart.0);
        heart.0.button.connect_clicked(move |_| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let album = inner.album.borrow().clone();
            let favorites = inner.favorites.borrow().clone();
            if let (Some(album), Some(favorites)) = (album, favorites) {
                favorites.toggle(&album);
            }
        });
        heart
    }

    pub fn widget(&self) -> &gtk::Button {
        &self.0.button
    }

    /// Be the heart of `album`, or of none.
    pub fn show(&self, album: Option<Rc<Album>>, favorites: &Favorites) {
        let first = self.0.favorites.borrow().is_none();
        if first {
            self.0.favorites.replace(Some(favorites.clone()));
            favorites.register(&self.0);
        }
        self.0.album.replace(album);
        self.refresh();
    }

    /// Be the heart of no album, as the tile was recycled.
    pub fn clear(&self) {
        self.0.album.replace(None);
        self.refresh();
    }

    fn refresh(&self) {
        let inner = &self.0;
        let album = inner.album.borrow();
        let favorites = inner.favorites.borrow();
        let (Some(album), Some(favorites)) = (album.as_ref(), favorites.as_ref()) else {
            inner.button.set_visible(false);
            return;
        };
        let favorite = favorites.contains(&album.id);
        inner.icon.set_filled(favorite);
        inner.button.set_visible(true);
        // No tooltip, which got in the way over covers, but a name for screen readers.
        inner
            .button
            .update_property(&[gtk::accessible::Property::Label(&if favorite {
                tr!("favorite-remove")
            } else {
                tr!("favorite-add")
            })]);
        if favorite {
            inner.button.add_css_class("pochette-favorite");
        } else {
            inner.button.remove_css_class("pochette-favorite");
        }
    }
}

/// `child`, here a cover, with `heart` over it.
pub fn with_heart(child: &impl IsA<gtk::Widget>, heart: &Heart) -> gtk::Overlay {
    let overlay = gtk::Overlay::builder()
        .child(child)
        .css_classes(["pochette-cover-hover"])
        .build();
    overlay.add_overlay(heart.widget());
    overlay
}

pub use icon::HeartIcon;

/// The heart, drawn rather than taken from the icon theme, which may have none: filled or
/// outlined, in the text color.
mod icon {
    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    glib::wrapper! {
        pub struct HeartIcon(ObjectSubclass<imp::HeartIcon>)
            @extends gtk::Widget,
            @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl Default for HeartIcon {
        fn default() -> Self {
            glib::Object::new()
        }
    }

    impl HeartIcon {
        pub fn set_filled(&self, filled: bool) {
            if self.imp().filled.replace(filled) != filled {
                self.queue_draw();
            }
        }
    }

    mod imp {
        use std::cell::Cell;

        use relm4::gtk::{self, glib, graphene, gsk, prelude::*, subclass::prelude::*};

        /// A heart in a 16 × 16 box.
        const HEART: &str = "M 8 14 C 8 14 1.75 10 1.75 5.75 C 1.75 3.6 3.35 2 5.25 2 \
             C 6.4 2 7.4 2.6 8 3.5 C 8.6 2.6 9.6 2 10.75 2 C 12.65 2 14.25 3.6 14.25 5.75 \
             C 14.25 10 8 14 8 14 Z";
        const SIZE: i32 = 16;

        #[derive(Default)]
        pub struct HeartIcon {
            pub filled: Cell<bool>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for HeartIcon {
            const NAME: &'static str = "PochetteHeartIcon";
            type Type = super::HeartIcon;
            type ParentType = gtk::Widget;
        }

        impl ObjectImpl for HeartIcon {}

        impl WidgetImpl for HeartIcon {
            fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
                (SIZE, SIZE, -1, -1)
            }

            fn snapshot(&self, snapshot: &gtk::Snapshot) {
                let obj = self.obj();
                #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
                let (width, height) = (obj.width() as f32, obj.height() as f32);
                let side = width.min(height);
                let Ok(path) = gsk::Path::parse(HEART) else {
                    return;
                };
                snapshot.save();
                snapshot.translate(&graphene::Point::new(
                    (width - side) / 2.0,
                    (height - side) / 2.0,
                ));
                #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
                let scale = side / SIZE as f32;
                snapshot.scale(scale, scale);
                let color = obj.color();
                if self.filled.get() {
                    snapshot.append_fill(&path, gsk::FillRule::Winding, &color);
                } else {
                    snapshot.append_stroke(&path, &gsk::Stroke::new(1.5), &color);
                }
                snapshot.restore();
            }
        }
    }
}
