use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::SharedState;
use crate::domain::views::{Report, StateView};
use crate::error::AppError;

pub async fn state(State(state): State<SharedState>) -> Json<StateView> {
    Json(state.state_view().await)
}

#[derive(Deserialize)]
pub struct ReportQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

pub async fn report(
    State(state): State<SharedState>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<Report>, AppError> {
    if q.to <= q.from {
        return Err(AppError::BadRequest("`to` must be after `from`".into()));
    }
    let report = state.read(|data, now| data.report(q.from, q.to, now)).await;
    Ok(Json(report))
}
