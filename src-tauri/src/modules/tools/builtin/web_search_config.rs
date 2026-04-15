//! Web search provider configuration.
//!
//! Stores and retrieves ordered list of configured search providers.
//! Config is persisted to `~/.if2ai/web-search-config.json`.
//! Priority is determined by the order in the `providers` list (first = highest).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single configured web search provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchProvider {
    /// Stable provider identifier (e.g. "tavily", "searxng").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// API key for the provider.  `None` for self-hosted instances like SearXNG.
    pub api_key: Option<String>,
    /// Base URL override.  Required for SearXNG; optional for others.
    pub base_url: Option<String>,
    /// Whether this provider is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Root config structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSearchConfig {
    /// Ordered list of providers — highest priority first.
    #[serde(default)]
    pub providers: Vec<WebSearchProvider>,
}

/// Returns the path to the persisted config file.
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".if2ai")
        .join("web-search-config.json")
}

/// Load config from disk.  Returns an empty config if the file is absent or
/// unparseable rather than propagating an error.
pub fn load() -> WebSearchConfig {
    let path = config_path();
    if !path.exists() {
        return WebSearchConfig::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return WebSearchConfig::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Persist config to disk, creating parent directories as needed.
pub fn save(config: &WebSearchConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Returns the first enabled provider from the config, if any.
pub fn active_provider() -> Option<WebSearchProvider> {
    load().providers.into_iter().find(|p| p.enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_providers() {
        let cfg = WebSearchConfig::default();
        assert!(cfg.providers.is_empty());
    }

    #[test]
    fn round_trip_serialisation() {
        let cfg = WebSearchConfig {
            providers: vec![WebSearchProvider {
                id: "tavily".into(),
                name: "Tavily".into(),
                api_key: Some("tvly-test".into()),
                base_url: None,
                enabled: true,
            }],
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: WebSearchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.providers.len(), 1);
        assert_eq!(decoded.providers[0].id, "tavily");
    }
}
