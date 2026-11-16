//! Unofficial client for the Qobuz API.
//!
//! This crate knows nothing about any UI: it finds the web player's app id,
//! runs the browser login and calls the API.

pub mod auth;
mod client;
mod error;
pub mod models;
mod web_player;

pub use client::{Client, FileUrl, Quality, StreamingEnd};
pub use error::{Error, Result};
pub use web_player::{WebPlayer, fetch_app_id, fetch_web_player};

/// Base URL of every API endpoint.
pub(crate) const API_URL: &str = "https://www.qobuz.com/api.json/0.2/";
