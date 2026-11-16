//! Qobuz doesn't hand out app ids anymore, so we use what the web player ships with: its app
//! id, and the secrets it signs streaming requests with.

use base64::{Engine, engine::general_purpose::URL_SAFE};
use regex::Regex;

use crate::{Error, Result};

const PLAY_URL: &str = "https://play.qobuz.com";

/// What the web player bundle holds.
#[derive(Debug, Clone)]
pub struct WebPlayer {
    pub app_id: String,
    /// The secrets that sign streaming requests, one per region. Which one Qobuz accepts
    /// depends on where the user is, so they're tried in turn.
    pub secrets: Vec<String>,
}

/// Read the app id from the current web player bundle.
pub async fn fetch_app_id(http: &reqwest::Client) -> Result<String> {
    Ok(fetch_web_player(http).await?.app_id)
}

/// Read the current web player bundle.
///
/// The bundle path changes with each web player release, so it's read from the login page first.
pub async fn fetch_web_player(http: &reqwest::Client) -> Result<WebPlayer> {
    let login_page = http
        .get(format!("{PLAY_URL}/login"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let bundle_path = bundle_path(&login_page).ok_or(Error::AppIdNotFound)?;
    tracing::debug!("reading web player bundle {bundle_path}");

    let bundle = http
        .get(format!("{PLAY_URL}{bundle_path}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(WebPlayer {
        app_id: app_id(&bundle).ok_or(Error::AppIdNotFound)?,
        secrets: secrets(&bundle),
    })
}

fn bundle_path(login_page: &str) -> Option<&str> {
    let re = Regex::new(r#"<script src="(/resources/[^"]+/bundle\.js)""#).expect("valid regex");
    Some(re.captures(login_page)?.get(1)?.as_str())
}

fn app_id(bundle: &str) -> Option<String> {
    let re = Regex::new(r#"production:\{api:\{appId:"(\d+)""#).expect("valid regex");
    Some(re.captures(bundle)?.get(1)?.as_str().to_owned())
}

/// The bundle splits each secret in three: a seed passed with a timezone name, then an
/// `info` and an `extras` next to that timezone's full name. Joined, minus the last 44
/// characters, they're the secret in base64.
fn secrets(bundle: &str) -> Vec<String> {
    let seeds = Regex::new(
        r#"[a-z]\.initialSeed\("(?P<seed>[\w=]+)",window\.utimezone\.(?P<timezone>[a-z]+)\)"#,
    )
    .expect("valid regex");

    let mut secrets = Vec::new();
    for seed in seeds.captures_iter(bundle) {
        let timezone = capitalize(&seed["timezone"]);
        let parts = Regex::new(&format!(
            r#"name:"\w+/{timezone}[a-z]?",info:"(?P<info>[\w=]+)",extras:"(?P<extras>[\w=]+)""#
        ))
        .expect("valid regex");
        for part in parts.captures_iter(bundle) {
            let joined = format!("{}{}{}", &seed["seed"], &part["info"], &part["extras"]);
            let Some(encoded) = joined.get(..joined.len().saturating_sub(44)) else {
                continue;
            };
            let secret = URL_SAFE
                .decode(encoded)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok());
            if let Some(secret) = secret
                && !secret.is_empty()
                && !secrets.contains(&secret)
            {
                secrets.push(secret);
            }
        }
    }
    secrets
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_in_the_login_page() {
        let html = r#"<head><script src="/resources/8.1.0-b019/bundle.js"></script></head>"#;
        assert_eq!(bundle_path(html), Some("/resources/8.1.0-b019/bundle.js"));
    }

    #[test]
    fn finds_the_production_app_id() {
        let js = r#"x={development:{api:{appId:"111"}},production:{api:{appId:"798273057",appSecret:"abc"}}}"#;
        assert_eq!(app_id(js).as_deref(), Some("798273057"));
    }

    #[test]
    fn assembles_the_secrets() {
        // "my-secret" in base64 is "bXktc2VjcmV0", split across the three parts, then
        // padded with the 44 characters the bundle adds.
        let padding = "x".repeat(44);
        let js = format!(
            r#"a.initialSeed("bXkt",window.utimezone.berlin)…
            {{name:"Europe/Berlin",info:"c2Vj",extras:"cmV0{padding}"}}"#
        );
        assert_eq!(secrets(&js), ["my-secret"]);
    }

    /// Run with `cargo test -- --ignored` to check the web player still looks the way we expect.
    #[tokio::test]
    #[ignore = "calls play.qobuz.com"]
    async fn reads_the_live_web_player() {
        let web_player = fetch_web_player(&reqwest::Client::new()).await.unwrap();
        assert!(web_player.app_id.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(web_player.secrets, Vec::<String>::new());
    }
}
