//! How values read in the UI, in the language [`crate::i18n::set_languages`] set.

use crate::{i18n::tr, time};

/// A track length: "3:33", or "1:02:05" past an hour.
#[must_use]
pub fn track_duration(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// An album length: "45 min", or "1 h 2 min".
#[must_use]
pub fn album_duration(seconds: u32) -> String {
    // Rounded to the nearest minute.
    let minutes = (seconds + 30) / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => tr!("duration-minutes", minutes = minutes),
        (hours, 0) => tr!("duration-hours", hours = hours),
        (hours, minutes) => tr!("duration-hours-minutes", hours = hours, minutes = minutes),
    }
}

/// The best quality available: "24-bit / 96 kHz".
#[must_use]
pub fn audio_quality(bit_depth: Option<u32>, sampling_rate_khz: Option<f32>) -> Option<String> {
    let (bits, rate) = (bit_depth?, sampling_rate_khz?);
    // 44.1 stays 44.1, 96.0 reads 96.
    let rate = format!("{rate:.1}");
    let rate = rate.strip_suffix(".0").map_or_else(
        || rate.replace('.', &tr!("number-decimal-separator")),
        str::to_owned,
    );
    Some(tr!("audio-quality", bits = bits, rate = rate))
}

/// "1 track", "12 tracks".
#[must_use]
pub fn track_count(count: u32) -> String {
    tr!("track-count", count = count)
}

/// Plain text from the simple HTML of Qobuz reviews: paragraphs and line breaks kept,
/// tags dropped, entities decoded.
pub fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(['<', '&']) {
        text.push_str(&rest[..start]);
        rest = &rest[start..];
        if rest.starts_with('<') {
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = rest[1..end].trim().to_ascii_lowercase();
            let name = tag
                .trim_start_matches('/')
                .split([' ', '/'])
                .next()
                .unwrap_or("");
            match name {
                "br" => text.push('\n'),
                "p" | "div" if tag.starts_with('/') => text.push_str("\n\n"),
                _ => {}
            }
            rest = &rest[end + 1..];
        } else {
            let entity_end = rest.find(';').filter(|&end| end <= 10);
            let decoded = entity_end.and_then(|end| Some((decode_entity(&rest[1..end])?, end + 1)));
            if let Some((c, length)) = decoded.or_else(|| legacy_entity(rest)) {
                text.push(c);
                rest = &rest[length..];
            } else {
                // A bare ampersand.
                text.push('&');
                rest = &rest[1..];
            }
        }
    }
    text.push_str(rest);

    // Collapse the blank lines that tags leave behind.
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut paragraphs = lines
        .split(|line| line.is_empty())
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| paragraph.join("\n"));
    let mut result = paragraphs.next().unwrap_or_default();
    for paragraph in paragraphs {
        result.push_str("\n\n");
        result.push_str(&paragraph);
    }
    result
}

/// An entity browsers also read without its semicolon, as Qobuz sometimes writes them:
/// `&copy TiVo`. `rest` starts with the ampersand; returns the character and the length read.
fn legacy_entity(rest: &str) -> Option<(char, usize)> {
    let name_length = rest[1..]
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len() - 1);
    let name = &rest[1..=name_length];
    let legacy = ["amp", "lt", "gt", "quot", "nbsp", "copy", "reg"];
    legacy
        .contains(&name)
        .then(|| Some((decode_entity(name)?, 1 + name_length)))
        .flatten()
}

fn decode_entity(entity: &str) -> Option<char> {
    Some(match entity {
        "amp" => '&',
        "copy" => '\u{a9}',
        "reg" => '\u{ae}',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => {
            let code = entity.strip_prefix('#')?;
            let code = match code.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

/// What kind of release Qobuz's `release_type` is: "Album", "EP", "Single"…
#[must_use]
pub fn release_type(release_type: &str) -> String {
    match release_type {
        "single" => tr!("release-type-single"),
        "epk" | "ep" => tr!("release-type-ep"),
        "live" => tr!("release-type-live"),
        "compilation" => tr!("release-type-compilation"),
        "boxset" | "box-set" => tr!("release-type-box-set"),
        _ => tr!("release-type-album"),
    }
}

/// A release date from Qobuz's `YYYY-MM-DD`: "13 Nov 2026". Whatever doesn't parse stays as is.
#[must_use]
pub fn release_date(date: &str) -> String {
    let mut parts = date.splitn(3, '-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return date.to_owned();
    };
    let month = month
        .parse::<i32>()
        .ok()
        .filter(|month| (1..=12).contains(month));
    match (month, day.parse::<u8>()) {
        (Some(number), Ok(day)) => tr!(
            "release-date",
            day = day,
            month = short_month(number),
            number = number,
            year = year.to_owned(),
        ),
        _ => date.to_owned(),
    }
}

/// "1 play", "12 plays".
#[must_use]
pub fn play_count(count: u32) -> String {
    tr!("play-count", count = count)
}

/// Time spent listening: "45 min", "12 h", or "1,204 h" past a thousand hours.
#[must_use]
pub fn listening_time(seconds: u64) -> String {
    let minutes = (seconds + 30) / 60;
    if minutes < 60 {
        return tr!("duration-minutes", minutes = minutes);
    }
    let hours = (minutes + 30) / 60;
    let (thousands, rest) = (hours / 1000, hours % 1000);
    if thousands > 0 {
        let separator = tr!("number-group-separator");
        tr!(
            "duration-hours",
            hours = format!("{thousands}{separator}{rest:03}")
        )
    } else {
        tr!("duration-hours", hours = hours)
    }
}

/// When something happened, from `now`, both in Unix time: "today", "yesterday", "3 days
/// ago", "2 weeks ago", "in March", "in March 2025".
#[must_use]
pub fn ago(at: i64, now: i64) -> String {
    let (then, today) = (time::local(at), time::local(now));
    // Calendar days apart, whatever the hours.
    let day = |(year, month, day): (i32, i32, i32)| {
        time::local_unix(year, month, day, 12).map_or(0, |noon| noon / 86_400)
    };
    let days = day(today) - day(then);
    let (year, number, _) = then;
    match days {
        ..=0 => tr!("ago-today"),
        1 => tr!("ago-yesterday"),
        2..=6 => tr!("ago-days", days = days),
        7..=13 => tr!("ago-week"),
        14..=31 => tr!("ago-weeks", weeks = days / 7),
        _ if year == today.0 => tr!("ago-month", month = month(number), number = number),
        _ => tr!(
            "ago-month-year",
            month = month(number),
            number = number,
            year = year.to_string(),
        ),
    }
}

/// How long ago `at` was, from `now`, closer than [`ago`] while it's recent: "just now",
/// "3 minutes ago", "an hour ago", then as [`ago`] says.
#[must_use]
pub fn since(at: i64, now: i64) -> String {
    match now - at {
        ..60 => tr!("since-now"),
        seconds @ ..3600 => tr!("since-minutes", minutes = seconds / 60),
        seconds @ ..86_400 => tr!("since-hours", hours = seconds / 3600),
        _ => ago(at, now),
    }
}

/// A month by its number, from 1: "January"; empty for no month.
#[must_use]
pub fn month(month: i32) -> String {
    if (1..=12).contains(&month) {
        crate::i18n::text(&format!("month-{month}"), None)
    } else {
        String::new()
    }
}

/// A month by its number, from 1: "Jan"; empty for no month.
#[must_use]
pub fn short_month(month: i32) -> String {
    if (1..=12).contains(&month) {
        crate::i18n::text(&format!("month-short-{month}"), None)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_release_dates() {
        assert_eq!(release_date("2026-11-13"), "13 Nov 2026");
        assert_eq!(release_date("1969-01-05"), "5 Jan 1969");
        assert_eq!(release_date("1969-00-05"), "1969-00-05");
        assert_eq!(release_date("1969"), "1969");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(track_duration(213), "3:33");
        assert_eq!(track_duration(59), "0:59");
        assert_eq!(track_duration(3725), "1:02:05");
        assert_eq!(album_duration(3558), "59 min");
        assert_eq!(album_duration(3725), "1 h 2 min");
        assert_eq!(album_duration(7190), "2 h");
    }

    #[test]
    fn formats_plays() {
        assert_eq!(play_count(1), "1 play");
        assert_eq!(play_count(0), "0 plays");
        assert_eq!(listening_time(1500), "25 min");
        assert_eq!(listening_time(5 * 3600 + 1000), "5 h");
        assert_eq!(listening_time(1_204 * 3600), "1,204 h");
    }

    #[test]
    fn formats_how_long_ago() {
        let at = |month, day, hour| time::local_unix(2026, month, day, hour).unwrap();
        let now = at(11, 20, 9);
        assert_eq!(ago(at(11, 20, 1), now), "today");
        assert_eq!(ago(at(11, 19, 23), now), "yesterday");
        assert_eq!(ago(at(11, 17, 12), now), "3 days ago");
        assert_eq!(ago(at(11, 10, 12), now), "a week ago");
        assert_eq!(ago(at(10, 30, 12), now), "3 weeks ago");
        assert_eq!(ago(at(3, 2, 12), now), "in March");
        let last_year = time::local_unix(2025, 3, 2, 12).unwrap();
        assert_eq!(ago(last_year, now), "in March 2025");
    }

    #[test]
    fn says_how_long_ago_closely_while_recent() {
        let now = time::local_unix(2026, 11, 20, 9).unwrap();
        assert_eq!(since(now - 20, now), "just now");
        assert_eq!(since(now - 60, now), "a minute ago");
        assert_eq!(since(now - 7 * 60, now), "7 minutes ago");
        assert_eq!(since(now - 3600, now), "an hour ago");
        assert_eq!(since(now - 5 * 3600, now), "5 hours ago");
        assert_eq!(since(now - 3 * 86_400, now), "3 days ago");
    }

    #[test]
    fn formats_audio_quality() {
        assert_eq!(
            audio_quality(Some(24), Some(44.1)).as_deref(),
            Some("24-bit / 44.1 kHz")
        );
        assert_eq!(
            audio_quality(Some(24), Some(96.0)).as_deref(),
            Some("24-bit / 96 kHz")
        );
        assert_eq!(audio_quality(None, Some(44.1)), None);
    }

    #[test]
    fn turns_review_html_into_text() {
        assert_eq!(
            html_to_text(
                "<p>Rock &amp; roll<br/>from the <b>heart</b>.</p><p>Second&nbsp;one &#8212; &#x2019;</p>"
            ),
            "Rock & roll\nfrom the heart.\n\nSecond\u{a0}one \u{2014} \u{2019}"
        );
        assert_eq!(html_to_text("AT&T < 3"), "AT&T < 3");
        assert_eq!(
            html_to_text("attendu.\n<br />&copy TiVo"),
            "attendu.\n\n\u{a9} TiVo"
        );
        assert_eq!(
            html_to_text("&copy; 2026 &amp so on"),
            "\u{a9} 2026 & so on"
        );
        assert_eq!(html_to_text(""), "");
    }
}
