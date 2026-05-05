//! claw-cli compatibility export
//!
//! Exports if2Ai memory entries in claw-cli-compatible JSON format.
//! Only includes base fields: key, content, category, created_at, updated_at.
//! Extended fields (importance, access_count, trust_score) are stripped.

#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modules::memory::MemoryEntry;

/// Claw-cli compatible memory entry
///
/// Only includes fields that claw-cli understands.
/// if2Ai-specific fields (importance, access_count, trust_score) are omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawCliMemoryEntry {
    pub key: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ClawCliMemoryEntry {
    /// Convert an if2Ai MemoryEntry to claw-cli format
    pub fn from_memory(entry: &MemoryEntry) -> Self {
        Self {
            key: entry.key.clone(),
            content: entry.content.clone(),
            category: entry.category.as_str().to_string(),
            created_at: entry.created_at.to_rfc3339(),
            updated_at: entry.updated_at.to_rfc3339(),
        }
    }
}

/// Export memory entries to a claw-cli-compatible JSON file
pub fn export_for_clawcli(entries: &[MemoryEntry], path: &Path) -> Result<usize, std::io::Error> {
    let claw_entries: Vec<ClawCliMemoryEntry> = entries
        .iter()
        .map(ClawCliMemoryEntry::from_memory)
        .collect();

    let json = serde_json::to_string_pretty(&claw_entries).map_err(std::io::Error::other)?;

    std::fs::write(path, json)?;
    Ok(claw_entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::{MemoryCategory, MemoryEntry};
    use chrono::Utc;

    fn make_entry(key: &str, content: &str, category: MemoryCategory) -> MemoryEntry {
        let now = Utc::now();
        let cl = crate::modules::memory::CognitiveLayer::from_category(&category);
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category,
            created_at: now,
            updated_at: now,
            importance: 0.8,
            access_count: 5,
            trust_score: 0.3,
            session_id: None,
            project_id: None,
            quality_score: 0.5,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: cl,
            context_tags: Vec::new(),
        }
    }

    #[test]
    fn claw_cli_entry_strips_extended_fields() {
        let entry = make_entry("test_key", "test content", MemoryCategory::Core);
        let claw = ClawCliMemoryEntry::from_memory(&entry);

        assert_eq!(claw.key, "test_key");
        assert_eq!(claw.content, "test content");
        assert_eq!(claw.category, "core");
        // Extended fields should not be in ClawCliMemoryEntry
        // (compile-time guarantee — no importance/access_count/trust_score fields)
    }

    #[test]
    fn export_for_clawcli_produces_valid_json() {
        let entries = vec![
            make_entry("key1", "content1", MemoryCategory::Core),
            make_entry("key2", "content2", MemoryCategory::Daily),
        ];

        let temp_path = std::env::temp_dir().join(format!(
            "clawcli_export_{}.json",
            Utc::now().timestamp_millis()
        ));

        let count = export_for_clawcli(&entries, &temp_path).unwrap();
        assert_eq!(count, 2);

        // Verify file content
        let json = std::fs::read_to_string(&temp_path).unwrap();
        let parsed: Vec<ClawCliMemoryEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].key, "key1");
        assert_eq!(parsed[1].category, "daily");

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn export_empty_entries() {
        let entries: Vec<MemoryEntry> = vec![];
        let temp_path = std::env::temp_dir().join(format!(
            "clawcli_empty_{}.json",
            Utc::now().timestamp_millis()
        ));

        let count = export_for_clawcli(&entries, &temp_path).unwrap();
        assert_eq!(count, 0);

        let json = std::fs::read_to_string(&temp_path).unwrap();
        assert_eq!(json, "[]");

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn category_conversion_is_correct() {
        let core = make_entry("k", "c", MemoryCategory::Core);
        let daily = make_entry("k", "c", MemoryCategory::Daily);
        let conversation = make_entry("k", "c", MemoryCategory::Conversation);
        let custom = make_entry("k", "c", MemoryCategory::Custom("my_custom".to_string()));

        assert_eq!(ClawCliMemoryEntry::from_memory(&core).category, "core");
        assert_eq!(ClawCliMemoryEntry::from_memory(&daily).category, "daily");
        assert_eq!(
            ClawCliMemoryEntry::from_memory(&conversation).category,
            "conversation"
        );
        assert_eq!(
            ClawCliMemoryEntry::from_memory(&custom).category,
            "my_custom"
        );
    }
}
