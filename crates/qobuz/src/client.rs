use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
    API_URL, Error, Result,
    auth::Session,
    models::{Album, Page, Track, User},
    web_player::fetch_web_player,
};

/// The Qobuz API, called as a signed-in user.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    session: Session,
    /// The secret that signs streaming requests, once found. Shared by all clones.
    secret: Arc<Mutex<Option<String>>>,
}

impl Client {
    #[must_use]
    pub fn new(http: reqwest::Client, session: Session) -> Self {
        Self {
            http,
            session,
            secret: Arc::default(),
        }
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

    /// Add an album to the user's favorites, which is their library.
    pub async fn add_favorite_album(&self, id: &str) -> Result<()> {
        let response = self
            .post("favorite/create")
            .form(&[("album_ids", id)])
            .send()
            .await?;
        read_json::<serde_json::Value>(response).await.map(drop)
    }

    /// Remove an album from the user's favorites.
    pub async fn remove_favorite_album(&self, id: &str) -> Result<()> {
        let response = self
            .post("favorite/delete")
            .form(&[("album_ids", id)])
            .send()
            .await?;
        read_json::<serde_json::Value>(response).await.map(drop)
    }

    /// An album, with its tracks and review.
    pub async fn album(&self, id: &str) -> Result<Album> {
        let response = self
            .get("album/get")
            .query(&[("album_id", id)])
            .send()
            .await?;
        read_json(response).await
    }

    /// A track, with the album it's on.
    pub async fn track(&self, id: u64) -> Result<Track> {
        let response = self
            .get("track/get")
            .query(&[("track_id", id)])
            .send()
            .await?;
        read_json(response).await
    }

    /// Where to stream a track from, in the best quality up to `quality` that the track and
    /// the subscription allow.
    ///
    /// Signs the request with the web player's secret for the user's region, found on the
    /// first call by trying each.
    pub async fn file_url(&self, track_id: u64, quality: Quality) -> Result<FileUrl> {
        let mut secret = self.secret.lock().await;
        if let Some(known) = secret.as_deref() {
            match self.signed_file_url(track_id, quality, known).await {
                // The web player changed its secrets: find the new one.
                Err(Error::Api {
                    status: 400,
                    message,
                }) => {
                    tracing::info!("the signing secret no longer works: {message}");
                    *secret = None;
                }
                result => return result,
            }
        }

        let web_player = fetch_web_player(&self.http).await?;
        if web_player.app_id != self.session.app_id {
            tracing::warn!("the web player's app id changed since signing in");
        }
        let mut last_error = None;
        for candidate in web_player.secrets {
            match self.signed_file_url(track_id, quality, &candidate).await {
                Ok(file_url) => {
                    *secret = Some(candidate);
                    return Ok(file_url);
                }
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or(Error::NoSigningSecret))
    }

    async fn signed_file_url(
        &self,
        track_id: u64,
        quality: Quality,
        secret: &str,
    ) -> Result<FileUrl> {
        let timestamp = unix_time().to_string();
        let track_id = track_id.to_string();
        let format_id = (quality as u8).to_string();
        let signature = format!(
            "{:x}",
            md5::compute(format!(
                "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{secret}"
            ))
        );
        let response = self
            .get("track/getFileUrl")
            .query(&[
                ("request_ts", timestamp.as_str()),
                ("request_sig", signature.as_str()),
                ("track_id", track_id.as_str()),
                ("format_id", format_id.as_str()),
                ("intent", "stream"),
            ])
            .send()
            .await?;
        read_json(response).await
    }

    /// Tell Qobuz a track started streaming, once per track, as the web player does.
    pub async fn report_streaming_start(&self, track_id: u64, format_id: u32) -> Result<()> {
        let events = serde_json::json!([{
            "track_id": track_id,
            "date": unix_time(),
            "user_id": self.session.user_id,
            "format_id": format_id,
        }]);
        let response = self
            .post("track/reportStreamingStart")
            .form(&[("events", events.to_string())])
            .send()
            .await?;
        read_json::<serde_json::Value>(response).await.map(drop)
    }

    /// Tell Qobuz how long a track was streamed without interruption, which is what pays
    /// the artists.
    pub async fn report_streaming_end(&self, event: &StreamingEnd) -> Result<()> {
        let report = serde_json::json!({
            "events": [event],
            "renderer_context": {"software_version": concat!("pochette-", env!("CARGO_PKG_VERSION"))},
        });
        let response = self
            .post("track/reportStreamingEndJson")
            .json(&report)
            .send()
            .await?;
        read_json::<serde_json::Value>(response).await.map(drop)
    }

    pub(crate) fn get(&self, endpoint: &str) -> RequestBuilder {
        self.http
            .get(format!("{API_URL}{endpoint}"))
            .header("X-App-Id", &self.session.app_id)
            .header("X-User-Auth-Token", &self.session.user_auth_token)
    }

    fn post(&self, endpoint: &str) -> RequestBuilder {
        self.http
            .post(format!("{API_URL}{endpoint}"))
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

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The best quality to ask for. Qobuz answers with the best that the track and the
/// subscription allow, up to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Quality {
    Mp3 = 5,
    /// FLAC, 16-bit / 44.1 kHz.
    Cd = 6,
    /// FLAC, up to 24-bit / 96 kHz.
    HiRes96 = 7,
    /// FLAC, up to 24-bit / 192 kHz.
    #[default]
    HiRes192 = 27,
}

impl Quality {
    /// From the lowest to the highest.
    pub const ALL: [Self; 4] = [Self::Mp3, Self::Cd, Self::HiRes96, Self::HiRes192];

    /// The quality of a [`FileUrl::format_id`].
    #[must_use]
    pub fn of_format(format_id: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|quality| *quality as u32 == format_id)
    }
}

/// Where a track streams from.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileUrl {
    pub url: String,
    /// The [`Quality`] Qobuz picked.
    pub format_id: u32,
    pub mime_type: Option<String>,
    /// In kHz.
    pub sampling_rate: Option<f32>,
    pub bit_depth: Option<u32>,
    /// Identifies this stream in the streaming reports.
    pub blob: Option<String>,
    /// Whether it's only a 30-second preview, as for free accounts.
    #[serde(default)]
    pub sample: bool,
}

/// A stretch of a track streamed without interruption.
#[derive(Debug, Clone, Serialize)]
pub struct StreamingEnd {
    /// The [`FileUrl::blob`] of the stream.
    pub blob: String,
    /// Tells the stretches of one play of a track apart from those of another.
    pub track_context_uuid: String,
    /// When the stretch started, as `2026-11-16T19:00:00.000Z`.
    pub start_stream: String,
    /// Always true: streamed, not played from a download.
    pub online: bool,
    /// Always false: played here, not on another device.
    pub local: bool,
    /// In seconds.
    pub duration: u64,
}
