//! SkillSearch tool — search available skills by name or description.
//!
//! Reuses `collect_skill_index_entries` from `runtime::prompt` so discovery
//! is identical to the system-prompt index and `skills_list`.
//! Previous independent scanner used `discover_skill_roots_with_metadata`
//! but only searched workspace-relative roots, missing `~/.if2ai/skills/`.

use std::sync::Arc;

use crate::modules::runtime::prompt::collect_skill_index_entries;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// One search result returned by `skill_search`.
#[derive(serde::Serialize)]
struct SkillMatch {
    name: String,
    description: String,
    path: String,
    source: String,
}

/// Creates the `skill_search` ToolEntry.
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

            // Use the same shadow-resolved index as the system prompt and skills_list.
            // Pass None for available_toolsets so all skills are searched regardless of toolset.
            let entries = collect_skill_index_entries(&workdir, None);

            let results: Vec<SkillMatch> = entries
                .into_iter()
                .filter(|e| {
                    if query.is_empty() {
                        return true;
                    }
                    e.name.to_lowercase().contains(&query)
                        || e.description.to_lowercase().contains(&query)
                })
                .map(|e| SkillMatch {
                    name: e.name,
                    description: e.description,
                    path: e.skill_dir.to_string_lossy().into_owned(),
                    source: e.source,
                })
                .collect();

            serde_json::to_string_pretty(&results).map_err(|e| ToolError::Handler(e.to_string()))
        })
    });

    ToolEntry {
        name: "skill_search".to_string(),
        toolset: "utility".to_string(),
        description: "Search available skills by name or description. \
                      Returns matching skills; call skill(skill=\"<name>\") to load one."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for skill name or description (leave empty to list all)"
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(16 * 1024),
        timeout_secs: Some(5),
        // Disabled: redundant with skills_list + find-skills skill.
        // skill_search was causing empty results when queries were in non-English,
        // and the find-skills skill handles discovery use-cases more reliably.
        disabled: true,
        handler,
    }
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
    }
}
