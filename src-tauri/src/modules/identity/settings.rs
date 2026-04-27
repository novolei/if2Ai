use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Path to the persisted prompt control plane settings file.
/// Mirrors `runtime::config::default_prompt_control_config_path` so
/// identity-only consumers can read naming without depending on the
/// full ConfigLoader pipeline (which is heavy per-turn).
#[must_use]
pub fn default_prompt_control_settings_path() -> PathBuf {
    let config_home = std::env::var_os("IF2AI_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".if2ai")))
        .unwrap_or_else(|| PathBuf::from(".if2ai"));
    config_home.join("prompt").join("control-plane.json")
}

/// Lightweight result of reading just the naming-related identity
/// fields from `~/.if2ai/prompt/control-plane.json`. Used by the
/// prompt planner to render the Identity Naming block without paying
/// for a full settings parse on every turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityNamingSettings {
    pub agent_name: Option<String>,
    pub user_name: Option<String>,
}

impl IdentityNamingSettings {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.agent_name.is_none() && self.user_name.is_none()
    }
}

/// Read agent_name / user_name from the persisted control plane JSON.
/// Returns `Default` (both `None`) when the file does not exist or any
/// I/O / parse error occurs — naming is best-effort and must never
/// fail the prompt build.
#[must_use]
pub fn read_identity_naming_settings() -> IdentityNamingSettings {
    let path = default_prompt_control_settings_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return IdentityNamingSettings::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return IdentityNamingSettings::default();
    };
    let identity = value.get("identity").and_then(Value::as_object);
    let extract = |key: &str| -> Option<String> {
        identity
            .and_then(|object| object.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    };
    IdentityNamingSettings {
        agent_name: extract("agentName"),
        user_name: extract("userName"),
    }
}

/// Global identity defaults loaded from runtime settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_soul_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_persona_id: Option<String>,
    /// Optional global agent name (恒定 Agent 真名, 跨所有 Persona 共享).
    /// When set, planner injects an "Identity Naming" prompt block so the
    /// LLM always self-identifies with this name regardless of the active
    /// persona. Inspired by openhanako's `{{agentName}}` template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// Optional name the user wants the agent to call them. Mirrors
    /// openhanako's `{{userName}}` template variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

/// Session-scoped identity override applied on top of global defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentityOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soul_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
}
