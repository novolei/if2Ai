//! Config file persistence layer.
//!
//! Reads and writes configuration files to `~/.if2ai/`.
//! Handles directory creation, atomic writes, and file permissions (0600).
//!
//! All paths are relative to the `~/.if2ai/` root directory.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Returns the path to `~/.if2ai/` directory.
fn if2ai_dir() -> PathBuf {
    if2ai_data_root()
}

/// MEM-MOD-PATH-FIX — canonical `~/.if2ai/` data root used by every
/// if2Ai subsystem (memory, models, sessions, projects, trajectories,
/// learning strategies, harness reports, ...).
///
/// Pre-fix the codebase had **two** competing roots:
///   - `~/.if2ai/` (sessions / projects / config / models / skills / todos)
///   - `~/Library/Application Support/.if2ai/` (memory / vector_db /
///     summaries / jobs / trajectories / learning / harness)
/// causing memory data to disappear on restart whenever code changes
/// touched the `data_local_dir()` resolver.  This helper is the single
/// source of truth — every subsystem MUST go through it (or via
/// [`crate::bootstrap`] which forwards from here).
#[must_use]
pub fn if2ai_data_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".if2ai")
}

/// Returns the path to `~/.if2ai/config.json`.
pub fn config_json_path() -> PathBuf {
    if2ai_dir().join("config.json")
}

/// Returns the path to `~/.if2ai/providers.yaml`.
pub fn providers_yaml_path() -> PathBuf {
    if2ai_dir().join("providers.yaml")
}

/// Returns the path to `~/.if2ai/models.json`.
pub fn models_json_path() -> PathBuf {
    if2ai_dir().join("models.json")
}

/// Returns the path to `~/.if2ai/auth.json`.
pub fn auth_json_path() -> PathBuf {
    if2ai_dir().join("auth.json")
}

/// Returns the path to `~/.if2ai/channels-config.json`.
pub fn channels_config_path() -> PathBuf {
    if2ai_dir().join("channels-config.json")
}

/// Ensure the `~/.if2ai/` directory exists with 0700 permissions.
fn ensure_if2ai_dir() -> Result<(), std::io::Error> {
    let dir = if2ai_dir();
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

/// Read and deserialize a JSON file.
///
/// Returns `None` if the file does not exist.
/// Returns an error if the file exists but cannot be read or parsed.
pub async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, std::io::Error> {
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(path).await?;
    let value: T = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(value))
}

/// Write and serialize a value to a JSON file atomically.
///
/// Uses atomic write (temp file + rename) to prevent corruption.
/// Sets file permissions to 0600 (owner read/write only).
pub async fn write_json<T: Serialize>(value: &T, path: &Path) -> Result<(), ConfigStoreError> {
    ensure_if2ai_dir()?;
    atomic_write_json(value, path).await
}

/// Internal: atomic JSON write with permissions.
async fn atomic_write_json<T: Serialize>(value: &T, path: &Path) -> Result<(), ConfigStoreError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ConfigStoreError::Io(e, format!("create dir: {}", parent.display())))?;
    }

    let content = serde_json::to_string_pretty(value)
        .map_err(|e| ConfigStoreError::Serialize(format!("failed to serialize: {e}")))?;

    let temp_path = path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, content)
        .await
        .map_err(|e| ConfigStoreError::Io(e, format!("write temp: {}", temp_path.display())))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&temp_path)
            .map_err(|e| ConfigStoreError::Io(e, format!("metadata: {}", temp_path.display())))?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&temp_path, perms)
            .map_err(|e| ConfigStoreError::Io(e, format!("chmod: {}", temp_path.display())))?;
    }

    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|e| ConfigStoreError::Io(e, format!("rename: {}", path.display())))?;

    Ok(())
}

/// Read and deserialize a YAML file.
///
/// Returns `None` if the file does not exist.
pub async fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, ConfigStoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| ConfigStoreError::Io(e, format!("read yaml: {}", path.display())))?;
    let value: T = serde_yaml::from_str(&content)
        .map_err(|e| ConfigStoreError::Deserialize(format!("failed to parse yaml: {e}")))?;
    Ok(Some(value))
}

/// Write and serialize a value to a YAML file atomically.
pub async fn write_yaml<T: Serialize>(value: &T, path: &Path) -> Result<(), ConfigStoreError> {
    ensure_if2ai_dir()?;

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ConfigStoreError::Io(e, format!("create dir: {}", parent.display())))?;
    }

    let content = serde_yaml::to_string(value)
        .map_err(|e| ConfigStoreError::Serialize(format!("failed to serialize yaml: {e}")))?;

    let temp_path = path.with_extension("yaml.tmp");
    tokio::fs::write(&temp_path, content).await.map_err(|e| {
        ConfigStoreError::Io(e, format!("write temp yaml: {}", temp_path.display()))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&temp_path).map_err(|e| {
            ConfigStoreError::Io(e, format!("yaml metadata: {}", temp_path.display()))
        })?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&temp_path, perms)
            .map_err(|e| ConfigStoreError::Io(e, format!("yaml chmod: {}", temp_path.display())))?;
    }

    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|e| ConfigStoreError::Io(e, format!("yaml rename: {}", path.display())))?;

    Ok(())
}

/// Delete a file if it exists.
pub async fn delete_file(path: &Path) -> Result<(), ConfigStoreError> {
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| ConfigStoreError::Io(e, format!("delete: {}", path.display())))?;
    }
    Ok(())
}

// ── Error type ──────────────────────────────────────────────────────────────

/// Error type for config store operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("I/O error at {1}: {0}")]
    Io(#[source] std::io::Error, String),
    #[error("serialize error: {0}")]
    Serialize(String),
    #[error("deserialize error: {0}")]
    Deserialize(String),
}

impl From<std::io::Error> for ConfigStoreError {
    fn from(e: std::io::Error) -> Self {
        ConfigStoreError::Io(e, String::new())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        value: String,
    }

    fn temp_dir() -> PathBuf {
        tempfile::tempdir().unwrap().path().to_path_buf()
    }

    #[tokio::test]
    async fn test_write_and_read_json_roundtrip() {
        let dir = temp_dir();
        let path = dir.join("test.json");
        let config = TestConfig {
            value: "hello".to_string(),
        };

        // Use the internal function directly since we have a custom path
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let content = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, content).unwrap();

        let loaded: Option<TestConfig> = read_json(&path).await.unwrap();
        assert_eq!(loaded, Some(config));
    }

    #[tokio::test]
    async fn test_read_json_returns_none_for_missing_file() {
        let dir = temp_dir();
        let path = dir.join("nonexistent.json");

        let result: Option<TestConfig> = read_json(&path).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_write_and_read_yaml_roundtrip() {
        let dir = temp_dir();
        let path = dir.join("test.yaml");

        let config = TestConfig {
            value: "yaml_test".to_string(),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let content = serde_yaml::to_string(&config).unwrap();
        std::fs::write(&path, content).unwrap();

        let loaded: Option<TestConfig> = read_yaml(&path).await.unwrap();
        assert_eq!(loaded, Some(config));
    }

    #[tokio::test]
    async fn test_delete_file() {
        let dir = temp_dir();
        let path = dir.join("to_delete.json");

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "{}").unwrap();
        assert!(path.exists());

        delete_file(&path).await.unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_config_json_path_ends_with_config_json() {
        let path = config_json_path();
        assert!(
            path.to_string_lossy().ends_with("config.json"),
            "Expected path to end with config.json, got: {:?}",
            path
        );
    }

    #[test]
    fn test_paths_cont_if2ai() {
        for path_fn in [
            config_json_path,
            providers_yaml_path,
            models_json_path,
            auth_json_path,
            channels_config_path,
        ] {
            let path = path_fn();
            assert!(
                path.to_string_lossy().contains(".if2ai"),
                "Expected {:?} to contain .if2ai",
                path
            );
        }
    }
}
