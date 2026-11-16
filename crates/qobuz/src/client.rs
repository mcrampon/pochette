use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{API_URL, Error, Result, auth::Session, models::User};

/// The Qobuz API, called as a signed-in user.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    session: Session,
}

impl Client {
    #[must_use]
    pub fn new(http: reqwest::Client, session: Session) -> Self {
        Self { http, session }
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Check the token and fetch the user's profile and subscription.
    ///
    /// Fails with [`Error::Unauthorized`] when the token no longer works: sign in again.
    /// Qobuz may hand out a renewed token, which then replaces the current one; check
    /// [`Client::session`] afterwards to persist it.
    pub async fn login(&mut self) -> Result<User> {
        #[derive(Deserialize)]
        struct LoginResponse {
            user: User,
            user_auth_token: Option<String>,
        }

        let user_id = self.session.user_id.to_string();
        let response = self
            .get("user/login")
            .query(&[
                ("user_id", user_id.as_str()),
                ("user_auth_token", self.session.user_auth_token.as_str()),
            ])
            .send()
            .await?;
        let response: LoginResponse = read_json(response).await?;

        if let Some(token) = response.user_auth_token
            && token != self.session.user_auth_token
        {
            tracing::debug!("Qobuz renewed the user token");
            self.session.user_auth_token = token;
        }
        Ok(response.user)
    }

    fn get(&self, endpoint: &str) -> RequestBuilder {
        self.http
            .get(format!("{API_URL}{endpoint}"))
            .header("X-App-Id", &self.session.app_id)
            .header("X-User-Auth-Token", &self.session.user_auth_token)
    }
}

/// Read a JSON response, turning Qobuz's error responses into [`Error`]s.
pub(crate) async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T> {
    #[derive(Deserialize)]
    struct ApiError {
        message: Option<String>,
    }

    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        return serde_json::from_str(&body).map_err(|err| {
            tracing::debug!("cannot read Qobuz response: {err}: {body}");
            Error::UnexpectedResponse(err.to_string())
        });
    }

    let message = serde_json::from_str::<ApiError>(&body)
        .ok()
        .and_then(|e| e.message)
        .unwrap_or(body);
    Err(match status {
        StatusCode::UNAUTHORIZED => Error::Unauthorized { message },
        _ => Error::Api {
            status: status.as_u16(),
            message,
        },
    })
}
