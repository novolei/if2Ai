//! Snapshot module for skill export/import.
//!
//! Provides functionality to export skills configuration to a snapshot file
//! and import skills from a snapshot file.

pub mod types;

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[allow(unused_imports)]
pub use types::{ExportedSkill, SkillSnapshot, TapConfig, SNAPSHOT_VERSION};

/// Errors for snapshot operations.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid snapshot: {0}")]
    Invalid(String),
    #[error("snapshot not found: {0}")]
    NotFound(String),
}

/// Result type for snapshot operations.
pub type SnapshotResult<T> = Result<T, SnapshotError>;

/// Snapshot manager for export/import operations.
#[derive(Debug, Clone)]
pub struct SnapshotManager {
    /// Default snapshot directory.
    snapshot_dir: PathBuf,
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SnapshotManager {
    /// Create a new SnapshotManager.
    ///
    /// Uses ~/.if2ai/snapshots/ as the default directory.
    pub fn new() -> Self {
        let snapshot_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".if2ai")
            .join("snapshots");

        Self { snapshot_dir }
    }

    /// Create with a specific snapshot directory.
    pub fn with_dir(snapshot_dir: PathBuf) -> Self {
        Self { snapshot_dir }
    }

    /// Get the snapshot directory.
    pub fn snapshot_dir(&self) -> &Path {
        &self.snapshot_dir
    }

    /// Ensure the snapshot directory exists.
    pub fn ensure_dir(&self) -> SnapshotResult<()> {
        fs::create_dir_all(&self.snapshot_dir)?;
        Ok(())
    }

    /// Export a snapshot to a file.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The snapshot to export
    /// * `filename` - Optional filename (defaults to snapshot-<timestamp>.json)
    ///
    /// # Returns
    ///
    /// The path to the exported snapshot file.
    pub fn export(
        &self,
        snapshot: &SkillSnapshot,
        filename: Option<&str>,
    ) -> SnapshotResult<PathBuf> {
        self.ensure_dir()?;

        let filename = if let Some(f) = filename {
            f.to_string()
        } else {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            format!("snapshot-{}.json", timestamp)
        };

        let path = self.snapshot_dir.join(&filename);
        let json = serde_json::to_string_pretty(snapshot)?;
        fs::write(&path, json)?;

        Ok(path)
    }

    /// Import a snapshot from a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the snapshot file
    ///
    /// # Returns
    ///
    /// The imported snapshot.
    pub fn import(&self, path: &Path) -> SnapshotResult<SkillSnapshot> {
        if !path.exists() {
            return Err(SnapshotError::NotFound(path.display().to_string()));
        }

        let content = fs::read_to_string(path)?;
        let snapshot: SkillSnapshot = serde_json::from_str(&content)?;

        // Validate the snapshot
        self.validate(&snapshot)?;

        Ok(snapshot)
    }

    /// Import a snapshot from a string.
    ///
    /// # Arguments
    ///
    /// * `content` - JSON string content
    ///
    /// # Returns
    ///
    /// The imported snapshot.
    pub fn import_from_string(&self, content: &str) -> SnapshotResult<SkillSnapshot> {
        let snapshot: SkillSnapshot = serde_json::from_str(content)?;
        self.validate(&snapshot)?;
        Ok(snapshot)
    }

    /// List all snapshots in the snapshot directory.
    ///
    /// # Returns
    ///
    /// A list of snapshot file paths.
    pub fn list(&self) -> SnapshotResult<Vec<PathBuf>> {
        if !self.snapshot_dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();

        for entry in fs::read_dir(&self.snapshot_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "json" {
                        snapshots.push(path);
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        snapshots.sort_by(|a, b| {
            let a_meta = fs::metadata(a).ok();
            let b_meta = fs::metadata(b).ok();
            match (a_meta, b_meta) {
                (Some(a_m), Some(b_m)) => b_m
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                    .cmp(&a_m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)),
                _ => std::cmp::Ordering::Equal,
            }
        });

        Ok(snapshots)
    }

    /// Delete a snapshot file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the snapshot file to delete
    ///
    /// # Returns
    ///
    /// Ok if deleted, error otherwise.
    pub fn delete(&self, path: &Path) -> SnapshotResult<()> {
        if !path.exists() {
            return Err(SnapshotError::NotFound(path.display().to_string()));
        }

        fs::remove_file(path)?;
        Ok(())
    }

    /// Validate a snapshot.
    ///
    /// Checks that the snapshot has a valid format and version.
    pub fn validate(&self, snapshot: &SkillSnapshot) -> SnapshotResult<()> {
        if snapshot.snapshot_version.is_empty() {
            return Err(SnapshotError::Invalid(
                "snapshot_version is required".to_string(),
            ));
        }

        if snapshot.hermes_version.is_empty() {
            return Err(SnapshotError::Invalid(
                "hermes_version is required".to_string(),
            ));
        }

        // Validate skill entries
        for skill in &snapshot.skills {
            if skill.name.is_empty() {
                return Err(SnapshotError::Invalid("skill name is required".to_string()));
            }
            if skill.source.is_empty() {
                return Err(SnapshotError::Invalid(format!(
                    "skill '{}' source is required",
                    skill.name
                )));
            }
        }

        // Validate tap entries
        for tap in &snapshot.taps {
            if tap.id.is_empty() {
                return Err(SnapshotError::Invalid("tap id is required".to_string()));
            }
            if tap.source_url.is_empty() {
                return Err(SnapshotError::Invalid(format!(
                    "tap '{}' source_url is required",
                    tap.id
                )));
            }
        }

        Ok(())
    }

    /// Build a snapshot from current hub state.
    ///
    /// This is a placeholder that would be connected to the HubStateManager
    /// in a full implementation.
    #[allow(dead_code)]
    pub fn build_from_hub(&self) -> SkillSnapshot {
        // In a full implementation, this would query HubStateManager
        // to get current skills and taps
        SkillSnapshot::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_manager_new() {
        let manager = SnapshotManager::new();
        assert!(manager.snapshot_dir().to_string_lossy().contains(".if2ai"));
    }

    #[test]
    fn test_snapshot_manager_with_dir() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());
        assert_eq!(manager.snapshot_dir(), temp.path());
    }

    #[test]
    fn test_export_and_import() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        let snapshot = SkillSnapshot::new()
            .add_skill(ExportedSkill::new(
                "test-skill".to_string(),
                "github".to_string(),
                "owner/repo/test-skill".to_string(),
            ))
            .add_tap(TapConfig::new(
                "test-tap".to_string(),
                "https://example.com".to_string(),
                "github".to_string(),
            ));

        let path = manager
            .export(&snapshot, Some("test-snapshot.json"))
            .unwrap();
        assert!(path.exists());

        let imported = manager.import(&path).unwrap();
        assert_eq!(imported.skill_count(), 1);
        assert_eq!(imported.tap_count(), 1);
        assert_eq!(imported.skills[0].name, "test-skill");
    }

    #[test]
    fn test_import_from_string() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        let json = r#"{
            "hermes_version": "2.0",
            "snapshot_version": "1.0",
            "exported_at": "2024-01-01T00:00:00Z",
            "skills": [
                {
                    "name": "my-skill",
                    "source": "github",
                    "identifier": "owner/repo/my-skill",
                    "enabled": true,
                    "trust_level": "community",
                    "metadata": {}
                }
            ],
            "taps": [],
            "metadata": {}
        }"#;

        let snapshot = manager.import_from_string(json).unwrap();
        assert_eq!(snapshot.skill_count(), 1);
        assert_eq!(snapshot.skills[0].name, "my-skill");
    }

    #[test]
    fn test_validate_valid_snapshot() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        let snapshot = SkillSnapshot::new().add_skill(ExportedSkill::new(
            "test".to_string(),
            "github".to_string(),
            "owner/repo".to_string(),
        ));

        assert!(manager.validate(&snapshot).is_ok());
    }

    #[test]
    fn test_validate_invalid_snapshot() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        // Empty snapshot is actually valid (empty skills and taps are allowed)
        let snapshot = SkillSnapshot::new();
        let result = manager.validate(&snapshot);
        assert!(result.is_ok());

        // Missing version
        let mut bad_snapshot = SkillSnapshot::new();
        bad_snapshot.snapshot_version = "".to_string();
        let result = manager.validate(&bad_snapshot);
        assert!(result.is_err());

        // Missing hermes_version
        let mut bad_snapshot2 = SkillSnapshot::new();
        bad_snapshot2.hermes_version = "".to_string();
        let result2 = manager.validate(&bad_snapshot2);
        assert!(result2.is_err());

        // Skill with empty name
        let mut bad_snapshot3 = SkillSnapshot::new();
        bad_snapshot3.skills.push(ExportedSkill::new(
            "".to_string(),
            "github".to_string(),
            "owner/repo".to_string(),
        ));
        let result3 = manager.validate(&bad_snapshot3);
        assert!(result3.is_err());
    }

    #[test]
    fn test_list_snapshots() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        // Initially empty
        let snapshots = manager.list().unwrap();
        assert!(snapshots.is_empty());

        // Create some snapshots
        let snapshot = SkillSnapshot::new();
        manager.export(&snapshot, Some("snapshot-1.json")).unwrap();
        manager.export(&snapshot, Some("snapshot-2.json")).unwrap();

        let snapshots = manager.list().unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_delete_snapshot() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        let snapshot = SkillSnapshot::new();
        let path = manager.export(&snapshot, Some("to-delete.json")).unwrap();
        assert!(path.exists());

        manager.delete(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_import_nonexistent() {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::with_dir(temp.path().to_path_buf());

        let result = manager.import(&temp.path().join("nonexistent.json"));
        assert!(result.is_err());
    }
}
