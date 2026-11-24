//! What goes wrong, as Kotlin catches it.

/// Why a call failed, with a message to show: Kotlin gets it as the exception's message.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
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

/// `err` and what caused it: reqwest's own message alone says little ("error sending
/// request").
fn with_causes(err: &dyn std::error::Error) -> String {
    let mut message = err.to_string();
    let mut cause = err.source();
    while let Some(err) = cause {
        let text = err.to_string();
        if !message.contains(&text) {
            message = format!("{message}: {text}");
        }
        cause = err.source();
    }
    message
}

impl From<qobuz::Error> for PochetteError {
    fn from(err: qobuz::Error) -> Self {
        let message = with_causes(&err);
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
            message: with_causes(&err),
        }
    }
}
