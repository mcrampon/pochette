//! The accent color the user picked for their desktop, as `@pochette_accent` in the style
//! sheet, with `@pochette_accent_fg` for text on it.
//!
//! Desktops share the accent through the settings portal, and GTK 4.20 and later pass it on
//! as `gtk-accent-color`. GTK's own theme keeps painting its widgets in a fixed blue, and only
//! defines `@accent_color` when the desktop gives one: the style sheet uses these colors
//! instead, and paints the theme's blue widgets with them.

use relm4::gtk::{self, gdk, prelude::*};

/// GNOME's blue, for desktops that share no accent.
const DEFAULT: gdk::RGBA = gdk::RGBA::new(0.208, 0.518, 0.894, 1.0);

/// The setting GTK keeps the desktop's accent in, since 4.20.
const SETTING: &str = "gtk-accent-color";

/// Define the accent colors for the whole app, and keep them in step with the desktop's.
pub fn follow() {
    let (Some(display), Some(settings)) = (gdk::Display::default(), gtk::Settings::default())
    else {
        return;
    };
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    provider.load_from_string(&css(desktop_accent(&settings)));

    // An older GTK knows of no accent: the default stays.
    if settings.find_property(SETTING).is_some() {
        settings.connect_notify_local(Some(SETTING), move |settings, _| {
            provider.load_from_string(&css(desktop_accent(settings)));
        });
    }
}

fn desktop_accent(settings: &gtk::Settings) -> Option<gdk::RGBA> {
    settings.find_property(SETTING)?;
    settings.property::<Option<gdk::RGBA>>(SETTING)
}

fn css(accent: Option<gdk::RGBA>) -> String {
    let accent = accent.unwrap_or(DEFAULT);
    let text = if is_light(accent) {
        "rgba(0, 0, 0, 0.8)"
    } else {
        "#ffffff"
    };
    format!("@define-color pochette_accent {accent};\n@define-color pochette_accent_fg {text};\n")
}

/// Whether `color` is light enough to take dark text: as on GNOME, accents take white text
/// unless they're very light, even where dark text would contrast a little more.
fn is_light(color: gdk::RGBA) -> bool {
    let linear = |channel: f32| {
        if channel <= 0.040_45 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(color.red())
        + 0.7152 * linear(color.green())
        + 0.0722 * linear(color.blue());
    luminance > 0.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_white_text_on_dark_accents_and_dark_text_on_light_ones() {
        // GNOME's blue, purple, red and yellow; then a pale green and a pale pink.
        for dark in [
            DEFAULT,
            gdk::RGBA::new(0.569, 0.255, 0.675, 1.0),
            gdk::RGBA::new(0.878, 0.106, 0.141, 1.0),
            gdk::RGBA::new(0.784, 0.533, 0.0, 1.0),
        ] {
            assert!(!is_light(dark), "{dark}");
        }
        for light in [
            gdk::RGBA::new(0.6, 0.9, 0.6, 1.0),
            gdk::RGBA::new(1.0, 0.8, 0.85, 1.0),
        ] {
            assert!(is_light(light), "{light}");
        }
    }

    #[test]
    fn defines_the_colors() {
        let css = css(None);
        assert!(css.contains("@define-color pochette_accent rgb(53,132,228);"));
        assert!(css.contains("@define-color pochette_accent_fg #ffffff;"));
    }
}
