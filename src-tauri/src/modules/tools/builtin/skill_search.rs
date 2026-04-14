//! SkillSearch tool - search for available skills by name or description.
//!
//! Provides a SkillSearch ToolHandler that searches across all skill roots
//! and returns matching skills with name, description, and path.
//! See docs/bs_gap/08-critical-fix-priority.md §F14.

use std::sync::Arc;

use crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Skill metadata returned by search.
#[derive(serde::Serialize)]
struct SkillMatch {
    name: String,
    description: String,
    path: String,
    source: String,
    precedence: usize,
    shadowed_by: Option<String>,
}

#[derive(Clone)]
struct SkillCandidate {
    key: String,
    name: String,
    description: String,
    path: String,
    source: String,
    precedence: usize,
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
    let roots = discover_skill_roots_with_metadata(workdir);
    let mut candidates = Vec::new();

    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root.path) {
            let mut entry_list: Vec<std::fs::DirEntry> = entries.flatten().collect();
            entry_list.sort_by_key(|item| item.path());
            for entry in entry_list {
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
                            candidates.push(SkillCandidate {
                                key: name.to_lowercase(),
                                name,
                                description: desc,
                                path: entry.path().to_string_lossy().into_owned(),
                                source: root.source.as_label().to_string(),
                                precedence: root.precedence,
                            });
                        }
                    }
                }
            }
        }
    }

    candidates.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then(left.precedence.cmp(&right.precedence))
            .then(left.path.cmp(&right.path))
    });

    let mut winners: std::collections::BTreeMap<String, SkillCandidate> =
        std::collections::BTreeMap::new();
    for candidate in &candidates {
        let update = match winners.get(&candidate.key) {
            Some(existing) => {
                (candidate.precedence < existing.precedence)
                    || (candidate.precedence == existing.precedence
                        && candidate.path < existing.path)
            }
            None => true,
        };
        if update {
            winners.insert(candidate.key.clone(), candidate.clone());
        }
    }

    let mut results = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let shadowed_by = winners.get(&candidate.key).and_then(|winner| {
            if winner.path == candidate.path {
                None
            } else {
                Some(format!(
                    "{} ({}) at {} precedence={} path_tiebreak=lexicographic",
                    winner.name, winner.source, winner.path, winner.precedence
                ))
            }
        });
        results.push(SkillMatch {
            name: candidate.name,
            description: candidate.description,
            path: candidate.path,
            source: candidate.source,
            precedence: candidate.precedence,
            shadowed_by,
        });
    }

    results.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.precedence.cmp(&right.precedence))
            .then(left.path.cmp(&right.path))
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn write_skill(root: &std::path::Path, relative_dir: &str, name: &str, description: &str) {
        let skill_dir = root.join(relative_dir).join(name);
        fs::create_dir_all(&skill_dir).expect("create skill directory");
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n# {name}\n");
        fs::write(skill_dir.join("SKILL.md"), content).expect("write skill file");
    }

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

    #[test]
    fn search_shadow_is_deterministic_and_explainable() {
        let root = std::env::temp_dir().join(format!("if2ai-skill-search-{}", Uuid::new_v4()));
        write_skill(&root, ".if2ai/skills", "alpha", "workspace winner");
        write_skill(
            &root,
            "src-tauri/resources/bundled-skills",
            "alpha",
            "builtin loser",
        );

        let results = search_skills_internal("alpha", &root);
        let mut workspace_item = None;
        let mut builtin_item = None;
        for item in results {
            if item.source == "workspace" {
                workspace_item = Some(item);
            } else if item.source == "builtin" {
                builtin_item = Some(item);
            }
        }

        let workspace = workspace_item.expect("workspace item should exist");
        let builtin = builtin_item.expect("builtin item should exist");
        assert!(
            workspace.shadowed_by.is_none(),
            "winner should not be shadowed"
        );
        let explain = builtin
            .shadowed_by
            .expect("loser should include shadow explanation");
        assert!(
            explain.contains("workspace"),
            "explanation should include winner source"
        );
        assert!(
            explain.contains("precedence="),
            "explanation should include precedence"
        );

        let _ = fs::remove_dir_all(root);
    }
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
