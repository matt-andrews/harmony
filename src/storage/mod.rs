//! Persistence of the single `AppData` document.
//!
//! Two backends behind one enum (no trait objects needed):
//! - [`blob::BlobStorage`]: Azure Blob Storage via a container SAS URL.
//! - [`file::FileStorage`]: a local JSON file for tests and offline use.

pub mod blob;
pub mod file;

use thiserror::Error;

use crate::domain::AppData;

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
    /// Parse `HARMONY_STORAGE`: `sas:<container SAS URL>` or `file:<path>`.
    pub fn from_spec(spec: &str) -> anyhow::Result<Self> {
        let spec = spec.trim();
        if let Some(url) = spec.strip_prefix("sas:") {
            Ok(Storage::Blob(blob::BlobStorage::new(url)?))
        } else if let Some(path) = spec.strip_prefix("file:") {
            Ok(Storage::File(file::FileStorage::new(path)))
        } else {
            anyhow::bail!(
                "HARMONY_STORAGE must start with `sas:` or `file:` (got {spec:?})"
            )
        }
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
            Storage::File(f) => f.save(data).await,
        }
    }
}

pub(crate) fn parse_document(bytes: &[u8]) -> Result<AppData> {
    serde_json::from_slice(bytes).map_err(|e| StorageError::Corrupt(e.to_string()))
}

pub(crate) fn serialize_document(data: &AppData) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(data).map_err(|e| StorageError::Other(e.into()))
}
