use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::{SharedState, double_option};
use crate::domain::views::StateView;
use crate::domain::{Assignment, SessionPatch};
use crate::error::AppError;

#[derive(Deserialize, Default)]
pub struct StartSession {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub new_task: bool,
}

pub async fn start(
    State(state): State<SharedState>,
    body: Option<Json<StartSession>>,
) -> Result<Json<StateView>, AppError> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let (_, view) = state
        .mutate(|data, now| {
            data.start_session(now, body.project_id, body.new_task)
                .map(|_| ())
        })
        .await?;
    Ok(Json(view))
}

pub async fn stop(State(state): State<SharedState>) -> Result<Json<StateView>, AppError> {
    let (_, view) = state
        .mutate(|data, now| data.stop_session(now).map(|_| ()))
        .await?;
    Ok(Json(view))
}

#[derive(Deserialize, Default)]
pub struct UpdateSession {
    /// `null` untags; a uuid files under that project (see `new_task`).
    #[serde(default, deserialize_with = "double_option")]
    pub project_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub new_task: bool,
    /// File under a specific existing task (ignored if `project_id` is given).
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    /// `null` re-opens the session (makes it the running one).
    #[serde(default, deserialize_with = "double_option")]
    pub ended_at: Option<Option<DateTime<Utc>>>,
    #[serde(default, deserialize_with = "double_option")]
    pub note: Option<Option<String>>,
}

pub async fn update(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSession>,
) -> Result<Json<StateView>, AppError> {
    let assignment = match (body.project_id, body.task_id) {
        (Some(Some(project_id)), _) => Some(Assignment::Project {
            project_id,
            new_task: body.new_task,
        }),
        (Some(None), _) => Some(Assignment::Untag),
        (None, Some(task_id)) => Some(Assignment::Task(task_id)),
        (None, None) => None,
    };
    let patch = SessionPatch {
        assignment,
        started_at: body.started_at,
        ended_at: body.ended_at,
        note: body.note,
    };
    let (_, view) = state
        .mutate(|data, now| data.update_session(id, patch, now).map(|_| ()))
        .await?;
    Ok(Json(view))
}

pub async fn delete(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StateView>, AppError> {
    let (_, view) = state.mutate(|data, _| data.delete_session(id)).await?;
    Ok(Json(view))
}
