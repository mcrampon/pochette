//! A button that picks any number of choices, such as genres or decades, from a
//! checklist.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use relm4::gtk::{self, prelude::*};

/// Cheap to clone: every clone is the same picker.
#[derive(Clone)]
pub struct Checklist(Rc<Inner>);

struct Inner {
    /// The button, and the count of choices checked floating over its corner.
    overlay: gtk::Overlay,
    button: gtk::MenuButton,
    /// How many choices are checked, when some are.
    count: gtk::Label,
    list: gtk::Box,
    choices: RefCell<Vec<(String, gtk::CheckButton)>>,
    /// Set while the picker changes its own checks, which isn't the user picking.
    quiet: Cell<bool>,
    /// What the choices are, in the plural: "Genres".
    what: String,
    #[expect(clippy::type_complexity)]
    on_change: RefCell<Option<Box<dyn Fn(Vec<usize>)>>>,
}

impl Checklist {
    /// A checklist of `what`, in the plural: "Genres".
    pub fn new(what: &str) -> Self {
        let all_label = format!("All {what}");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .max_content_height(420)
            .child(&list)
            .build();
        let all = gtk::Button::builder()
            .label(&all_label)
            .css_classes(["flat"])
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.append(&all);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&scroller);
        let popover = gtk::Popover::builder().child(&content).build();
        let button = gtk::MenuButton::builder()
            .label(what)
            .always_show_arrow(true)
            .tooltip_text(what)
            .popover(&popover)
            .build();
        let count = gtk::Label::builder()
            .css_classes(["pochette-count"])
            .can_target(false)
            .visible(false)
            .build();
        let overlay = gtk::Overlay::builder().child(&button).build();
        overlay.add_overlay(&count);
        // Over the button's top left corner, sticking out of it only a little so it stays
        // clear of what's around.
        overlay.connect_get_child_position(|_, child| {
            let (_, natural) = child.preferred_size();
            let (width, height) = (natural.width(), natural.height());
            Some(gtk::gdk::Rectangle::new(-2, -height / 2 + 6, width, height))
        });

        let picker = Self(Rc::new(Inner {
            overlay,
            button,
            count,
            list,
            choices: RefCell::default(),
            quiet: Cell::new(false),
            what: what.to_owned(),
            on_change: RefCell::default(),
        }));
        let weak = Rc::downgrade(&picker.0);
        all.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                Self(inner).clear();
            }
        });
        picker
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.0.overlay
    }

    /// Offer `names`, with those at `selected` checked.
    pub fn set_choices(&self, names: &[String], selected: &[usize]) {
        let inner = &self.0;
        while let Some(child) = inner.list.first_child() {
            inner.list.remove(&child);
        }
        let mut choices = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let check = gtk::CheckButton::builder()
                .label(name)
                .active(selected.contains(&index))
                .build();
            let weak: Weak<Inner> = Rc::downgrade(inner);
            check.connect_toggled(move |_| {
                if let Some(inner) = weak.upgrade() {
                    Self(inner).changed();
                }
            });
            inner.list.append(&check);
            choices.push((name.clone(), check));
        }
        inner.choices.replace(choices);
        self.show_selection();
    }

    /// Uncheck everything, which counts as one change.
    pub fn clear(&self) {
        let inner = &self.0;
        let had_some = !self.selected().is_empty();
        inner.quiet.set(true);
        for (_, check) in inner.choices.borrow().iter() {
            check.set_active(false);
        }
        inner.quiet.set(false);
        if had_some {
            self.changed();
        }
    }

    /// Call `f` with the positions of the choices checked, whenever the user changes them.
    pub fn connect_changed(&self, f: impl Fn(Vec<usize>) + 'static) {
        self.0.on_change.replace(Some(Box::new(f)));
    }

    fn selected(&self) -> Vec<usize> {
        let choices = self.0.choices.borrow();
        (0..choices.len())
            .filter(|&index| choices[index].1.is_active())
            .collect()
    }

    fn changed(&self) {
        if self.0.quiet.get() {
            return;
        }
        self.show_selection();
        let selected = self.selected();
        if let Some(on_change) = self.0.on_change.borrow().as_ref() {
            on_change(selected);
        }
    }

    /// The button in the accent color and the count over its corner when some choices are
    /// checked, with their names in the tooltip.
    fn show_selection(&self) {
        let selected = self.selected();
        let choices = self.0.choices.borrow();
        let names: Vec<&str> = selected
            .iter()
            .map(|&index| choices[index].0.as_str())
            .collect();
        let (button, count) = (&self.0.button, &self.0.count);
        if names.is_empty() {
            button.remove_css_class("pochette-filtered");
        } else {
            button.add_css_class("pochette-filtered");
        }
        count.set_label(&names.len().to_string());
        count.set_visible(!names.is_empty());
        let tooltip = if names.is_empty() {
            self.0.what.clone()
        } else {
            names.join("\n")
        };
        self.0.button.set_tooltip_text(Some(&tooltip));
    }
}
