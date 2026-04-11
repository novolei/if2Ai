//! Tool Registry - DashMap-based tool registration and dispatch
//!
//! Provides a high-performance tool registry with async dispatch and timeout support.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde_json::{json, Value};
use tokio::time::timeout;

/// Errors that can occur during tool dispatch.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ToolError {
    /// Tool was not found in the registry.
    NotFound(String),
    /// Tool is disabled.
    Disabled(String),
    /// Tool execution timed out.
    Timeout(String),
    /// Tool result exceeded the configured maximum size.
    OutputTooLarge { size: usize, max: usize },
    /// Tool registration failed.
    Register(String),
    /// Handler execution failed.
    Handler(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "tool not found: {name}"),
            Self::Disabled(name) => write!(f, "tool is disabled: {name}"),
            Self::Timeout(name) => write!(f, "tool execution timed out: {name}"),
            Self::OutputTooLarge { size, max } => {
                write!(
                    f,
                    "tool output too large: {size} bytes exceeds limit of {max}"
                )
            }
            Self::Register(msg) => write!(f, "tool registration failed: {msg}"),
            Self::Handler(msg) => write!(f, "tool handler error: {msg}"),
        }
    }
}

impl std::error::Error for ToolError {}

/// ToolEntry represents a single tool with its metadata and handler.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ToolEntry {
    /// Tool name (e.g., "bash", "file_read")
    pub name: String,
    /// Toolset this tool belongs to (e.g., "system", "files")
    pub toolset: String,
    /// Human-readable description
    pub description: String,
    /// JSON schema for input validation
    pub input_schema: Value,
    /// Maximum result size in bytes (None = unlimited)
    pub max_result_size: Option<usize>,
    /// Execution timeout in seconds (None = no timeout)
    pub timeout_secs: Option<u32>,
    /// Whether this tool is disabled
    pub disabled: bool,
    /// Handler function type
    pub handler: ToolHandler,
}

/// Async tool handler function type
#[allow(dead_code)]
pub type ToolHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn std::future::Future<Output = Result<String, ToolError>> + Send>>
        + Send
        + Sync,
>;

impl fmt::Debug for ToolEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolEntry")
            .field("name", &self.name)
            .field("toolset", &self.toolset)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("max_result_size", &self.max_result_size)
            .field("timeout_secs", &self.timeout_secs)
            .field("disabled", &self.disabled)
            .finish()
    }
}

/// DashMap-based ToolRegistry for high-performance concurrent access.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolRegistry {
    tools: Arc<DashMap<String, ToolEntry>>,
    names_to_toolsets: Arc<DashMap<String, String>>,
}

#[allow(dead_code)]
impl ToolRegistry {
    /// Creates a new empty ToolRegistry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: Arc::new(DashMap::new()),
            names_to_toolsets: Arc::new(DashMap::new()),
        }
    }

    /// Registers a tool entry.
    ///
    /// # Errors
    ///
    /// Returns an error if a tool with the same name is already registered.
    pub fn register(&self, entry: ToolEntry) -> Result<(), ToolError> {
        let name = entry.name.clone();
        if self.tools.contains_key(&name) {
            return Err(ToolError::Register(format!(
                "tool `{name}` is already registered"
            )));
        }
        self.names_to_toolsets
            .insert(name.clone(), entry.toolset.clone());
        self.tools.insert(name, entry);
        Ok(())
    }

    /// Gets a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<ToolEntry> {
        self.tools.get(name).map(|r| (*r).clone())
    }

    /// Checks if a tool with the given name exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Gets all tool names in a toolset.
    #[must_use]
    pub fn names_in_toolset(&self, toolset: &str) -> Vec<String> {
        self.names_to_toolsets
            .iter()
            .filter(|entry| entry.value() == toolset)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Gets all registered tool names.
    #[must_use]
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|r| r.key().clone()).collect()
    }

    /// Gets tool definitions in OpenAI format.
    ///
    /// # Arguments
    ///
    /// * `allowed` - Optional list of allowed tool names. If None, all non-disabled tools are included.
    #[must_use]
    pub fn get_definitions(&self, allowed: Option<&[String]>) -> Vec<Value> {
        self.tools
            .iter()
            .filter(|entry| {
                if entry.disabled {
                    return false;
                }
                if let Some(allowed) = allowed {
                    return allowed.contains(entry.key());
                }
                true
            })
            .map(|entry| {
                json!({
                    "type": "function",
                    "function": {
                        "name": entry.name,
                        "description": entry.description,
                        "parameters": entry.input_schema,
                    }
                })
            })
            .collect()
    }

    /// Dispatches a tool call with timeout protection.
    ///
    /// # Errors
    ///
    /// Returns `ToolError::NotFound` if the tool doesn't exist.
    /// Returns `ToolError::Disabled` if the tool is disabled.
    /// Returns `ToolError::Timeout` if execution exceeds the configured timeout.
    /// Returns `ToolError::OutputTooLarge` if the result exceeds `max_result_size`.
    pub async fn dispatch(&self, name: &str, args: Value) -> Result<String, ToolError> {
        let entry = self
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        if entry.disabled {
            return Err(ToolError::Disabled(name.to_string()));
        }

        let handler = entry.handler.clone();
        let max_size = entry.max_result_size;
        let timeout_duration = entry.timeout_secs.unwrap_or(300);

        let result = timeout(Duration::from_secs(timeout_duration as u64), handler(args)).await;

        match result {
            Ok(Ok(result)) => {
                if let Some(max_size) = max_size {
                    if result.len() > max_size {
                        return Err(ToolError::OutputTooLarge {
                            size: result.len(),
                            max: max_size,
                        });
                    }
                }
                Ok(result)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ToolError::Timeout(name.to_string())),
        }
    }

    /// Validates that a tool exists and arguments are valid.
    #[must_use]
    pub fn validate(&self, name: &str, args: &Value) -> Option<String> {
        let entry = self.get(name)?;

        // Basic schema validation
        if let Some(obj) = args.as_object() {
            if let Some(schema_obj) = entry
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
            {
                for (key, schema) in schema_obj {
                    if schema
                        .get("required")
                        .and_then(|r| r.as_bool())
                        .unwrap_or(false)
                        && !obj.contains_key(key)
                    {
                        return Some(format!("missing required parameter: {key}"));
                    }
                }
            }
        }

        None
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_handler(output: &'static str) -> ToolHandler {
        Arc::new(move |_input| {
            let output = output.to_string();
            Box::pin(async move { Ok(output) })
        })
    }

    #[tokio::test]
    async fn registers_and_retrieves_tool() {
        let registry = ToolRegistry::new();
        let entry = ToolEntry {
            name: "test".to_string(),
            toolset: "testing".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("test result"),
        };

        registry.register(entry).unwrap();
        assert!(registry.has("test"));

        let retrieved = registry.get("test").unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.toolset, "testing");
    }

    #[tokio::test]
    async fn dispatch_calls_handler() {
        let registry = ToolRegistry::new();
        let entry = ToolEntry {
            name: "hello".to_string(),
            toolset: "test".to_string(),
            description: "Says hello".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("Hello, World!"),
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("hello", json!({})).await.unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[tokio::test]
    async fn dispatch_not_found_returns_error() {
        let registry = ToolRegistry::new();
        let result = registry.dispatch("nonexistent", json!({})).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }

    #[tokio::test]
    async fn dispatch_disabled_tool_returns_error() {
        let registry = ToolRegistry::new();
        let entry = ToolEntry {
            name: "disabled".to_string(),
            toolset: "test".to_string(),
            description: "Disabled tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            timeout_secs: None,
            disabled: true,
            handler: make_test_handler("should not run"),
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("disabled", json!({})).await;
        assert!(matches!(result, Err(ToolError::Disabled(_))));
    }

    #[tokio::test]
    async fn dispatch_enforces_timeout() {
        let registry = ToolRegistry::new();
        let entry = ToolEntry {
            name: "slow".to_string(),
            toolset: "test".to_string(),
            description: "Slow tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            timeout_secs: Some(1),
            disabled: false,
            handler: Arc::new(|_input| {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    Ok("done".to_string())
                })
                    as Pin<Box<dyn std::future::Future<Output = Result<String, ToolError>> + Send>>
            }),
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("slow", json!({})).await;
        assert!(matches!(result, Err(ToolError::Timeout(_))));
    }

    #[tokio::test]
    async fn dispatch_enforces_max_result_size() {
        let registry = ToolRegistry::new();
        let entry = ToolEntry {
            name: "large".to_string(),
            toolset: "test".to_string(),
            description: "Large output tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: Some(10),
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("this is a long output that exceeds the limit"),
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("large", json!({})).await;
        assert!(matches!(result, Err(ToolError::OutputTooLarge { .. })));
    }

    #[tokio::test]
    async fn get_definitions_filters_disabled() {
        let registry = ToolRegistry::new();

        // Add a disabled tool
        registry
            .register(ToolEntry {
                name: "disabled".to_string(),
                toolset: "test".to_string(),
                description: "Disabled".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: true,
                handler: make_test_handler(""),
            })
            .unwrap();

        // Add an enabled tool
        registry
            .register(ToolEntry {
                name: "enabled".to_string(),
                toolset: "test".to_string(),
                description: "Enabled".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
            })
            .unwrap();

        let defs = registry.get_definitions(None);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["function"]["name"], "enabled");
    }

    #[tokio::test]
    async fn get_definitions_respects_allowed_list() {
        let registry = ToolRegistry::new();

        registry
            .register(ToolEntry {
                name: "tool_a".to_string(),
                toolset: "test".to_string(),
                description: "Tool A".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
            })
            .unwrap();

        registry
            .register(ToolEntry {
                name: "tool_b".to_string(),
                toolset: "test".to_string(),
                description: "Tool B".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
            })
            .unwrap();

        let defs = registry.get_definitions(Some(&["tool_a".to_string()]));
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["function"]["name"], "tool_a");
    }
}
