//! Hub state management — quarantine, lock, audit, and unified search.
//!
//! Ported from Hermes `tools/skills_hub.py` lines 46-53, 129-245.

use super::source::{BoxedSkillSource, SkillSource};
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use crate::modules::skills::guard::policy::TrustLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Hub directory paths — matches Hermes `~/.if2ai/skills/.hub/` structure.
///
/// Hermes reference: tools/skills_hub.py lines 46-53.
#[derive(Debug, Clone)]
pub struct HubPaths {
    /// Root skills directory: `~/.if2ai/skills/`
    pub skills_dir: PathBuf,
    /// Hub state directory: `~/.if2ai/skills/.hub/`
    pub hub_dir: PathBuf,
    /// Quarantine directory for suspicious skills: `~/.if2ai/skills/.hub/quarantine/`
    pub quarantine_dir: PathBuf,
    /// Lock file tracking installed skills: `~/.if2ai/skills/.hub/lock.json`
    pub lock_file: PathBuf,
    /// Audit log file: `~/.if2ai/skills/.hub/audit.log`
    pub audit_log: PathBuf,
    /// TAPS (Trusted Agent Package Sources) config: `~/.if2ai/skills/.hub/taps.json`
    pub taps_file: PathBuf,
    /// Index cache directory: `~/.if2ai/skills/.hub/index-cache/`
    pub index_cache_dir: PathBuf,
}

impl HubPaths {
    /// Create HubPaths from a root directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();
        let hub_dir = root.join(".hub");
        Self {
            skills_dir: root,
            hub_dir: hub_dir.clone(),
            quarantine_dir: hub_dir.join("quarantine"),
            lock_file: hub_dir.join("lock.json"),
            audit_log: hub_dir.join("audit.log"),
            taps_file: hub_dir.join("taps.json"),
            index_cache_dir: hub_dir.join("index-cache"),
        }
    }

    /// Create all required hub directories.
    pub fn ensure_dirs(&self) -> HubResult<()> {
        fs::create_dir_all(&self.quarantine_dir)?;
        fs::create_dir_all(&self.index_cache_dir)?;
        Ok(())
    }
}

/// Default HubPaths pointing to `~/.if2ai/skills/`.
impl Default for HubPaths {
    fn default() -> Self {
        // Use HOME environment variable or fall back to current directory
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::new(home.join(".if2ai").join("skills"))
    }
}

/// Entry in the hub lock file — tracks an installed skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubLockEntry {
    /// Skill name.
    pub skill_name: String,
    /// Source identifier (e.g., "github", "clawhub").
    pub source: String,
    /// Source-specific identifier (e.g., "owner/repo/path").
    pub identifier: String,
    /// When this skill was installed.
    pub installed_at: DateTime<Utc>,
    /// Version string if available.
    pub version: Option<String>,
}

/// Hub lock file manager — tracks all installed skills.
///
/// Uses a RwLock for thread-safe read/write access to the lock file.
#[derive(Debug)]
pub struct HubLock {
    entries: RwLock<Vec<HubLockEntry>>,
    lock_file: PathBuf,
}

impl HubLock {
    /// Get a clone of all entries.
    pub fn get_entries(&self) -> Vec<HubLockEntry> {
        self.entries
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Load hub lock from file, or return empty lock if file doesn't exist.
    pub fn load(lock_file: impl Into<PathBuf>) -> HubResult<Self> {
        let lock_file: PathBuf = lock_file.into();
        let entries = if lock_file.exists() {
            let content = fs::read_to_string(&lock_file)?;
            let loaded: Vec<HubLockEntry> = serde_json::from_str(&content)
                .map_err(|e| HubError::Parse(format!("failed to parse lock.json: {}", e)))?;
            loaded
        } else {
            Vec::new()
        };
        Ok(Self {
            entries: RwLock::new(entries),
            lock_file,
        })
    }

    /// Save current entries to the lock file.
    pub fn save(&self) -> HubResult<()> {
        let entries = self
            .entries
            .read()
            .map_err(|e| HubError::Other(format!("lock poisoned: {}", e)))?;
        let content = serde_json::to_string_pretty(&*entries)?;
        fs::write(&self.lock_file, content)?;
        Ok(())
    }

    /// Add a new entry to the lock.
    pub fn add(&self, entry: HubLockEntry) -> HubResult<()> {
        {
            let mut entries = self
                .entries
                .write()
                .map_err(|e| HubError::Other(format!("lock poisoned: {}", e)))?;
            // Remove existing entry for same skill if present
            entries.retain(|e| e.skill_name != entry.skill_name);
            entries.push(entry);
        }
        self.save()
    }

    /// Remove an entry from the lock by skill name.
    pub fn remove(&self, skill_name: &str) -> HubResult<()> {
        {
            let mut entries = self
                .entries
                .write()
                .map_err(|e| HubError::Other(format!("lock poisoned: {}", e)))?;
            entries.retain(|e| e.skill_name != skill_name);
        }
        self.save()
    }

    /// Get an entry by skill name.
    pub fn get(&self, skill_name: &str) -> HubResult<Option<HubLockEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| HubError::Other(format!("lock poisoned: {}", e)))?;
        Ok(entries.iter().find(|e| e.skill_name == skill_name).cloned())
    }
}

/// Audit event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum AuditEventType {
    /// Skill was installed.
    Install,
    /// Skill was uninstalled.
    Uninstall,
    /// Skill was updated.
    Update,
    /// Skill was quarantined.
    Quarantine,
    /// Skill failed to install.
    InstallFailed { error: String },
    /// Skill was blocked by security scan.
    Blocked { reason: String },
    /// General action.
    Action { action: String },
}

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// When this event occurred.
    pub timestamp: DateTime<Utc>,
    /// Type of event.
    pub event: AuditEventType,
    /// Skill name if applicable.
    pub skill_name: Option<String>,
    /// Source identifier if applicable.
    pub source: Option<String>,
    /// Source-specific identifier if applicable.
    pub identifier: Option<String>,
    /// Additional details as JSON.
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

impl AuditEvent {
    /// Create a new audit event with the current timestamp.
    pub fn new(
        event: AuditEventType,
        skill_name: Option<String>,
        source: Option<String>,
        identifier: Option<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event,
            skill_name,
            source,
            identifier,
            details: HashMap::new(),
        }
    }

    /// Create an install event.
    pub fn install(skill_name: &str, source: &str, identifier: &str) -> Self {
        Self::new(
            AuditEventType::Install,
            Some(skill_name.to_string()),
            Some(source.to_string()),
            Some(identifier.to_string()),
        )
    }

    /// Create an uninstall event.
    pub fn uninstall(skill_name: &str) -> Self {
        Self::new(
            AuditEventType::Uninstall,
            Some(skill_name.to_string()),
            None,
            None,
        )
    }

    /// Create a quarantine event.
    pub fn quarantine(skill_name: &str, reason: &str) -> Self {
        let mut details = HashMap::new();
        details.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
        let mut event = Self::new(
            AuditEventType::Quarantine,
            Some(skill_name.to_string()),
            None,
            None,
        );
        event.details = details;
        event
    }

    /// Create a blocked event.
    pub fn blocked(skill_name: &str, reason: &str) -> Self {
        let mut details = HashMap::new();
        details.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
        let mut event = Self::new(
            AuditEventType::Blocked {
                reason: reason.to_string(),
            },
            Some(skill_name.to_string()),
            None,
            None,
        );
        event.details = details;
        event
    }

    /// Create a failed install event.
    pub fn install_failed(skill_name: &str, error: &str) -> Self {
        Self::new(
            AuditEventType::InstallFailed {
                error: error.to_string(),
            },
            Some(skill_name.to_string()),
            None,
            None,
        )
    }
}

/// Trust rank for deduplication — higher rank wins.
fn trust_rank(level: &TrustLevel) -> i32 {
    match level {
        TrustLevel::Builtin => 2,
        TrustLevel::Trusted => 1,
        TrustLevel::AgentCreated => 0,
        TrustLevel::Community => 0,
    }
}

/// Hub state manager — coordinates sources, quarantine, lock, and audit.
///
/// Hermes reference: tools/skills_hub.py lines 46-53, 129-245.
pub struct HubState {
    /// Hub directory paths.
    pub paths: HubPaths,
    /// Available skill sources.
    pub sources: Vec<BoxedSkillSource>,
    /// GitHub authentication if configured.
    pub auth: Option<crate::modules::skills::hub::github::GitHubAuth>,
    /// Lock file manager for tracking installed skills.
    pub lock: HubLock,
}

impl HubState {
    /// Create a new HubState with the given paths and sources.
    pub fn new(
        paths: HubPaths,
        sources: Vec<BoxedSkillSource>,
        auth: Option<crate::modules::skills::hub::github::GitHubAuth>,
    ) -> HubResult<Self> {
        paths.ensure_dirs()?;
        let lock = HubLock::load(&paths.lock_file)?;
        Ok(Self {
            paths,
            sources,
            auth,
            lock,
        })
    }

    /// Create a HubState with default paths and the given sources.
    pub fn with_sources(sources: Vec<BoxedSkillSource>) -> HubResult<Self> {
        let paths = HubPaths::default();
        Self::new(paths, sources, None)
    }

    /// Quarantine a skill bundle — copy to quarantine directory.
    ///
    /// Returns the path to the quarantined bundle.
    #[allow(clippy::collapsible_str_replace)]
    pub fn quarantine_bundle(&self, bundle: &SkillBundle) -> HubResult<PathBuf> {
        // Create a safe name for quarantine
        let safe_name = bundle
            .name
            .replace('/', "_")
            .replace('\\', "_")
            .replace(' ', "_")
            .replace("..", "_");
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let quarantine_name = format!("{}_{}", timestamp, safe_name);
        let quarantine_path = self.paths.quarantine_dir.join(&quarantine_name);

        // Create quarantine subdirectory
        fs::create_dir_all(&quarantine_path)?;

        // Write each file in the bundle
        for (relative_path, content) in &bundle.files {
            let file_path = quarantine_path.join(relative_path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file_path, content)?;
        }

        Ok(quarantine_path)
    }

    /// Install a skill from quarantine to the skills directory.
    ///
    /// Returns the path to the installed skill.
    #[allow(clippy::collapsible_str_replace)]
    pub fn install_from_quarantine(&self, skill_name: &str) -> HubResult<PathBuf> {
        // Find the skill in quarantine
        let quarantine_name = format!(
            "*{}*",
            skill_name
                .replace('/', "_")
                .replace('\\', "_")
                .replace(' ', "_")
                .replace("..", "_")
        );
        let entries = fs::read_dir(&self.paths.quarantine_dir)?;
        let mut found_path: Option<PathBuf> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains(skill_name))
                    .unwrap_or(false)
            {
                found_path = Some(path);
                break;
            }
        }

        let from_path = found_path.ok_or_else(|| {
            HubError::NotFound(format!("skill '{}' not found in quarantine", skill_name))
        })?;

        // Install to skills directory
        let to_path = self.paths.skills_dir.join(skill_name);
        if to_path.exists() {
            fs::remove_dir_all(&to_path)?;
        }
        copy_dir_recursive(&from_path, &to_path)?;

        Ok(to_path)
    }

    /// Record a skill installation in the lock file.
    pub fn record_install(
        &self,
        skill_name: &str,
        source: &str,
        identifier: &str,
    ) -> HubResult<()> {
        let entry = HubLockEntry {
            skill_name: skill_name.to_string(),
            source: source.to_string(),
            identifier: identifier.to_string(),
            installed_at: chrono::Utc::now(),
            version: None,
        };
        self.lock.add(entry)
    }

    /// Unified search across all sources with deduplication.
    ///
    /// Results are deduplicated by skill name, with higher trust level winning.
    pub async fn unified_search(
        &self,
        query: &str,
        source_filter: Option<&str>,
        limit: usize,
    ) -> HubResult<Vec<SkillMeta>> {
        let mut all_results: HashMap<String, (SkillMeta, i32)> = HashMap::new();

        for source in &self.sources {
            // Apply source filter if specified
            if let Some(filter) = source_filter {
                if source.source_id() != filter {
                    continue;
                }
            }

            match source.search(query, limit).await {
                Ok(metas) => {
                    for meta in metas {
                        let rank = trust_rank(&meta.trust_level);
                        let entry = all_results
                            .entry(meta.name.clone())
                            .or_insert_with(|| (meta.clone(), rank));

                        // Replace if higher trust
                        if rank > entry.1 {
                            entry.0 = meta;
                            entry.1 = rank;
                        }
                    }
                }
                Err(e) => {
                    // Log error but continue with other sources
                    eprintln!("search error from {}: {}", source.source_id(), e);
                }
            }
        }

        let mut results: Vec<_> = all_results.into_values().map(|(meta, _)| meta).collect();

        // Sort by trust rank descending
        results.sort_by(|a, b| {
            let rank_a = trust_rank(&a.trust_level);
            let rank_b = trust_rank(&b.trust_level);
            rank_b.cmp(&rank_a)
        });

        results.truncate(limit);
        Ok(results)
    }

    /// Append an audit event to the audit log.
    pub fn append_audit_log(&self, event: &AuditEvent) -> HubResult<()> {
        let line = serde_json::to_string(event)?;
        let line = format!("{}\n", line);
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.audit_log)?
            .write_all(line.as_bytes())?;
        Ok(())
    }
}

/// Copy a directory recursively.
fn copy_dir_recursive(from: &Path, to: &Path) -> HubResult<()> {
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
    fn test_hub_paths_default() {
        let paths = HubPaths::default();
        assert!(paths.skills_dir.ends_with(".if2ai/skills"));
        assert!(paths.hub_dir.ends_with(".if2ai/skills/.hub"));
    }

    #[test]
    fn test_hub_paths_new() {
        let temp = TempDir::new().unwrap();
        let paths = HubPaths::new(temp.path());
        assert_eq!(paths.skills_dir, temp.path());
        assert_eq!(paths.hub_dir, temp.path().join(".hub"));
    }

    #[test]
    fn test_hub_lock_load_save() {
        let temp = TempDir::new().unwrap();
        let lock_file = temp.path().join("lock.json");
        let lock = HubLock::load(&lock_file).unwrap();

        // Should be empty initially
        let entries = lock.entries.read().unwrap();
        assert!(entries.is_empty());

        // Add an entry
        drop(entries);
        let entry = HubLockEntry {
            skill_name: "test-skill".to_string(),
            source: "github".to_string(),
            identifier: "owner/repo".to_string(),
            installed_at: chrono::Utc::now(),
            version: Some("1.0.0".to_string()),
        };
        lock.add(entry).unwrap();

        // Reload and verify
        let lock2 = HubLock::load(&lock_file).unwrap();
        let retrieved = lock2.get("test-skill").unwrap().unwrap();
        assert_eq!(retrieved.skill_name, "test-skill");
        assert_eq!(retrieved.source, "github");
    }

    #[test]
    fn test_hub_lock_remove() {
        let temp = TempDir::new().unwrap();
        let lock_file = temp.path().join("lock.json");
        let lock = HubLock::load(&lock_file).unwrap();

        let entry = HubLockEntry {
            skill_name: "remove-me".to_string(),
            source: "github".to_string(),
            identifier: "owner/repo".to_string(),
            installed_at: chrono::Utc::now(),
            version: None,
        };
        lock.add(entry).unwrap();
        lock.remove("remove-me").unwrap();

        let retrieved = lock.get("remove-me").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_audit_event_create() {
        let event = AuditEvent::install("test-skill", "github", "owner/repo");
        match event.event {
            AuditEventType::Install => {}
            _ => panic!("expected Install event"),
        }
        assert_eq!(event.skill_name, Some("test-skill".to_string()));
    }

    #[test]
    fn test_audit_event_quarantine() {
        let event = AuditEvent::quarantine("test-skill", "suspicious content");
        match event.event {
            AuditEventType::Quarantine => {}
            _ => panic!("expected Quarantine event"),
        }
        assert_eq!(
            event.details.get("reason").unwrap().as_str().unwrap(),
            "suspicious content"
        );
    }

    #[test]
    fn test_trust_rank_ordering() {
        assert!(trust_rank(&TrustLevel::Builtin) > trust_rank(&TrustLevel::Trusted));
        assert!(trust_rank(&TrustLevel::Trusted) > trust_rank(&TrustLevel::Community));
        assert!(trust_rank(&TrustLevel::Community) == trust_rank(&TrustLevel::AgentCreated));
    }
}
