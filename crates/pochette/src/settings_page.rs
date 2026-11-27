//! The settings, a page beside the tabs, from the main menu: the language, and the devices
//! the plays sync with.

use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, prelude::*},
};

use pochette_core::i18n::LANGUAGES;

use crate::{
    choice_picker::ChoicePicker,
    clamp::Clamp,
    devices::{Devices, DevicesMsg},
    i18n::tr,
    settings::Settings,
};

/// The widest the settings get, so their text stays readable.
const MAX_WIDTH: i32 = 680;

pub struct SettingsPage {
    devices: Controller<Devices>,
    /// The language picked when Pochette opened, as a tag, `None` for the system's.
    language_at_start: Option<String>,
    /// The language picked.
    language: Option<String>,
    /// Kept, as its rows only hold on to it weakly.
    language_picker: ChoicePicker,
}

#[derive(Debug)]
pub enum SettingsMsg {
    /// The page shows.
    Show,
    /// The page no longer shows.
    Hidden,
    /// A language was picked, by its place in the picker: the system's first.
    PickLanguage(usize),
}

#[relm4::component(pub)]
impl Component for SettingsPage {
    type Init = ();
    type Input = SettingsMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,

            Clamp {
                set_max_width: MAX_WIDTH,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 36,
                    set_margin_top: 18,
                    set_margin_bottom: 48,
                    set_margin_start: 36,
                    set_margin_end: 36,

                    gtk::Label {
                        set_label: &tr!("settings"),
                        set_xalign: 0.0,
                        add_css_class: "pochette-heading",
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Label {
                            set_label: &tr!("settings-language"),
                            set_xalign: 0.0,
                            add_css_class: "pochette-subheading",
                        },
                        #[local_ref]
                        language_picker -> gtk::MenuButton {
                            set_halign: gtk::Align::Start,
                        },
                        gtk::Label {
                            set_label: &tr!("settings-language-restart"),
                            #[watch]
                            set_visible: model.language != model.language_at_start,
                            set_wrap: true,
                            set_xalign: 0.0,
                            add_css_class: "dim-label",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Label {
                            set_label: &tr!("settings-devices"),
                            set_xalign: 0.0,
                            add_css_class: "pochette-subheading",
                        },
                        #[local_ref]
                        devices -> gtk::Stack {},
                    },
                },
            },
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let language = Settings::load().language;
        let picker = ChoicePicker::new(&tr!("settings-language"));
        let names: Vec<String> = std::iter::once(tr!("settings-language-system"))
            .chain(LANGUAGES.iter().map(|(_, name)| (*name).to_owned()))
            .collect();
        let current = language.as_deref().map_or(0, |language| {
            LANGUAGES
                .iter()
                .position(|(tag, _)| *tag == language)
                .map_or(0, |index| index + 1)
        });
        picker.set_choices(&names, current);
        let input = sender.input_sender().clone();
        picker.connect_picked(move |index| input.emit(SettingsMsg::PickLanguage(index)));

        let model = Self {
            devices: Devices::builder().launch(()).detach(),
            language_at_start: language.clone(),
            language,
            language_picker: picker,
        };
        let devices = model.devices.widget();
        let language_picker = model.language_picker.widget();

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SettingsMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SettingsMsg::Show => self.devices.emit(DevicesMsg::Open),
            SettingsMsg::Hidden => self.devices.emit(DevicesMsg::Closed),
            SettingsMsg::PickLanguage(index) => {
                let language = index
                    .checked_sub(1)
                    .and_then(|index| LANGUAGES.get(index))
                    .map(|(tag, _)| (*tag).to_owned());
                Settings::update(|settings| settings.language.clone_from(&language));
                self.language = language;
            }
        }
    }
}
