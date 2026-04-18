//! `pin_memory` builtin tool (Phase 8A.10 / T-F2).
//!
//! Lets the agent persist a fact that survives every future system-prompt
//! injection (8A.11), so users do not need to re-tell the agent
//! foundational context (their name, project conventions, language
//! preference, etc.).
//!
//! The handler is intentionally `Auto` (no permission gate): pinning a
//! memory is a low-risk operation already gated by [`ThreatScanner`]
//! PII scrubbing inside [`PinnedStore::add`].  Forcing a user
//! confirmation here would slow down the "remember X" reflex without
//! adding safety.
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-F2 + §0.5 Δ-11 + Δ-14 for the full design.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::modules::memory::pinned::{PinScope, PinSource, PinnedStore, MAX_PIN_CONTENT_CHARS};
use crate::modules::memory::scope::MemoryScopeResolver;
use crate::modules::memory::MemoryExecutionScope;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Wire name of the tool exposed to the LLM.
pub const TOOL_NAME: &str = "pin_memory";

/// Build the `pin_memory` [`ToolEntry`].
///
/// `pinned` is the shared [`PinnedStore`] held on `AppState` so the same
/// SQLite connection + sidecar are used by every tool invocation across
/// every session.
#[must_use]
pub fn entry(pinned: Arc<dyn PinnedStore>) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: Value, context| {
        let pinned = pinned.clone();
        Box::pin(async move {
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("[pin_memory] missing 'content' (string)".to_string())
                })?
                .trim()
                .to_string();
            if content.is_empty() {
                return Err(ToolError::Handler(
                    "[pin_memory] 'content' must be non-empty".to_string(),
                ));
            }
            if content.chars().count() > MAX_PIN_CONTENT_CHARS {
                return Err(ToolError::Handler(format!(
                    "[pin_memory] 'content' exceeds {MAX_PIN_CONTENT_CHARS} chars"
                )));
            }

            let scope_str = args
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("project");
            let scope = match scope_str {
                "project" => PinScope::Project,
                "global" => PinScope::Global,
                other => {
                    return Err(ToolError::Handler(format!(
                        "[pin_memory] unknown scope {other:?}; expected 'project' or 'global'"
                    )));
                }
            };

            // Resolve project / session from the tool execution context so
            // a project-scoped pin lands in the right `(scope, project_id)`
            // bucket and the [`PinSource::Tool`] payload carries the
            // originating session for the TelemetryDrawer "agent-pinned
            // this turn" badge.
            let exec_scope = context
                .lock()
                .ok()
                .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                .unwrap_or_else(MemoryExecutionScope::global);
            let project_id_opt = match scope {
                PinScope::Project => exec_scope.project_id.clone(),
                PinScope::Global => None,
            };
            let session_id = exec_scope
                .session_id
                .clone()
                .unwrap_or_else(|| "-".to_string());

            let item = pinned
                .add(
                    &content,
                    scope,
                    PinSource::Tool {
                        tool_name: TOOL_NAME.to_string(),
                        session_id,
                    },
                    project_id_opt.as_deref(),
                )
                .await
                .map_err(|e| ToolError::Handler(format!("[pin_memory] add failed: {e}")))?;

            let total = pinned
                .list(scope, project_id_opt.as_deref())
                .await
                .map(|v| v.len())
                .unwrap_or(0);

            let result = json!({
                "status": "pinned",
                "id": item.id,
                "scope": scope.as_str(),
                "content": item.content,
                "total_pins": total,
                "message": format!("已置顶: {} (当前 {} 条)", item.content, total),
            });
            Ok(result.to_string())
        })
    });

    ToolEntry {
        name: TOOL_NAME.to_string(),
        toolset: "memory".to_string(),
        description: "Pin a fact so the agent remembers it across every \
                      future turn (always injected into the system prompt). \
                      Returns JSON with `status='pinned'`, `id`, `scope`, \
                      `content` (PII-scrubbed), and `total_pins`. \
                      Use sparingly — there is a 50-pin/scope cap and pins \
                      consume system-prompt budget on every request."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The fact to pin (max 500 chars)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "global"],
                    "default": "project",
                    "description": "Visibility tier: project (default) or global"
                }
            },
            "required": ["content"]
        }),
        max_result_size: Some(2048),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::pinned::SqlitePinnedStore;
    use crate::modules::tools::context::ToolContext;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn test_store() -> (Arc<dyn PinnedStore>, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("memory.db");
        let store =
            SqlitePinnedStore::open(&db_path, dir.path().to_path_buf(), None).expect("open store");
        (Arc::new(store), dir)
    }

    fn empty_context() -> crate::modules::tools::context::SharedToolContext {
        Arc::new(Mutex::new(ToolContext::default_for_workdir(PathBuf::from(
            ".",
        ))))
    }

    #[tokio::test]
    async fn tool_entry_has_correct_name_and_schema() {
        let (store, _g) = test_store();
        let e = entry(store);
        assert_eq!(e.name, "pin_memory");
        assert_eq!(e.toolset, "memory");
        assert!(!e.disabled);
        let props = e.input_schema.get("properties").expect("properties");
        assert!(props.get("content").is_some(), "content schema missing");
        assert!(props.get("scope").is_some(), "scope schema missing");
        let required = e.input_schema.get("required").and_then(|v| v.as_array());
        let required = required.expect("required array");
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
    }

    #[tokio::test]
    async fn pin_memory_persists_to_store() {
        let (store, _g) = test_store();
        let e = entry(store.clone());
        let args = json!({"content": "我叫 Ryan"});
        let out = (e.handler)(args, empty_context()).await.expect("ok");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("pinned"));
        assert_eq!(v.get("scope").and_then(|x| x.as_str()), Some("project"));
        let pins = store.list(PinScope::Project, None).await.expect("list");
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].content, "我叫 Ryan");
    }

    #[tokio::test]
    async fn pin_memory_rejects_too_long_content() {
        let (store, _g) = test_store();
        let e = entry(store);
        let long = "x".repeat(MAX_PIN_CONTENT_CHARS + 1);
        let args = json!({"content": long});
        let err = (e.handler)(args, empty_context()).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("exceeds"),
            "expected length-guard message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn pin_memory_rejects_invalid_scope() {
        let (store, _g) = test_store();
        let e = entry(store);
        let args = json!({"content": "x", "scope": "bogus"});
        let err = (e.handler)(args, empty_context()).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("unknown scope"),
            "expected scope-validation message, got: {msg}"
        );
    }
}
