//! A cover blurred to fill its widget, as a background: behind the lyrics, in Now Playing.

use relm4::gtk::{self, gdk, glib, prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct BlurredCover(ObjectSubclass<imp::BlurredCover>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for BlurredCover {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl BlurredCover {
    pub fn set_paintable(&self, paintable: Option<&gdk::Paintable>) {
        self.imp().paintable.replace(paintable.cloned());
        self.queue_draw();
    }
}

mod imp {
    use std::cell::RefCell;

    use relm4::gtk::{self, gdk, glib, graphene, prelude::*, subclass::prelude::*};

    /// How much the cover is blurred, in logical pixels.
    const BLUR: f32 = 40.0;

    #[derive(Default)]
    pub struct BlurredCover {
        pub paintable: RefCell<Option<gdk::Paintable>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BlurredCover {
        const NAME: &'static str = "PochetteBlurredCover";
        type Type = super::BlurredCover;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for BlurredCover {}

    impl WidgetImpl for BlurredCover {
        /// Any size: it takes the size of what it's behind.
        fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(paintable) = self.paintable.borrow().clone() else {
                return;
            };
            let obj = self.obj();
            #[expect(
                clippy::cast_precision_loss,
                reason = "widget and cover sizes are small"
            )]
            let ((width, height), (cover_width, cover_height)) = (
                (obj.width() as f32, obj.height() as f32),
                (
                    paintable.intrinsic_width().max(1) as f32,
                    paintable.intrinsic_height().max(1) as f32,
                ),
            );
            // Covering the widget and a margin as wide as the blur, cut off: blurring only
            // what's inside would fade the edges into nothing.
            let scale =
                ((width + 2.0 * BLUR) / cover_width).max((height + 2.0 * BLUR) / cover_height);
            let (drawn_width, drawn_height) = (cover_width * scale, cover_height * scale);

            snapshot.push_clip(&graphene::Rect::new(0.0, 0.0, width, height));
            snapshot.push_blur(f64::from(BLUR));
            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                (width - drawn_width) / 2.0,
                (height - drawn_height) / 2.0,
            ));
            paintable.snapshot(snapshot, f64::from(drawn_width), f64::from(drawn_height));
            snapshot.restore();
            snapshot.pop();
            snapshot.pop();
        }
    }
}
