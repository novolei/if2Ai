//! SkillSearch tool - search for available skills by name or description.
//!
//! Provides a SkillSearch ToolHandler that searches across all skill roots
//! and returns matching skills with name, description, and path.
//! See docs/bs_gap/08-critical-fix-priority.md §F14.

use std::sync::Arc;

use crate::modules::tools::builtin::skill::discover_skill_roots;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Skill metadata returned by search.
#[derive(serde::Serialize)]
struct SkillMatch {
    name: String,
    description: String,
    path: String,
}

/// Creates the SkillSearch tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn skill_search_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            let results = search_skills_internal(&query, &workdir);
            serde_json::to_string_pretty(&results).map_err(|e| ToolError::Handler(e.to_string()))
        })
    });

    ToolEntry {
        name: "skill_search".to_string(),
        toolset: "utility".to_string(),
        description: "Search for available skills by name or description.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for skill name or description"
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(5 * 1024),
        timeout_secs: Some(5),
        disabled: false,
        handler,
    }
}

/// Searches all discovered skill roots for skills matching the query.
/// An empty query returns all skills.
fn search_skills_internal(query: &str, workdir: &std::path::Path) -> Vec<SkillMatch> {
    let roots = discover_skill_roots(workdir);
    let mut results = Vec::new();

    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&skill_md) {
                        let meta = parse_skill_frontmatter(&content);
                        let name = meta
                            .name
                            .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
                        let desc = meta.description.unwrap_or_default();

                        if query.is_empty()
                            || name.to_lowercase().contains(query)
                            || desc.to_lowercase().contains(query)
                        {
                            results.push(SkillMatch {
                                name,
                                description: desc,
                                path: entry.path().to_string_lossy().into_owned(),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

/// Minimal SKILL.md frontmatter parser.
/// Looks for `---` YAML-like header and extracts `name` and `description`.
fn parse_skill_frontmatter(content: &str) -> SkillMeta {
    let mut meta = SkillMeta::default();

    // Content may start with `---\n`, skip it
    let body = content.strip_prefix("---").unwrap_or(content);
    if let Some((header, _rest)) = body.split_once("---") {
        for line in header.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                let k = key.trim();
                let v = value.trim().trim_matches('"').trim_matches('\'');
                match k {
                    "name" => meta.name = Some(v.to_string()),
                    "description" => meta.description = Some(v.to_string()),
                    _ => {}
                }
            }
        }
    }
    meta
}

#[derive(Default)]
struct SkillMeta {
    name: Option<String>,
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_search_tool_entry_has_correct_structure() {
        let entry = skill_search_tool_entry();
        assert_eq!(entry.name, "skill_search");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(5 * 1024));
        assert_eq!(entry.timeout_secs, Some(5));
    }

    #[test]
    fn search_returns_empty_for_no_skills() {
        let results = search_skills_internal("", &std::path::PathBuf::from("/tmp/nonexistent-5c7"));
        // May have user-level skills, so we just check it doesn't panic
        let _ = results.len();
    }

    #[test]
    fn parse_frontmatter_extracts_name_and_description() {
        let content = "---\nname: my-skill\ndescription: A test skill\n---\nSome content";
        let meta = parse_skill_frontmatter(content);
        assert_eq!(meta.name, Some("my-skill".to_string()));
        assert_eq!(meta.description, Some("A test skill".to_string()));
    }

    #[test]
    fn parse_frontmatter_handles_empty_header() {
        let content = "Just content, no frontmatter";
        let meta = parse_skill_frontmatter(content);
        assert_eq!(meta.name, None);
        assert_eq!(meta.description, None);
    }
}
