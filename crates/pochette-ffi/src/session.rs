//! Signing in to Qobuz, through its own page in the browser, as on desktop, and keeping the
//! session: Kotlin stores it encrypted, and hands it back at the next start.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use qobuz::{
    Client,
    auth::{BrowserLogin, Session},
};

use crate::{Account, Inner, Pochette, PochetteError, Result, lock, run};

/// How long the user has to sign in, once the page opened.
const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(10);

/// The signed-in user, and their client.
pub(crate) struct SignedIn {
    pub client: Client,
    pub account: Account,
}

#[uniffi::export]
impl Pochette {
    /// Sign in with a session kept from before: see [`Pochette::session`]. Fails with
    /// `SignedOut` when Qobuz no longer takes it.
    pub async fn restore(&self, session: String) -> Result<Account> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let session: Session =
                serde_json::from_str(&session).map_err(|err| PochetteError::SignedOut {
                    message: format!("The kept session can't be read: {err}"),
                })?;
            inner
                .sign_in(Client::new(inner.http.clone(), session))
                .await
        })
        .await
    }

    /// Start signing in: open the [`SignIn::url`] in a browser, then wait for
    /// [`SignIn::finish`].
    pub async fn begin_sign_in(&self) -> Result<Arc<SignIn>> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let app_id = qobuz::fetch_app_id(&inner.http).await?;
            let login = BrowserLogin::start(&app_id).await?;
            Ok(Arc::new(SignIn {
                url: login.url().to_string(),
                login: Mutex::new(Some((login, app_id))),
                inner,
            }))
        })
        .await
    }

    /// The session to keep, encrypted, for [`Pochette::restore`]: after signing in, and
    /// after restoring, as Qobuz may have renewed it.
    #[must_use]
    pub fn session(&self) -> Option<String> {
        let account = lock(&self.inner.account);
        let session = account.as_ref()?.client.session();
        serde_json::to_string(session).ok()
    }

    /// The signed-in user.
    #[must_use]
    pub fn account(&self) -> Option<Account> {
        lock(&self.inner.account)
            .as_ref()
            .map(|signed_in| signed_in.account.clone())
    }

    /// Forget the session and what came with it. The play history stays.
    pub fn sign_out(&self) {
        *lock(&self.inner.account) = None;
        lock(&self.inner.favorites).clear();
        lock(&self.inner.albums).clear();
    }
}

/// A sign-in in progress: Qobuz's page, and the server on this device that its redirect
/// comes back to.
#[derive(uniffi::Object)]
pub struct SignIn {
    url: String,
    login: Mutex<Option<(BrowserLogin, String)>>,
    inner: Arc<Inner>,
}

#[uniffi::export]
impl SignIn {
    /// Qobuz's sign-in page, to open in a browser.
    #[must_use]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Wait for the user to sign in on the page, up to 10 minutes. Cancel the call to give
    /// up.
    pub async fn finish(&self) -> Result<Account> {
        let Some((login, app_id)) = lock(&self.login).take() else {
            return Err(PochetteError::Failed {
                message: "This sign-in is over: start another".to_owned(),
            });
        };
        let inner = Arc::clone(&self.inner);
        run(async move {
            let code = tokio::time::timeout(SIGN_IN_TIMEOUT, login.wait_for_code())
                .await
                .map_err(|_| PochetteError::Failed {
                    message: "The sign-in took too long. Try again.".to_owned(),
                })??;
            let session = qobuz::auth::exchange_code(&inner.http, &app_id, &code).await?;
            inner
                .sign_in(Client::new(inner.http.clone(), session))
                .await
        })
        .await
    }
}

impl Inner {
    /// Check `client`'s session with Qobuz, and sign in with it.
    async fn sign_in(&self, mut client: Client) -> Result<Account> {
        let user = client.login().await?;
        let account = Account::from(&user);
        *lock(&self.account) = Some(SignedIn {
            client,
            account: account.clone(),
        });
        Ok(account)
    }
}
