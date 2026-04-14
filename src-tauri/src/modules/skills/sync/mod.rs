//! Skill synchronization — bundled skill manifest management.
//!
//! Ported from Hermes `tools/skills_sync.py` lines 52-108, 141-280.

pub mod manifest;

use manifest::{SyncError, SyncResult as ManifestSyncResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub use manifest::SkillManifest;

/// Result of a sync operation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SyncOpResult {
    /// Newly copied skills (not in manifest before sync).
    pub copied: Vec<String>,
    /// Skills that were updated (hash changed).
    pub updated: Vec<String>,
    /// Skills that were skipped (no changes).
    pub skipped: usize,
    /// Skills that were skipped because user modified them.
    pub user_modified: Vec<String>,
    /// Skills that were cleaned (removed from manifest but bundled still exists).
    pub cleaned: Vec<String>,
    /// Total number of bundled skills found.
    pub total_bundled: usize,
}

/// Skill synchronizer — syncs bundled skills to user directory with hash tracking.
///
/// Tracks user modifications to prevent overwriting their changes.
#[allow(dead_code)]
pub struct SkillSync {
    /// Directory containing bundled skills.
    bundled_dir: PathBuf,
    /// User's skills directory.
    user_dir: PathBuf,
    /// Path to the manifest file.
    manifest_path: PathBuf,
    /// In-memory manifest cache.
    manifest: RwLock<SkillManifest>,
}

#[allow(dead_code)]
impl SkillSync {
    /// Create a new SkillSync with the given directories.
    pub fn new(
        bundled_dir: impl Into<PathBuf>,
        user_dir: impl Into<PathBuf>,
        manifest_path: impl Into<PathBuf>,
    ) -> ManifestSyncResult<Self> {
        let manifest_path_buf: PathBuf = manifest_path.into();
        let manifest = SkillManifest::read(&manifest_path_buf).unwrap_or_default();
        Ok(Self {
            bundled_dir: bundled_dir.into(),
            user_dir: user_dir.into(),
            manifest_path: manifest_path_buf,
            manifest: RwLock::new(manifest),
        })
    }

    /// Create a SkillSync with default paths under the given root.
    ///
    /// Default paths:
    /// - bundled_dir: `{root}/bundled-skills/`
    /// - user_dir: `{root}/skills/`
    /// - manifest_path: `{root}/bundled-skills/.manifest`
    #[allow(dead_code)]
    pub fn with_root(root: impl Into<PathBuf>) -> ManifestSyncResult<Self> {
        let root = root.into();
        Self::new(
            root.join("bundled-skills"),
            root.join("skills"),
            root.join("bundled-skills/.manifest"),
        )
    }

    /// Discover all bundled skills by finding SKILL.md files.
    ///
    /// Returns a list of (skill_name, skill_dir_path) tuples.
    pub fn discover_bundled_skills(&self) -> Vec<(String, PathBuf)> {
        let mut skills = Vec::new();

        if !self.bundled_dir.exists() {
            return skills;
        }

        if let Ok(entries) = fs::read_dir(&self.bundled_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("SKILL.md").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        skills.push((name.to_string(), path));
                    }
                }
            }
        }

        skills.sort_by(|a, b| a.0.cmp(&b.0));
        skills
    }

    /// Compute MD5 hash of a directory's contents.
    ///
    /// Hashes all files sorted by path, combining content.
    /// Matches Hermes `tools/skills_sync.py` lines 141-152.
    pub fn compute_dir_hash(dir: &Path) -> String {
        use std::collections::BTreeSet;

        let mut file_hashes: BTreeSet<PathBuf> = BTreeSet::new();

        // Collect all files recursively
        collect_files(dir, dir, &mut file_hashes);

        // Combine all file contents
        let mut hasher = md5::Context::new();
        for file_path in file_hashes {
            if let Ok(content) = fs::read(&file_path) {
                let mut file_hasher = md5::Context::new();
                file_hasher.consume(file_path.display().to_string());
                file_hasher.consume(content.as_slice());
                hasher.consume(file_hasher.compute().0);
            }
        }

        format!("{:x}", hasher.compute())
    }

    /// Perform full sync from bundled to user directory.
    ///
    /// Logic per Hermes lines 180-250:
    /// - NEW skill: copy to user dir, record hash
    /// - EXISTING + user unchanged: safe to update
    /// - EXISTING + user modified: SKIP (don't overwrite user changes)
    /// - DELETED by user: respected, not re-added
    pub fn sync(&self) -> ManifestSyncResult<SyncOpResult> {
        let bundled_skills = self.discover_bundled_skills();
        let total_bundled = bundled_skills.len();

        let mut copied = Vec::new();
        let mut updated = Vec::new();
        let mut skipped = 0;
        let mut user_modified = Vec::new();
        let mut cleaned = Vec::new();

        // Ensure user dir exists
        fs::create_dir_all(&self.user_dir).map_err(SyncError::Io)?;

        // Load manifest
        let mut manifest = self
            .manifest
            .write()
            .map_err(|e| SyncError::Other(format!("manifest lock poisoned: {}", e)))?;

        // Second pass: sync bundled to user
        for (skill_name, bundled_path) in &bundled_skills {
            let user_skill_path = self.user_dir.join(skill_name);
            let bundled_hash = Self::compute_dir_hash(bundled_path);

            let manifest_hash = manifest.get_hash(skill_name);
            let user_has_skill = user_skill_path.exists();

            match (manifest_hash, user_has_skill) {
                // NEW skill: not in manifest, not in user dir
                (None, false) => {
                    // Copy to user dir
                    if let Err(e) = copy_dir_recursive(bundled_path, &user_skill_path) {
                        eprintln!("failed to copy {}: {}", skill_name, e);
                        continue;
                    }
                    manifest.set_hash(skill_name, &bundled_hash);
                    copied.push(skill_name.clone());
                }
                // EXISTING in manifest
                (Some(old_hash), _) => {
                    // Check if bundled changed
                    if old_hash == bundled_hash {
                        // Bundled unchanged - check user modification
                        if user_skill_path.exists() {
                            let user_hash = Self::compute_dir_hash(&user_skill_path);
                            if user_hash != old_hash {
                                // User modified - skip
                                skipped += 1;
                                user_modified.push(skill_name.clone());
                                continue;
                            }
                        }
                        // User unchanged - safe to update bundled
                        if user_skill_path.exists() {
                            // Backup before update
                            let backup_path = user_skill_path.with_extension("bak");
                            let _ = fs::rename(&user_skill_path, &backup_path);
                        }
                        if let Err(e) = copy_dir_recursive(bundled_path, &user_skill_path) {
                            eprintln!("failed to update {}: {}", skill_name, e);
                            // Restore backup
                            let _ =
                                fs::rename(user_skill_path.with_extension("bak"), &user_skill_path);
                            continue;
                        }
                        // Clean up backup
                        let _ = fs::remove_file(user_skill_path.with_extension("bak"));
                        updated.push(skill_name.clone());
                        manifest.set_hash(skill_name, &bundled_hash);
                    } else {
                        // Bundled changed - check user modification
                        if user_skill_path.exists() {
                            let user_hash = Self::compute_dir_hash(&user_skill_path);
                            if user_hash != *old_hash {
                                // User modified - skip update
                                skipped += 1;
                                user_modified.push(skill_name.clone());
                                manifest.set_hash(skill_name, &user_hash); // Update manifest with user hash
                                continue;
                            }
                        }
                        // Backup before update
                        if user_skill_path.exists() {
                            let backup_path = user_skill_path.with_extension("bak");
                            let _ = fs::rename(&user_skill_path, &backup_path);
                        }
                        if let Err(e) = copy_dir_recursive(bundled_path, &user_skill_path) {
                            eprintln!("failed to update {}: {}", skill_name, e);
                            // Restore backup
                            let backup_path = user_skill_path.with_extension("bak");
                            if backup_path.exists() {
                                let _ = fs::rename(&backup_path, &user_skill_path);
                            }
                            continue;
                        }
                        let _ = fs::remove_file(user_skill_path.with_extension("bak"));
                        updated.push(skill_name.clone());
                        manifest.set_hash(skill_name, &bundled_hash);
                    }
                }
                // Not in manifest, but user dir has it (edge case)
                (None, true) => {
                    // Treat as user-modified, don't overwrite
                    let user_hash = Self::compute_dir_hash(&user_skill_path);
                    manifest.set_hash(skill_name, &user_hash);
                    skipped += 1;
                    user_modified.push(skill_name.clone());
                }
            }
        }

        // Third pass: handle manifest entries for skills no longer bundled
        for skill_name in manifest.skill_names() {
            if !bundled_skills.iter().any(|(n, _)| n == &skill_name) {
                // Skill in manifest but not in bundled
                cleaned.push(skill_name.clone());
                manifest.remove(&skill_name);
            }
        }

        // Save manifest
        manifest
            .write(&self.manifest_path)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(SyncOpResult {
            copied,
            updated,
            skipped,
            user_modified,
            cleaned,
            total_bundled,
        })
    }
}

/// Recursively collect all files in a directory.
#[allow(dead_code)]
fn collect_files(base: &Path, current: &Path, files: &mut std::collections::BTreeSet<PathBuf>) {
    if let Ok(entries) = fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(base, &path, files);
            } else {
                // Store relative path for consistent ordering
                if let Ok(rel) = path.strip_prefix(base) {
                    let mut full_path = base.to_path_buf();
                    full_path.push(rel);
                    files.insert(full_path);
                } else {
                    files.insert(path);
                }
            }
        }
    }
}

/// Copy a directory recursively.
#[allow(dead_code)]
fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        let dest = to.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_discover_bundled_skills() {
        let temp = TempDir::new().unwrap();
        let bundled = temp.path().join("bundled");
        fs::create_dir_all(&bundled).unwrap();

        // Create some skills
        let skill_a = bundled.join("skill-a");
        fs::create_dir_all(&skill_a).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# Skill A").unwrap();

        let skill_b = bundled.join("skill-b");
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_b.join("SKILL.md"), "# Skill B").unwrap();

        let sync = SkillSync::new(
            &bundled,
            temp.path().join("user"),
            temp.path().join("manifest"),
        )
        .unwrap();
        let skills = sync.discover_bundled_skills();

        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].0, "skill-a");
        assert_eq!(skills[1].0, "skill-b");
    }

    #[test]
    fn test_compute_dir_hash() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("test-dir");
        fs::create_dir_all(&dir).unwrap();

        // Create some files
        fs::write(dir.join("file1.txt"), "content1").unwrap();
        fs::write(dir.join("file2.txt"), "content2").unwrap();

        let hash1 = SkillSync::compute_dir_hash(&dir);
        assert!(!hash1.is_empty());

        // Same content should give same hash
        let hash2 = SkillSync::compute_dir_hash(&dir);
        assert_eq!(hash1, hash2);

        // Different content should give different hash
        fs::write(dir.join("file1.txt"), "modified").unwrap();
        let hash3 = SkillSync::compute_dir_hash(&dir);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_sync_new_skill() {
        let temp = TempDir::new().unwrap();
        let bundled = temp.path().join("bundled");
        let user = temp.path().join("user");
        let manifest = temp.path().join("manifest");

        fs::create_dir_all(&bundled).unwrap();

        // Create a skill
        let skill_a = bundled.join("skill-a");
        fs::create_dir_all(&skill_a).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# Skill A").unwrap();

        let sync = SkillSync::new(&bundled, &user, &manifest).unwrap();
        let result = sync.sync().unwrap();

        assert_eq!(result.copied.len(), 1);
        assert_eq!(result.copied[0], "skill-a");
        assert!(user.join("skill-a/SKILL.md").exists());
    }
}
