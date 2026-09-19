//! Per-machine window preferences, kept next to the log file.
//!
//! These can't live in the webview's `localStorage`: the embedded server binds a
//! random port on every launch, so the origin (and its storage) never survives.

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::warn;

pub const FILE_NAME: &str = "desktop-settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Collapse into the floating compact window whenever a session starts.
    pub compact_on_start: bool,
    /// Where the compact window was last left (physical pixels, outer position).
    pub compact_position: Option<(i32, i32)>,
}

impl Settings {
    /// A missing or unreadable file is just the defaults; preferences must never
    /// keep the app from starting.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                warn!("reading {}: {e}; using default settings", path.display());
                return Self::default();
            }
        };
        serde_json::from_str(&text).unwrap_or_else(|e| {
            warn!("parsing {}: {e}; using default settings", path.display());
            Self::default()
        })
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(&dir.path().join(FILE_NAME)), Settings::default());
    }

    #[test]
    fn round_trips_and_creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Harmony").join(FILE_NAME);
        let s = Settings { compact_on_start: true, compact_position: Some((-1200, 40)) };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn tolerates_partial_and_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, r#"{"compact_on_start":true,"from_the_future":1}"#).unwrap();
        assert_eq!(Settings::load(&path), Settings { compact_on_start: true, compact_position: None });
    }
}
