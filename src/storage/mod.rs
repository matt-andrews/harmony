//! Persistence of the single `AppData` document.
//!
//! Two backends behind one enum (no trait objects needed):
//! - [`blob::BlobStorage`]: Azure Blob Storage via a container SAS URL.
//! - [`file::FileStorage`]: a local directory (a mounted volume in Docker).
//!
//! Both store the same layout: `harmony.json` at the root and one copy per
//! UTC day under `backups/YYYY-MM-DD.json`.

pub mod blob;
pub mod file;

use std::sync::Mutex;

use chrono::NaiveDate;
use thiserror::Error;

use crate::domain::AppData;

/// Name of the document in every backend.
pub const DOCUMENT: &str = "harmony.json";
/// Folder (blob prefix) holding daily copies.
pub const BACKUP_DIR: &str = "backups";

/// Opaque version tag returned by a backend; passed back on the next save so
/// the backend can refuse to overwrite a document someone else changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Etag(pub String);

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(
        "the stored document changed underneath us (another Harmony instance?); restart to reload"
    )]
    Conflict,
    #[error("stored document is not valid Harmony data: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub enum Storage {
    Blob(blob::BlobStorage),
    File(file::FileStorage),
}

impl Storage {
    /// Parse `HARMONY_STORAGE`. The backend is inferred from the string:
    ///
    /// | value | backend |
    /// |---|---|
    /// | `sas:<url>` or a bare `https://…` URL | Azure Blob (container SAS) |
    /// | `file:<dir>` or a bare path | local directory |
    pub fn from_spec(spec: &str) -> anyhow::Result<Self> {
        let spec = spec.trim();
        if spec.is_empty() {
            anyhow::bail!("HARMONY_STORAGE is empty (see .env.example)");
        }
        if let Some(url) = spec.strip_prefix("sas:") {
            return Ok(Storage::Blob(blob::BlobStorage::new(url.trim())?));
        }
        if spec.starts_with("https://") || spec.starts_with("http://") {
            return Ok(Storage::Blob(blob::BlobStorage::new(spec)?));
        }
        let dir = spec.strip_prefix("file:").unwrap_or(spec).trim();
        if dir.is_empty() {
            anyhow::bail!("HARMONY_STORAGE=file: needs a directory path");
        }
        Ok(Storage::File(file::FileStorage::new(dir)))
    }

    /// Human-readable, secret-free description for logs.
    pub fn describe(&self) -> String {
        match self {
            Storage::Blob(b) => b.describe(),
            Storage::File(f) => f.describe(),
        }
    }

    pub async fn load(&self) -> Result<(AppData, Option<Etag>)> {
        match self {
            Storage::Blob(b) => b.load().await,
            Storage::File(f) => f.load().await,
        }
    }

    pub async fn save(&self, data: &AppData, etag: Option<&Etag>) -> Result<Option<Etag>> {
        match self {
            Storage::Blob(b) => b.save(data, etag).await,
            Storage::File(f) => f.save(data, etag).await,
        }
    }
}

/// Remembers which UTC day this process last backed up, so each backend
/// writes at most one `backups/<date>.json` per day.
#[derive(Default)]
pub struct BackupTracker(Mutex<Option<NaiveDate>>);

impl BackupTracker {
    pub fn due(&self, today: NaiveDate) -> bool {
        *self.0.lock().expect("backup tracker lock") != Some(today)
    }

    pub fn mark(&self, today: NaiveDate) {
        *self.0.lock().expect("backup tracker lock") = Some(today);
    }
}

/// `backups/YYYY-MM-DD.json`
pub fn backup_name(day: NaiveDate) -> String {
    format!("{BACKUP_DIR}/{day}.json")
}

pub(crate) fn parse_document(bytes: &[u8]) -> Result<AppData> {
    let mut data: AppData =
        serde_json::from_slice(bytes).map_err(|e| StorageError::Corrupt(e.to_string()))?;
    data.migrate();
    Ok(data)
}

pub(crate) fn serialize_document(data: &AppData) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(data).map_err(|e| StorageError::Other(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_infers_backend() {
        assert!(matches!(Storage::from_spec("file:./data").unwrap(), Storage::File(_)));
        assert!(matches!(Storage::from_spec("./data").unwrap(), Storage::File(_)));
        assert!(matches!(Storage::from_spec("/data").unwrap(), Storage::File(_)));
        assert!(matches!(Storage::from_spec(r"C:\data").unwrap(), Storage::File(_)));
        assert!(matches!(
            Storage::from_spec("sas:https://a.blob.core.windows.net/c?sig=x").unwrap(),
            Storage::Blob(_)
        ));
        assert!(matches!(
            Storage::from_spec("https://a.blob.core.windows.net/c?sig=x").unwrap(),
            Storage::Blob(_)
        ));
        assert!(Storage::from_spec("").is_err());
        assert!(Storage::from_spec("file:").is_err());
        assert!(Storage::from_spec("sas:https://a.blob.core.windows.net/c").is_err());
    }

    #[test]
    fn backup_tracker_is_once_per_day() {
        let t = BackupTracker::default();
        let d1 = NaiveDate::from_ymd_opt(2026, 9, 16).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();
        assert!(t.due(d1));
        t.mark(d1);
        assert!(!t.due(d1));
        assert!(t.due(d2));
        assert_eq!(backup_name(d1), "backups/2026-09-16.json");
    }
}
