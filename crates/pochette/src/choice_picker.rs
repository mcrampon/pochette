//! A button that picks one of a few choices, such as a year: its label is the choice
//! picked, or it's an icon, and its popover lists them all, a check at the end of the one
//! picked.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use relm4::gtk::{self, prelude::*};

/// Cheap to clone: every clone is the same picker.
#[derive(Clone)]
pub struct ChoicePicker(Rc<Inner>);

struct Inner {
    button: gtk::MenuButton,
    popover: gtk::Popover,
    list: gtk::Box,
    /// The choices' rows, and their checks.
    rows: RefCell<Vec<(String, gtk::Image)>>,
    /// Whether the button shows the choice picked, rather than an icon.
    labelled: Cell<bool>,
    #[expect(clippy::type_complexity)]
    on_pick: RefCell<Option<Box<dyn Fn(usize)>>>,
}

impl ChoicePicker {
    pub fn new(tooltip: &str) -> Self {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let popover = gtk::Popover::builder().child(&list).build();
        let button = gtk::MenuButton::builder()
            .always_show_arrow(true)
            .tooltip_text(tooltip)
            .popover(&popover)
            .valign(gtk::Align::Center)
            .build();
        Self(Rc::new(Inner {
            button,
            popover,
            list,
            rows: RefCell::default(),
            labelled: Cell::new(true),
            on_pick: RefCell::default(),
        }))
    }

    /// A picker showing `icon` rather than the choice picked, which its tooltip tells.
    pub fn with_icon(icon: &str, tooltip: &str) -> Self {
        let picker = Self::new(tooltip);
        let button = &picker.0.button;
        button.set_always_show_arrow(false);
        button.set_icon_name(icon);
        button.set_has_frame(false);
        picker.0.labelled.set(false);
        picker
    }

    pub fn widget(&self) -> &gtk::MenuButton {
        &self.0.button
    }

    /// Offer `names`, with the one at `picked` picked.
    pub fn set_choices(&self, names: &[String], picked: usize) {
        let inner = &self.0;
        while let Some(child) = inner.list.first_child() {
            inner.list.remove(&child);
        }
        let mut rows = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            content.append(
                &gtk::Label::builder()
                    .label(name)
                    .xalign(0.0)
                    .hexpand(true)
                    // Long names, as some audio outputs have, cut short.
                    .max_width_chars(40)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .tooltip_text(name)
                    .build(),
            );
            let check = gtk::Image::builder()
                .icon_name("object-select-symbolic")
                .margin_start(24)
                .build();
            content.append(&check);
            let button = gtk::Button::builder()
                .child(&content)
                .css_classes(["flat"])
                .build();
            let weak = Rc::downgrade(&self.0);
            button.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    let this = Self(inner);
                    this.0.popover.popdown();
                    this.select(index);
                    if let Some(on_pick) = this.0.on_pick.borrow().as_ref() {
                        on_pick(index);
                    }
                }
            });
            inner.list.append(&button);
            rows.push((name.clone(), check));
        }
        inner.rows.replace(rows);
        self.select(picked);
    }

    /// Show the choice at `picked` as picked, without it counting as the user's pick.
    pub fn select(&self, picked: usize) {
        let rows = self.0.rows.borrow();
        for (index, (name, check)) in rows.iter().enumerate() {
            // Hidden rather than gone, so the rows stay as wide whichever is picked.
            check.set_opacity(if index == picked { 1.0 } else { 0.0 });
            if index == picked && self.0.labelled.get() {
                self.0.button.set_label(name);
            }
        }
    }

    /// Call `f` as the popover is about to show, to offer the choices of the moment.
    pub fn connect_opening(&self, f: impl Fn() + 'static) {
        self.0.button.set_create_popup_func(move |_| f());
    }

    /// Call `f` with the position of the choice the user picks.
    pub fn connect_picked(&self, f: impl Fn(usize) + 'static) {
        self.0.on_pick.replace(Some(Box::new(f)));
    }
}
