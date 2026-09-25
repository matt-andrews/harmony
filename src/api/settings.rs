use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use serde::Deserialize;

use super::{SharedState, double_option};
use crate::domain::SettingsPatch;
use crate::domain::views::StateView;
use crate::error::AppError;

#[derive(Deserialize, Default)]
pub struct UpdateSettings {
    /// `"YYYY-MM-DD"` sets the cycle start; `null` clears it.
    #[serde(default, deserialize_with = "double_option")]
    pub cycle_start: Option<Option<NaiveDate>>,
    #[serde(default)]
    pub payout_delay_days: Option<u32>,
}

pub async fn update(
    State(state): State<SharedState>,
    Json(body): Json<UpdateSettings>,
) -> Result<Json<StateView>, AppError> {
    let patch = SettingsPatch {
        cycle_start: body.cycle_start,
        payout_delay_days: body.payout_delay_days,
    };
    let (_, view) = state
        .mutate(|data, _| data.update_settings(patch).map(|_| ()))
        .await?;
    Ok(Json(view))
}
