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

/// How many times to try reaching Qobuz with the code, a second apart: Android keeps an app
/// in the background off the network, and the app may not be in front yet.
const EXCHANGE_TRIES: u32 = 10;

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

    /// Start signing in: open the [`SignIn::url`] in a browser, wait for
    /// [`SignIn::redirected`], then [`SignIn::finish`]. Once signed in on the page, the
    /// browser goes on to `return_to`, a link that brings the app back.
    pub async fn begin_sign_in(&self, return_to: String) -> Result<Arc<SignIn>> {
        let inner = Arc::clone(&self.inner);
        run(async move {
            let app_id = qobuz::fetch_app_id(&inner.http).await?;
            let login = BrowserLogin::start(&app_id).await?.return_to(&return_to);
            Ok(Arc::new(SignIn {
                url: login.url().to_string(),
                app_id,
                progress: Mutex::new(Progress::Waiting(login)),
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
    app_id: String,
    progress: Mutex<Progress>,
    inner: Arc<Inner>,
}

enum Progress {
    /// For the user to sign in on the page.
    Waiting(BrowserLogin),
    /// Signed in on the page: the code to trade for a session.
    Redirected(String),
    Over,
}

fn over() -> PochetteError {
    PochetteError::Failed {
        message: "This sign-in is over: start another".to_owned(),
    }
}

#[uniffi::export]
impl SignIn {
    /// Qobuz's sign-in page, to open in a browser.
    #[must_use]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Wait for the user to sign in on the page, up to 10 minutes: then bring the app to
    /// the front, if the browser didn't, and [`SignIn::finish`]. Cancel the call to give up.
    pub async fn redirected(&self) -> Result<()> {
        let progress = std::mem::replace(&mut *lock(&self.progress), Progress::Over);
        let Progress::Waiting(login) = progress else {
            return Err(over());
        };
        let code = run(async move {
            match tokio::time::timeout(SIGN_IN_TIMEOUT, login.wait_for_code()).await {
                Ok(code) => Ok(code?),
                Err(_) => Err(PochetteError::Failed {
                    message: "The sign-in took too long. Try again.".to_owned(),
                }),
            }
        })
        .await?;
        *lock(&self.progress) = Progress::Redirected(code);
        Ok(())
    }

    /// Trade the code from the page for a session, and sign in with it.
    pub async fn finish(&self) -> Result<Account> {
        let progress = std::mem::replace(&mut *lock(&self.progress), Progress::Over);
        let Progress::Redirected(code) = progress else {
            return Err(over());
        };
        let inner = Arc::clone(&self.inner);
        let app_id = self.app_id.clone();
        run(async move {
            let mut tries = 1;
            let session = loop {
                match qobuz::auth::exchange_code(&inner.http, &app_id, &code).await {
                    // Qobuz wasn't reached, so the code is still good.
                    Err(qobuz::Error::Http(err)) if err.is_connect() && tries < EXCHANGE_TRIES => {
                        tracing::info!("cannot reach Qobuz yet: {err}");
                        tries += 1;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    result => break result?,
                }
            };
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
