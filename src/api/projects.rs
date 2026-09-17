use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SharedState;
use crate::domain::ProjectPatch;
use crate::domain::views::{ProjectSummary, ProjectView, StateView};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct CreateProject {
    pub name: String,
    #[serde(default)]
    pub hourly_rate: Option<f64>,
}

#[derive(Serialize)]
pub struct CreatedProject {
    pub project: ProjectView,
    pub state: StateView,
}

pub async fn create(
    State(state): State<SharedState>,
    Json(body): Json<CreateProject>,
) -> Result<Json<CreatedProject>, AppError> {
    let (project, view) = state
        .mutate(|data, now| {
            let p = data
                .create_project(&body.name, body.hourly_rate.unwrap_or(0.0), now)?
                .clone();
            Ok(data.project_view(&p, now))
        })
        .await?;
    Ok(Json(CreatedProject {
        project,
        state: view,
    }))
}

#[derive(Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub hourly_rate: Option<f64>,
    pub color: Option<String>,
    pub archived: Option<bool>,
}

pub async fn update(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProject>,
) -> Result<Json<StateView>, AppError> {
    let patch = ProjectPatch {
        name: body.name,
        hourly_rate: body.hourly_rate,
        color: body.color,
        archived: body.archived,
    };
    let (_, view) = state
        .mutate(|data, _| data.update_project(id, patch).map(|_| ()))
        .await?;
    Ok(Json(view))
}

pub async fn summary(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectSummary>, AppError> {
    let summary = state.read(|data, now| data.project_summary(id, now)).await?;
    Ok(Json(summary))
}
