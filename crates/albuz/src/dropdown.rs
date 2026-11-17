//! Drop-downs for the toolbars.

use relm4::gtk::{self, pango, prelude::*};

/// A drop-down whose button shortens long choices, so it doesn't widen the toolbar.
pub fn dropdown(choices: &[&str]) -> gtk::DropDown {
    let dropdown = gtk::DropDown::new(Some(gtk::StringList::new(choices)), gtk::Expression::NONE);
    dropdown.set_factory(Some(&label_factory(true)));
    dropdown.set_list_factory(Some(&label_factory(false)));
    dropdown
}

fn label_factory(shorten: bool) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        if shorten {
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.set_max_width_chars(14);
        }
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let text = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .map(|choice| choice.string());
        if let Some(label) = item.child().and_downcast::<gtk::Label>() {
            label.set_label(text.as_deref().unwrap_or_default());
        }
    });
    factory
}

/// Offer `labels` instead, with the one at `selected` picked.
pub fn replace_choices(dropdown: &gtk::DropDown, labels: &[String], selected: u32) {
    let Some(list) = dropdown.model().and_downcast::<gtk::StringList>() else {
        return;
    };
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    list.splice(0, list.n_items(), &labels);
    dropdown.set_selected(selected);
}
