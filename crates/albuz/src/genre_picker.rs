//! A button that picks any number of genres from a checklist.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use relm4::gtk::{self, prelude::*};

/// Cheap to clone: every clone is the same picker.
#[derive(Clone)]
pub struct GenrePicker(Rc<Inner>);

struct Inner {
    button: gtk::MenuButton,
    /// The button's text, shortened when long so it doesn't widen the toolbar.
    label: gtk::Label,
    list: gtk::Box,
    choices: RefCell<Vec<(String, gtk::CheckButton)>>,
    /// Set while the picker changes its own checks, which isn't the user picking.
    quiet: Cell<bool>,
    #[expect(clippy::type_complexity)]
    on_change: RefCell<Option<Box<dyn Fn(Vec<usize>)>>>,
}

impl GenrePicker {
    pub fn new() -> Self {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .max_content_height(420)
            .child(&list)
            .build();
        let all = gtk::Button::builder()
            .label("All Genres")
            .css_classes(["flat"])
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.append(&all);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&scroller);
        let popover = gtk::Popover::builder().child(&content).build();
        let label = gtk::Label::builder()
            .label("All Genres")
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(14)
            .build();
        let button = gtk::MenuButton::builder()
            .child(&label)
            .always_show_arrow(true)
            .tooltip_text("Genres")
            .popover(&popover)
            .build();

        let picker = Self(Rc::new(Inner {
            button,
            label,
            list,
            choices: RefCell::default(),
            quiet: Cell::new(false),
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

    pub fn widget(&self) -> &gtk::MenuButton {
        &self.0.button
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

    /// Call `f` with the positions of the genres checked, whenever the user changes them.
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

    /// "All Genres", "Jazz", "3 Genres", with the names in the tooltip.
    fn show_selection(&self) {
        let selected = self.selected();
        let choices = self.0.choices.borrow();
        let names: Vec<&str> = selected
            .iter()
            .map(|&index| choices[index].0.as_str())
            .collect();
        let label = match names.as_slice() {
            [] => "All Genres".to_owned(),
            [name] => (*name).to_owned(),
            names => format!("{} Genres", names.len()),
        };
        self.0.label.set_label(&label);
        let tooltip = if names.is_empty() {
            "Genres".to_owned()
        } else {
            names.join("\n")
        };
        self.0.button.set_tooltip_text(Some(&tooltip));
    }
}
