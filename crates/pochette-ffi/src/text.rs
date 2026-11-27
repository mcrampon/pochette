//! The words and numbers the screens show, worded as on desktop.

use pochette_core::{format, i18n};

/// Have the texts made here speak the first language of `tags` they can, BCP 47 tags in
/// order of preference, as the app's locales: then US English.
#[uniffi::export]
#[expect(clippy::needless_pass_by_value, reason = "UniFFI hands it over")]
pub fn set_languages(tags: Vec<String>) {
    i18n::set_languages(&tags);
}

/// A track length: "3:33", or "1:02:05" past an hour.
#[uniffi::export]
#[must_use]
pub fn track_duration(seconds: u32) -> String {
    format::track_duration(seconds)
}

/// An album length: "45 min", or "1 h 2 min".
#[uniffi::export]
#[must_use]
pub fn album_duration(seconds: u32) -> String {
    format::album_duration(seconds)
}

/// "1 play", "12 plays".
#[uniffi::export]
#[must_use]
pub fn play_count(count: u32) -> String {
    format::play_count(count)
}

/// Time spent listening: "45 min", "12 h".
#[uniffi::export]
#[must_use]
pub fn listening_time(seconds: u64) -> String {
    format::listening_time(seconds)
}

/// When something happened, in Unix time: "today", "3 days ago", "in March".
#[uniffi::export]
#[must_use]
pub fn ago(at: i64) -> String {
    format::ago(at, pochette_core::time::now())
}

/// How long ago something happened, in Unix time, closer than [`ago`] while it's recent:
/// "just now", "3 minutes ago".
#[uniffi::export]
#[must_use]
pub fn since(at: i64) -> String {
    format::since(at, pochette_core::time::now())
}

/// A month by its number, from 1: "January".
#[uniffi::export]
#[must_use]
pub fn month_name(month: i32) -> String {
    format::month(month)
}

/// A month by its number, from 1: "Jan".
#[uniffi::export]
#[must_use]
pub fn short_month_name(month: i32) -> String {
    format::short_month(month)
}

/// The link to share an album by, on Qobuz.
#[uniffi::export]
#[must_use]
pub fn album_link(id: &str) -> String {
    pochette_core::album_link(id)
}
