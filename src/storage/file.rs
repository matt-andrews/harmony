//! Local JSON file backend. Writes are atomic (temp file + rename).

use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{Etag, Result, parse_document, serialize_document};
use crate::domain::AppData;

pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn describe(&self) -> String {
        format!("file {}", self.path.display())
    }

    pub async fn load(&self) -> Result<(AppData, Option<Etag>)> {
        match tokio::fs::read(&self.path).await {
            Ok(bytes) => Ok((parse_document(&bytes)?, None)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((AppData::default(), None)),
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("reading {}", self.path.display()))
                .into()),
        }
    }

    pub async fn save(&self, data: &AppData) -> Result<Option<Etag>> {
        let bytes = serialize_document(data)?;
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &bytes)
            .await
            .with_context(|| format!("writing {}", tmp.display()))?;
        tokio::fs::rename(&tmp, &self.path)
            .await
            .with_context(|| format!("replacing {}", self.path.display()))?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trips_and_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FileStorage::new(dir.path().join("nested/harmony.json"));

        let (data, etag) = storage.load().await.unwrap();
        assert_eq!(data, AppData::default());
        assert!(etag.is_none());

        let mut data = AppData::default();
        data.create_project("X", 1.0, chrono::Utc::now()).unwrap();
        storage.save(&data).await.unwrap();

        let (back, _) = storage.load().await.unwrap();
        assert_eq!(back, data);
    }

    #[tokio::test]
    async fn corrupt_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("harmony.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let err = FileStorage::new(&path).load().await.unwrap_err();
        assert!(matches!(err, super::super::StorageError::Corrupt(_)));
    }
}
