//! ToolSet module - Tool classification and grouping
//!
//! Provides ToolSet structure, predefined toolset constants, and ToolSetRegistry
//! for managing tool classifications.

use std::collections::HashMap;

/// A named group of tools with metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolSet {
    /// Toolset name (e.g., "files", "terminal").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// List of tool names in this toolset.
    pub tools: Vec<String>,
    /// Whether this toolset is enabled by default.
    pub enabled: bool,
}

/// Predefined toolset constants.
///
/// Each entry is: (name, description, &[tool_names])
pub const TOOLSETS: &[(&str, &str, &[&str])] = &[
    (
        "files",
        "File operations",
        &[
            "read_file",
            "file_write",
            "file_edit",
            "glob_search",
            "content_search",
        ],
    ),
    ("terminal", "Terminal execution", &["bash"]),
    ("utility", "Utility tools", &["json_parse", "calculator"]),
    (
        "web",
        "Web fetching and searching",
        &["web_fetch", "web_search", "http_request"],
    ),
    (
        "memory",
        "Long-term memory",
        &[
            "memory_store",
            "memory_recall",
            "memory_forget",
            "memory_purge",
            "memory_export",
        ],
    ),
    (
        "scheduler",
        "Cron/scheduled tasks",
        &[
            "cron_add",
            "cron_list",
            "cron_remove",
            "cron_run",
            "cron_runs",
        ],
    ),
    ("minimal", "Minimal set", &["read_file", "bash"]),
    (
        "development",
        "Full development stack",
        &["files", "terminal"],
    ),
];

/// Registry for managing toolset classifications.
#[derive(Debug, Clone)]
pub struct ToolSetRegistry {
    toolsets: HashMap<String, ToolSet>,
    tool_to_toolset: HashMap<String, String>,
}

impl ToolSetRegistry {
    /// Creates a new ToolSetRegistry from the predefined TOOLSETS constants.
    #[must_use]
    pub fn new() -> Self {
        let mut toolsets = HashMap::new();
        let mut tool_to_toolset = HashMap::new();

        for (name, description, tool_names) in TOOLSETS {
            let toolset = ToolSet {
                name: (*name).to_string(),
                description: (*description).to_string(),
                tools: (*tool_names).iter().map(|s| s.to_string()).collect(),
                enabled: true,
            };
            toolsets.insert((*name).to_string(), toolset.clone());

            for tool_name in *tool_names {
                tool_to_toolset.insert(tool_name.to_string(), (*name).to_string());
            }
        }

        Self {
            toolsets,
            tool_to_toolset,
        }
    }

    /// Gets all toolsets.
    #[must_use]
    pub fn all_toolsets(&self) -> Vec<ToolSet> {
        self.toolsets.values().cloned().collect()
    }

    /// Gets tool names that belong to the specified toolsets.
    ///
    /// If toolsets is empty, returns all tools.
    #[must_use]
    pub fn tools_from_toolsets(&self, toolsets: &[String]) -> Vec<String> {
        if toolsets.is_empty() {
            // Return all tools if no toolsets specified
            return self.tool_to_toolset.keys().cloned().collect();
        }

        let mut result = Vec::new();
        for toolset_name in toolsets {
            if let Some(toolset) = self.toolsets.get(toolset_name) {
                result.extend(toolset.tools.clone());
            }
        }
        result
    }

    /// Gets all tool names in a specific toolset.
    #[allow(dead_code)]
    #[must_use]
    pub fn tools_in_toolset(&self, toolset_name: &str) -> Vec<String> {
        self.toolsets
            .get(toolset_name)
            .map(|ts| ts.tools.clone())
            .unwrap_or_default()
    }

    /// Gets the toolset name for a given tool.
    #[allow(dead_code)]
    #[must_use]
    pub fn toolset_for_tool(&self, tool_name: &str) -> Option<String> {
        self.tool_to_toolset.get(tool_name).cloned()
    }

    /// Gets a toolset by name.
    #[allow(dead_code)]
    #[must_use]
    pub fn get(&self, name: &str) -> Option<ToolSet> {
        self.toolsets.get(name).cloned()
    }
}

impl Default for ToolSetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolset_registry_creates_all_toolsets() {
        let registry = ToolSetRegistry::new();
        let toolsets = registry.all_toolsets();
        assert_eq!(toolsets.len(), TOOLSETS.len());
    }

    #[test]
    fn tools_in_toolset_returns_correct_tools() {
        let registry = ToolSetRegistry::new();
        let files_tools = registry.tools_in_toolset("files");
        assert!(files_tools.contains(&"read_file".to_string()));
        assert!(files_tools.contains(&"file_write".to_string()));
    }

    #[test]
    fn toolset_for_tool_returns_correct_toolset() {
        let registry = ToolSetRegistry::new();
        // Note: bash appears in both minimal and terminal, last one wins
        assert_eq!(
            registry.toolset_for_tool("bash"),
            Some("minimal".to_string())
        );
        assert_eq!(
            registry.toolset_for_tool("json_parse"),
            Some("utility".to_string())
        );
    }

    #[test]
    fn tools_from_toolsets_filters_correctly() {
        let registry = ToolSetRegistry::new();
        let tools = registry.tools_from_toolsets(&["terminal".to_string()]);
        assert!(tools.contains(&"bash".to_string()));
        assert!(!tools.contains(&"read_file".to_string()));
    }

    #[test]
    fn tools_from_empty_toolsets_returns_all() {
        let registry = ToolSetRegistry::new();
        let tools = registry.tools_from_toolsets(&[]);
        // Should include bash from terminal toolset
        assert!(tools.contains(&"bash".to_string()));
    }

    #[test]
    fn tools_from_multiple_toolsets_combines() {
        let registry = ToolSetRegistry::new();
        let tools = registry.tools_from_toolsets(&["terminal".to_string(), "utility".to_string()]);
        assert!(tools.contains(&"bash".to_string()));
        assert!(tools.contains(&"json_parse".to_string()));
    }
}
