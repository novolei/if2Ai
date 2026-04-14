//! skill_manage tool — create, edit, patch, and delete skills from within Agent context.
//!
//! Equivalent to Hermes `skill_manage` tool (`tools/skill_manager_tool.py`).
//! Delegates to `DefaultSkillManager` which includes security scanning before any write.

use std::sync::Arc;

use crate::modules::skills::manager::{
    DefaultSkillManager, SkillManageAction, SkillManageInput, SkillManager,
};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the `skill_manage` ToolEntry.
#[must_use]
pub fn skill_manage_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            let action_str = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'action' argument".into()))?;

            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'name' argument".into()))?;

            let action = parse_action(action_str)?;

            // All managed skills are written to the user/workspace skills directory
            let skills_dir = workdir.join(".if2ai/skills");

            let input = SkillManageInput {
                action,
                name: name.to_string(),
                content: args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                category: args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                file_path: args
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                file_content: args
                    .get("file_content")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                old_string: args
                    .get("old_string")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                new_string: args
                    .get("new_string")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                replace_all: args
                    .get("replace_all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            };

            let manager = DefaultSkillManager::new(skills_dir);
            let ctx_inner =
                crate::modules::skills::manager::SkillContext::new(workdir.join(".if2ai/skills"));

            let result = manager
                .manage(input, &ctx_inner)
                .map_err(|e| ToolError::Handler(e.to_string()))?;

            // Invalidate skills index cache after a write operation
            if matches!(
                action,
                SkillManageAction::Create
                    | SkillManageAction::Edit
                    | SkillManageAction::Delete
                    | SkillManageAction::Patch
            ) {
                crate::modules::runtime::prompt::invalidate_skills_index_cache();
            }

            Ok(format!(
                "skill_manage({action_str}) {name}: {}{}",
                result.message,
                result
                    .skill_path
                    .as_ref()
                    .map(|p| format!(" → {}", p.display()))
                    .unwrap_or_default()
            ))
        })
    });

    ToolEntry {
        name: "skill_manage".to_string(),
        toolset: "utility".to_string(),
        description: "Create, edit, patch, or delete a skill. \
                      Use action=create to build a new skill from scratch, \
                      action=edit to overwrite SKILL.md, \
                      action=patch to find-and-replace text inside SKILL.md, \
                      action=delete to remove a skill, \
                      action=write_file to add a supporting file, \
                      action=remove_file to delete a supporting file."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "edit", "patch", "delete", "write_file", "remove_file"],
                    "description": "The management operation to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (directory name, lowercase with hyphens)"
                },
                "content": {
                    "type": "string",
                    "description": "SKILL.md content for create/edit actions"
                },
                "category": {
                    "type": "string",
                    "description": "Optional skill category for create action"
                },
                "file_path": {
                    "type": "string",
                    "description": "Relative file path inside skill directory for write_file/remove_file"
                },
                "file_content": {
                    "type": "string",
                    "description": "Content of the supporting file for write_file action"
                },
                "old_string": {
                    "type": "string",
                    "description": "Text to find for patch action"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text for patch action"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences for patch action (default false)"
                }
            },
            "required": ["action", "name"]
        }),
        max_result_size: Some(8 * 1024),
        timeout_secs: Some(15),
        disabled: false,
        handler,
    }
}

/// Parse the action string into a `SkillManageAction`.
fn parse_action(action: &str) -> Result<SkillManageAction, ToolError> {
    match action {
        "create" => Ok(SkillManageAction::Create),
        "edit" => Ok(SkillManageAction::Edit),
        "patch" => Ok(SkillManageAction::Patch),
        "delete" => Ok(SkillManageAction::Delete),
        "write_file" => Ok(SkillManageAction::WriteFile),
        "remove_file" => Ok(SkillManageAction::RemoveFile),
        other => Err(ToolError::Handler(format!(
            "Unknown action '{other}'. Must be one of: create, edit, patch, delete, write_file, remove_file"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_manage_tool_entry_has_correct_structure() {
        let entry = skill_manage_tool_entry();
        assert_eq!(entry.name, "skill_manage");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }

    #[test]
    fn parse_action_succeeds_for_all_variants() {
        assert!(matches!(
            parse_action("create"),
            Ok(SkillManageAction::Create)
        ));
        assert!(matches!(parse_action("edit"), Ok(SkillManageAction::Edit)));
        assert!(matches!(
            parse_action("patch"),
            Ok(SkillManageAction::Patch)
        ));
        assert!(matches!(
            parse_action("delete"),
            Ok(SkillManageAction::Delete)
        ));
        assert!(matches!(
            parse_action("write_file"),
            Ok(SkillManageAction::WriteFile)
        ));
        assert!(matches!(
            parse_action("remove_file"),
            Ok(SkillManageAction::RemoveFile)
        ));
    }

    #[test]
    fn parse_action_fails_for_unknown() {
        assert!(parse_action("unknown").is_err());
    }
}
