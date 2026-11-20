//! An icon and a label for a button in the header bar, whose label shows only when the header
//! bar has room for it: when it doesn't, the button is an icon, rather than the other widgets
//! getting squeezed or the title moving off center.
//!
//! A button can't give back width its header bar hands it, so [`HeaderFit`], around the
//! header bar, decides: it shows the label when the header bar is as wide as it would like to
//! be with it.

use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

/// Between the icon and the label, and after the label, to balance the button's padding,
/// which is that of an icon.
const SPACING: i32 = 6;
const END_PADDING: i32 = 4;

glib::wrapper! {
    pub struct IconLabel(ObjectSubclass<imp::IconLabel>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl IconLabel {
    pub fn new(icon_name: &str, label: &str) -> Self {
        let this: Self = glib::Object::new();
        gtk::Image::from_icon_name(icon_name).set_parent(&this);
        gtk::Label::new(Some(label)).set_parent(&this);
        this
    }

    fn parts(&self) -> (gtk::Widget, gtk::Widget) {
        let image = self.first_child().expect("an icon");
        let label = image.next_sibling().expect("a label");
        (image, label)
    }

    fn label_shown(&self) -> bool {
        self.parts().1.is_visible()
    }

    fn set_label_shown(&self, shown: bool) {
        self.parts().1.set_visible(shown);
    }

    /// How much wider the label makes it.
    fn label_width(&self) -> i32 {
        let (_, label_width, _, _) = self.parts().1.measure(gtk::Orientation::Horizontal, -1);
        SPACING + label_width + END_PADDING
    }
}

glib::wrapper! {
    pub struct HeaderFit(ObjectSubclass<imp::HeaderFit>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for HeaderFit {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl HeaderFit {
    pub fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        while let Some(old) = self.first_child() {
            old.unparent();
        }
        if let Some(child) = child {
            child.set_parent(self);
        }
    }

    /// Show `label`'s label only when there's room for it.
    pub fn set_label(&self, label: &IconLabel) {
        self.imp().label.replace(Some(label.clone()));
        self.queue_resize();
    }
}

mod imp {
    use std::cell::RefCell;

    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    use super::{END_PADDING, SPACING};

    #[derive(Default)]
    pub struct IconLabel;

    #[glib::object_subclass]
    impl ObjectSubclass for IconLabel {
        const NAME: &'static str = "PochetteIconLabel";
        type Type = super::IconLabel;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for IconLabel {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for IconLabel {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (image, label) = self.obj().parts();
            let (image_minimum, image_natural, _, _) = image.measure(orientation, -1);
            if !label.is_visible() {
                return (image_minimum, image_natural, -1, -1);
            }
            let (label_minimum, label_natural, _, _) = label.measure(orientation, -1);
            if orientation == gtk::Orientation::Horizontal {
                let width = image_natural + SPACING + label_natural + END_PADDING;
                (width, width, -1, -1)
            } else {
                (
                    image_minimum.max(label_minimum),
                    image_natural.max(label_natural),
                    -1,
                    -1,
                )
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let (image, label) = self.obj().parts();
            let (_, image_width, _, _) = image.measure(gtk::Orientation::Horizontal, -1);
            if label.is_visible() {
                let (_, label_width, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                image.size_allocate(&gtk::Allocation::new(0, 0, image_width, height), -1);
                label.size_allocate(
                    &gtk::Allocation::new(image_width + SPACING, 0, label_width, height),
                    -1,
                );
            } else {
                let x = (width - image_width) / 2;
                image.size_allocate(&gtk::Allocation::new(x, 0, image_width, height), -1);
            }
        }
    }

    #[derive(Default)]
    pub struct HeaderFit {
        pub label: RefCell<Option<super::IconLabel>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HeaderFit {
        const NAME: &'static str = "PochetteHeaderFit";
        type Type = super::HeaderFit;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for HeaderFit {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for HeaderFit {
        /// As narrow as the header bar without the label, and as wide as it would like to be
        /// with it.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else {
                return (0, 0, -1, -1);
            };
            let (minimum, natural, _, _) = child.measure(orientation, for_size);
            if orientation == gtk::Orientation::Vertical {
                return (minimum, natural, -1, -1);
            }
            let (shown, label_width) = self.label_state();
            if shown {
                (minimum - label_width, natural, -1, -1)
            } else {
                (minimum, natural + label_width, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.obj().first_child() else {
                return;
            };
            let (minimum, natural, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            let (shown, label_width) = self.label_state();
            let wanted = if shown {
                natural
            } else {
                natural + label_width
            };
            let show = width >= wanted;
            if show != shown {
                // Not while laying out: showing or hiding it changes the sizes.
                let label = self.label.borrow().clone();
                glib::idle_add_local_once(move || {
                    if let Some(label) = label {
                        label.set_label_shown(show);
                    }
                });
            }
            // Until the label goes, the header bar can't be narrower than with it.
            child.size_allocate(
                &gtk::Allocation::new(0, 0, width.max(minimum), height),
                baseline,
            );
        }
    }

    impl HeaderFit {
        /// Whether the label shows, and how much wider it makes the header bar; none when
        /// the button isn't there.
        fn label_state(&self) -> (bool, i32) {
            match self.label.borrow().as_ref() {
                Some(label) if label.parent().is_some_and(|button| button.is_visible()) => {
                    (label.label_shown(), label.label_width())
                }
                _ => (false, 0),
            }
        }
    }
}
