//! SkillFind tool — precisely locate a skill by exact name match.
//!
//! Unlike `skill_search` which does fuzzy matching on name and description,
//! `skill_find` performs exact-match lookup to resolve a skill uniquely.
//! This avoids ambiguity when multiple skills have similar names.

use std::sync::Arc;

use crate::modules::runtime::prompt::collect_skill_index_entries;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// One skill found by `skill_find`.
#[derive(serde::Serialize)]
struct SkillFindResult {
    success: bool,
    name: String,
    description: String,
    path: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Creates the `skill_find` ToolEntry.
#[must_use]
pub fn skill_find_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'name' argument".into()))?;

            // Get all skills via the same index as skill_search
            let entries = collect_skill_index_entries(&workdir, None);

            // Exact match by name (case-insensitive)
            let normalized_name = name.to_lowercase();
            let found = entries
                .into_iter()
                .find(|e| e.name.to_lowercase() == normalized_name);

            match found {
                Some(entry) => {
                    let result = SkillFindResult {
                        success: true,
                        name: entry.name,
                        description: entry.description,
                        path: entry.skill_dir.to_string_lossy().into_owned(),
                        source: entry.source,
                        error: None,
                    };
                    serde_json::to_string_pretty(&result)
                        .map_err(|e| ToolError::Handler(e.to_string()))
                }
                None => {
                    let result = SkillFindResult {
                        success: false,
                        name: name.to_string(),
                        description: String::new(),
                        path: String::new(),
                        source: String::new(),
                        error: Some(format!("Skill '{}' not found.", name)),
                    };
                    serde_json::to_string_pretty(&result)
                        .map_err(|e| ToolError::Handler(e.to_string()))
                }
            }
        })
    });

    ToolEntry {
        name: "skill_find".to_string(),
        toolset: "utility".to_string(),
        description: "Precisely locate a skill by exact name match. \
                      Use this when you know the exact skill name and want to avoid \
                      the fuzzy matching of skill_search."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Exact skill name to find (case-insensitive)"
                }
            },
            "required": ["name"]
        }),
        max_result_size: Some(16 * 1024),
        timeout_secs: Some(5),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_find_tool_entry_has_correct_structure() {
        let entry = skill_find_tool_entry();
        assert_eq!(entry.name, "skill_find");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }
}
