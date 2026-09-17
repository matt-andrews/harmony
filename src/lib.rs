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
