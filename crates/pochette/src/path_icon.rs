//! Icons drawn from a path rather than taken from the icon theme, which may have none: the
//! outline of a shape on a 16 × 16 grid, in the text color, scaled to the widget.

use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

/// Two arrows, one each way: swap.
pub const SWAP: &str = "M 2.5 5 H 13 M 10 2 L 13 5 L 10 8 M 13.5 11 H 3 M 6 8 L 3 11 L 6 14";

/// An arrow pointing up: ascending.
pub const UP: &str = "M 8 13 V 3 M 4 7 L 8 3 L 12 7";

/// An arrow pointing down: descending.
pub const DOWN: &str = "M 8 3 V 13 M 4 9 L 8 13 L 12 9";

glib::wrapper! {
    pub struct PathIcon(ObjectSubclass<imp::PathIcon>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PathIcon {
    /// The icon of `path`, in SVG path syntax on a 16 × 16 grid.
    pub fn new(path: &'static str) -> Self {
        let icon: Self = glib::Object::new();
        icon.imp().path.set(path);
        // At the size of the heart's icon, in the middle of a button rather than as big.
        icon.set_size_request(20, 20);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        icon
    }

    pub fn set_path(&self, path: &'static str) {
        if self.imp().path.replace(path) != path {
            self.queue_draw();
        }
    }
}

mod imp {
    use std::cell::Cell;

    use relm4::gtk::{self, glib, graphene, gsk, prelude::*, subclass::prelude::*};

    const SIZE: i32 = 16;

    #[derive(Default)]
    pub struct PathIcon {
        pub path: Cell<&'static str>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PathIcon {
        const NAME: &'static str = "PochettePathIcon";
        type Type = super::PathIcon;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PathIcon {}

    impl WidgetImpl for PathIcon {
        fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            (SIZE, SIZE, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
            let (width, height) = (obj.width() as f32, obj.height() as f32);
            let side = width.min(height);
            let Ok(path) = gsk::Path::parse(self.path.get()) else {
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
            let stroke = gsk::Stroke::new(1.5);
            stroke.set_line_cap(gsk::LineCap::Round);
            stroke.set_line_join(gsk::LineJoin::Round);
            snapshot.append_stroke(&path, &stroke, &obj.color());
            snapshot.restore();
        }
    }
}
