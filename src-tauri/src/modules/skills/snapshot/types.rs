//! Snapshot types for skill export/import.
//!
//! Defines the snapshot format for exporting and importing skill configurations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Snapshot format version.
pub const SNAPSHOT_VERSION: &str = "1.0";

/// A complete snapshot of skill configuration.
///
/// This format allows skills to be exported from one environment
/// and imported into another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    /// Hermes version that created this snapshot.
    pub hermes_version: String,
    /// Snapshot format version.
    pub snapshot_version: String,
    /// When this snapshot was created.
    pub exported_at: DateTime<Utc>,
    /// Exported skills configuration.
    pub skills: Vec<ExportedSkill>,
    /// Hub taps configuration.
    pub taps: Vec<TapConfig>,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Default for SkillSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SkillSnapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self {
            hermes_version: "2.0".to_string(),
            snapshot_version: SNAPSHOT_VERSION.to_string(),
            exported_at: Utc::now(),
            skills: Vec::new(),
            taps: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a snapshot with the given skills and taps.
    pub fn with_content(skills: Vec<ExportedSkill>, taps: Vec<TapConfig>) -> Self {
        Self {
            hermes_version: "2.0".to_string(),
            snapshot_version: SNAPSHOT_VERSION.to_string(),
            exported_at: Utc::now(),
            skills,
            taps,
            metadata: HashMap::new(),
        }
    }

    /// Add a skill to the snapshot.
    pub fn add_skill(mut self, skill: ExportedSkill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Add a tap to the snapshot.
    pub fn add_tap(mut self, tap: TapConfig) -> Self {
        self.taps.push(tap);
        self
    }

    /// Get the count of skills in this snapshot.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Get the count of taps in this snapshot.
    pub fn tap_count(&self) -> usize {
        self.taps.len()
    }
}

/// An exported skill configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSkill {
    /// Skill name.
    pub name: String,
    /// Skill source (e.g., "github", "skills.sh").
    pub source: String,
    /// Source identifier (e.g., "owner/repo/path").
    pub identifier: String,
    /// Whether this skill is enabled.
    pub enabled: bool,
    /// Trust level of the skill.
    pub trust_level: String,
    /// When this skill was first discovered.
    pub discovered_at: Option<DateTime<Utc>>,
    /// When this skill was last used.
    pub last_used_at: Option<DateTime<Utc>>,
    /// Custom metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[allow(dead_code)]
impl ExportedSkill {
    /// Create a new exported skill.
    pub fn new(name: String, source: String, identifier: String) -> Self {
        Self {
            name,
            source,
            identifier,
            enabled: true,
            trust_level: "community".to_string(),
            discovered_at: None,
            last_used_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the trust level.
    pub fn with_trust_level(mut self, trust_level: &str) -> Self {
        self.trust_level = trust_level.to_string();
        self
    }

    /// Set enabled status.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A tap (Third-Party Addition Point) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapConfig {
    /// Tap identifier.
    pub id: String,
    /// Tap source URL.
    pub source_url: String,
    /// Tap type.
    pub tap_type: String,
    /// Whether this tap is enabled.
    pub enabled: bool,
    /// Configuration for the tap.
    #[serde(default)]
    pub config: HashMap<String, String>,
}

#[allow(dead_code)]
impl TapConfig {
    /// Create a new tap config.
    pub fn new(id: String, source_url: String, tap_type: String) -> Self {
        Self {
            id,
            source_url,
            tap_type,
            enabled: true,
            config: HashMap::new(),
        }
    }

    /// Set enabled status.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add a configuration key-value pair.
    pub fn with_config(mut self, key: &str, value: &str) -> Self {
        self.config.insert(key.to_string(), value.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_snapshot_new() {
        let snapshot = SkillSnapshot::new();
        assert_eq!(snapshot.snapshot_version, SNAPSHOT_VERSION);
        assert!(snapshot.skills.is_empty());
        assert!(snapshot.taps.is_empty());
    }

    #[test]
    fn test_skill_snapshot_with_content() {
        let skill = ExportedSkill::new(
            "test-skill".to_string(),
            "github".to_string(),
            "owner/repo/test-skill".to_string(),
        );
        let tap = TapConfig::new(
            "test-tap".to_string(),
            "https://example.com/tap".to_string(),
            "github".to_string(),
        );

        let snapshot = SkillSnapshot::with_content(vec![skill], vec![tap]);

        assert_eq!(snapshot.skill_count(), 1);
        assert_eq!(snapshot.tap_count(), 1);
    }

    #[test]
    fn test_skill_snapshot_add() {
        let skill = ExportedSkill::new(
            "test-skill".to_string(),
            "github".to_string(),
            "owner/repo/test-skill".to_string(),
        );

        let snapshot = SkillSnapshot::new()
            .add_skill(skill)
            .add_tap(TapConfig::new(
                "tap1".to_string(),
                "https://example.com".to_string(),
                "github".to_string(),
            ));

        assert_eq!(snapshot.skill_count(), 1);
        assert_eq!(snapshot.tap_count(), 1);
    }

    #[test]
    fn test_exported_skill_builder() {
        let skill = ExportedSkill::new(
            "my-skill".to_string(),
            "github".to_string(),
            "owner/repo/my-skill".to_string(),
        )
        .with_trust_level("trusted")
        .with_enabled(false);

        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.trust_level, "trusted");
        assert!(!skill.enabled);
    }

    #[test]
    fn test_tap_config_builder() {
        let tap = TapConfig::new(
            "my-tap".to_string(),
            "https://example.com/tap".to_string(),
            "github".to_string(),
        )
        .with_enabled(false)
        .with_config("token", "abc123");

        assert_eq!(tap.id, "my-tap");
        assert!(!tap.enabled);
        assert_eq!(tap.config.get("token"), Some(&"abc123".to_string()));
    }

    #[test]
    fn test_skill_snapshot_serialization() {
        let snapshot = SkillSnapshot::new()
            .add_skill(
                ExportedSkill::new(
                    "test".to_string(),
                    "github".to_string(),
                    "owner/repo".to_string(),
                )
                .with_trust_level("trusted"),
            )
            .add_tap(TapConfig::new(
                "tap1".to_string(),
                "https://example.com".to_string(),
                "github".to_string(),
            ));

        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("tap1"));
        assert!(json.contains("snapshot_version"));
    }

    #[test]
    fn test_skill_snapshot_deserialization() {
        let json = r#"{
            "hermes_version": "2.0",
            "snapshot_version": "1.0",
            "exported_at": "2024-01-01T00:00:00Z",
            "skills": [
                {
                    "name": "test-skill",
                    "source": "github",
                    "identifier": "owner/repo/test-skill",
                    "enabled": true,
                    "trust_level": "community",
                    "metadata": {}
                }
            ],
            "taps": [],
            "metadata": {}
        }"#;

        let snapshot: SkillSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.skill_count(), 1);
        assert_eq!(snapshot.skills[0].name, "test-skill");
    }
}
