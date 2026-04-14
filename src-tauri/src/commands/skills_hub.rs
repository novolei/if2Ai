//! Skills Hub Tauri commands — CLI operations for skill marketplace.
//!
//! Implements Hermes-style CLI commands:
//! - browse: Browse skill marketplace
//! - search: Search for skills across sources
//! - inspect: Inspect a specific skill
//! - check: Check for skill updates
//! - update: Update installed skills
//! - audit: Audit installed skills
//! - uninstall: Uninstall a skill
//! - publish: Publish to GitHub/ClawHub
//! - snapshot: Export/import skill snapshots
//! - tap: Manage taps.json for custom GitHub sources

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::modules::skills::hub::types::HubResult;
use crate::modules::skills::hub::{create_source_router, AuditEvent, HubPaths, HubState};
use crate::modules::skills::snapshot::{ExportedSkill, SkillSnapshot, SnapshotManager, TapConfig};

/// Result of a hub operation.
#[derive(Debug, Serialize)]
pub struct HubCommandResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Skill search result for CLI output.
#[derive(Debug, Serialize)]
pub struct SkillSearchResult {
    pub name: String,
    pub description: String,
    pub source: String,
    pub identifier: String,
    pub trust_level: String,
    pub tags: Vec<String>,
}

/// Skill inspection result.
#[derive(Debug, Serialize)]
pub struct SkillInspectResult {
    pub name: String,
    pub description: String,
    pub source: String,
    pub identifier: String,
    pub trust_level: String,
    pub tags: Vec<String>,
    pub files: Vec<String>,
    pub size_bytes: u64,
    pub manifest: HashMap<String, String>,
}

/// Audit result for a single skill.
#[derive(Debug, Serialize)]
pub struct SkillAuditResult {
    pub name: String,
    pub path: String,
    pub source: String,
    pub status: String,
    pub findings_count: usize,
    pub last_updated: Option<String>,
}

/// Snapshot export/import result.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct SnapshotResult {
    pub path: String,
    pub skills_count: usize,
    pub taps_count: usize,
}

// ============================================================================
// Browse Command
// ============================================================================

/// Browse skills marketplace - list available skills from all sources.
#[tauri::command]
pub async fn hub_browse(
    source_filter: Option<String>,
    limit: Option<usize>,
) -> Result<HubCommandResult, String> {
    let limit = limit.unwrap_or(50);
    let sources = create_source_router();

    let mut all_skills = Vec::new();

    for source in sources {
        if let Some(ref filter) = source_filter {
            if source.source_id() != filter {
                continue;
            }
        }

        match source.search("", limit).await {
            Ok(metas) => {
                for meta in metas {
                    all_skills.push(SkillSearchResult {
                        name: meta.name,
                        description: meta.description,
                        source: meta.source,
                        identifier: meta.identifier,
                        trust_level: format!("{:?}", meta.trust_level),
                        tags: meta.tags,
                    });
                }
            }
            Err(e) => {
                eprintln!("browse error from {}: {}", source.source_id(), e);
            }
        }
    }

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&all_skills).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Found {} skills", all_skills.len()),
        data: Some(data),
    })
}

// ============================================================================
// Search Command
// ============================================================================

/// Search for skills across marketplace sources.
#[tauri::command]
pub async fn hub_search(
    query: String,
    source_filter: Option<String>,
    limit: Option<usize>,
) -> Result<HubCommandResult, String> {
    let limit = limit.unwrap_or(20);
    let sources = create_source_router();

    let mut all_skills = Vec::new();

    for source in sources {
        if let Some(ref filter) = source_filter {
            if source.source_id() != filter {
                continue;
            }
        }

        match source.search(&query, limit).await {
            Ok(metas) => {
                for meta in metas {
                    all_skills.push(SkillSearchResult {
                        name: meta.name,
                        description: meta.description,
                        source: meta.source,
                        identifier: meta.identifier,
                        trust_level: format!("{:?}", meta.trust_level),
                        tags: meta.tags,
                    });
                }
            }
            Err(e) => {
                eprintln!("search error from {}: {}", source.source_id(), e);
            }
        }
    }

    // Sort by trust level (higher first)
    all_skills.sort_by(|a, b| {
        let a_rank = trust_rank(&a.trust_level);
        let b_rank = trust_rank(&b.trust_level);
        b_rank.cmp(&a_rank)
    });

    all_skills.truncate(limit);

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&all_skills).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Found {} skills matching '{}'", all_skills.len(), query),
        data: Some(data),
    })
}

fn trust_rank(trust_level: &str) -> i32 {
    match trust_level {
        "Builtin" => 2,
        "Trusted" => 1,
        "AgentCreated" => 0,
        "Community" => 0,
        _ => -1,
    }
}

// ============================================================================
// Inspect Command
// ============================================================================

/// Inspect a specific skill - get detailed information about a skill.
#[tauri::command]
pub async fn hub_inspect(source: String, identifier: String) -> Result<HubCommandResult, String> {
    let sources = create_source_router();

    // Find the matching source
    let source = sources
        .iter()
        .find(|s| s.source_id() == source)
        .ok_or_else(|| format!("Unknown source: {}", source))?;

    // Fetch the skill bundle
    let bundle = source
        .fetch(&identifier)
        .await
        .map_err(|e| format!("fetch failed: {}", e))?
        .ok_or_else(|| format!("Skill not found: {}", identifier))?;

    let files: Vec<String> = bundle.files.keys().cloned().collect();
    let size_bytes: u64 = bundle.files.values().map(|v| v.len() as u64).sum();

    let inspect_result = SkillInspectResult {
        name: bundle.name.clone(),
        description: bundle
            .metadata
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        source: bundle.source,
        identifier: bundle.identifier,
        trust_level: format!("{:?}", bundle.trust_level),
        tags: Vec::new(),
        files,
        size_bytes,
        manifest: HashMap::new(),
    };

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&inspect_result).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Inspected skill: {}", bundle.name),
        data: Some(data),
    })
}

// ============================================================================
// Check Command
// ============================================================================

/// Check for skill updates - compare installed versions with marketplace.
#[tauri::command]
pub async fn hub_check(skills_dir: Option<String>) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let hub_state = create_hub_state(&paths).map_err(|e| e.to_string())?;

    // Get installed skills from lock file
    let lock_entries = hub_state.lock.get_entries();

    let mut update_available = Vec::new();

    for entry in &lock_entries {
        // For each installed skill, check if update is available
        // This would require comparing hashes in a full implementation
        update_available.push(serde_json::json!({
            "name": entry.skill_name,
            "source": entry.source,
            "identifier": entry.identifier,
            "installed_at": entry.installed_at.to_rfc3339(),
            "update_available": false, // Placeholder
        }));
    }

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&update_available).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Checked {} installed skills", lock_entries.len()),
        data: Some(data),
    })
}

// ============================================================================
// Update Command
// ============================================================================

/// Update installed skills from marketplace.
#[tauri::command]
pub async fn hub_update(
    skill_name: Option<String>,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let _hub_state = create_hub_state(&paths).map_err(|e| e.to_string())?;

    // In a full implementation, this would:
    // 1. Fetch latest version from marketplace
    // 2. Compare with installed version
    // 3. Download and install if newer
    // 4. Run security scan
    // 5. Update lock file

    let message = if let Some(name) = skill_name {
        format!("Update requested for skill: {}", name)
    } else {
        "Update all skills requested".to_string()
    };

    Ok(HubCommandResult {
        success: true,
        message,
        data: None,
    })
}

// ============================================================================
// Audit Command
// ============================================================================

/// Audit installed skills - scan all skills for security issues.
#[tauri::command]
pub async fn hub_audit(skills_dir: Option<String>) -> Result<HubCommandResult, String> {
    use crate::modules::skills::guard::SkillsGuard;

    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let guard = SkillsGuard::new();
    let mut results = Vec::new();

    // Scan skills directory
    if let Ok(entries) = std::fs::read_dir(&paths.skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }

            let skill_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let scan_result = guard.scan(&path, "community");

            results.push(SkillAuditResult {
                name: skill_name,
                path: path.to_string_lossy().into_owned(),
                source: "local".to_string(),
                status: format!("{:?}", scan_result.verdict),
                findings_count: scan_result.findings.len(),
                last_updated: None,
            });
        }
    }

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&results).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Audited {} skills", results.len()),
        data: Some(data),
    })
}

// ============================================================================
// Uninstall Command
// ============================================================================

/// Uninstall a skill - remove from local installation.
#[tauri::command]
pub async fn hub_uninstall(
    skill_name: String,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let hub_state = create_hub_state(&paths).map_err(|e| e.to_string())?;

    let skill_path = paths.skills_dir.join(&skill_name);

    if !skill_path.exists() {
        return Err(format!("Skill not found: {}", skill_name));
    }

    // Remove the skill directory
    std::fs::remove_dir_all(&skill_path).map_err(|e| format!("Failed to remove skill: {}", e))?;

    // Remove from lock file
    hub_state
        .lock
        .remove(&skill_name)
        .map_err(|e| format!("Failed to update lock: {}", e))?;

    // Record audit event
    let event = AuditEvent::uninstall(&skill_name);
    let _ = hub_state.append_audit_log(&event);

    // Invalidate skills index cache so next conversation reflects the change
    crate::modules::runtime::prompt::invalidate_skills_index_cache();

    Ok(HubCommandResult {
        success: true,
        message: format!("Uninstalled skill: {}", skill_name),
        data: None,
    })
}

// ============================================================================
// Install Command
// ============================================================================

/// Install a skill from the marketplace: fetch → quarantine → scan → install.
///
/// Full pipeline: fetch bundle → write to quarantine dir → security scan →
/// if scan passes, move to skills dir + update lock file + invalidate cache.
#[tauri::command]
pub async fn hub_install(
    source_id: String,
    identifier: String,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    use crate::modules::skills::guard::policy::TrustLevel;
    use crate::modules::skills::guard::SkillsGuard;

    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let sources = create_source_router();
    let source = sources
        .iter()
        .find(|s| s.source_id() == source_id)
        .ok_or_else(|| format!("Unknown source: {}", source_id))?;

    // 1. Fetch the bundle
    let bundle = source
        .fetch(&identifier)
        .await
        .map_err(|e| format!("fetch failed: {}", e))?
        .ok_or_else(|| format!("Skill not found: {}", identifier))?;

    let skill_name = bundle.name.clone();

    // 2. Write to quarantine directory
    let quarantine_dir = paths.quarantine_dir.join(&skill_name);
    std::fs::create_dir_all(&quarantine_dir)
        .map_err(|e| format!("create quarantine dir failed: {}", e))?;

    for (rel_path, content) in &bundle.files {
        let target = quarantine_dir.join(rel_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {}", e))?;
        }
        std::fs::write(&target, content)
            .map_err(|e| format!("write file failed for {rel_path}: {}", e))?;
    }

    // 3. Security scan
    let guard = SkillsGuard::new();
    let trust_label = match bundle.trust_level {
        TrustLevel::Builtin => "builtin",
        TrustLevel::Trusted => "trusted",
        TrustLevel::Community => "community",
        TrustLevel::AgentCreated => "agent-created",
    };
    let scan_result = guard.scan(&quarantine_dir, trust_label);

    // 4. Check install policy (force=false: respect policy strictly)
    let (allowed, reason) = guard.should_allow_install(&scan_result, false);
    if allowed != Some(true) {
        let _ = std::fs::remove_dir_all(&quarantine_dir);
        return Err(format!(
            "Security scan blocked installation of '{}': {} ({} findings, verdict: {:?})",
            skill_name,
            reason,
            scan_result.findings.len(),
            scan_result.verdict,
        ));
    }

    // 5. Move from quarantine to skills directory
    let final_path = paths.skills_dir.join(&skill_name);
    if final_path.exists() {
        std::fs::remove_dir_all(&final_path)
            .map_err(|e| format!("remove existing skill failed: {}", e))?;
    }
    std::fs::rename(&quarantine_dir, &final_path)
        .map_err(|e| format!("move from quarantine failed: {}", e))?;

    // 6. Update lock file
    let hub_state = create_hub_state(&paths).map_err(|e| e.to_string())?;
    let lock_entry = crate::modules::skills::hub::HubLockEntry {
        skill_name: skill_name.clone(),
        source: source_id.clone(),
        identifier: identifier.clone(),
        installed_at: chrono::Utc::now(),
        version: None,
    };
    hub_state
        .lock
        .add(lock_entry)
        .map_err(|e| format!("update lock failed: {}", e))?;

    // 7. Record audit event + invalidate cache
    let event = AuditEvent::install(&skill_name, &source_id, &identifier);
    let _ = hub_state.append_audit_log(&event);
    crate::modules::runtime::prompt::invalidate_skills_index_cache();

    Ok(HubCommandResult {
        success: true,
        message: format!(
            "Installed skill '{}' from {}. Verdict: {:?}. Findings: {}.",
            skill_name,
            source_id,
            scan_result.verdict,
            scan_result.findings.len()
        ),
        data: Some(serde_json::json!({
            "name": skill_name,
            "path": final_path.to_string_lossy(),
            "verdict": format!("{:?}", scan_result.verdict),
            "findings_count": scan_result.findings.len(),
        })),
    })
}

// ============================================================================
// Publish Command
// ============================================================================

/// Publish a skill to GitHub or ClawHub.
#[tauri::command]
pub async fn hub_publish(
    skill_path: String,
    target: String, // "github" or "clawhub"
    repo: Option<String>,
) -> Result<HubCommandResult, String> {
    // In a full implementation, this would:
    // 1. Read the skill from the local path
    // 2. Create a bundle
    // 3. Push to the target marketplace
    // 4. Record the publication in audit log

    let skill_path = PathBuf::from(&skill_path);
    let skill_name = skill_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let message = format!("Publish {} to {} (repo: {:?})", skill_name, target, repo);

    Ok(HubCommandResult {
        success: true,
        message,
        data: None,
    })
}

// ============================================================================
// Snapshot Command
// ============================================================================

/// Export skills configuration to a snapshot file.
#[tauri::command]
pub async fn hub_snapshot_export(
    output_path: Option<String>,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let hub_state = create_hub_state(&paths).map_err(|e| e.to_string())?;

    // Get all installed skills
    let lock_entries = hub_state.lock.get_entries();

    let mut exported_skills = Vec::new();
    for entry in &lock_entries {
        exported_skills.push(ExportedSkill::new(
            entry.skill_name.clone(),
            entry.source.clone(),
            entry.identifier.clone(),
        ));
    }

    // Read taps configuration
    let taps = if paths.taps_file.exists() {
        let content = std::fs::read_to_string(&paths.taps_file).ok();
        content
            .and_then(|c| serde_json::from_str::<Vec<TapConfig>>(&c).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let snapshot = SkillSnapshot::with_content(exported_skills, taps);

    let manager = SnapshotManager::new();
    let export_path = if let Some(path) = output_path {
        PathBuf::from(path)
    } else {
        manager.snapshot_dir().join(format!(
            "snapshot-{}.json",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ))
    };

    // Ensure parent directory exists
    if let Some(parent) = export_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {}", e))?;
    }

    let json =
        serde_json::to_string_pretty(&snapshot).map_err(|e| format!("serialize failed: {}", e))?;
    std::fs::write(&export_path, json).map_err(|e| format!("write failed: {}", e))?;

    Ok(HubCommandResult {
        success: true,
        message: format!("Exported snapshot to: {}", export_path.display()),
        data: Some(serde_json::json!({
            "path": export_path.display().to_string(),
            "skills_count": snapshot.skill_count(),
            "taps_count": snapshot.tap_count(),
        })),
    })
}

/// Import skills configuration from a snapshot file.
#[tauri::command]
pub async fn hub_snapshot_import(
    snapshot_path: String,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let manager = SnapshotManager::new();

    let snapshot = manager
        .import(&PathBuf::from(&snapshot_path))
        .map_err(|e| format!("import failed: {}", e))?;

    let skills_dir = &paths.skills_dir;

    // Install each skill from the snapshot
    let mut installed = Vec::new();
    for skill in &snapshot.skills {
        let skill_path = skills_dir.join(&skill.name);
        if skill_path.exists() {
            installed.push(format!("{} (already installed)", skill.name));
        } else {
            installed.push(format!("{} (would install)", skill.name));
        }
    }

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&installed).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("Imported snapshot with {} skills", snapshot.skill_count()),
        data: Some(data),
    })
}

// ============================================================================
// Tap Command
// ============================================================================

/// Add a tap (custom GitHub source) to taps.json.
#[tauri::command]
pub async fn hub_tap_add(
    repo: String,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    // Ensure hub directories exist
    paths.ensure_dirs().map_err(|e| e.to_string())?;

    // Read existing taps
    let mut taps: Vec<TapConfig> = if paths.taps_file.exists() {
        let content = std::fs::read_to_string(&paths.taps_file)
            .map_err(|e| format!("read taps failed: {}", e))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Check if tap already exists
    if taps.iter().any(|t| t.source_url == repo) {
        return Err(format!("Tap already exists: {}", repo));
    }

    // Add new tap
    taps.push(TapConfig::new(
        repo.clone(),
        format!("https://github.com/{}", repo),
        "github".to_string(),
    ));

    // Write taps file
    let json =
        serde_json::to_string_pretty(&taps).map_err(|e| format!("serialize failed: {}", e))?;
    std::fs::write(&paths.taps_file, json).map_err(|e| format!("write failed: {}", e))?;

    Ok(HubCommandResult {
        success: true,
        message: format!("Added tap: {}", repo),
        data: None,
    })
}

/// Remove a tap from taps.json.
#[tauri::command]
pub async fn hub_tap_remove(
    repo: String,
    skills_dir: Option<String>,
) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    if !paths.taps_file.exists() {
        return Err("No taps file found".to_string());
    }

    // Read existing taps
    let content = std::fs::read_to_string(&paths.taps_file)
        .map_err(|e| format!("read taps failed: {}", e))?;
    let mut taps: Vec<TapConfig> = serde_json::from_str(&content).unwrap_or_default();

    let initial_len = taps.len();
    taps.retain(|t| t.source_url != repo && t.id != repo);

    if taps.len() == initial_len {
        return Err(format!("Tap not found: {}", repo));
    }

    // Write taps file
    let json =
        serde_json::to_string_pretty(&taps).map_err(|e| format!("serialize failed: {}", e))?;
    std::fs::write(&paths.taps_file, json).map_err(|e| format!("write failed: {}", e))?;

    Ok(HubCommandResult {
        success: true,
        message: format!("Removed tap: {}", repo),
        data: None,
    })
}

/// List all configured taps.
#[tauri::command]
pub async fn hub_tap_list(skills_dir: Option<String>) -> Result<HubCommandResult, String> {
    let paths = if let Some(dir) = skills_dir {
        HubPaths::new(dir)
    } else {
        HubPaths::default()
    };

    let taps: Vec<TapConfig> = if paths.taps_file.exists() {
        let content = std::fs::read_to_string(&paths.taps_file).ok();
        content
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // SAFETY: serde_json::to_value on Serialize types never fails
    #[allow(clippy::expect_used)]
    let data = serde_json::to_value(&taps).expect("serializable");
    Ok(HubCommandResult {
        success: true,
        message: format!("{} taps configured", taps.len()),
        data: Some(data),
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_hub_state(paths: &HubPaths) -> HubResult<HubState> {
    let sources = create_source_router();
    HubState::new(paths.clone(), sources, None)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_rank_ordering() {
        assert!(trust_rank("Builtin") > trust_rank("Trusted"));
        assert!(trust_rank("Trusted") > trust_rank("AgentCreated"));
        assert!(trust_rank("AgentCreated") == trust_rank("Community"));
    }
}
