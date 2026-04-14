//! Skill configuration variable resolution.
//!
//! Ported from Hermes `agent/skill_commands.py` lines 82-118
//! and `agent/skill_utils.py` for config extraction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors for skill config operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[allow(dead_code)]
    #[error("config error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Parse(e.to_string())
    }
}

/// Result type for config operations.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// A skill configuration variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigVar {
    /// Variable key/name.
    pub key: String,
    /// Human-readable description of the variable.
    pub description: String,
    /// Default value if not set in config.
    #[serde(default)]
    pub default: Option<String>,
    /// Interactive prompt message shown when requesting this config value.
    /// If present, the user will be prompted to enter a value interactively.
    #[serde(default)]
    pub prompt: Option<String>,
}

#[allow(dead_code)]
impl SkillConfigVar {
    /// Create a new config variable.
    pub fn new(key: &str, description: &str, default: Option<&str>) -> Self {
        Self {
            key: key.to_string(),
            description: description.to_string(),
            default: default.map(String::from),
            prompt: None,
        }
    }

    /// Create a new config variable with a prompt.
    pub fn with_prompt(key: &str, description: &str, default: Option<&str>, prompt: &str) -> Self {
        Self {
            key: key.to_string(),
            description: description.to_string(),
            default: default.map(String::from),
            prompt: Some(prompt.to_string()),
        }
    }

    /// Check if this config variable requires interactive prompting.
    pub fn requires_prompt(&self) -> bool {
        self.prompt.is_some()
    }
}

/// Resolves skill configuration variables from a config file.
#[derive(Debug, Clone)]
pub struct SkillConfigResolver {
    config_values: HashMap<String, String>,
}

impl Default for SkillConfigResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SkillConfigResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self {
            config_values: HashMap::new(),
        }
    }

    /// Load config from a YAML file.
    ///
    /// Expected format:
    /// ```yaml
    /// skill_name:
    ///   key1: value1
    ///   key2: value2
    /// ```
    pub fn from_config_file(config_path: &Path) -> ConfigResult<Self> {
        if !config_path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(config_path)?;
        Self::from_yaml(&content)
    }

    /// Parse config from YAML string.
    pub fn from_yaml(yaml_content: &str) -> ConfigResult<Self> {
        // Simple YAML parsing for flat key-value pairs
        // Format: "skill.key: value" or just "key: value"
        let mut config_values = HashMap::new();

        for line in yaml_content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse "key: value" format
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();

                // Remove quotes from value if present
                let value = value
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .trim_start_matches('\'')
                    .trim_end_matches('\'');

                if !key.is_empty() {
                    config_values.insert(key, value.to_string());
                }
            }
        }

        Ok(Self { config_values })
    }

    /// Resolve multiple config variables.
    ///
    /// Returns a HashMap of key -> resolved value.
    /// Uses config file value if present, otherwise uses default.
    pub fn resolve(&self, vars: &[SkillConfigVar]) -> HashMap<String, String> {
        vars.iter()
            .map(|var| {
                let value = self.resolve_single(var);
                (var.key.clone(), value)
            })
            .collect()
    }

    /// Resolve a single config variable.
    ///
    /// Returns the config file value if present,
    /// otherwise the default value, otherwise empty string.
    pub fn resolve_single(&self, var: &SkillConfigVar) -> String {
        self.config_values
            .get(&var.key)
            .cloned()
            .or_else(|| var.default.clone())
            .unwrap_or_default()
    }

    /// Get a raw config value by key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.config_values.get(key)
    }

    /// Check if a key exists in the config.
    pub fn contains(&self, key: &str) -> bool {
        self.config_values.contains_key(key)
    }

    /// Get all config keys for a specific skill prefix.
    ///
    /// If skill_name is "my-skill", returns keys like "my-skill.key1", "my-skill.key2".
    pub fn get_for_skill(&self, skill_name: &str) -> HashMap<String, String> {
        let prefix = format!("{}.", skill_name);
        self.config_values
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, value)| {
                // Strip prefix to get local key
                let local_key = key[prefix.len()..].to_string();
                (local_key, value.clone())
            })
            .collect()
    }
}

/// Extract configuration variables from skill frontmatter.
///
/// The frontmatter should contain a `config` section like:
/// ```yaml
/// config:
///   - key: API_KEY
///     description: Your API key
///     default: ""
///     prompt: "Enter your API key:"
/// ```
///
/// The `prompt` field is optional and enables interactive configuration.
#[allow(dead_code)]
pub fn extract_config_vars(frontmatter: &serde_json::Value) -> Vec<SkillConfigVar> {
    let mut vars = Vec::new();

    // Look for "config" key in frontmatter
    if let Some(config) = frontmatter.get("config") {
        if let Some(config_array) = config.as_array() {
            for item in config_array {
                if let Some(key) = item.get("key").and_then(|k| k.as_str()) {
                    let description = item
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let default = item
                        .get("default")
                        .and_then(|d| d.as_str())
                        .map(String::from);
                    let prompt = item
                        .get("prompt")
                        .and_then(|p| p.as_str())
                        .map(String::from);

                    vars.push(SkillConfigVar {
                        key: key.to_string(),
                        description,
                        default,
                        prompt,
                    });
                }
            }
        }
    }

    // Also check "hermes" section with nested config
    if let Some(hermes) = frontmatter.get("hermes") {
        if let Some(config_obj) = hermes.get("config") {
            if let Some(config_array) = config_obj.as_array() {
                for item in config_array {
                    if let Some(key) = item.get("key").and_then(|k| k.as_str()) {
                        let description = item
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let default = item
                            .get("default")
                            .and_then(|d| d.as_str())
                            .map(String::from);
                        let prompt = item
                            .get("prompt")
                            .and_then(|p| p.as_str())
                            .map(String::from);

                        vars.push(SkillConfigVar {
                            key: key.to_string(),
                            description,
                            default,
                            prompt,
                        });
                    }
                }
            }
        }
    }

    vars
}

/// Format a resolved config block for skill invocation.
///
/// Format: "[Skill config (from <skill_name>): key1 = value1, key2 = value2]"
#[allow(dead_code)]
pub fn format_config_block(resolved: &HashMap<String, String>) -> String {
    if resolved.is_empty() {
        return String::new();
    }

    let pairs: Vec<String> = resolved
        .iter()
        .map(|(key, value)| format!("{} = {}", key, value))
        .collect();

    format!("[Skill config: {}]", pairs.join(", "))
}

/// Format a resolved config block for a specific skill.
///
/// Format: "[Skill config (from <skill_name>): key1 = value1, key2 = value2]"
#[allow(dead_code)]
pub fn format_skill_config_block(skill_name: &str, resolved: &HashMap<String, String>) -> String {
    if resolved.is_empty() {
        return String::new();
    }

    let pairs: Vec<String> = resolved
        .iter()
        .map(|(key, value)| format!("{} = {}", key, value))
        .collect();

    format!("[Skill config (from {}): {}]", skill_name, pairs.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_config_var_new() {
        let var = SkillConfigVar::new("API_KEY", "Your API key", Some("default_key"));
        assert_eq!(var.key, "API_KEY");
        assert_eq!(var.description, "Your API key");
        assert_eq!(var.default, Some("default_key".to_string()));
    }

    #[test]
    fn test_resolver_from_yaml() {
        let yaml = r#"
# Comment
key1: value1
key2: value2
"#;
        let resolver = SkillConfigResolver::from_yaml(yaml).unwrap();
        assert_eq!(resolver.get("key1"), Some(&"value1".to_string()));
        assert_eq!(resolver.get("key2"), Some(&"value2".to_string()));
        assert_eq!(resolver.get("key3"), None);
    }

    #[test]
    fn test_resolver_resolve_single() {
        let resolver = SkillConfigResolver::from_yaml("key: value").unwrap();
        let var = SkillConfigVar::new("key", "desc", Some("default"));
        assert_eq!(resolver.resolve_single(&var), "value");

        // Without config value, should use default
        let var2 = SkillConfigVar::new("unknown", "desc", Some("default"));
        assert_eq!(resolver.resolve_single(&var2), "default");

        // Without config or default, should be empty
        let var3 = SkillConfigVar::new("unknown", "desc", None);
        assert_eq!(resolver.resolve_single(&var3), "");
    }

    #[test]
    fn test_resolver_get_for_skill() {
        let resolver = SkillConfigResolver::from_yaml(
            "my-skill.key1: value1\nmy-skill.key2: value2\nother.something: other",
        )
        .unwrap();
        let skill_config = resolver.get_for_skill("my-skill");
        assert_eq!(skill_config.get("key1"), Some(&"value1".to_string()));
        assert_eq!(skill_config.get("key2"), Some(&"value2".to_string()));
        assert!(!skill_config.contains_key("something"));
    }

    #[test]
    fn test_extract_config_vars() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "config": [
                {"key": "API_KEY", "description": "Your API key", "default": ""},
                {"key": "MODEL", "description": "Model to use", "default": "gpt-4"}
            ]
        }"#,
        )
        .unwrap();

        let vars = extract_config_vars(&json);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].key, "API_KEY");
        assert_eq!(vars[1].key, "MODEL");
        assert_eq!(vars[1].default, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_format_config_block() {
        let mut resolved = HashMap::new();
        resolved.insert("API_KEY".to_string(), "secret123".to_string());
        resolved.insert("MODEL".to_string(), "gpt-4".to_string());

        let block = format_config_block(&resolved);
        assert!(block.contains("API_KEY = secret123"));
        assert!(block.contains("MODEL = gpt-4"));
    }

    #[test]
    fn test_format_skill_config_block() {
        let mut resolved = HashMap::new();
        resolved.insert("API_KEY".to_string(), "secret123".to_string());

        let block = format_skill_config_block("my-skill", &resolved);
        assert!(block.contains("from my-skill"));
        assert!(block.contains("API_KEY = secret123"));
    }

    #[test]
    fn test_empty_resolved() {
        let resolved = HashMap::new();
        let block = format_config_block(&resolved);
        assert!(block.is_empty());

        let skill_block = format_skill_config_block("my-skill", &resolved);
        assert!(skill_block.is_empty());
    }
}
