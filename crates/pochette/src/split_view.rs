//! Content with a pane on its right, when there's room for both. When there isn't, the
//! pane takes the whole space, over the content. Plain GTK has no such widget; libadwaita's
//! is `AdwOverlaySplitView`.
//!
//! Dragging the line between them resizes the pane.

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
        // Under the handle, which must stay on top to catch the pointer.
        pane.insert_before(self, imp.handle.get());
        pane.set_child_visible(imp.show_pane.get());
    }

    /// How wide the pane would like to be next to the content. It gets narrower when the
    /// window does, down to its minimum, before taking the whole space.
    pub fn set_pane_width(&self, width: i32) {
        self.imp()
            .pane_width
            .set(width.clamp(0, imp::MAX_PANE_WIDTH));
        self.queue_resize();
    }

    /// The narrowest the content gets next to the pane.
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

    /// Call `f` with the pane's new width once the user is done resizing it.
    pub fn connect_pane_resized(&self, f: impl Fn(i32) + 'static) {
        self.imp().on_pane_resized.replace(Some(Box::new(f)));
    }
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use relm4::gtk::{self, glib, graphene, prelude::*, subclass::prelude::*};

    /// The narrowest the pane gets, unless its content needs more.
    const MIN_PANE_WIDTH: i32 = 300;
    pub const MAX_PANE_WIDTH: i32 = 640;
    /// How wide the strip that catches the pointer is, centered on the line.
    const HANDLE_WIDTH: i32 = 8;

    pub struct SplitView {
        pub content: RefCell<Option<gtk::Widget>>,
        pub pane: RefCell<Option<gtk::Widget>>,
        /// The strip over the line between content and pane, that shows the resize cursor.
        pub handle: OnceCell<gtk::Widget>,
        pub pane_width: Cell<i32>,
        pub min_content_width: Cell<i32>,
        pub show_pane: Cell<bool>,
        pub collapsed: Cell<bool>,
        /// The pane's width when the drag started.
        drag_start: Cell<i32>,
        #[expect(clippy::type_complexity)]
        pub on_collapsed: RefCell<Option<Box<dyn Fn(bool)>>>,
        #[expect(clippy::type_complexity)]
        pub on_pane_resized: RefCell<Option<Box<dyn Fn(i32)>>>,
    }

    impl Default for SplitView {
        fn default() -> Self {
            Self {
                content: RefCell::default(),
                pane: RefCell::default(),
                handle: OnceCell::new(),
                pane_width: Cell::new(360),
                min_content_width: Cell::new(480),
                show_pane: Cell::new(false),
                collapsed: Cell::new(false),
                drag_start: Cell::new(0),
                on_collapsed: RefCell::default(),
                on_pane_resized: RefCell::default(),
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

        /// The narrowest the content gets next to the pane.
        fn content_needs(&self) -> i32 {
            let content_min = self.content.borrow().as_ref().map_or(0, |content| {
                content.measure(gtk::Orientation::Horizontal, -1).0
            });
            self.min_content_width.get().max(content_min)
        }

        /// The narrowest the pane gets next to the content.
        fn pane_needs(&self) -> i32 {
            let pane_min = self
                .pane
                .borrow()
                .as_ref()
                .map_or(0, |pane| pane.measure(gtk::Orientation::Horizontal, -1).0);
            MIN_PANE_WIDTH.max(pane_min)
        }

        fn is_too_narrow(&self, width: i32) -> bool {
            width - self.content_needs() < self.pane_needs()
        }

        /// The pane's width next to the content, in `width`.
        fn side_pane_width(&self, width: i32) -> i32 {
            self.pane_width
                .get()
                .min(width - self.content_needs())
                .max(self.pane_needs())
        }

        fn layout(&self, width: i32) -> Layout {
            if !self.show_pane.get() || self.pane.borrow().is_none() {
                Layout::ContentOnly
            } else if self.is_too_narrow(width) {
                Layout::PaneOnly
            } else {
                Layout::Side {
                    content_width: width - self.side_pane_width(width),
                }
            }
        }

        fn drag_began(&self, gesture: &gtk::GestureDrag, x: f64) {
            let obj = self.obj();
            let Layout::Side { content_width } = self.layout(obj.width()) else {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            };
            if (x - f64::from(content_width)).abs() > f64::from(HANDLE_WIDTH) {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            self.drag_start.set(obj.width() - content_width);
        }

        fn dragged(&self, offset_x: f64) {
            let obj = self.obj();
            // Dragging left widens the pane.
            #[expect(clippy::cast_possible_truncation, reason = "a drag is short")]
            let wanted = self.drag_start.get() - offset_x.round() as i32;
            let widest = (obj.width() - self.content_needs()).min(MAX_PANE_WIDTH);
            let width = wanted.min(widest).max(self.pane_needs());
            if self.pane_width.replace(width) != width {
                obj.queue_resize();
            }
        }

        fn drag_ended(&self) {
            if let Some(on_pane_resized) = self.on_pane_resized.borrow().as_ref() {
                on_pane_resized(self.pane_width.get());
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SplitView {
        const NAME: &'static str = "PochetteSplitView";
        type Type = super::SplitView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SplitView {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let handle = gtk::Box::new(gtk::Orientation::Vertical, 0);
            handle.set_cursor_from_name(Some("col-resize"));
            handle.set_parent(&*obj);
            let _ = self.handle.set(handle.upcast());

            // On the view rather than the handle: the handle moves along with the drag,
            // which would throw its offsets off.
            let drag = gtk::GestureDrag::new();
            drag.connect_drag_begin(glib::clone!(
                #[weak]
                obj,
                move |gesture, x, _| obj.imp().drag_began(gesture, x)
            ));
            drag.connect_drag_update(glib::clone!(
                #[weak]
                obj,
                move |_, offset_x, _| obj.imp().dragged(offset_x)
            ));
            drag.connect_drag_end(glib::clone!(
                #[weak]
                obj,
                move |_, _, _| obj.imp().drag_ended()
            ));
            obj.add_controller(drag);
        }

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
                    .max(measure(&pane, for_size - content_width).1),
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
            let allocate = |child: Option<gtk::Widget>, x, width| {
                if let Some(child) = child {
                    child.measure(gtk::Orientation::Horizontal, -1);
                    child.size_allocate(&gtk::Allocation::new(x, 0, width, height), baseline);
                }
            };
            let handle = self.handle.get().cloned();
            if let Some(handle) = &handle {
                handle.set_child_visible(matches!(layout, Layout::Side { .. }));
            }
            match layout {
                Layout::ContentOnly => allocate(content, 0, width),
                Layout::PaneOnly => allocate(pane, 0, width),
                Layout::Side { content_width } => {
                    allocate(content, 0, content_width);
                    allocate(pane, content_width, width - content_width);
                    allocate(handle, content_width - HANDLE_WIDTH / 2, HANDLE_WIDTH);
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
