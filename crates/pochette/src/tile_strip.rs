//! A row of tiles side by side that shows as many as fit, whole, and hides the others,
//! for rows of albums like those of the Discover tab: what doesn't fit is in "See All".

use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct TileStrip(ObjectSubclass<imp::TileStrip>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for TileStrip {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl TileStrip {
    pub fn append(&self, tile: &impl IsA<gtk::Widget>) {
        tile.set_parent(self);
    }

    /// Call `f` with whether some tiles are hidden, each time that changes.
    pub fn connect_overflow(&self, f: impl Fn(bool) + 'static) {
        self.imp().on_overflow.replace(Some(Box::new(f)));
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    type OverflowHandler = Box<dyn Fn(bool)>;

    #[derive(Default)]
    pub struct TileStrip {
        pub on_overflow: RefCell<Option<OverflowHandler>>,
        /// Whether some tiles were hidden at the last allocation.
        overflowing: Cell<Option<bool>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TileStrip {
        const NAME: &'static str = "PochetteTileStrip";
        type Type = super::TileStrip;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for TileStrip {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for TileStrip {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        /// As narrow as one tile and as wide as all of them; as tall as the tallest.
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (mut minimum, mut natural) = (0, 0);
            for (index, child) in children(&self.obj()).enumerate() {
                let (child_minimum, child_natural, _, _) = child.measure(orientation, -1);
                if orientation == gtk::Orientation::Horizontal {
                    if index == 0 {
                        minimum = child_minimum;
                    }
                    natural += child_natural;
                } else {
                    minimum = minimum.max(child_minimum);
                    natural = natural.max(child_natural);
                }
            }
            (minimum, natural, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let mut x = 0;
            let mut fits = true;
            for child in children(&self.obj()) {
                let (_, child_width, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                // Once one tile is cut off, so are all those after it.
                fits = fits && x + child_width <= width;
                if child.is_child_visible() != fits {
                    child.set_child_visible(fits);
                }
                if fits {
                    child.size_allocate(&gtk::Allocation::new(x, 0, child_width, height), -1);
                    x += child_width;
                }
            }
            self.report_overflow(!fits);
        }
    }

    impl TileStrip {
        /// Tell [`super::TileStrip::connect_overflow`] whether some tiles are hidden, if that
        /// changed: once the layout is done, as what it does may change sizes.
        fn report_overflow(&self, overflowing: bool) {
            if self.overflowing.replace(Some(overflowing)) == Some(overflowing) {
                return;
            }
            let strip = self.obj().downgrade();
            glib::idle_add_local_once(move || {
                if let Some(strip) = strip.upgrade()
                    && let Some(f) = strip.imp().on_overflow.borrow().as_ref()
                {
                    f(overflowing);
                }
            });
        }
    }

    fn children(strip: &super::TileStrip) -> impl Iterator<Item = gtk::Widget> {
        std::iter::successors(strip.first_child(), gtk::Widget::next_sibling)
    }
}
