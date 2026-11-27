//! What a Qobuz link points to: an album, an artist or a track, from the links Qobuz shares,
//! its web player's, its store's and its desktop app's.

use url::Url;

/// What a Qobuz link points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Album(String),
    Artist(u64),
    Track(u64),
}

/// What `link` points to, if it's a Qobuz link to an album, an artist or a track:
/// - `https://open.qobuz.com/album/<id>`, as Qobuz shares them, and the same for artists and
///   tracks, and from its web player, `https://play.qobuz.com/album/<id>`;
/// - `https://www.qobuz.com/<locale>/album/<name>/<id>` and
///   `…/interpreter/<name>/<id>`, from its store;
/// - `qobuzapp://album/<id>`, which its pages hand to its desktop app.
#[must_use]
pub fn parse(link: &str) -> Option<Link> {
    let url = Url::parse(link.trim()).ok()?;
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    let (kind, id) = match (url.scheme(), url.host_str()) {
        // qobuzapp://album/<id>: the kind is the host.
        ("qobuzapp", Some(kind)) => (kind, *segments.first()?),
        ("https" | "http", Some("open.qobuz.com" | "play.qobuz.com")) => match segments[..] {
            [kind, id, ..] => (kind, id),
            _ => return None,
        },
        // The store's pages end with the id, after the locale and the name.
        ("https" | "http", Some("www.qobuz.com" | "qobuz.com")) => {
            let kind_at = segments
                .iter()
                .position(|segment| matches!(*segment, "album" | "interpreter" | "track"))?;
            let kind = match segments[kind_at] {
                "interpreter" => "artist",
                kind => kind,
            };
            (
                kind,
                *segments.last().filter(|_| segments.len() > kind_at + 1)?,
            )
        }
        _ => return None,
    };
    match kind {
        "album" if is_album_id(id) => Some(Link::Album(id.to_owned())),
        "artist" => id.parse().ok().map(Link::Artist),
        "track" => id.parse().ok().map(Link::Track),
        _ => None,
    }
}

/// The first Qobuz link in `text`, as an app shares it: "Listen to … on Qobuz https://…".
#[must_use]
pub fn find(text: &str) -> Option<Link> {
    text.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '/'))
        .find_map(parse)
}

/// Album ids are letters and digits: "0886443927087", "ul5jmt9mpqc4b".
fn is_album_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_qobuz_links() {
        let album = Some(Link::Album("0886443927087".to_owned()));
        assert_eq!(parse("https://open.qobuz.com/album/0886443927087"), album);
        assert_eq!(
            parse("https://open.qobuz.com/album/0886443927087/?utm=x#y"),
            album
        );
        assert_eq!(parse("https://play.qobuz.com/album/0886443927087"), album);
        assert_eq!(parse("qobuzapp://album/0886443927087"), album);
        assert_eq!(
            parse("https://www.qobuz.com/fr-fr/album/ok-computer-radiohead/0886443927087"),
            album
        );
        assert_eq!(
            parse("https://open.qobuz.com/artist/36819"),
            Some(Link::Artist(36819))
        );
        assert_eq!(
            parse("https://www.qobuz.com/gb-en/interpreter/radiohead/36819"),
            Some(Link::Artist(36819))
        );
        assert_eq!(
            parse("https://open.qobuz.com/track/1234567"),
            Some(Link::Track(1_234_567))
        );
        assert_eq!(
            parse("qobuzapp://track/1234567"),
            Some(Link::Track(1_234_567))
        );
    }

    #[test]
    fn leaves_other_links() {
        assert_eq!(parse("https://open.qobuz.com/playlist/123"), None);
        assert_eq!(parse("https://open.qobuz.com/artist/radiohead"), None);
        assert_eq!(parse("https://open.qobuz.com/"), None);
        assert_eq!(parse("https://example.com/album/0886443927087"), None);
        assert_eq!(parse("https://www.qobuz.com/fr-fr/album"), None);
        assert_eq!(parse("not a link"), None);
    }

    #[test]
    fn finds_the_link_in_a_shared_text() {
        assert_eq!(
            find(
                "Listen to OK Computer by Radiohead on Qobuz\nhttps://open.qobuz.com/album/0886443927087."
            ),
            Some(Link::Album("0886443927087".to_owned()))
        );
        assert_eq!(find("Nothing to see"), None);
    }
}
