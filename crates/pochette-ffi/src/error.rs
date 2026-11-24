//! What goes wrong, as Kotlin catches it.

/// Why a call failed, with a message to show.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum PochetteError {
    /// Not signed in, or Qobuz no longer takes the session: sign in again.
    #[error("{message}")]
    SignedOut { message: String },
    /// Qobuz, or the network, can't be reached.
    #[error("{message}")]
    Network { message: String },
    /// Qobuz answered, but not with what was asked.
    #[error("{message}")]
    Qobuz { message: String },
    #[error("{message}")]
    Failed { message: String },
}

impl From<qobuz::Error> for PochetteError {
    fn from(err: qobuz::Error) -> Self {
        let message = err.to_string();
        match err {
            qobuz::Error::Unauthorized { .. } => Self::SignedOut { message },
            qobuz::Error::Http(_) => Self::Network { message },
            _ => Self::Qobuz { message },
        }
    }
}

impl From<reqwest::Error> for PochetteError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network {
            message: err.to_string(),
        }
    }
}
