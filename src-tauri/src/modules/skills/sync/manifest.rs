//! Skill manifest — hash tracking for bundled skill synchronization.
//!
//! Ported from Hermes `tools/skills_sync.py` lines 52-108.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Sync-specific error type.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[allow(dead_code)]
    #[error("sync error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Parse(e.to_string())
    }
}

/// Result type for sync operations.
pub type SyncResult<T> = Result<T, SyncError>;

/// Manifest entry representing a single skill's origin hash.
///
/// Hermes reference: tools/skills_sync.py line 68 (v2 format: "name:hash")
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ManifestEntry {
    /// Skill name.
    pub name: String,
    /// Origin hash of the bundled skill (MD5 hash of directory).
    /// Empty string indicates v1 migration entry (no hash tracked).
    pub hash: String,
}

impl ManifestEntry {
    /// Parse a line in v2 format: "name:hash"
    fn parse_v2(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(Self {
                name: parts[0].to_string(),
                hash: parts[1].to_string(),
            })
        } else {
            None
        }
    }

    /// Format as v2 line: "name:hash"
    #[allow(dead_code)]
    fn format_v2(&self) -> String {
        format!("{}:{}", self.name, self.hash)
    }
}

/// Skill manifest — tracks bundled skills and their origin hashes.
///
/// v1 format: plain skill names, one per line
/// v2 format: "name:hash" per line
///
/// Hermes reference: tools/skills_sync.py lines 52-108.
#[derive(Debug, Clone, Default)]
pub struct SkillManifest {
    #[allow(dead_code)]
    entries: HashMap<String, String>, // skill_name -> origin_hash
}

#[allow(dead_code)]
impl SkillManifest {
    /// Read manifest from file, auto-detecting v1 vs v2 format.
    ///
    /// v1: plain skill names, one per line
    /// v2: "name:hash" per line
    pub fn read(manifest_path: &Path) -> SyncResult<Self> {
        if !manifest_path.exists() {
            return Ok(Self::default());
        }

        let file = fs::File::open(manifest_path)?;
        let reader = BufReader::new(file);
        let mut entries = HashMap::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(entry) = ManifestEntry::parse_v2(line) {
                // v2 format
                entries.insert(entry.name, entry.hash);
            } else {
                // v1 format: plain name, hash is empty string
                entries.insert(line.to_string(), String::new());
            }
        }

        Ok(Self { entries })
    }

    /// Write manifest in v2 format atomically.
    ///
    /// Uses temp file + rename for atomic write.
    pub fn write(&self, manifest_path: &Path) -> SyncResult<()> {
        // Build content
        let mut content = String::new();
        content.push_str("# Skill Manifest v2\n");
        content.push_str("# Format: skill_name:origin_hash\n");
        content.push_str("# Do not edit manually unless you know what you're doing\n");

        let mut names: Vec<_> = self.entries.keys().collect();
        names.sort();

        for name in names {
            let empty_hash = String::new();
            let hash = self.entries.get(name).unwrap_or(&empty_hash);
            let entry = ManifestEntry {
                name: name.clone(),
                hash: hash.clone(),
            };
            content.push_str(&entry.format_v2());
            content.push('\n');
        }

        // Atomic write using temp file
        let temp_path = manifest_path.with_extension("tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
        }
        fs::rename(&temp_path, manifest_path)?;

        Ok(())
    }

    /// Get the hash for a skill, if tracked.
    pub fn get_hash(&self, skill_name: &str) -> Option<&str> {
        self.entries.get(skill_name).map(|s| s.as_str())
    }

    /// Set or update the hash for a skill.
    pub fn set_hash(&mut self, skill_name: &str, hash: &str) {
        self.entries
            .insert(skill_name.to_string(), hash.to_string());
    }

    /// Remove a skill from the manifest.
    pub fn remove(&mut self, skill_name: &str) {
        self.entries.remove(skill_name);
    }

    /// Check if a skill exists in the manifest.
    pub fn contains(&self, skill_name: &str) -> bool {
        self.entries.contains_key(skill_name)
    }

    /// Get all skill names in the manifest.
    pub fn skill_names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_manifest_entry_parse_v2() {
        let entry = ManifestEntry::parse_v2("my-skill:abc123hash");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "my-skill");
        assert_eq!(entry.hash, "abc123hash");
    }

    #[test]
    fn test_manifest_entry_format_v2() {
        let entry = ManifestEntry {
            name: "my-skill".to_string(),
            hash: "abc123hash".to_string(),
        };
        assert_eq!(entry.format_v2(), "my-skill:abc123hash");
    }

    #[test]
    fn test_manifest_read_v2() {
        let temp = TempDir::new().unwrap();
        let manifest_path = temp.path().join("manifest.txt");

        fs::write(&manifest_path, "skill-a:hash1\nskill-b:hash2\n").unwrap();

        let manifest = SkillManifest::read(&manifest_path).unwrap();
        assert_eq!(manifest.get_hash("skill-a"), Some("hash1"));
        assert_eq!(manifest.get_hash("skill-b"), Some("hash2"));
    }

    #[test]
    fn test_manifest_read_v1() {
        let temp = TempDir::new().unwrap();
        let manifest_path = temp.path().join("manifest.txt");

        // v1 format: plain names
        fs::write(&manifest_path, "skill-a\nskill-b\n").unwrap();

        let manifest = SkillManifest::read(&manifest_path).unwrap();
        // v1 entries have empty hash
        assert_eq!(manifest.get_hash("skill-a"), Some(""));
        assert_eq!(manifest.get_hash("skill-b"), Some(""));
    }

    #[test]
    fn test_manifest_write_and_read() {
        let temp = TempDir::new().unwrap();
        let manifest_path = temp.path().join("manifest.txt");

        let mut manifest = SkillManifest::default();
        manifest.set_hash("skill-a", "hash1");
        manifest.set_hash("skill-b", "hash2");

        manifest.write(&manifest_path).unwrap();

        let read_manifest = SkillManifest::read(&manifest_path).unwrap();
        assert_eq!(read_manifest.get_hash("skill-a"), Some("hash1"));
        assert_eq!(read_manifest.get_hash("skill-b"), Some("hash2"));
    }

    #[test]
    fn test_manifest_remove() {
        let mut manifest = SkillManifest::default();
        manifest.set_hash("skill-a", "hash1");
        manifest.set_hash("skill-b", "hash2");

        manifest.remove("skill-a");

        assert!(!manifest.contains("skill-a"));
        assert!(manifest.contains("skill-b"));
    }

    #[test]
    fn test_manifest_nonexistent_file() {
        let manifest = SkillManifest::read(Path::new("/nonexistent/path")).unwrap();
        assert!(manifest.entries.is_empty());
    }
}
