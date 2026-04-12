//! NotebookEdit tool - edit Jupyter notebook cells.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs NotebookEdit.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
// Input schema declares optional fields; handler reads only notebook_path and edit_mode.
#[allow(dead_code)]
struct NotebookEditInput {
    notebook_path: String,
    cell_id: Option<String>,
    new_source: Option<String>,
    cell_type: Option<String>,
    edit_mode: Option<String>,
}

/// Creates the NotebookEdit tool entry.
#[must_use]
pub fn notebook_edit_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: NotebookEditInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            let path = std::path::PathBuf::from(&input.notebook_path);
            if !path.exists() {
                return Err(ToolError::Handler(format!(
                    "Notebook not found: {}",
                    input.notebook_path
                )));
            }

            let content = std::fs::read_to_string(&path)
                .map_err(|e| ToolError::Handler(format!("Failed to read notebook: {e}")))?;

            let _notebook: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| ToolError::Handler(format!("Invalid notebook JSON: {e}")))?;

            Ok(format!(
                "[NotebookEdit] Would edit {} (mode: {:?})",
                input.notebook_path, input.edit_mode
            ))
        })
    });

    ToolEntry {
        name: "NotebookEdit".to_string(),
        toolset: "write".to_string(),
        description: "Replace, insert, or delete a cell in a Jupyter notebook.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "notebook_path": { "type": "string", "description": "Path to the notebook file" },
                "cell_id": { "type": "string", "description": "ID of the cell to edit" },
                "new_source": { "type": "string", "description": "New cell content" },
                "cell_type": { "type": "string", "enum": ["code", "markdown"] },
                "edit_mode": { "type": "string", "enum": ["replace", "insert", "delete"] }
            },
            "required": ["notebook_path"]
        }),
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn notebook_edit_tool_entry_has_correct_structure() {
        let entry = notebook_edit_tool_entry();
        assert_eq!(entry.name, "NotebookEdit");
        assert_eq!(entry.toolset, "write");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(30));
    }
}
