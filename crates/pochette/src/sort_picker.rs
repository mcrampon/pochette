//! A button that picks how to sort, and which way: its popover lists the sorts, an arrow
//! on the one picked shows the direction, and picking that one again turns it around.

use std::rc::Rc;

use relm4::gtk::{self, prelude::*};

use crate::{
    i18n::tr,
    path_icon::{self, PathIcon},
};

pub struct SortPicker {
    button: gtk::MenuButton,
    icon: gtk::Image,
    rows: Vec<Row>,
}

/// A sort in the popover.
struct Row {
    name: String,
    button: gtk::Button,
    /// At the end of the row, shown on the sort picked.
    arrow: PathIcon,
}

impl SortPicker {
    /// A picker of the sorts named `names`, calling `pick` with the position of the one
    /// clicked: it stays open, to turn that sort around with another click.
    pub fn new(names: &[String], pick: impl Fn(usize) + 'static) -> Self {
        let pick = Rc::new(pick);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let mut rows = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let arrow = PathIcon::new(path_icon::DOWN);
            arrow.set_size_request(16, 16);
            arrow.set_margin_start(18);
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            content.append(
                &gtk::Label::builder()
                    .label(name.as_str())
                    .xalign(0.0)
                    .hexpand(true)
                    .build(),
            );
            content.append(&arrow);
            let button = gtk::Button::builder()
                .child(&content)
                .css_classes(["flat"])
                .build();
            let pick = pick.clone();
            button.connect_clicked(move |_| pick(index));
            list.append(&button);
            rows.push(Row {
                name: name.clone(),
                button,
                arrow,
            });
        }
        let popover = gtk::Popover::builder().child(&list).build();

        let icon = gtk::Image::new();
        let button = gtk::MenuButton::builder()
            .child(&icon)
            .always_show_arrow(true)
            .popover(&popover)
            .build();
        Self { button, icon, rows }
    }

    pub fn widget(&self) -> &gtk::MenuButton {
        &self.button
    }

    /// Show the sort at `picked` as picked, going `descending` or not; `order` says how it
    /// reads: "Newest First".
    pub fn show(&self, picked: usize, descending: bool, order: &str) {
        let Some(name) = self.rows.get(picked).map(|row| row.name.as_str()) else {
            return;
        };
        self.icon.set_icon_name(Some(if descending {
            "view-sort-descending-symbolic"
        } else {
            "view-sort-ascending-symbolic"
        }));
        self.button
            .set_tooltip_text(Some(&tr!("sort-by", sort = name, order = order)));
        let arrow = if descending {
            path_icon::DOWN
        } else {
            path_icon::UP
        };
        for (index, row) in self.rows.iter().enumerate() {
            let is_picked = index == picked;
            // Hidden rather than gone, so the rows stay as wide whichever is picked.
            row.arrow.set_opacity(if is_picked { 1.0 } else { 0.0 });
            row.arrow.set_path(arrow);
            let label = if is_picked {
                tr!("sort-picked", sort = row.name.as_str(), order = order)
            } else {
                row.name.clone()
            };
            row.button
                .update_property(&[gtk::accessible::Property::Label(&label)]);
        }
    }
}
