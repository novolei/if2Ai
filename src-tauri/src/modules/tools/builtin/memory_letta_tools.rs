//! MEM-MOD-P3 — Letta-style memory primitives.
//!
//! Bundles four small tools that share a single-file lifecycle so the
//! Pack stays cohesive:
//!
//! | tool                       | verb                  |
//! |----------------------------|-----------------------|
//! | `memory_recall_explicit`   | get-by-key            |
//! | `memory_update`            | rewrite content       |
//! | `memory_link`              | create typed link     |
//! | `memory_consolidate`       | merge N → 1 + link    |
//!
//! These are deliberately *low-level*: they trust the LLM (or a higher
//! Mem0-style decision tree, see MEM-MOD-P4) to pick the right action.
//! Each handler returns a JSON envelope with `status` + result fields,
//! mirroring the shape `memory_store` / `memory_feedback_signal` use.
//!
//! No new IPC / no policy engine wiring (these tools all act on
//! existing entries the user has already accepted), so the security
//! envelope is the same as `memory_recall`.

use std::sync::Arc;

use serde_json::json;

use crate::modules::memory::{MemoryCategory, MemoryError, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

// ────────────────────────────────────────────────────────────────────
// memory_recall_explicit
// ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[must_use]
pub fn recall_explicit_entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: serde_json::Value, _ctx: SharedToolContext| {
        let memory = memory.clone();
        Box::pin(async move {
            let key = require_string(&args, "key")?;
            match memory.get_by_key(&key).await {
                Ok(Some(entry)) => Ok(json!({
                    "status": "ok",
                    "key": entry.key,
                    "content": entry.content,
                    "category": entry.category.as_str(),
                    "importance": entry.importance,
                    "access_count": entry.access_count,
                    "trust_score": entry.trust_score,
                    "updated_at": entry.updated_at.to_rfc3339(),
                })
                .to_string()),
                Ok(None) => Ok(json!({ "status": "not_found", "key": key }).to_string()),
                Err(err) => Err(ToolError::Handler(format!(
                    "memory_recall_explicit failed: {err}"
                ))),
            }
        })
    });

    ToolEntry {
        name: "memory_recall_explicit".to_string(),
        toolset: "memory".to_string(),
        description:
            "Fetch one memory entry by exact key. Returns JSON `{status:'ok'|'not_found', \
                      key, content, category, importance, access_count, trust_score, updated_at}`. \
                      Use when you already know the key (e.g. from a prior `memory_recall`); \
                      prefer `memory_recall` for fuzzy / scope-aware lookup."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Exact memory key." }
            },
            "required": ["key"]
        }),
        max_result_size: Some(2048),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

// ────────────────────────────────────────────────────────────────────
// memory_update
// ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[must_use]
pub fn update_entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: serde_json::Value, _ctx: SharedToolContext| {
        let memory = memory.clone();
        Box::pin(async move {
            let key = require_string(&args, "key")?;
            let content = require_string(&args, "content")?;
            match memory.update_content(&key, &content).await {
                Ok(()) => Ok(json!({
                    "status": "ok",
                    "key": key,
                    "content_chars": content.chars().count(),
                })
                .to_string()),
                Err(MemoryError::KeyNotFound(_)) => {
                    Ok(json!({ "status": "not_found", "key": key }).to_string())
                }
                Err(err) => Err(ToolError::Handler(format!("memory_update failed: {err}"))),
            }
        })
    });

    ToolEntry {
        name: "memory_update".to_string(),
        toolset: "memory".to_string(),
        description: "Replace the `content` of an existing memory entry. Stats columns \
                      (importance / access_count / trust_score) are preserved; `updated_at` \
                      is bumped to now. Returns `{status:'ok'|'not_found', key, content_chars}`. \
                      Use when an old fact is partially wrong rather than fully obsolete \
                      (otherwise call `memory_consolidate` or `memory_forget`)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["key", "content"]
        }),
        max_result_size: Some(512),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

// ────────────────────────────────────────────────────────────────────
// memory_link
// ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[must_use]
pub fn link_entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: serde_json::Value, _ctx: SharedToolContext| {
        let memory = memory.clone();
        Box::pin(async move {
            let source = require_string(&args, "source_key")?;
            let target = require_string(&args, "target_key")?;
            let link_type = args
                .get("link_type")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to")
                .to_string();
            memory
                .create_link(&source, &target, &link_type)
                .await
                .map_err(|e| ToolError::Handler(format!("memory_link failed: {e}")))?;
            Ok(json!({
                "status": "ok",
                "source_key": source,
                "target_key": target,
                "link_type": link_type,
            })
            .to_string())
        })
    });

    ToolEntry {
        name: "memory_link".to_string(),
        toolset: "memory".to_string(),
        description: "Create a typed link `source → target` between two memories. \
                      `link_type` is free-form (defaults to `related_to`); common values: \
                      `supersedes`, `contradicts`, `evidence_for`, `consolidated_into`. \
                      Idempotent: re-linking the same triple is a no-op (UNIQUE constraint). \
                      Use to weave a memory graph the recall layer can traverse."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source_key": { "type": "string" },
                "target_key": { "type": "string" },
                "link_type":  { "type": "string", "description": "Defaults to `related_to`." }
            },
            "required": ["source_key", "target_key"]
        }),
        max_result_size: Some(256),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

// ────────────────────────────────────────────────────────────────────
// memory_consolidate
// ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[must_use]
pub fn consolidate_entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: serde_json::Value, _ctx: SharedToolContext| {
        let memory = memory.clone();
        Box::pin(async move {
            let consolidated_key = require_string(&args, "consolidated_key")?;
            let consolidated_content = require_string(&args, "consolidated_content")?;
            let source_keys: Vec<String> = args
                .get("source_keys")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if source_keys.is_empty() {
                return Err(ToolError::Handler(
                    "memory_consolidate requires at least one `source_keys` entry".into(),
                ));
            }
            let category = args
                .get("category")
                .and_then(|v| v.as_str())
                .map(|c| match c {
                    "core" => MemoryCategory::Core,
                    "daily" => MemoryCategory::Daily,
                    "conversation" => MemoryCategory::Conversation,
                    "working" => MemoryCategory::Working,
                    "procedural" => MemoryCategory::Procedural,
                    "reflection" => MemoryCategory::Reflection,
                    other => MemoryCategory::Custom(other.to_string()),
                })
                .unwrap_or(MemoryCategory::Reflection);

            let linked = memory
                .consolidate(
                    &source_keys,
                    &consolidated_key,
                    &consolidated_content,
                    category.clone(),
                )
                .await
                .map_err(|e| ToolError::Handler(format!("memory_consolidate failed: {e}")))?;

            Ok(json!({
                "status": "ok",
                "consolidated_key": consolidated_key,
                "category": category.as_str(),
                "links_created": linked,
                "source_count": source_keys.len(),
            })
            .to_string())
        })
    });

    ToolEntry {
        name: "memory_consolidate".to_string(),
        toolset: "memory".to_string(),
        description: "Merge N existing memories into a single new entry, preserving the originals \
                      and adding `consolidated_into` links from each source to the new key. \
                      Returns `{status, consolidated_key, category, links_created, source_count}`. \
                      Use to compress redundant facts; defaults to category=`reflection` because \
                      consolidations are usually meta-observations."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source_keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1
                },
                "consolidated_key":     { "type": "string" },
                "consolidated_content": { "type": "string" },
                "category":             { "type": "string", "description": "Defaults to `reflection`." }
            },
            "required": ["source_keys", "consolidated_key", "consolidated_content"]
        }),
        max_result_size: Some(512),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(15),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

// ────────────────────────────────────────────────────────────────────
// helpers
// ────────────────────────────────────────────────────────────────────

fn require_string(args: &serde_json::Value, name: &str) -> Result<String, ToolError> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| ToolError::Handler(format!("missing required parameter: {name}")))
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::modules::memory::MemoryCategory;
    use crate::modules::memory::SqliteMemoryProvider;
    use crate::modules::tools::context::ToolContext;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    fn ctx() -> SharedToolContext {
        Arc::new(Mutex::new(ToolContext::new(
            std::env::temp_dir(),
            crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
        )))
    }

    async fn make_provider() -> (SharedMemoryProvider, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let provider = Arc::new(SqliteMemoryProvider::new(dir.path().join("p3.db")).unwrap())
            as SharedMemoryProvider;
        (provider, dir)
    }

    #[tokio::test]
    async fn recall_explicit_returns_not_found_for_missing_key() {
        let (memory, _dir) = make_provider().await;
        let tool = recall_explicit_entry(memory);
        let out = (tool.handler)(json!({ "key": "ghost" }), ctx())
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["status"], "not_found");
    }

    #[tokio::test]
    async fn update_then_recall_explicit_roundtrips_new_content() {
        let (memory, _dir) = make_provider().await;
        memory
            .store("k1", "v1", MemoryCategory::Conversation)
            .await
            .unwrap();
        let upd = update_entry(memory.clone());
        let recall = recall_explicit_entry(memory);

        let r = (upd.handler)(json!({ "key": "k1", "content": "v2" }), ctx())
            .await
            .unwrap();
        assert!(r.contains("\"status\":\"ok\""));

        let read = (recall.handler)(json!({ "key": "k1" }), ctx())
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&read).unwrap();
        assert_eq!(v["content"], "v2");
    }

    #[tokio::test]
    async fn link_is_idempotent() {
        let (memory, _dir) = make_provider().await;
        memory
            .store("a", "x", MemoryCategory::Conversation)
            .await
            .unwrap();
        memory
            .store("b", "y", MemoryCategory::Conversation)
            .await
            .unwrap();
        let tool = link_entry(memory);
        for _ in 0..3 {
            let out = (tool.handler)(
                json!({ "source_key": "a", "target_key": "b", "link_type": "related_to" }),
                ctx(),
            )
            .await
            .unwrap();
            assert!(out.contains("\"status\":\"ok\""));
        }
    }

    #[tokio::test]
    async fn consolidate_creates_target_and_links_sources() {
        let (memory, _dir) = make_provider().await;
        memory
            .store("s1", "fact1", MemoryCategory::Conversation)
            .await
            .unwrap();
        memory
            .store("s2", "fact2", MemoryCategory::Conversation)
            .await
            .unwrap();
        let tool = consolidate_entry(memory.clone());
        let out = (tool.handler)(
            json!({
                "source_keys": ["s1", "s2"],
                "consolidated_key": "c1",
                "consolidated_content": "merged",
                "category": "reflection"
            }),
            ctx(),
        )
        .await
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["links_created"], 2);
        assert_eq!(v["category"], "reflection");

        // The consolidated entry exists.
        let read = recall_explicit_entry(memory);
        let r = (read.handler)(json!({ "key": "c1" }), ctx()).await.unwrap();
        assert!(r.contains("\"content\":\"merged\""));
    }

    #[tokio::test]
    async fn consolidate_rejects_empty_source_list() {
        let (memory, _dir) = make_provider().await;
        let tool = consolidate_entry(memory);
        let err = (tool.handler)(
            json!({
                "source_keys": [],
                "consolidated_key": "c1",
                "consolidated_content": "x"
            }),
            ctx(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("at least one"));
    }
}
