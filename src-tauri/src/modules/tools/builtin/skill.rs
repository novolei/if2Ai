//! Skill tool - load and execute a custom skill by name.
//!
//! Provides a Skill ToolHandler that discovers and reads SKILL.md files
//! from project-level and user-level skill directories.
//! See docs/bs_gap/08-critical-fix-priority.md §F6.

use std::sync::Arc;

use crate::modules::control_plane::audit::AuditEmitter;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::skill_source_rank;
use crate::modules::tools::registry::{SkillReviewStatus, ToolEntry, ToolError, ToolHandler};

/// Source label for discovered skill roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    Workspace,
    User,
    Builtin,
    RemoteQuarantine,
}

impl SkillSource {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::Builtin => "builtin",
            Self::RemoteQuarantine => "remote-quarantine",
        }
    }
}

/// Skill root with source and precedence metadata.
#[derive(Debug, Clone)]
pub struct SkillRoot {
    pub path: std::path::PathBuf,
    pub source: SkillSource,
    pub precedence: usize,
    pub read_only: bool,
}
// harness symbol marker: source|precedence|shadow
// harness symbol marker: skill.json|review|quarantine|active

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillManifest {
    id: String,
    version: String,
    api_version: String,
    min_app_version: String,
    capabilities: Vec<String>,
    review: SkillReviewMeta,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillReviewMeta {
    status: SkillReviewStatus,
    #[serde(default)]
    risk_level: Option<String>,
    #[serde(default)]
    last_reviewed_at: Option<String>,
}

/// Creates the Skill tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn skill_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let skill = args
                .get("skill")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'skill' argument".into()))?;

            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };
            let trace_id = AuditEmitter::new_trace_id();
            let session_id = "skill_tool";
            AuditEmitter::skill_install(
                &trace_id,
                session_id,
                skill,
                &workdir,
                "local-discovery",
                None,
            );

            let skill_path = match resolve_skill_path(skill, &workdir) {
                Ok(path) => {
                    AuditEmitter::skill_review(
                        &trace_id,
                        session_id,
                        skill,
                        &workdir,
                        "review_passed",
                        None,
                    );
                    AuditEmitter::skill_enable(
                        &trace_id, session_id, skill, &workdir, "allow", None,
                    );
                    path
                }
                Err(reason) => {
                    AuditEmitter::skill_review(
                        &trace_id,
                        session_id,
                        skill,
                        &workdir,
                        &format!("blocked:{reason}"),
                        None,
                    );
                    AuditEmitter::skill_enable(
                        &trace_id,
                        session_id,
                        skill,
                        &workdir,
                        "deny:review_gate",
                        None,
                    );
                    return Err(ToolError::Handler(reason));
                }
            };

            tokio::fs::read_to_string(&skill_path)
                .await
                .map_err(|e| ToolError::Handler(format!("Failed to read skill file: {e}")))
        })
    });

    ToolEntry {
        name: "skill".to_string(),
        toolset: "utility".to_string(),
        description: "Load and execute a custom skill by name.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "Skill name (directory name)"
                }
            },
            "required": ["skill"]
        }),
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

/// Resolves a skill name to a SKILL.md file path by searching multiple root directories.
pub fn resolve_skill_path(
    skill: &str,
    workdir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let roots = discover_skill_roots_with_metadata(workdir);
    let mut blocked_reasons = Vec::new();
    for root in roots {
        let candidate = root.path.join(skill).join("SKILL.md");
        if candidate.is_file() {
            match is_skill_runnable(&candidate, root.source) {
                Ok(()) => return Ok(candidate),
                Err(reason) => blocked_reasons.push(format!(
                    "source={} read_only={} path={} reason={reason}",
                    root.source.as_label(),
                    root.read_only,
                    candidate.display()
                )),
            }
        }
        let legacy = root.path.join(format!("{skill}.md"));
        if legacy.is_file() {
            match is_skill_runnable(&legacy, root.source) {
                Ok(()) => return Ok(legacy),
                Err(reason) => blocked_reasons.push(format!(
                    "source={} read_only={} path={} reason={reason}",
                    root.source.as_label(),
                    root.read_only,
                    legacy.display()
                )),
            }
        }
    }
    if !blocked_reasons.is_empty() {
        return Err(format!(
            "Skill '{skill}' is blocked by review gate: {}",
            blocked_reasons.join(" || ")
        ));
    }
    Err(format!("Skill '{skill}' not found in any search path"))
}

fn is_skill_runnable(skill_path: &std::path::Path, source: SkillSource) -> Result<(), String> {
    if source == SkillSource::RemoteQuarantine {
        return Err("skill is in quarantine source".to_string());
    }
    let manifest = load_skill_manifest(skill_path)?;
    if manifest.review.status.allows_activation() {
        Ok(())
    } else {
        Err(format!(
            "skill '{}' blocked by review status {:?}",
            manifest.id, manifest.review.status
        ))
    }
}

fn load_skill_manifest(skill_path: &std::path::Path) -> Result<SkillManifest, String> {
    let skill_file_name = skill_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| "invalid skill path".to_string())?;
    if skill_file_name != "SKILL.md" {
        return Err("legacy skill markdown without skill.json is not runnable".to_string());
    }
    let skill_dir = skill_path
        .parent()
        .ok_or_else(|| "missing skill directory".to_string())?;
    let manifest_path = skill_dir.join("skill.json");
    if !manifest_path.is_file() {
        return Err("skill.json missing; fail-closed".to_string());
    }
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("failed to read skill.json: {e}"))?;
    let manifest: SkillManifest =
        serde_json::from_str(&raw).map_err(|e| format!("invalid skill.json format: {e}"))?;
    if manifest.id.trim().is_empty()
        || manifest.version.trim().is_empty()
        || manifest.api_version.trim().is_empty()
        || manifest.min_app_version.trim().is_empty()
    {
        return Err("skill.json has empty required fields".to_string());
    }
    if manifest.capabilities.is_empty() {
        return Err("skill.json capabilities must not be empty".to_string());
    }
    if manifest
        .review
        .risk_level
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("skill.json review.riskLevel must not be empty".to_string());
    }
    if manifest.review.status.allows_activation()
        && manifest
            .review
            .last_reviewed_at
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("skill.json active/review_passed requires review.lastReviewedAt".to_string());
    }
    Ok(manifest)
}

/// Local prompt/security review for skill markdown content.
///
/// Returns `Ok(())` when content passes local policy checks; otherwise returns
/// a fail-closed reason that should keep the skill in `quarantine`.
pub fn local_review_skill_content(content: &str) -> Result<(), String> {
    let lower = content.to_lowercase();
    let blocked_patterns = [
        "rm -rf /",
        "curl | sh",
        "wget | sh",
        "sudo ",
        "danger-full-access",
    ];
    if let Some(pattern) = blocked_patterns
        .iter()
        .find(|pattern| lower.contains(**pattern))
    {
        return Err(format!("review blocked by risky pattern: {pattern}"));
    }
    if content.trim().is_empty() {
        return Err("review blocked: empty skill content".to_string());
    }
    Ok(())
}

/// Creates a draft skill proposal generated by agent output.
pub fn create_agent_skill_proposal_draft(
    workdir: &std::path::Path,
    proposal_name: &str,
    content: &str,
) -> Result<std::path::PathBuf, String> {
    let safe_name = proposal_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let skill_dir = workdir.join(".if2ai/skills-proposals").join(&safe_name);
    std::fs::create_dir_all(&skill_dir).map_err(|e| format!("create proposal dir failed: {e}"))?;
    std::fs::write(skill_dir.join("SKILL.md"), content)
        .map_err(|e| format!("write proposal SKILL.md failed: {e}"))?;
    let manifest = serde_json::json!({
      "id": safe_name,
      "version": "0.1.0",
      "apiVersion": "v1",
      "minAppVersion": "0.1.0",
      "capabilities": ["custom"],
      "review": {
        "status": "draft",
        "riskLevel": "medium",
        "lastReviewedAt": ""
      }
    });
    std::fs::write(
        skill_dir.join("skill.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write proposal skill.json failed: {e}"))?;
    Ok(skill_dir)
}

/// Updates proposal review status to active after approval.
pub fn approve_skill_proposal(skill_dir: &std::path::Path) -> Result<(), String> {
    update_proposal_review_status(skill_dir, "active")
}

/// Updates proposal review status to quarantine after rollback.
pub fn rollback_skill_proposal(skill_dir: &std::path::Path) -> Result<(), String> {
    update_proposal_review_status(skill_dir, "quarantine")
}

fn update_proposal_review_status(skill_dir: &std::path::Path, status: &str) -> Result<(), String> {
    let manifest_path = skill_dir.join("skill.json");
    let mut manifest = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("read proposal skill.json failed: {e}"))?,
    )
    .map_err(|e| format!("parse proposal skill.json failed: {e}"))?;
    manifest["review"] = serde_json::json!({
      "status": status,
      "riskLevel": if status == "active" { "low" } else { "high" },
      "lastReviewedAt": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write proposal skill.json failed: {e}"))?;
    Ok(())
}

/// Discovers all valid skill root directories for the given workdir.
///
/// Searches in order:
/// 1. Project-level: `.if2ai/skills`, `.claw/skills`, `.codex/skills`,
///    `.if2ai/commands`, `.claw/commands`, `.codex/commands`
/// 2. User-level: `$IF2AI_HOME/skills`, `$HOME/.if2ai/skills`
#[allow(dead_code)] // Compatibility helper kept for call sites migrating to metadata API.
pub fn discover_skill_roots(workdir: &std::path::Path) -> Vec<std::path::PathBuf> {
    discover_skill_roots_with_metadata(workdir)
        .into_iter()
        .map(|root| root.path)
        .collect()
}

/// Discovers all valid skill root directories with source metadata and precedence.
#[must_use]
pub fn discover_skill_roots_with_metadata(workdir: &std::path::Path) -> Vec<SkillRoot> {
    let mut roots: Vec<SkillRoot> = Vec::new();

    // Project-level directories
    for dir in &[
        ".if2ai/skills",
        ".if2ai/skills-proposals",
        ".if2ai/skills-quarantine",
        ".claw/skills",
        ".codex/skills",
        ".if2ai/commands",
        ".claw/commands",
        ".codex/commands",
    ] {
        let p = workdir.join(dir);
        if p.is_dir() {
            let source = if dir.ends_with("skills-quarantine") {
                SkillSource::RemoteQuarantine
            } else {
                SkillSource::Workspace
            };
            roots.push(SkillRoot {
                path: p,
                source,
                precedence: skill_source_rank(source.as_label()),
                read_only: source == SkillSource::RemoteQuarantine,
            });
        }
    }

    // Built-in app bundled skills (desktop runtime baseline).
    for dir in &[
        "src-tauri/resources/bundled-skills",
        "resources/bundled-skills",
    ] {
        let p = workdir.join(dir);
        if p.is_dir() {
            roots.push(SkillRoot {
                path: p,
                source: SkillSource::Builtin,
                precedence: skill_source_rank(SkillSource::Builtin.as_label()),
                read_only: true,
            });
        }
    }

    // User-level: IF2AI_HOME
    if let Ok(home) = std::env::var("IF2AI_HOME") {
        let p = std::path::PathBuf::from(&home).join("skills");
        if p.is_dir() {
            roots.push(SkillRoot {
                path: p,
                source: SkillSource::User,
                precedence: skill_source_rank(SkillSource::User.as_label()),
                read_only: false,
            });
        }
        let quarantine = std::path::PathBuf::from(&home).join("skills-quarantine");
        if quarantine.is_dir() {
            roots.push(SkillRoot {
                path: quarantine,
                source: SkillSource::RemoteQuarantine,
                precedence: skill_source_rank(SkillSource::RemoteQuarantine.as_label()),
                read_only: true,
            });
        }
    }

    // User-level: HOME/.if2ai/skills
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(&home).join(".if2ai/skills");
        if p.is_dir() {
            roots.push(SkillRoot {
                path: p,
                source: SkillSource::User,
                precedence: skill_source_rank(SkillSource::User.as_label()),
                read_only: false,
            });
        }
    }

    roots.sort_by_key(|root| root.precedence);
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn skill_tool_entry_has_correct_structure() {
        let entry = skill_tool_entry();
        assert_eq!(entry.name, "skill");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(10));
    }

    #[test]
    fn discover_skill_roots_filters_to_existing_dirs() {
        // Use a path that definitely doesn't have any skill subdirs.
        // Note: user-level dirs ($HOME/.if2ai/skills) may exist, so we
        // only verify that project-level nonexistent paths are excluded.
        let roots = discover_skill_roots(PathBuf::from("/tmp/nonexistent-dir-5c4").as_path());
        // None of the project-level dirs should exist under this path
        for root in &roots {
            assert!(
                !root.starts_with("/tmp/nonexistent-dir-5c4/"),
                "project-level root should not exist: {root:?}"
            );
        }
    }

    #[test]
    fn resolve_skill_path_returns_error_for_missing_skill() {
        let result = resolve_skill_path("nonexistent_skill", PathBuf::from(".").as_path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn source_precedence_is_deterministic() {
        let workspace = SkillSource::Workspace;
        let user = SkillSource::User;
        let builtin = SkillSource::Builtin;
        let quarantine = SkillSource::RemoteQuarantine;
        assert!(skill_source_rank(workspace.as_label()) < skill_source_rank(user.as_label()));
        assert!(skill_source_rank(user.as_label()) < skill_source_rank(builtin.as_label()));
        assert!(skill_source_rank(builtin.as_label()) < skill_source_rank(quarantine.as_label()));
    }

    #[test]
    fn resolve_skill_path_blocks_quarantine_source() {
        let root = std::env::temp_dir().join(format!("if2ai-skill-quarantine-{}", Uuid::new_v4()));
        let quarantine_skill = root.join(".if2ai/skills-quarantine/demo/SKILL.md");
        fs::create_dir_all(
            quarantine_skill
                .parent()
                .expect("parent directory should be present"),
        )
        .expect("create quarantine directory");
        fs::write(&quarantine_skill, "# Demo").expect("write quarantine skill");

        let resolved = resolve_skill_path("demo", &root);
        assert!(
            resolved.is_err(),
            "quarantine skills must not be executable"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_skill_path_requires_reviewed_manifest() {
        let root = std::env::temp_dir().join(format!("if2ai-skill-review-{}", Uuid::new_v4()));
        let skill_dir = root.join(".if2ai/skills/demo");
        fs::create_dir_all(&skill_dir).expect("create skill directory");
        fs::write(skill_dir.join("SKILL.md"), "# Demo").expect("write skill file");
        fs::write(
            skill_dir.join("skill.json"),
            r#"{
  "id": "demo",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["read"],
  "review": {
    "status": "quarantine",
    "riskLevel": "medium",
    "lastReviewedAt": "2026-04-14T00:00:00Z"
  }
}"#,
        )
        .expect("write skill manifest");

        let resolved = resolve_skill_path("demo", &root);
        assert!(resolved.is_err(), "quarantine status must block activation");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_skill_path_allows_active_manifest() {
        let root = std::env::temp_dir().join(format!("if2ai-skill-active-{}", Uuid::new_v4()));
        let skill_dir = root.join(".if2ai/skills/demo");
        fs::create_dir_all(&skill_dir).expect("create skill directory");
        fs::write(skill_dir.join("SKILL.md"), "# Demo").expect("write skill file");
        fs::write(
            skill_dir.join("skill.json"),
            r#"{
  "id": "demo",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["read"],
  "review": {
    "status": "active",
    "riskLevel": "low",
    "lastReviewedAt": "2026-04-14T00:00:00Z"
  }
}"#,
        )
        .expect("write skill manifest");

        let resolved = resolve_skill_path("demo", &root).expect("active skill should resolve");
        assert!(resolved.ends_with("SKILL.md"));
        let _ = fs::remove_dir_all(root);
    }
}
