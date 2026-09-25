use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use super::SharedState;
use crate::domain::views::StateView;
use crate::error::AppError;

pub async fn complete(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StateView>, AppError> {
    let (_, view) = state
        .mutate(|data, _| data.complete_task(id).map(|_| ()))
        .await?;
    Ok(Json(view))
}

pub async fn reopen(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StateView>, AppError> {
    let (_, view) = state
        .mutate(|data, _| data.reopen_task(id).map(|_| ()))
        .await?;
    Ok(Json(view))
}
