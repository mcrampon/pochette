//! The audio outputs to play through, as GStreamer finds them on the system (the sinks of
//! PipeWire or PulseAudio), and the button in Now Playing that picks one.

use std::{cell::RefCell, rc::Rc};

use gstreamer::{self as gst, prelude::*};
use relm4::gtk;

use crate::{choice_picker::ChoicePicker, player::Player, settings::Settings};

/// An audio output.
struct Output {
    /// What tells it apart across runs: the sink's name on the sound server.
    id: String,
    name: String,
    /// It, from each device provider that lists it: PipeWire's and PulseAudio's, on
    /// PipeWire.
    devices: Vec<gst::Device>,
}

/// Finds the audio outputs. Without a device monitor, as when there's no sound server to
/// ask, there are none but the default.
fn monitor() -> Option<gst::DeviceMonitor> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Audio/Sink"), None);
    match monitor.start() {
        Ok(()) => Some(monitor),
        Err(err) => {
            tracing::warn!("cannot list the audio outputs: {err}");
            None
        }
    }
}

/// The outputs of the moment, each once: several device providers may offer the same, as
/// PipeWire's and PulseAudio's both do on PipeWire with GStreamer's PipeWire plugin.
fn outputs(monitor: Option<&gst::DeviceMonitor>) -> Vec<Output> {
    let mut outputs: Vec<Output> = Vec::new();
    // PipeWire's provider hides PulseAudio's from the monitor, but PipeWire's sink
    // crackles where PulseAudio's, through PipeWire's PulseAudio server, doesn't: ask
    // PulseAudio's too. Without a PulseAudio server, it has none.
    let pulse = gst::DeviceProviderFactory::by_name("pulsedeviceprovider")
        .map(|provider| provider.devices())
        .unwrap_or_default()
        .into_iter()
        .filter(|device| device.has_classes("Audio/Sink"));
    let devices = monitor.map(gst::DeviceMonitor::devices).unwrap_or_default();
    for device in devices.into_iter().chain(pulse) {
        let id = id(&device);
        match outputs.iter_mut().find(|output| output.id == id) {
            Some(output) => output.devices.push(device),
            None => outputs.push(Output {
                id,
                name: device.display_name().to_string(),
                devices: vec![device],
            }),
        }
    }
    outputs
}

/// The output's name on the sound server where there is one, the same whichever device
/// provider lists it: PipeWire's node name, which both PipeWire's provider and PulseAudio's
/// on PipeWire give; or PulseAudio's sink name. Otherwise, as with plain ALSA, the name
/// shown.
fn id(device: &gst::Device) -> String {
    let node = device
        .properties()
        .and_then(|properties| properties.get::<String>("node.name").ok());
    let sink = || {
        device
            .has_property("internal-name")
            .then(|| device.property::<Option<String>>("internal-name"))
            .flatten()
    };
    node.or_else(sink)
        .unwrap_or_else(|| device.display_name().to_string())
}

/// The sink to play through output `id`, if it's there: of its providers', the one
/// GStreamer ranks highest, which is also the one it plays the default output through.
/// On PipeWire, that's PulseAudio's sink, through PipeWire's PulseAudio server: PipeWire's
/// own sink, which GStreamer doesn't rank, crackles.
fn sink(outputs: &[Output], id: &str) -> Option<gst::Element> {
    let output = outputs.iter().find(|output| output.id == id)?;
    let rank = |sink: &gst::Element| {
        sink.factory()
            .map_or(gst::Rank::NONE, |factory| factory.rank())
    };
    let sink = output
        .devices
        .iter()
        .filter_map(|device| match device.create_element(None) {
            Ok(sink) => Some(sink),
            Err(err) => {
                tracing::warn!("cannot play through {}: {err}", output.name);
                None
            }
        })
        .max_by_key(rank)?;
    tracing::info!(
        "playing through {} with {}",
        output.name,
        sink.factory()
            .map(|factory| factory.name().to_string())
            .unwrap_or_default()
    );
    Some(sink)
}

/// A button picking the output to play through: the system's default, or one of the
/// outputs, as they are when its popover opens. The pick is remembered for the next run.
pub struct OutputPicker {
    picker: ChoicePicker,
}

impl OutputPicker {
    /// The picker of `player`'s output, which plays through the output picked last time.
    pub fn new(player: &Player) -> Self {
        let monitor = monitor();
        let saved = Settings::load().output;
        if let Some(id) = &saved {
            let sink = sink(&outputs(monitor.as_ref()), id);
            if sink.is_none() {
                tracing::info!("output {id} isn't there, playing through the default");
            }
            player.set_output(sink);
        }

        let picker = ChoicePicker::with_icon("audio-speakers-symbolic", "Output");
        // The outputs offered, after the default.
        let offered: Rc<RefCell<Vec<Output>>> = Rc::default();
        let chosen = Rc::new(RefCell::new(saved));

        let (refresh, shown, current_id) = (picker.clone(), offered.clone(), chosen.clone());
        picker.connect_opening(move || {
            let outputs = outputs(monitor.as_ref());
            let mut names = vec!["System Default".to_owned()];
            names.extend(outputs.iter().map(|output| output.name.clone()));
            let position = current_id
                .borrow()
                .as_ref()
                .and_then(|id| outputs.iter().position(|output| output.id == *id))
                .map_or(0, |index| index + 1);
            refresh.set_choices(&names, position);
            shown.replace(outputs);
        });

        let player = player.clone();
        picker.connect_picked(move |position| {
            let offered = offered.borrow();
            let output = position.checked_sub(1).and_then(|index| offered.get(index));
            let id = output.map(|output| output.id.clone());
            if *chosen.borrow() == id {
                return;
            }
            let sink = id.as_deref().and_then(|id| sink(&offered, id));
            // An output that can't be played through is the default.
            let id = id.filter(|_| sink.is_some());
            player.set_output(sink);
            chosen.replace(id.clone());
            Settings::update(|settings| settings.output = id);
        });
        Self { picker }
    }

    pub fn widget(&self) -> &gtk::MenuButton {
        self.picker.widget()
    }
}
