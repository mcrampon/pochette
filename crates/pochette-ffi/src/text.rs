//! The words and numbers the screens show, worded as on desktop.

use pochette_core::format;

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

/// A month by its number, from 1: "January".
#[uniffi::export]
#[must_use]
pub fn month_name(month: i32) -> String {
    format::month(month).to_owned()
}

/// A month by its number, from 1: "Jan".
#[uniffi::export]
#[must_use]
pub fn short_month_name(month: i32) -> String {
    format::short_month(month).to_owned()
}
