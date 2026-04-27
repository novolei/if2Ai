//! Local license cache.
//!
//! `trait LicenseStore` lets us swap the on-disk file for a future
//! macOS Keychain / Windows Credential Manager / Linux Secret Service
//! backend without touching IPC or UI.  The default
//! [`FileLicenseStore`] writes `~/.if2ai/activation/license.json`
//! atomically with 0o600 permissions.

use async_trait::async_trait;
use std::path::PathBuf;

use super::installation_id::{activation_dir, license_path};
use super::models::StoredLicense;

#[derive(Debug, thiserror::Error)]
pub enum LicenseStoreError {
    #[error("io error at {1}: {0}")]
    Io(#[source] std::io::Error, String),
    #[error("serialize error: {0}")]
    Serialize(String),
    #[error("deserialize error: {0}")]
    Deserialize(String),
}

#[async_trait]
pub trait LicenseStore: Send + Sync {
    async fn load(&self) -> Result<Option<StoredLicense>, LicenseStoreError>;
    async fn save(&self, license: &StoredLicense) -> Result<(), LicenseStoreError>;
    async fn clear(&self) -> Result<(), LicenseStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct FileLicenseStore {
    /// Override the on-disk path (tests). `None` → real
    /// [`license_path`].
    path_override: Option<PathBuf>,
}

impl FileLicenseStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path_override: Some(path),
        }
    }

    fn path(&self) -> PathBuf {
        self.path_override.clone().unwrap_or_else(license_path)
    }

    fn dir(&self) -> PathBuf {
        self.path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(activation_dir)
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        let dir = self.dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dir)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&dir, perms)?;
        }
        Ok(())
    }
}

#[async_trait]
impl LicenseStore for FileLicenseStore {
    async fn load(&self) -> Result<Option<StoredLicense>, LicenseStoreError> {
        let path = self.path();
        if !path.exists() {
            tracing::info!(
                target: "if2ai::activation",
                "license_store cache missing at {}",
                path.display()
            );
            return Ok(None);
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| LicenseStoreError::Io(e, path.display().to_string()))?;
        let license: StoredLicense = serde_json::from_slice(&bytes).map_err(|e| {
            tracing::warn!(
                target: "if2ai::activation",
                "license_store failed to deserialize cache at {}: {}",
                path.display(),
                e
            );
            LicenseStoreError::Deserialize(e.to_string())
        })?;
        tracing::info!(
            target: "if2ai::activation",
            "license_store loaded cache at {} license_id={}",
            path.display(),
            license.license_id
        );
        Ok(Some(license))
    }

    async fn save(&self, license: &StoredLicense) -> Result<(), LicenseStoreError> {
        self.ensure_dir()
            .map_err(|e| LicenseStoreError::Io(e, "ensure activation dir".into()))?;
        let path = self.path();
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(license)
            .map_err(|e| LicenseStoreError::Serialize(e.to_string()))?;
        tokio::fs::write(&tmp, &bytes)
            .await
            .map_err(|e| LicenseStoreError::Io(e, tmp.display().to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&tmp)
                .map_err(|e| LicenseStoreError::Io(e, tmp.display().to_string()))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&tmp, perms)
                .map_err(|e| LicenseStoreError::Io(e, tmp.display().to_string()))?;
        }
        tokio::fs::rename(&tmp, &path)
            .await
            .map_err(|e| LicenseStoreError::Io(e, path.display().to_string()))?;
        tracing::info!(
            target: "if2ai::activation",
            "license_store saved cache at {} license_id={}",
            path.display(),
            license.license_id
        );
        Ok(())
    }

    async fn clear(&self) -> Result<(), LicenseStoreError> {
        let path = self.path();
        if !path.exists() {
            tracing::info!(
                target: "if2ai::activation",
                "license_store clear skipped; cache missing at {}",
                path.display()
            );
            return Ok(());
        }
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| LicenseStoreError::Io(e, path.display().to_string()))?;
        tracing::info!(
            target: "if2ai::activation",
            "license_store cleared cache at {}",
            path.display()
        );
        Ok(())
    }
}

/// Take the larger (newer) of two RFC3339 timestamps.  Comparison is
/// done as `chrono::DateTime<Utc>`; on parse failure the existing
/// value wins so a malformed server response cannot blow away a good
/// high-water mark.
pub fn max_trusted_server_time(existing: &str, incoming: &str) -> String {
    use chrono::DateTime;
    match (
        DateTime::parse_from_rfc3339(existing),
        DateTime::parse_from_rfc3339(incoming),
    ) {
        (Ok(a), Ok(b)) => {
            if b > a {
                incoming.to_string()
            } else {
                existing.to_string()
            }
        }
        (Err(_), Ok(_)) => incoming.to_string(),
        _ => existing.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample(installation_id: &str) -> StoredLicense {
        StoredLicense {
            license_id: "lic_test".into(),
            refresh_token: "rt_test".into(),
            license_jws: "h.p.s".into(),
            installation_id: installation_id.into(),
            last_trusted_server_time: "2026-04-23T10:00:00+00:00".into(),
        }
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let store = FileLicenseStore::with_path(dir.path().join("license.json"));
        let l = sample("sha256_a");
        store.save(&l).await.unwrap();
        let loaded = store.load().await.unwrap().unwrap();
        assert_eq!(loaded.license_id, "lic_test");
    }

    #[tokio::test]
    async fn clear_removes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("license.json");
        let store = FileLicenseStore::with_path(path.clone());
        store.save(&sample("sha256_a")).await.unwrap();
        assert!(path.exists());
        store.clear().await.unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn max_trusted_server_time_picks_newer() {
        let older = "2026-01-01T00:00:00+00:00";
        let newer = "2026-04-01T00:00:00+00:00";
        assert_eq!(max_trusted_server_time(older, newer), newer);
        assert_eq!(max_trusted_server_time(newer, older), newer);
    }

    #[test]
    fn max_trusted_server_time_keeps_existing_on_parse_error() {
        let good = "2026-01-01T00:00:00+00:00";
        assert_eq!(max_trusted_server_time(good, "garbage"), good);
    }
}
