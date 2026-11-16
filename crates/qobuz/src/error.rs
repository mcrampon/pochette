pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    /// The token is missing, expired or revoked: the user must sign in again.
    #[error("Qobuz rejected the credentials: {message}")]
    Unauthorized { message: String },

    #[error("Qobuz API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("unexpected response from Qobuz: {0}")]
    UnexpectedResponse(String),

    #[error("could not find the app id in the Qobuz web player")]
    AppIdNotFound,

    #[error("the browser login failed: {0}")]
    Login(String),

    #[error("could not listen for the browser login: {0}")]
    LoopbackServer(#[from] std::io::Error),
}
