use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    API_URL, Error, Result,
    auth::Session,
    models::{Album, Page, User},
};

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

    /// All the albums the user saved to their favorites, in the order Qobuz returns them.
    pub async fn favorite_albums(&self) -> Result<Vec<Album>> {
        #[derive(Deserialize)]
        struct Response {
            albums: Page<Album>,
        }

        /// The most Qobuz returns at once.
        const PAGE_SIZE: usize = 500;

        let mut albums = Vec::new();
        loop {
            let response = self
                .get("favorite/getUserFavorites")
                .query(&[
                    ("type", "albums".to_owned()),
                    ("limit", PAGE_SIZE.to_string()),
                    ("offset", albums.len().to_string()),
                ])
                .send()
                .await?;
            let page = read_json::<Response>(response).await?.albums;
            let last = page.items.is_empty() || albums.len() + page.items.len() >= page.total;
            albums.extend(page.items);
            if last {
                return Ok(albums);
            }
        }
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
