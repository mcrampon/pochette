//! A container that caps its child's width and centers it, so text stays readable in a
//! wide window. Plain GTK has no such widget; libadwaita's is `AdwClamp`.

use relm4::{
    ContainerChild, RelmSetChildExt,
    gtk::{self, glib, prelude::*, subclass::prelude::*},
};

glib::wrapper! {
    pub struct Clamp(ObjectSubclass<imp::Clamp>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Clamp {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Clamp {
    /// The widest the child gets, in logical pixels.
    pub fn set_max_width(&self, max_width: i32) {
        self.imp().max_width.set(max_width);
        self.queue_resize();
    }

    pub fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        if let Some(old) = self.first_child() {
            old.unparent();
        }
        if let Some(child) = child {
            child.set_parent(self);
        }
    }
}

impl ContainerChild for Clamp {
    type Child = gtk::Widget;
}

impl RelmSetChildExt for Clamp {
    fn container_set_child(&self, widget: Option<&impl AsRef<gtk::Widget>>) {
        self.set_child(widget.map(AsRef::as_ref));
    }

    fn container_get_child(&self) -> Option<gtk::Widget> {
        self.first_child()
    }
}

mod imp {
    use std::cell::Cell;

    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    pub struct Clamp {
        pub max_width: Cell<i32>,
    }

    impl Default for Clamp {
        fn default() -> Self {
            Self {
                max_width: Cell::new(i32::MAX),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Clamp {
        const NAME: &'static str = "AlbuzClamp";
        type Type = super::Clamp;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Clamp {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Clamp {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else {
                return (0, 0, -1, -1);
            };
            let max_width = self.max_width.get();
            if orientation == gtk::Orientation::Horizontal {
                let (min, natural, _, _) = child.measure(orientation, for_size);
                (min, natural.min(max_width).max(min), -1, -1)
            } else {
                // The child gets at most the maximum width, so it's as tall as at that width.
                let width = if for_size < 0 {
                    -1
                } else {
                    for_size.min(max_width)
                };
                let (min, natural, _, _) = child.measure(orientation, width);
                (min, natural, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.obj().first_child() else {
                return;
            };
            let (min_width, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            let child_width = width.min(self.max_width.get()).max(min_width);
            let x = (width - child_width).max(0) / 2;
            child.size_allocate(&gtk::Allocation::new(x, 0, child_width, height), baseline);
        }
    }
}
