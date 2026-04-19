//! Config tool - get/set configuration settings.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs Config.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
struct ConfigInput {
    setting: String,
    value: Option<serde_json::Value>,
}

/// Creates the Config tool entry.
#[must_use]
pub fn config_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: ConfigInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let config_path = std::path::PathBuf::from(&home).join(".if2ai/config.json");

            if input.value.is_some() {
                // Set operation
                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let current_config = if config_path.exists() {
                    std::fs::read_to_string(&config_path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .and_then(|mut v| v.as_object_mut().map(|o| o.clone()))
                        .unwrap_or_default()
                } else {
                    serde_json::Map::new()
                };

                // Would set the value in the config
                let _ = current_config;
                Ok(format!("Setting '{}' would be updated", input.setting))
            } else {
                // Get operation
                if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
                    if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(val) = config.get(&input.setting) {
                            return serde_json::to_string_pretty(val)
                                .map_err(|e| ToolError::Handler(e.to_string()));
                        }
                    }
                }
                Ok(format!("Setting '{}' not found", input.setting))
            }
        })
    });

    ToolEntry {
        name: "Config".to_string(),
        toolset: "utility".to_string(),
        description: "Get or set If2Ai configuration settings.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "setting": { "type": "string", "description": "Configuration key to get or set" },
                "value": { "type": ["string", "boolean", "number"], "description": "Value to set (omit to get)" }
            },
            "required": ["setting"]
        }),
        max_result_size: Some(5 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(5),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn config_tool_entry_has_correct_structure() {
        let entry = config_tool_entry();
        assert_eq!(entry.name, "Config");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(5 * 1024));
        assert_eq!(entry.timeout_secs, Some(5));
    }
}
