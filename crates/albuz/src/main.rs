//! Albuz, a native Linux player for Qobuz, built around albums.
//!
//! For now this only signs in: run `albuz` to sign in (or check the stored session) and
//! `albuz logout` to forget it.

mod session_store;

use std::time::Duration;

use qobuz::{Client, auth::BrowserLogin, models::User};
use session_store::SessionStore;
use tracing_subscriber::EnvFilter;

/// How long to wait for the user to finish signing in in the browser.
const LOGIN_TIMEOUT: Duration = Duration::from_mins(10);

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let store = SessionStore::open().await;
    let http = reqwest_client()?;

    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("logout") => {
            store.clear().await?;
            println!("Signed out.");
            return Ok(());
        }
        Some(other) => return Err(format!("unknown command {other:?}").into()),
    }

    let (client, user) = match resume(&store, &http).await? {
        Some(signed_in) => signed_in,
        None => sign_in(&store, &http).await?,
    };

    let plan = user
        .credential
        .as_ref()
        .and_then(|c| c.label.as_deref())
        .unwrap_or("no subscription");
    println!("Signed in as {} ({plan}).", user.name());
    if !user.can_stream() {
        println!("This account can only play 30 second samples.");
    }
    tracing::debug!("session: {:?}", client.session());
    Ok(())
}

fn reqwest_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("Albuz/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Pick up the stored session, if it still works.
async fn resume(store: &SessionStore, http: &reqwest::Client) -> Result<Option<(Client, User)>> {
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
            Ok(Some((client, user)))
        }
        Err(qobuz::Error::Unauthorized { message }) => {
            tracing::info!("the stored session expired ({message}), signing in again");
            store.clear().await?;
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

/// Sign in through the default browser.
async fn sign_in(store: &SessionStore, http: &reqwest::Client) -> Result<(Client, User)> {
    let app_id = qobuz::fetch_app_id(http).await?;
    let login = BrowserLogin::start(&app_id).await?;

    println!("Sign in to Qobuz in your browser.");
    println!("If it didn't open, go to:\n\n  {}\n", login.url());
    if let Err(err) = open::that_detached(login.url().as_str()) {
        tracing::warn!("cannot open the browser: {err}");
    }

    let code = tokio::time::timeout(LOGIN_TIMEOUT, login.wait_for_code())
        .await
        .map_err(|_| "the sign-in timed out")??;
    let session = qobuz::auth::exchange_code(http, &app_id, &code).await?;

    let mut client = Client::new(http.clone(), session);
    let user = client.login().await?;
    store.save(client.session()).await?;
    Ok((client, user))
}
