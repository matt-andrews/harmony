//! Persisted data model. Everything in here is serialised verbatim into the
//! single `harmony.json` document, so changes must stay backward compatible
//! (or bump `DATA_VERSION` and migrate).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::palette::migrate_color;

/// 2: project colours moved to the Catppuccin palette.
pub const DATA_VERSION: u32 = 2;

/// A reusable label: a client / gig you can pick up repeatedly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    /// USD per hour. Not versioned: changing it re-prices history.
    pub hourly_rate: f64,
    /// `#rrggbb`
    pub color: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub archived: bool,
}

/// One pickup of a project. Ephemeral grouping for a run of sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    /// 1-based, sequential per project.
    pub number: u32,
    pub created_at: DateTime<Utc>,
}

/// A contiguous block of work. `ended_at == None` means it is running.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Session {
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }

    /// Effective end: the real end, or `now` for a running session.
    pub fn end_or(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        self.ended_at.unwrap_or(now)
    }

    pub fn duration_secs(&self, now: DateTime<Utc>) -> i64 {
        (self.end_or(now) - self.started_at).num_seconds().max(0)
    }
}

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub sessions: Vec<Session>,
}

/// Documents written before the field existed are version 1.
fn default_version() -> u32 {
    1
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            projects: Vec::new(),
            tasks: Vec::new(),
            sessions: Vec::new(),
        }
    }
}

impl AppData {
    /// Bring an older document up to [`DATA_VERSION`]. In memory only: the
    /// result is persisted by the next ordinary save.
    pub fn migrate(&mut self) {
        if self.version < 2 {
            for p in &mut self.projects {
                if let Some(color) = migrate_color(&p.color) {
                    p.color = color.to_string();
                }
            }
        }
        self.version = self.version.max(DATA_VERSION);
    }
}

/// Round to whole cents.
pub fn round_cents(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Pay for a duration at an hourly rate, rounded to cents.
pub fn pay_for(secs: i64, hourly_rate: f64) -> f64 {
    round_cents(secs as f64 / 3600.0 * hourly_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pay_rounds_to_cents() {
        assert_eq!(pay_for(3600, 40.0), 40.0);
        assert_eq!(pay_for(1800, 40.0), 20.0);
        assert_eq!(pay_for(1, 40.0), 0.01);
        assert_eq!(pay_for(0, 40.0), 0.0);
        // 7688s at $40/h = 85.4222.. -> 85.42
        assert_eq!(pay_for(7688, 40.0), 85.42);
    }

    #[test]
    fn default_data_carries_version() {
        let d = AppData::default();
        assert_eq!(d.version, DATA_VERSION);
        let json = serde_json::to_string(&d).unwrap();
        let back: AppData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn old_document_without_version_defaults() {
        let back: AppData = serde_json::from_str(r#"{"projects":[],"tasks":[],"sessions":[]}"#).unwrap();
        assert_eq!(back.version, 1);
    }

    fn with_colors(version: u32, colors: &[&str]) -> AppData {
        let mut d = AppData { version, ..AppData::default() };
        for (i, c) in colors.iter().enumerate() {
            d.projects.push(Project {
                id: Uuid::new_v4(),
                name: format!("p{i}"),
                hourly_rate: 0.0,
                color: c.to_string(),
                created_at: Utc::now(),
                archived: false,
            });
        }
        d
    }

    #[test]
    fn migrate_remaps_legacy_colors_and_keeps_custom_ones() {
        let mut d = with_colors(1, &["#3b82f6", "#123456"]);
        d.migrate();
        assert_eq!(d.version, DATA_VERSION);
        assert_eq!(d.projects[0].color, "#8aadf4");
        assert_eq!(d.projects[1].color, "#123456");
    }

    #[test]
    fn migrate_leaves_current_documents_alone() {
        // A v2 user may have hand-picked an old palette colour; keep it.
        let mut d = with_colors(DATA_VERSION, &["#3b82f6"]);
        let before = d.clone();
        d.migrate();
        assert_eq!(d, before);
    }
}
