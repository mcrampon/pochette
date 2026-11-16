//! Signing in and out, without any UI.

use std::time::Duration;

use qobuz::{Client, auth::BrowserLogin, models::User};

use crate::session_store::{self, SessionStore};

/// How long the sign-in page stays usable before we stop listening for its redirect.
const LOGIN_TIMEOUT: Duration = Duration::from_mins(10);

/// The signed-in user, and the client that calls the API on their behalf.
#[derive(Debug, Clone)]
pub struct Account {
    #[expect(dead_code, reason = "the library screens will call the API through it")]
    pub client: Client,
    pub user: User,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Qobuz(#[from] qobuz::Error),
    #[error("Can't save the session: {0}")]
    Store(#[from] session_store::Error),
    #[error("The sign-in took too long. Try again.")]
    TimedOut,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Albuz/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("the TLS backend initializes")
}

/// Pick up the stored session, if there is one and it still works.
pub async fn resume(
    store: &SessionStore,
    http: &reqwest::Client,
) -> Result<Option<Account>, Error> {
    let session = match store.load().await {
        Ok(Some(session)) => session,
        Ok(None) => return Ok(None),
        Err(err) => {
            tracing::warn!("cannot read the stored session, signing in again: {err}");
            return Ok(None);
        }
    };

    let mut client = Client::new(http.clone(), session);
    match client.login().await {
        Ok(user) => {
            store.save(client.session()).await?;
            Ok(Some(Account { client, user }))
        }
        Err(qobuz::Error::Unauthorized { message }) => {
            tracing::info!("the stored session expired ({message}), signing in again");
            store.clear().await?;
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

/// Sign in through the browser.
///
/// `open_browser` gets the sign-in page once it's ready; the user signs in there, and this
/// returns when Qobuz redirects back. Drop the future to cancel.
pub async fn sign_in(
    store: &SessionStore,
    http: &reqwest::Client,
    open_browser: impl FnOnce(String),
) -> Result<Account, Error> {
    let app_id = qobuz::fetch_app_id(http).await?;
    let login = BrowserLogin::start(&app_id).await?;
    open_browser(login.url().to_string());

    let code = tokio::time::timeout(LOGIN_TIMEOUT, login.wait_for_code())
        .await
        .map_err(|_| Error::TimedOut)??;
    let session = qobuz::auth::exchange_code(http, &app_id, &code).await?;

    let mut client = Client::new(http.clone(), session);
    let user = client.login().await?;
    store.save(client.session()).await?;
    Ok(Account { client, user })
}
