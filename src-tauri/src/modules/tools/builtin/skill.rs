//! Skill tool - load and execute a custom skill by name.
//!
//! Provides a Skill ToolHandler that discovers and reads SKILL.md files
//! from project-level and user-level skill directories.
//! See docs/bs_gap/08-critical-fix-priority.md §F6.

use std::sync::Arc;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

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

            let skill_path = resolve_skill_path(skill, &workdir).map_err(ToolError::Handler)?;

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
    let roots = discover_skill_roots(workdir);
    for root in roots {
        let candidate = root.join(skill).join("SKILL.md");
        if candidate.is_file() {
            return Ok(candidate);
        }
        let legacy = root.join(format!("{skill}.md"));
        if legacy.is_file() {
            return Ok(legacy);
        }
    }
    Err(format!("Skill '{skill}' not found in any search path"))
}

/// Discovers all valid skill root directories for the given workdir.
///
/// Searches in order:
/// 1. Project-level: `.if2ai/skills`, `.claw/skills`, `.codex/skills`,
///    `.if2ai/commands`, `.claw/commands`, `.codex/commands`
/// 2. User-level: `$IF2AI_HOME/skills`, `$HOME/.if2ai/skills`
pub fn discover_skill_roots(workdir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();

    // Project-level directories
    for dir in &[
        ".if2ai/skills",
        ".claw/skills",
        ".codex/skills",
        ".if2ai/commands",
        ".claw/commands",
        ".codex/commands",
    ] {
        let p = workdir.join(dir);
        if p.is_dir() {
            roots.push(p);
        }
    }

    // User-level: IF2AI_HOME
    if let Ok(home) = std::env::var("IF2AI_HOME") {
        let p = std::path::PathBuf::from(&home).join("skills");
        if p.is_dir() {
            roots.push(p);
        }
    }

    // User-level: HOME/.if2ai/skills
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(&home).join(".if2ai/skills");
        if p.is_dir() {
            roots.push(p);
        }
    }

    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            assert!(!root.starts_with("/tmp/nonexistent-dir-5c4/"),
                "project-level root should not exist: {root:?}");
        }
    }

    #[test]
    fn resolve_skill_path_returns_error_for_missing_skill() {
        let result = resolve_skill_path("nonexistent_skill", PathBuf::from(".").as_path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
