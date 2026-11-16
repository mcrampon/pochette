//! Qobuz doesn't hand out app ids anymore, so we use the one the web player ships with.

use regex::Regex;

use crate::{Error, Result};

const PLAY_URL: &str = "https://play.qobuz.com";

/// Read the app id from the current web player bundle.
///
/// The bundle path changes with each web player release, so it's read from the login page first.
pub async fn fetch_app_id(http: &reqwest::Client) -> Result<String> {
    let login_page = http
        .get(format!("{PLAY_URL}/login"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let bundle_path = bundle_path(&login_page).ok_or(Error::AppIdNotFound)?;
    tracing::debug!("reading app id from web player bundle {bundle_path}");

    let bundle = http
        .get(format!("{PLAY_URL}{bundle_path}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    app_id(&bundle).ok_or(Error::AppIdNotFound)
}

fn bundle_path(login_page: &str) -> Option<&str> {
    let re = Regex::new(r#"<script src="(/resources/[^"]+/bundle\.js)""#).expect("valid regex");
    Some(re.captures(login_page)?.get(1)?.as_str())
}

fn app_id(bundle: &str) -> Option<String> {
    let re = Regex::new(r#"production:\{api:\{appId:"(\d+)""#).expect("valid regex");
    Some(re.captures(bundle)?.get(1)?.as_str().to_owned())
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

    /// Run with `cargo test -- --ignored` to check the web player still looks the way we expect.
    #[tokio::test]
    #[ignore = "calls play.qobuz.com"]
    async fn reads_the_app_id_from_the_live_web_player() {
        let app_id = fetch_app_id(&reqwest::Client::new()).await.unwrap();
        assert!(app_id.chars().all(|c| c.is_ascii_digit()), "{app_id}");
    }
}
