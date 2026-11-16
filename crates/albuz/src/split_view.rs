//! Content with a pane on its right, when there's room for both. When there isn't, the
//! pane takes the whole space, over the content. Plain GTK has no such widget; libadwaita's
//! is `AdwOverlaySplitView`.

use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct SplitView(ObjectSubclass<imp::SplitView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SplitView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SplitView {
    pub fn set_content(&self, content: &impl IsA<gtk::Widget>) {
        let imp = self.imp();
        if let Some(old) = imp.content.replace(Some(content.clone().upcast())) {
            old.unparent();
        }
        // The content comes first, so focus goes through it before the pane.
        content.insert_after(self, None::<&gtk::Widget>);
    }

    pub fn set_pane(&self, pane: &impl IsA<gtk::Widget>) {
        let imp = self.imp();
        if let Some(old) = imp.pane.replace(Some(pane.clone().upcast())) {
            old.unparent();
        }
        pane.set_parent(self);
        pane.set_child_visible(imp.show_pane.get());
    }

    /// The pane's width when it sits next to the content.
    pub fn set_pane_width(&self, width: i32) {
        self.imp().pane_width.set(width);
        self.queue_resize();
    }

    /// The narrowest the content gets next to the pane: below that, the pane takes the
    /// whole space.
    pub fn set_min_content_width(&self, width: i32) {
        self.imp().min_content_width.set(width);
        self.queue_resize();
    }

    pub fn set_show_pane(&self, show: bool) {
        let imp = self.imp();
        if imp.show_pane.replace(show) != show {
            if let Some(pane) = imp.pane.borrow().as_ref() {
                pane.set_child_visible(show);
            }
            self.queue_resize();
        }
    }

    /// Whether there's too little room for the pane next to the content.
    pub fn is_collapsed(&self) -> bool {
        self.imp().collapsed.get()
    }

    /// Call `f` when the view collapses or expands. It's called while GTK lays the window
    /// out, so it must not change any widget, only send a message.
    pub fn connect_collapsed(&self, f: impl Fn(bool) + 'static) {
        self.imp().on_collapsed.replace(Some(Box::new(f)));
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use relm4::gtk::{self, glib, graphene, prelude::*, subclass::prelude::*};

    pub struct SplitView {
        pub content: RefCell<Option<gtk::Widget>>,
        pub pane: RefCell<Option<gtk::Widget>>,
        pub pane_width: Cell<i32>,
        pub min_content_width: Cell<i32>,
        pub show_pane: Cell<bool>,
        pub collapsed: Cell<bool>,
        #[expect(clippy::type_complexity)]
        pub on_collapsed: RefCell<Option<Box<dyn Fn(bool)>>>,
    }

    impl Default for SplitView {
        fn default() -> Self {
            Self {
                content: RefCell::default(),
                pane: RefCell::default(),
                pane_width: Cell::new(360),
                min_content_width: Cell::new(480),
                show_pane: Cell::new(false),
                collapsed: Cell::new(false),
                on_collapsed: RefCell::default(),
            }
        }
    }

    /// How the width is shared.
    enum Layout {
        ContentOnly,
        PaneOnly,
        Side { content_width: i32 },
    }

    impl SplitView {
        fn children(&self) -> (Option<gtk::Widget>, Option<gtk::Widget>) {
            (self.content.borrow().clone(), self.pane.borrow().clone())
        }

        fn is_too_narrow(&self, width: i32) -> bool {
            let content_min = self.content.borrow().as_ref().map_or(0, |content| {
                content.measure(gtk::Orientation::Horizontal, -1).0
            });
            width < self.pane_width.get() + self.min_content_width.get().max(content_min)
        }

        fn layout(&self, width: i32) -> Layout {
            if !self.show_pane.get() || self.pane.borrow().is_none() {
                Layout::ContentOnly
            } else if self.is_too_narrow(width) {
                Layout::PaneOnly
            } else {
                Layout::Side {
                    content_width: width - self.pane_width.get(),
                }
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SplitView {
        const NAME: &'static str = "AlbuzSplitView";
        type Type = super::SplitView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SplitView {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for SplitView {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (content, pane) = self.children();
            let measure = |child: &Option<gtk::Widget>, for_size| {
                child.as_ref().map_or((0, 0), |child| {
                    let (min, natural, _, _) = child.measure(orientation, for_size);
                    (min, natural)
                })
            };
            let show_pane = self.show_pane.get() && pane.is_some();

            if orientation == gtk::Orientation::Horizontal {
                // Either may have the whole width to itself, so the narrowest is what
                // both need; side by side is only what they'd like.
                let (content_min, content_natural) = measure(&content, for_size);
                let (pane_min, _) = measure(&pane, for_size);
                let min = content_min.max(if show_pane { pane_min } else { 0 });
                let natural = if show_pane {
                    content_natural.max(self.min_content_width.get()) + self.pane_width.get()
                } else {
                    content_natural
                };
                return (min, natural.max(min), -1, -1);
            }

            // Whichever shows, the height must fit both, so the minimum doesn't change
            // when the pane comes and goes.
            let (content_min, content_natural) = measure(&content, -1);
            let (pane_min, pane_natural) = if show_pane {
                measure(&pane, -1)
            } else {
                (0, 0)
            };
            let min = content_min.max(pane_min);
            let natural = match (for_size, self.layout(for_size)) {
                (-1, _) => content_natural.max(pane_natural),
                (_, Layout::ContentOnly) => measure(&content, for_size).1,
                (_, Layout::PaneOnly) => measure(&pane, for_size).1,
                (_, Layout::Side { content_width }) => measure(&content, content_width)
                    .1
                    .max(measure(&pane, self.pane_width.get()).1),
            };
            (min, natural.max(min), -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let collapsed = self.is_too_narrow(width);
            if self.collapsed.replace(collapsed) != collapsed
                && let Some(on_collapsed) = self.on_collapsed.borrow().as_ref()
            {
                on_collapsed(collapsed);
            }

            let (content, pane) = self.children();
            let layout = self.layout(width);
            if let Some(content) = &content {
                content.set_child_visible(!matches!(layout, Layout::PaneOnly));
            }
            match layout {
                Layout::ContentOnly => {
                    if let Some(content) = content {
                        content.measure(gtk::Orientation::Horizontal, -1);
                        content.size_allocate(&gtk::Allocation::new(0, 0, width, height), baseline);
                    }
                }
                Layout::PaneOnly => {
                    if let Some(pane) = pane {
                        pane.measure(gtk::Orientation::Horizontal, -1);
                        pane.size_allocate(&gtk::Allocation::new(0, 0, width, height), baseline);
                    }
                }
                Layout::Side { content_width } => {
                    if let Some(content) = content {
                        content.measure(gtk::Orientation::Horizontal, -1);
                        content.size_allocate(
                            &gtk::Allocation::new(0, 0, content_width, height),
                            baseline,
                        );
                    }
                    if let Some(pane) = pane {
                        pane.measure(gtk::Orientation::Horizontal, -1);
                        pane.size_allocate(
                            &gtk::Allocation::new(content_width, 0, width - content_width, height),
                            baseline,
                        );
                    }
                }
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);
            // A hairline between the content and the pane, when side by side.
            let obj = self.obj();
            if let Layout::Side { content_width } = self.layout(obj.width()) {
                let color = obj.color().with_alpha(0.15);
                #[expect(clippy::cast_precision_loss, reason = "widths are small")]
                let bounds =
                    graphene::Rect::new(content_width as f32, 0.0, 1.0, obj.height() as f32);
                snapshot.append_color(&color, &bounds);
            }
        }
    }
}
