//! HTTP layer. Every mutation goes through [`AppState::mutate`], which applies
//! the change to a copy, persists it, and only then swaps it in.

mod projects;
mod report;
mod sessions;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;

use crate::domain::views::StateView;
use crate::domain::{AppData, DomainError};
use crate::error::AppError;
use crate::storage::{Etag, Storage};

struct Persisted {
    data: AppData,
    etag: Option<Etag>,
}

pub struct AppState {
    persisted: Mutex<Persisted>,
    storage: Storage,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(data: AppData, etag: Option<Etag>, storage: Storage) -> Self {
        Self {
            persisted: Mutex::new(Persisted { data, etag }),
            storage,
        }
    }

    pub async fn state_view(&self) -> StateView {
        let now = Utc::now();
        self.persisted.lock().await.data.state_view(now)
    }

    /// Run `f` against the data and return its result. Read-only.
    pub async fn read<T>(&self, f: impl FnOnce(&AppData, DateTime<Utc>) -> T) -> T {
        let now = Utc::now();
        f(&self.persisted.lock().await.data, now)
    }

    /// Apply `f` to a copy of the data, persist it, and commit on success.
    /// A failed validation or save leaves memory untouched.
    pub async fn mutate<T>(
        &self,
        f: impl FnOnce(&mut AppData, DateTime<Utc>) -> Result<T, DomainError>,
    ) -> Result<(T, StateView), AppError> {
        let now = Utc::now();
        let mut guard = self.persisted.lock().await;
        let mut next = guard.data.clone();
        let out = f(&mut next, now)?;
        let etag = self.storage.save(&next, guard.etag.as_ref()).await?;
        guard.data = next;
        guard.etag = etag;
        let view = guard.data.state_view(now);
        Ok((out, view))
    }
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/api/state", get(report::state))
        .route("/api/report", get(report::report))
        .route("/api/projects", post(projects::create))
        .route("/api/projects/{id}", axum::routing::patch(projects::update))
        .route("/api/projects/{id}/summary", get(projects::summary))
        .route("/api/sessions/start", post(sessions::start))
        .route("/api/sessions/stop", post(sessions::stop))
        .route(
            "/api/sessions/{id}",
            axum::routing::patch(sessions::update).delete(sessions::delete),
        )
        .fallback(crate::frontend::serve)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Deserialize helper distinguishing "field absent" (`None`) from
/// "field present but null" (`Some(None)`). Use with
/// `#[serde(default, deserialize_with = "double_option")]` on `Option<Option<T>>`.
pub(crate) fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

use serde::Deserialize;
