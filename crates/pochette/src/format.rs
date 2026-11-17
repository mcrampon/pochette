//! How values read in the UI.

/// A track length: "3:33", or "1:02:05" past an hour.
pub fn track_duration(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// An album length: "45 min", or "1 h 2 min".
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
pub fn audio_quality(bit_depth: Option<u32>, sampling_rate_khz: Option<f32>) -> Option<String> {
    let (bits, rate) = (bit_depth?, sampling_rate_khz?);
    // 44.1 stays 44.1, 96.0 reads 96.
    let rate = format!("{rate:.1}");
    let rate = rate.strip_suffix(".0").unwrap_or(&rate);
    Some(format!("{bits}-bit / {rate} kHz"))
}

/// "1 track", "12 tracks".
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
            let decoded = entity_end.and_then(|end| Some((decode_entity(&rest[1..end])?, end)));
            if let Some((c, end)) = decoded {
                text.push(c);
                rest = &rest[end + 1..];
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

fn decode_entity(entity: &str) -> Option<char> {
    Some(match entity {
        "amp" => '&',
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

/// A release date from Qobuz's `YYYY-MM-DD`: "13 Nov 2026". Whatever doesn't parse stays as is.
pub fn release_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
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
        assert_eq!(html_to_text(""), "");
    }
}
