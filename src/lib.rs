//! Harmony: a tiny personal time tracker for contract RLHF work.
//!
//! Layers, top to bottom:
//! - [`api`]: axum handlers and the in-memory [`api::AppState`].
//! - [`domain`]: pure data model, mutations and read models.
//! - [`storage`]: the single JSON document, on Azure Blob or a local file.

pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod frontend;
pub mod storage;

pub use api::{AppState, router};

/// What the UI shows in its corner: the release tag version that CI injects
/// at compile time (`HARMONY_VERSION`), or `dev` for any other build.
pub const VERSION: &str = match option_env!("HARMONY_VERSION") {
    Some(v) if !v.is_empty() => v,
    _ => "dev",
};
