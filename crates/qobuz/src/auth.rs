//! Sign in through the user's default browser.
//!
//! The browser opens Qobuz's own sign-in page, so the user can sign in with their saved
//! password, their SSO provider or a session they already have. Qobuz then redirects to a
//! short-lived server on the loopback interface with an authorization code, and the code is
//! exchanged for a user token.
//!
//! ```no_run
//! # async fn run() -> qobuz::Result<()> {
//! let http = reqwest::Client::new();
//! let app_id = qobuz::fetch_app_id(&http).await?;
//! let login = qobuz::auth::BrowserLogin::start(&app_id).await?;
//! // Open `login.url()` in the browser, then:
//! let code = login.wait_for_code().await?;
//! let session = qobuz::auth::exchange_code(&http, &app_id, &code).await?;
//! # Ok(()) }
//! ```

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;

use crate::{API_URL, Error, Result};

const SIGN_IN_URL: &str = "https://www.qobuz.com/signin/oauth";

/// The key the web player sends along with the authorization code.
const OAUTH_PRIVATE_KEY: &str = "6lz8C03UDIC7";

/// Longest request the loopback server accepts; a redirect is far smaller.
const MAX_REQUEST_SIZE: usize = 16 * 1024;

/// What signing in gives us: everything needed to call the API as this user.
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    /// The app the token was issued for; the API only accepts the token with this app id.
    pub app_id: String,
    pub user_id: u64,
    pub user_auth_token: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("app_id", &self.app_id)
            .field("user_id", &self.user_id)
            .field("user_auth_token", &"<redacted>")
            .finish()
    }
}

/// A sign-in in progress: the loopback server is listening for the redirect from Qobuz.
pub struct BrowserLogin {
    url: Url,
    v4: TcpListener,
    /// Browsers may resolve `localhost` to `::1`, so listen there too when IPv6 is available.
    v6: Option<TcpListener>,
}

impl BrowserLogin {
    /// Start listening on a free loopback port.
    pub async fn start(app_id: &str) -> Result<Self> {
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = v4.local_addr()?.port();
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).await.ok();

        let mut url = Url::parse(SIGN_IN_URL).expect("valid URL");
        url.query_pairs_mut()
            .append_pair("ext_app_id", app_id)
            .append_pair("redirect_url", &format!("http://localhost:{port}"));

        Ok(Self { url, v4, v6 })
    }

    /// The Qobuz sign-in page to open in the browser.
    ///
    /// Show it to the user too, so they can open it by hand if no browser could be launched.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Wait for Qobuz to redirect the browser back to us, and return the authorization code.
    ///
    /// This waits forever: wrap it in a timeout or drop it to cancel.
    pub async fn wait_for_code(self) -> Result<String> {
        loop {
            let (stream, peer) = match &self.v6 {
                Some(v6) => tokio::select! {
                    accepted = self.v4.accept() => accepted?,
                    accepted = v6.accept() => accepted?,
                },
                None => self.v4.accept().await?,
            };
            match handle_connection(stream, peer).await {
                Ok(Some(code)) => return Ok(code),
                // A request without the code, such as the browser asking for a favicon.
                Ok(None) => {}
                Err(Error::LoopbackServer(err)) => {
                    tracing::debug!("loopback connection from {peer} failed: {err}");
                }
                Err(err) => return Err(err),
            }
        }
    }
}

/// What the redirect told us.
#[derive(Debug, PartialEq, Eq)]
enum Redirect {
    Code(String),
    Error(String),
    /// Not the redirect: some other request to the loopback server.
    Other,
}

async fn handle_connection(mut stream: TcpStream, peer: SocketAddr) -> Result<Option<String>> {
    let target = read_request_target(&mut stream).await?;
    tracing::debug!("loopback request from {peer}");

    match parse_redirect(target.as_deref().unwrap_or_default()) {
        Redirect::Code(code) => {
            respond(&mut stream, "200 OK", &page(SUCCESS_TITLE, SUCCESS_TEXT)).await?;
            Ok(Some(code))
        }
        Redirect::Error(reason) => {
            respond(&mut stream, "200 OK", &page(FAILURE_TITLE, FAILURE_TEXT)).await?;
            Err(Error::Login(reason))
        }
        Redirect::Other => {
            respond(&mut stream, "404 Not Found", "").await?;
            Ok(None)
        }
    }
}

/// Read the request head and return its target, such as `/?code_autorisation=…`.
async fn read_request_target(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST_SIZE {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
    }

    let head = String::from_utf8_lossy(&buf);
    let mut request_line = head.lines().next().unwrap_or_default().split(' ');
    Ok(match (request_line.next(), request_line.next()) {
        (Some("GET"), Some(target)) => Some(target.to_owned()),
        _ => None,
    })
}

fn parse_redirect(target: &str) -> Redirect {
    let Ok(url) = Url::parse("http://localhost").and_then(|base| base.join(target)) else {
        return Redirect::Other;
    };
    if url.path() != "/" {
        return Redirect::Other;
    }

    let mut error = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            // Qobuz names it in French.
            "code_autorisation" | "code" if !value.is_empty() => {
                return Redirect::Code(value.into_owned());
            }
            "error" | "error_description" => error = Some(value.into_owned()),
            _ => {}
        }
    }
    error.map_or(Redirect::Other, Redirect::Error)
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Referrer-Policy: no-referrer\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

const SUCCESS_TITLE: &str = "You're signed in";
const SUCCESS_TEXT: &str = "You can close this tab and go back to Pochette.";
const FAILURE_TITLE: &str = "Sign-in failed";
const FAILURE_TEXT: &str = "Qobuz didn't sign you in. Go back to Pochette to try again.";

fn page(title: &str, text: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Pochette · {title}</title>
<style>
  :root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
  body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; }}
  main {{ text-align: center; padding: 2rem; }}
  h1 {{ font-weight: 600; margin: 0 0 .5rem; }}
  p {{ margin: 0; opacity: .7; }}
</style>
</head>
<body><main><h1>{title}</h1><p>{text}</p></main></body>
</html>"#
    )
}

/// Exchange the authorization code from the redirect for a user token.
pub async fn exchange_code(http: &reqwest::Client, app_id: &str, code: &str) -> Result<Session> {
    #[derive(Deserialize)]
    struct Callback {
        token: Option<String>,
        user_auth_token: Option<String>,
        user_id: Option<serde_json::Value>,
    }

    let response = http
        .get(format!("{API_URL}oauth/callback"))
        .header("X-App-Id", app_id)
        .query(&[("code", code), ("private_key", OAUTH_PRIVATE_KEY)])
        .send()
        .await?;
    let callback: Callback = crate::client::read_json(response).await?;

    let user_auth_token = callback
        .token
        .or(callback.user_auth_token)
        .ok_or_else(|| Error::UnexpectedResponse("no token in oauth/callback".into()))?;
    // The user id comes as a number or as a string.
    let user_id = match callback.user_id {
        Some(serde_json::Value::Number(n)) => n.as_u64(),
        Some(serde_json::Value::String(s)) => s.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| Error::UnexpectedResponse("no user id in oauth/callback".into()))?;

    Ok(Session {
        app_id: app_id.to_owned(),
        user_id,
        user_auth_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_code_from_the_redirect() {
        assert_eq!(
            parse_redirect("/?code_autorisation=abc%2B123&foo=bar"),
            Redirect::Code("abc+123".into())
        );
    }

    #[test]
    fn reads_errors_from_the_redirect() {
        assert_eq!(
            parse_redirect("/?error=access_denied"),
            Redirect::Error("access_denied".into())
        );
    }

    #[test]
    fn ignores_other_requests() {
        assert_eq!(parse_redirect("/favicon.ico"), Redirect::Other);
        assert_eq!(parse_redirect("/"), Redirect::Other);
        assert_eq!(parse_redirect("/?code_autorisation="), Redirect::Other);
    }

    #[tokio::test]
    async fn serves_the_redirect_and_returns_the_code() {
        let login = BrowserLogin::start("123").await.unwrap();
        let redirect: Url = login
            .url()
            .query_pairs()
            .find(|(k, _)| k == "redirect_url")
            .unwrap()
            .1
            .parse()
            .unwrap();
        let port = redirect.port().unwrap();
        let waiting = tokio::spawn(login.wait_for_code());

        let mut favicon = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        favicon
            .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = String::new();
        favicon.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 404"));

        let mut browser = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        browser
            .write_all(b"GET /?code_autorisation=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = String::new();
        browser.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains(SUCCESS_TITLE));

        assert_eq!(waiting.await.unwrap().unwrap(), "xyz");
    }
}
