use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;
use tracing::error;

use crate::domain::DomainError;
use crate::storage::StorageError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("{0}")]
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Domain(DomainError::NotFound(_)) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Domain(DomainError::Conflict(_)) => (StatusCode::CONFLICT, self.to_string()),
            AppError::Domain(DomainError::Invalid(_)) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Storage(StorageError::Conflict) => (StatusCode::CONFLICT, self.to_string()),
            AppError::Storage(e) => {
                error!("storage error: {e:#}");
                (StatusCode::BAD_GATEWAY, format!("storage error: {e:#}"))
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
