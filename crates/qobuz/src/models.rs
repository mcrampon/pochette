//! What the API returns.
//!
//! Fields are optional unless Qobuz always sends them: its responses vary between accounts.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: u64,
    pub login: Option<String>,
    pub display_name: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub avatar: Option<String>,
    pub country_code: Option<String>,
    pub credential: Option<Credential>,
}

impl User {
    /// The name to greet the user with.
    #[must_use]
    pub fn name(&self) -> &str {
        [&self.display_name, &self.firstname, &self.login]
            .into_iter()
            .flatten()
            .find(|name| !name.is_empty())
            .map_or("Qobuz user", String::as_str)
    }

    /// Whether the subscription allows streaming full tracks.
    ///
    /// Free accounts have no credential parameters and only get 30 second samples.
    #[must_use]
    pub fn can_stream(&self) -> bool {
        self.credential
            .as_ref()
            .is_some_and(|c| c.parameters.is_some())
    }
}

/// The user's subscription.
#[derive(Debug, Clone, Deserialize)]
pub struct Credential {
    /// The plan's name, such as "Studio".
    pub label: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<CredentialParameters>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CredentialParameters {
    #[serde(default)]
    pub lossy_streaming: bool,
    #[serde(default)]
    pub lossless_streaming: bool,
    #[serde(default)]
    pub hires_streaming: bool,
    pub short_label: Option<String>,
}
