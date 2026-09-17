//! Local directory backend: `<dir>/harmony.json` plus `<dir>/backups/`.
//!
//! Writes are atomic (temp file + rename). The document's modification time
//! and length act as its version tag, so a save refuses to overwrite a file
//! something else changed since we loaded it.

use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::Utc;
use tracing::{info, warn};

use super::{
    BACKUP_DIR, BackupTracker, DOCUMENT, Etag, Result, StorageError, backup_name,
    parse_document, serialize_document,
};
use crate::domain::AppData;

pub struct FileStorage {
    dir: PathBuf,
    backups: BackupTracker,
}

impl FileStorage {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
            backups: BackupTracker::default(),
        }
    }

    fn document(&self) -> PathBuf {
        self.dir.join(DOCUMENT)
    }

    pub fn describe(&self) -> String {
        let shown = std::fs::canonicalize(&self.dir).unwrap_or_else(|_| self.dir.clone());
        let shown = shown.join(DOCUMENT).display().to_string();
        // Windows canonical paths carry a `\\?\` verbatim prefix; hide it.
        format!("local dir {}", shown.trim_start_matches(r"\\?\"))
    }

    /// Version tag of the document on disk, or `None` if it doesn't exist.
    async fn current_tag(&self) -> Result<Option<Etag>> {
        match tokio::fs::metadata(self.document()).await {
            Ok(meta) => {
                let nanos = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                Ok(Some(Etag(format!("{nanos}:{}", meta.len()))))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("inspecting {}", self.document().display()))
                .into()),
        }
    }

    pub async fn load(&self) -> Result<(AppData, Option<Etag>)> {
        let path = self.document();
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let data = parse_document(&bytes)?;
                Ok((data, self.current_tag().await?))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("no {DOCUMENT} in {} yet; starting empty", self.dir.display());
                Ok((AppData::default(), None))
            }
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("reading {}", path.display()))
                .into()),
        }
    }

    pub async fn save(&self, data: &AppData, etag: Option<&Etag>) -> Result<Option<Etag>> {
        if self.current_tag().await?.as_ref() != etag {
            return Err(StorageError::Conflict);
        }
        let bytes = serialize_document(data)?;
        tokio::fs::create_dir_all(&self.dir)
            .await
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let path = self.document();
        let tmp = self.dir.join(format!("{DOCUMENT}.tmp"));
        tokio::fs::write(&tmp, &bytes)
            .await
            .with_context(|| format!("writing {}", tmp.display()))?;
        tokio::fs::rename(&tmp, &path)
            .await
            .with_context(|| format!("replacing {}", path.display()))?;
        let tag = self.current_tag().await?;

        self.maybe_backup(&bytes).await;
        Ok(tag)
    }

    /// Best-effort daily snapshot. Never fails the save.
    async fn maybe_backup(&self, bytes: &[u8]) {
        let today = Utc::now().date_naive();
        if !self.backups.due(today) {
            return;
        }
        let name = backup_name(today);
        let path = self.dir.join(&name);
        let result = async {
            tokio::fs::create_dir_all(self.dir.join(BACKUP_DIR)).await?;
            tokio::fs::write(&path, bytes).await
        }
        .await;
        match result {
            Ok(()) => {
                info!("wrote backup {name}");
                self.backups.mark(today);
            }
            Err(e) => warn!("backup {} failed (will retry on next save): {e}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppData {
        let mut data = AppData::default();
        data.create_project("X", 1.0, Utc::now()).unwrap();
        data
    }

    #[tokio::test]
    async fn round_trips_and_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FileStorage::new(dir.path().join("nested/deeper"));

        let (data, etag) = storage.load().await.unwrap();
        assert_eq!(data, AppData::default());
        assert!(etag.is_none());

        let data = sample();
        let tag = storage.save(&data, None).await.unwrap();
        assert!(tag.is_some());
        assert!(dir.path().join("nested/deeper").join(DOCUMENT).exists());

        let (back, tag2) = storage.load().await.unwrap();
        assert_eq!(back, data);
        assert_eq!(tag, tag2);
    }

    #[tokio::test]
    async fn refuses_to_overwrite_external_changes() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FileStorage::new(dir.path());
        let tag = storage.save(&sample(), None).await.unwrap();

        // Someone else edits the file (different length guarantees a new tag).
        std::fs::write(
            dir.path().join(DOCUMENT),
            br#"{"version":1,"projects":[],"tasks":[],"sessions":[]}"#,
        )
        .unwrap();
        let err = storage.save(&sample(), tag.as_ref()).await.unwrap_err();
        assert!(matches!(err, StorageError::Conflict));

        // A stale "the file doesn't exist yet" claim is also a conflict.
        let err = storage.save(&sample(), None).await.unwrap_err();
        assert!(matches!(err, StorageError::Conflict));

        // Reloading gives a fresh tag and saving works again.
        let (_, fresh) = storage.load().await.unwrap();
        storage.save(&sample(), fresh.as_ref()).await.unwrap();
    }

    #[tokio::test]
    async fn writes_one_backup_per_day() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FileStorage::new(dir.path());
        let tag = storage.save(&sample(), None).await.unwrap();
        storage.save(&sample(), tag.as_ref()).await.unwrap();

        let backups: Vec<_> = std::fs::read_dir(dir.path().join(BACKUP_DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(backups[0].ends_with(".json"));
    }

    #[tokio::test]
    async fn corrupt_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DOCUMENT), b"{ not json").unwrap();
        let err = FileStorage::new(dir.path()).load().await.unwrap_err();
        assert!(matches!(err, StorageError::Corrupt(_)));
    }
}
