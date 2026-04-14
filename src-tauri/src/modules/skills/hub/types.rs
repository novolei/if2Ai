#![allow(unused)]

//! Hub types for skill source adapters.
//!
//! Ported from Hermes `tools/skills_hub.py` lines 63-85.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata for a skill from a hub source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Skill name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Source identifier (e.g., "github", "clawhub").
    pub source: String,
    /// Source-specific identifier (e.g., "owner/repo/path").
    pub identifier: String,
    /// Trust level of this skill.
    pub trust_level: crate::modules::skills::guard::policy::TrustLevel,
    /// Repository (e.g., "owner/repo").
    pub repo: Option<String>,
    /// Path within the repository.
    pub path: Option<String>,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Extra metadata as JSON.
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A complete skill bundle downloaded from a hub source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBundle {
    /// Skill name.
    pub name: String,
    /// Files in the skill: relative_path -> content.
    pub files: HashMap<String, Vec<u8>>,
    /// Source identifier.
    pub source: String,
    /// Source-specific identifier.
    pub identifier: String,
    /// Trust level of this skill.
    pub trust_level: crate::modules::skills::guard::policy::TrustLevel,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Hub-specific error type.
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hub error: {0}")]
    Other(String),
}

impl From<reqwest::Error> for HubError {
    fn from(e: reqwest::Error) -> Self {
        // Use status check to determine error type
        if e.is_connect() {
            HubError::Network(e.to_string())
        } else if let Some(status) = e.status() {
            if status.as_u16() == 401 || status.as_u16() == 403 {
                HubError::Auth(format!("HTTP {}: {}", status, e))
            } else {
                HubError::Other(format!("HTTP {}: {}", status, e))
            }
        } else {
            HubError::Other(e.to_string())
        }
    }
}

impl From<serde_json::Error> for HubError {
    fn from(e: serde_json::Error) -> Self {
        HubError::Parse(e.to_string())
    }
}

/// Result type for hub operations.
pub type HubResult<T> = Result<T, HubError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_meta_serialization() {
        let meta = SkillMeta {
            name: "test-skill".into(),
            description: "A test skill".into(),
            source: "github".into(),
            identifier: "owner/repo".into(),
            trust_level: crate::modules::skills::guard::policy::TrustLevel::Trusted,
            repo: Some("owner/repo".into()),
            path: Some("skills/test".into()),
            tags: vec!["test".into()],
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("test-skill"));
        assert!(json.contains("github"));
    }
}
