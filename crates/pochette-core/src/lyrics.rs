//! A song's lyrics, from LRCLIB (lrclib.net): a free, open database of lyrics, most of them
//! timed line by line. Qobuz has none.
//!
//! LRCLIB learns the title, artist, album and length of the songs looked up, and nothing
//! else: lyrics are only looked up while the user shows them.

use std::time::Duration;

use serde::Deserialize;

const API_URL: &str = "https://lrclib.net/api/";

/// How far apart Qobuz's and LRCLIB's lengths of a song may be, in seconds: the same song
/// can be a second or two longer on one or the other.
const LENGTH_TOLERANCE: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Lyrics {
    /// Lines timed to the song, in order.
    Synced(Vec<Line>),
    /// Lines without timing.
    Plain(String),
    Instrumental,
    /// LRCLIB doesn't know the song.
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// When the line is sung.
    pub at: Duration,
    /// Empty between verses, where the music plays alone.
    pub text: String,
}

/// The song to find the lyrics of.
#[derive(Debug, Clone)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// In seconds.
    pub duration: u32,
}

/// The lyrics of `song`: those of exactly that song if LRCLIB has it, or else of the closest
/// match of the same length.
pub async fn find(http: &reqwest::Client, song: &Song) -> reqwest::Result<Lyrics> {
    let duration = song.duration.to_string();
    let response = http
        .get(format!("{API_URL}get"))
        .query(&[
            ("track_name", song.title.as_str()),
            ("artist_name", song.artist.as_str()),
            ("album_name", song.album.as_str()),
            ("duration", duration.as_str()),
        ])
        .send()
        .await?;
    if response.status() != reqwest::StatusCode::NOT_FOUND {
        let record: Record = response.error_for_status()?.json().await?;
        return Ok(record.into());
    }

    // Not under that album, maybe under another, or a slightly different title.
    let found: Vec<Record> = http
        .get(format!("{API_URL}search"))
        .query(&[
            ("track_name", song.title.as_str()),
            ("artist_name", song.artist.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(best_match(found, song.duration))
}

/// Of the records found, one of the same length as the song, timed if one is.
fn best_match(found: Vec<Record>, duration: u32) -> Lyrics {
    let mut same_length: Vec<Record> = found
        .into_iter()
        .filter(|record| {
            record
                .duration
                .is_some_and(|length| (length - f64::from(duration)).abs() <= LENGTH_TOLERANCE)
        })
        .collect();
    let timed = same_length
        .iter()
        .position(|record| record.synced_lyrics.is_some());
    match timed {
        Some(index) => same_length.swap_remove(index).into(),
        None => same_length
            .into_iter()
            .next()
            .map_or(Lyrics::Missing, Lyrics::from),
    }
}

/// A song as LRCLIB lists it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    /// In seconds.
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl From<Record> for Lyrics {
    fn from(record: Record) -> Self {
        if let Some(lines) = record.synced_lyrics.as_deref().map(parse_lrc)
            && !lines.is_empty()
        {
            return Self::Synced(lines);
        }
        match record.plain_lyrics {
            Some(text) if !text.trim().is_empty() => Self::Plain(text.trim().to_owned()),
            _ if record.instrumental => Self::Instrumental,
            _ => Self::Missing,
        }
    }
}

/// The timed lines of lyrics in the LRC format, in order: `[01:02.50] A line`, where a line
/// sung several times may have several times, and tags such as `[ar: The Artist]` are
/// skipped.
fn parse_lrc(lrc: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for row in lrc.lines() {
        let mut rest = row.trim();
        let mut times = Vec::new();
        while let Some(tag) = rest.strip_prefix('[') {
            let Some((inside, after)) = tag.split_once(']') else {
                break;
            };
            let Some(at) = parse_time(inside) else {
                // A tag rather than a time.
                break;
            };
            times.push(at);
            rest = after;
        }
        let text = rest.trim();
        lines.extend(times.into_iter().map(|at| Line {
            at,
            text: text.to_owned(),
        }));
    }
    lines.sort_by_key(|line| line.at);
    lines
}

/// `01:02.50`, or `01:02`, as a time.
fn parse_time(time: &str) -> Option<Duration> {
    let (minutes, seconds) = time.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0)
        .then(|| Duration::from_secs(minutes * 60) + Duration::from_secs_f64(seconds))
}

/// Which of `lines` is sung at `position`: the last one started, if any is.
#[must_use]
pub fn current_line(lines: &[Line], position: Duration) -> Option<usize> {
    lines
        .partition_point(|line| line.at <= position)
        .checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seconds: f64, text: &str) -> Line {
        Line {
            at: Duration::from_secs_f64(seconds),
            text: text.to_owned(),
        }
    }

    #[test]
    fn parses_timed_lyrics() {
        let lrc = "[ar: Radiohead]\n[00:19.16] When you were here before\n\n\
                   [00:24.09] Couldn't look you in the eye\n[00:30.00]\n\
                   [01:02.5][02:10.00] But I'm a creep\n";
        assert_eq!(
            parse_lrc(lrc),
            [
                line(19.16, "When you were here before"),
                line(24.09, "Couldn't look you in the eye"),
                line(30.0, ""),
                line(62.5, "But I'm a creep"),
                line(130.0, "But I'm a creep"),
            ]
        );
    }

    #[test]
    fn finds_the_line_sung() {
        let lines = [line(10.0, "one"), line(20.0, "two"), line(30.0, "three")];
        assert_eq!(current_line(&lines, Duration::from_secs(5)), None);
        assert_eq!(current_line(&lines, Duration::from_secs(10)), Some(0));
        assert_eq!(current_line(&lines, Duration::from_secs(25)), Some(1));
        assert_eq!(current_line(&lines, Duration::from_secs(300)), Some(2));
    }

    #[test]
    fn reads_what_lrclib_has() {
        let record = |synced: Option<&str>, plain: Option<&str>, instrumental| Record {
            duration: Some(239.0),
            instrumental,
            plain_lyrics: plain.map(str::to_owned),
            synced_lyrics: synced.map(str::to_owned),
        };
        assert_eq!(
            Lyrics::from(record(Some("[00:19.16] Here"), Some("Here"), false)),
            Lyrics::Synced(vec![line(19.16, "Here")])
        );
        assert_eq!(
            Lyrics::from(record(None, Some(" Here \n"), false)),
            Lyrics::Plain("Here".to_owned())
        );
        assert_eq!(Lyrics::from(record(None, None, true)), Lyrics::Instrumental);
        assert_eq!(Lyrics::from(record(None, None, false)), Lyrics::Missing);
    }

    #[test]
    fn picks_a_timed_match_of_the_same_length() {
        let record = |duration, synced: Option<&str>| Record {
            duration: Some(duration),
            instrumental: false,
            plain_lyrics: Some("Plain".to_owned()),
            synced_lyrics: synced.map(str::to_owned),
        };
        let found = vec![
            record(180.0, Some("[00:01.00] Another song")),
            record(240.0, None),
            record(238.0, Some("[00:19.16] This one")),
        ];
        assert_eq!(
            best_match(found, 239),
            Lyrics::Synced(vec![line(19.16, "This one")])
        );
        assert_eq!(best_match(Vec::new(), 239), Lyrics::Missing);
    }
}

/// Checks against the real LRCLIB: `cargo test -- --ignored lyrics`.
#[cfg(test)]
mod live {
    use super::*;

    async fn look_up(title: &str, artist: &str, album: &str, duration: u32) -> Lyrics {
        let song = Song {
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.to_owned(),
            duration,
        };
        let lyrics = find(&reqwest::Client::new(), &song).await.unwrap();
        let kind = match &lyrics {
            Lyrics::Synced(lines) => format!("{} timed lines", lines.len()),
            Lyrics::Plain(text) => format!("plain, {} chars", text.len()),
            other => format!("{other:?}"),
        };
        println!("{artist} - {title}: {kind}");
        lyrics
    }

    #[tokio::test]
    #[ignore = "calls lrclib.net"]
    async fn finds_lyrics() {
        assert!(matches!(
            look_up("Creep", "Radiohead", "Pablo Honey", 239).await,
            Lyrics::Synced(_)
        ));
        // Under an album LRCLIB doesn't know it under: found by searching.
        assert!(matches!(
            look_up("Feel Good Inc.", "Gorillaz", "Not the Album", 222).await,
            Lyrics::Synced(_)
        ));
        look_up(
            "Hares on the Mountain",
            "Radie Peat",
            "No Human Tongue Can Tell",
            255,
        )
        .await;
    }
}
