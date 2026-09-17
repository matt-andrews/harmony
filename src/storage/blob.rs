//! Azure Blob Storage backend.
//!
//! Authenticates with a container-level SAS URL (`https://<acct>.blob.core.windows.net/<container>?<sas>`)
//! passed to the client with no token credential. The document lives at
//! `harmony.json`; a daily copy is written to `backups/<date>.json` on the
//! first save of each day.

use std::sync::Mutex;

use anyhow::Context;
use azure_core::error::ErrorKind;
use azure_core::http::{Etag as AzEtag, RequestContent, StatusCode, Url};
use azure_storage_blob::BlobContainerClient;
use azure_storage_blob::models::BlobClientUploadOptions;
use chrono::{NaiveDate, Utc};
use tracing::{info, warn};

use super::{Etag, Result, StorageError, parse_document, serialize_document};
use crate::domain::AppData;

const DOCUMENT: &str = "harmony.json";

pub struct BlobStorage {
    container: BlobContainerClient,
    /// Secret-free rendering of the container URL for logs.
    label: String,
    /// Date of the most recent backup this process wrote.
    backup_day: Mutex<Option<NaiveDate>>,
}

impl BlobStorage {
    pub fn new(sas_url: &str) -> anyhow::Result<Self> {
        let url = Url::parse(sas_url).context("HARMONY_STORAGE SAS URL is not a valid URL")?;
        if url.query().is_none() {
            anyhow::bail!("SAS URL has no query string; expected a shared access signature");
        }
        let mut label = url.clone();
        label.set_query(None);
        let container = BlobContainerClient::new(url, None, None)
            .context("creating blob container client")?;
        Ok(Self {
            container,
            label: label.to_string(),
            backup_day: Mutex::new(None),
        })
    }

    pub fn describe(&self) -> String {
        format!("azure blob {}/{DOCUMENT}", self.label)
    }

    pub async fn load(&self) -> Result<(AppData, Option<Etag>)> {
        let blob = self.container.blob_client(DOCUMENT);
        let exists = blob
            .exists()
            .await
            .map_err(|e| wrap(e, "checking whether the document exists"))?;
        if !exists {
            info!("no {DOCUMENT} in container yet; starting empty");
            return Ok((AppData::default(), None));
        }
        let resp = blob
            .download(None)
            .await
            .map_err(|e| wrap(e, "downloading the document"))?;
        let etag = resp
            .properties
            .etag
            .as_ref()
            .map(|e| Etag(e.as_ref().to_string()));
        let bytes: Vec<u8> = resp
            .body
            .collect()
            .await
            .map_err(|e| wrap(e, "reading the document body"))?
            .into();
        Ok((parse_document(&bytes)?, etag))
    }

    pub async fn save(&self, data: &AppData, etag: Option<&Etag>) -> Result<Option<Etag>> {
        let bytes = serialize_document(data)?;
        let blob = self.container.blob_client(DOCUMENT);

        let base = BlobClientUploadOptions {
            blob_content_type: Some("application/json".into()),
            ..Default::default()
        };
        let opts = match etag {
            Some(e) => BlobClientUploadOptions {
                if_match: Some(AzEtag::from(e.0.as_str())),
                ..base
            },
            None => base.if_not_exists(),
        };

        let result = blob
            .upload(RequestContent::from(bytes.clone()), Some(opts))
            .await
            .map_err(|e| match e.kind() {
                ErrorKind::HttpResponse { status, .. }
                    if *status == StatusCode::PreconditionFailed =>
                {
                    StorageError::Conflict
                }
                _ => wrap(e, "uploading the document"),
            })?;
        let new_etag = result.etag.map(|e| Etag(e.as_ref().to_string()));

        self.maybe_backup(&bytes).await;
        Ok(new_etag)
    }

    /// Best-effort daily snapshot. Never fails the save.
    async fn maybe_backup(&self, bytes: &[u8]) {
        let today = Utc::now().date_naive();
        let due = {
            let day = self.backup_day.lock().expect("backup_day lock");
            *day != Some(today)
        };
        if !due {
            return;
        }
        let name = format!("backups/{today}.json");
        let opts = BlobClientUploadOptions {
            blob_content_type: Some("application/json".into()),
            ..Default::default()
        };
        match self
            .container
            .blob_client(&name)
            .upload(RequestContent::from(bytes.to_vec()), Some(opts))
            .await
        {
            Ok(_) => {
                info!("wrote backup {name}");
                *self.backup_day.lock().expect("backup_day lock") = Some(today);
            }
            Err(e) => warn!("backup {name} failed (will retry on next save): {e}"),
        }
    }
}

fn wrap(e: azure_core::Error, what: &str) -> StorageError {
    StorageError::Other(anyhow::Error::new(e).context(format!("azure blob: {what}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_urls_without_sas() {
        assert!(BlobStorage::new("https://acct.blob.core.windows.net/harmony").is_err());
        assert!(BlobStorage::new("not a url").is_err());
    }

    #[test]
    fn describe_strips_the_secret() {
        let s = BlobStorage::new("https://acct.blob.core.windows.net/harmony?sv=1&sig=SECRET")
            .unwrap();
        assert!(!s.describe().contains("SECRET"));
        assert!(s.describe().contains("acct.blob.core.windows.net/harmony"));
    }
}
