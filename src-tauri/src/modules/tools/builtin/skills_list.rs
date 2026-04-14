//! skills_list tool — list available skills with name, description, source, and status.
//!
//! Reuses `collect_skill_index_entries` from `runtime::prompt` for shadow-resolved
//! discovery (no duplicate scan). Only adds lightweight per-skill lookups for
//! `review_status` (read skill.json) and `readiness` (check env vars from frontmatter).

use std::sync::Arc;

use crate::modules::runtime::prompt::collect_skill_index_entries;
use crate::modules::tools::builtin::skill::fallback_review_status_without_manifest;
use crate::modules::tools::builtin::skill::SkillSource;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// One skill entry returned by `skills_list`.
#[derive(serde::Serialize)]
struct SkillListItem {
    name: String,
    description: String,
    source: String,
    /// Readiness: "available" | "setup_needed".
    readiness: String,
    /// Review lifecycle status from `skill.json`, or "active" for trusted builtins.
    review_status: String,
    path: String,
}

/// Creates the `skills_list` ToolEntry.
#[must_use]
pub fn skills_list_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let category_filter = args
                .get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());

            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            // Use the same shadow-resolved index as the system prompt — no extra scan.
            let entries = collect_skill_index_entries(&workdir, None);

            let items: Vec<SkillListItem> = entries
                .into_iter()
                .filter_map(|entry| {
                    // Category filter against name, description, or inferred tags
                    if let Some(cat) = &category_filter {
                        let haystack = format!(
                            "{} {}",
                            entry.name.to_lowercase(),
                            entry.description.to_lowercase()
                        );
                        if !haystack.contains(cat.as_str()) {
                            return None;
                        }
                    }

                    let skill_dir = &entry.skill_dir;
                    let skill_md = skill_dir.join("SKILL.md");

                    // Readiness: check required_environment_variables in frontmatter
                    let readiness = if let Ok(content) = std::fs::read_to_string(&skill_md) {
                        check_readiness(&content)
                    } else {
                        "available".to_string()
                    };

                    // Review status: read skill.json, fall back to source heuristic
                    let source_enum = match entry.source.as_str() {
                        "workspace" => SkillSource::Workspace,
                        "user" => SkillSource::User,
                        "remote-quarantine" => SkillSource::RemoteQuarantine,
                        _ => SkillSource::Builtin,
                    };
                    let review_status = resolve_review_status(skill_dir, source_enum);

                    Some(SkillListItem {
                        name: entry.name,
                        description: entry.description,
                        source: entry.source,
                        readiness,
                        review_status,
                        path: skill_dir.to_string_lossy().into_owned(),
                    })
                })
                .collect();

            serde_json::to_string_pretty(&items).map_err(|e| ToolError::Handler(e.to_string()))
        })
    });

    ToolEntry {
        name: "skills_list".to_string(),
        toolset: "utility".to_string(),
        description: "List all available skills with name, description, source, and readiness \
                      status. Use this to discover which skills are installed before calling \
                      skill()."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Optional category filter (e.g. \"qa\", \"git\", \"coding\")"
                }
            }
        }),
        max_result_size: Some(64 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

/// Determine the review lifecycle status for a skill directory.
fn resolve_review_status(skill_dir: &std::path::Path, source: SkillSource) -> String {
    let manifest_path = skill_dir.join("skill.json");
    if manifest_path.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&manifest_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(status) = json
                    .get("review")
                    .and_then(|r| r.get("status"))
                    .and_then(|s| s.as_str())
                {
                    return status.to_string();
                }
            }
        }
    }
    match fallback_review_status_without_manifest(source, skill_dir) {
        Some(s) => s.to_string(),
        None => "draft".to_string(),
    }
}

/// Check whether required environment variables declared in frontmatter are present.
fn check_readiness(skill_content: &str) -> String {
    let body = skill_content.strip_prefix("---").unwrap_or(skill_content);
    let Some((header, _)) = body.split_once("---") else {
        return "available".to_string();
    };
    for line in header.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == "required_environment_variables" {
                let inner = v.trim().trim_start_matches('[').trim_end_matches(']');
                let has_missing = inner
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .any(|var| std::env::var(var).is_err());
                if has_missing {
                    return "setup_needed".to_string();
                }
            }
        }
    }
    "available".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_list_tool_entry_has_correct_structure() {
        let entry = skills_list_tool_entry();
        assert_eq!(entry.name, "skills_list");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }

    #[test]
    fn check_readiness_returns_available_for_no_env_vars() {
        let content = "---\nname: test\ndescription: no envs\n---\n# Test";
        assert_eq!(check_readiness(content), "available");
    }

    #[test]
    fn check_readiness_returns_setup_needed_for_missing_env_var() {
        let content =
            "---\nname: test\nrequired_environment_variables: [NONEXISTENT_VAR_12345]\n---\n";
        assert_eq!(check_readiness(content), "setup_needed");
    }
}
