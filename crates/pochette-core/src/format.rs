//! How values read in the UI.

use crate::time;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const FULL_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

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
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// The best quality available: "24-bit / 96 kHz".
#[must_use]
pub fn audio_quality(bit_depth: Option<u32>, sampling_rate_khz: Option<f32>) -> Option<String> {
    let (bits, rate) = (bit_depth?, sampling_rate_khz?);
    // 44.1 stays 44.1, 96.0 reads 96.
    let rate = format!("{rate:.1}");
    let rate = rate.strip_suffix(".0").unwrap_or(&rate);
    Some(format!("{bits}-bit / {rate} kHz"))
}

/// "1 track", "12 tracks".
#[must_use]
pub fn track_count(count: u32) -> String {
    if count == 1 {
        "1 track".to_owned()
    } else {
        format!("{count} tracks")
    }
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
pub fn release_type(release_type: &str) -> &'static str {
    match release_type {
        "single" => "Single",
        "epk" | "ep" => "EP",
        "live" => "Live",
        "compilation" => "Compilation",
        "boxset" | "box-set" => "Box Set",
        _ => "Album",
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
        .parse::<usize>()
        .ok()
        .and_then(|month| MONTHS.get(month.checked_sub(1)?));
    match (month, day.parse::<u8>()) {
        (Some(month), Ok(day)) => format!("{day} {month} {year}"),
        _ => date.to_owned(),
    }
}

/// "1 play", "12 plays".
#[must_use]
pub fn play_count(count: u32) -> String {
    if count == 1 {
        "1 play".to_owned()
    } else {
        format!("{count} plays")
    }
}

/// Time spent listening: "45 min", "12 h", or "1,204 h" past a thousand hours.
#[must_use]
pub fn listening_time(seconds: u64) -> String {
    let minutes = (seconds + 30) / 60;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = (minutes + 30) / 60;
    let (thousands, rest) = (hours / 1000, hours % 1000);
    if thousands > 0 {
        format!("{thousands},{rest:03} h")
    } else {
        format!("{hours} h")
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
    let (year, month, _) = then;
    let month_name = FULL_MONTHS
        .get(usize::try_from(month - 1).unwrap_or_default())
        .copied()
        .unwrap_or_default();
    match days {
        ..=0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        2..=6 => format!("{days} days ago"),
        7..=13 => "a week ago".to_owned(),
        14..=31 => format!("{} weeks ago", days / 7),
        _ if year == today.0 => format!("in {month_name}"),
        _ => format!("in {month_name} {year}"),
    }
}

/// How long ago `at` was, from `now`, closer than [`ago`] while it's recent: "just now",
/// "3 minutes ago", "an hour ago", then as [`ago`] says.
#[must_use]
pub fn since(at: i64, now: i64) -> String {
    match now - at {
        ..60 => "just now".to_owned(),
        seconds @ ..3600 => match seconds / 60 {
            1 => "a minute ago".to_owned(),
            minutes => format!("{minutes} minutes ago"),
        },
        seconds @ ..86_400 => match seconds / 3600 {
            1 => "an hour ago".to_owned(),
            hours => format!("{hours} hours ago"),
        },
        _ => ago(at, now),
    }
}

/// A month by its number, from 1: "January".
#[must_use]
pub fn month(month: i32) -> &'static str {
    usize::try_from(month - 1)
        .ok()
        .and_then(|index| FULL_MONTHS.get(index))
        .copied()
        .unwrap_or_default()
}

/// A month by its number, from 1: "Jan".
#[must_use]
pub fn short_month(month: i32) -> &'static str {
    usize::try_from(month - 1)
        .ok()
        .and_then(|index| MONTHS.get(index))
        .copied()
        .unwrap_or_default()
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
