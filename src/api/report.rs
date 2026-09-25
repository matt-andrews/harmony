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
    /// Completion window for the payout section: tasks turned in inside
    /// `[completed_from, completed_to)` are payable within `[from, to)`.
    /// The client computes it (period minus the payout delay, in its own
    /// calendar days). Both or neither.
    #[serde(default)]
    pub completed_from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub completed_to: Option<DateTime<Utc>>,
}

pub async fn report(
    State(state): State<SharedState>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<Report>, AppError> {
    if q.to <= q.from {
        return Err(AppError::BadRequest("`to` must be after `from`".into()));
    }
    let completed = match (q.completed_from, q.completed_to) {
        (None, None) => None,
        (Some(a), Some(b)) if b > a => Some((a, b)),
        (Some(_), Some(_)) => {
            return Err(AppError::BadRequest(
                "`completed_to` must be after `completed_from`".into(),
            ));
        }
        _ => {
            return Err(AppError::BadRequest(
                "`completed_from` and `completed_to` go together".into(),
            ));
        }
    };
    let report = state
        .read(|data, now| data.report(q.from, q.to, completed, now))
        .await;
    Ok(Json(report))
}
