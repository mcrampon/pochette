//! Unofficial client for the Qobuz API.
//!
//! This crate knows nothing about any UI: it finds the web player's app id,
//! runs the browser login and calls the API.

pub mod auth;
mod client;
mod error;
pub mod models;
mod web_player;

pub use client::Client;
pub use error::{Error, Result};
pub use web_player::fetch_app_id;

/// Base URL of every API endpoint.
pub(crate) const API_URL: &str = "https://www.qobuz.com/api.json/0.2/";
