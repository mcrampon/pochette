//! The settings, a page beside the tabs, from the main menu: the devices the plays sync
//! with.

use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller,
    gtk::{self, prelude::*},
};

use crate::{
    clamp::Clamp,
    devices::{Devices, DevicesMsg},
};

/// The widest the settings get, so their text stays readable.
const MAX_WIDTH: i32 = 680;

pub struct SettingsPage {
    devices: Controller<Devices>,
}

#[derive(Debug)]
pub enum SettingsMsg {
    /// The page shows.
    Show,
    /// The page no longer shows.
    Hidden,
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
                        set_label: "Settings",
                        set_xalign: 0.0,
                        add_css_class: "pochette-heading",
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,

                        gtk::Label {
                            set_label: "Devices",
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

    fn init((): (), root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            devices: Devices::builder().launch(()).detach(),
        };
        let devices = model.devices.widget();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SettingsMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SettingsMsg::Show => self.devices.emit(DevicesMsg::Open),
            SettingsMsg::Hidden => self.devices.emit(DevicesMsg::Closed),
        }
    }
}
