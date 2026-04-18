//! `unpin_memory` builtin tool (Phase 8A.10 / T-F2).
//!
//! Removes pins matching `keyword` (case-insensitive substring match
//! against `content`) from BOTH the project and global scopes.  Returns
//! the count of removed entries; zero matches is `Ok(...)`, NOT an
//! error, because "the user told me to unpin X but no X exists" is a
//! perfectly valid no-op the agent must surface to the user instead of
//! retrying.
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-F2 + §0.5 Δ-11 + Δ-14 for the full design.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::pinned::{PinScope, PinnedStore};
use crate::modules::memory::scope::MemoryScopeResolver;
use crate::modules::memory::MemoryExecutionScope;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Wire name of the tool exposed to the LLM.
pub const TOOL_NAME: &str = "unpin_memory";

/// Build the `unpin_memory` [`ToolEntry`].
#[must_use]
pub fn entry(pinned: Arc<dyn PinnedStore>) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: Value, context| {
        let pinned = pinned.clone();
        Box::pin(async move {
            let keyword = args
                .get("keyword")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("[unpin_memory] missing 'keyword' (string)".to_string())
                })?
                .trim()
                .to_string();
            if keyword.is_empty() {
                return Err(ToolError::Handler(
                    "[unpin_memory] 'keyword' must be non-empty".to_string(),
                ));
            }
            let needle_lc = keyword.to_lowercase();

            let exec_scope = context
                .lock()
                .ok()
                .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                .unwrap_or_else(MemoryExecutionScope::global);
            let project_id_opt = exec_scope.project_id.clone();

            // Search both tiers; substring match on lowercased content.
            // Track per-tier removed counts so the audit chip surfaces
            // the most precise scope label.
            let mut removed_project = 0usize;
            let mut removed_global = 0usize;
            for (s, pid) in [
                (PinScope::Project, project_id_opt.as_deref()),
                (PinScope::Global, None),
            ] {
                let list = pinned
                    .list(s, pid)
                    .await
                    .map_err(|e| ToolError::Handler(format!("[unpin_memory] list failed: {e}")))?;
                for item in list {
                    if item.content.to_lowercase().contains(&needle_lc) {
                        match pinned.delete(&item.id).await {
                            Ok(true) => match s {
                                PinScope::Project => removed_project += 1,
                                PinScope::Global => removed_global += 1,
                            },
                            Ok(false) => {}
                            Err(e) => {
                                tracing::warn!(
                                    id = %item.id,
                                    error = %e,
                                    "[unpin_memory] delete failed; continuing"
                                );
                            }
                        }
                    }
                }
            }
            let removed = removed_project + removed_global;

            // Single audit summary so the TelemetryDrawer renders one
            // "已取消置顶" event per user-visible action.  Per v2 §0.6
            // the audit reflects the user-visible action (one unpin
            // call), not each row delete.
            let scope_label: &'static str = match (removed_project, removed_global) {
                (0, 0) => "both",
                (_, 0) => "project",
                (0, _) => "global",
                (_, _) => "both",
            };
            let audit_ctx = AuditContext::from_scope(&exec_scope);
            MemoryAuditEmitter::memory_unpinned(&audit_ctx, scope_label, removed, &keyword);

            let result = if removed == 0 {
                json!({
                    "status": "not_found",
                    "removed": 0,
                    "message": format!("未找到匹配的置顶记忆 (keyword=\"{keyword}\")"),
                })
            } else {
                json!({
                    "status": "unpinned",
                    "removed": removed,
                    "removed_project": removed_project,
                    "removed_global": removed_global,
                    "message": format!("已取消置顶 {removed} 条 (keyword=\"{keyword}\")"),
                })
            };
            Ok(result.to_string())
        })
    });

    ToolEntry {
        name: TOOL_NAME.to_string(),
        toolset: "memory".to_string(),
        description: "Remove pinned memories matching `keyword` \
                      (case-insensitive substring match against pin content; \
                      searches both project and global scopes). \
                      Returns JSON with `status` (`unpinned` or `not_found`) \
                      and `removed` count.  Zero matches is NOT an error — \
                      surface the `not_found` status to the user and continue."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "keyword": {
                    "type": "string",
                    "description": "Substring to match against pin content (case-insensitive)"
                }
            },
            "required": ["keyword"]
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
    use crate::modules::memory::pinned::{PinSource, SqlitePinnedStore};
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
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

    fn empty_context() -> SharedToolContext {
        Arc::new(Mutex::new(ToolContext::default_for_workdir(PathBuf::from(
            ".",
        ))))
    }

    #[tokio::test]
    async fn unpin_memory_substring_match_case_insensitive() {
        let (store, _g) = test_store();
        store
            .add("FooBar baseline", PinScope::Project, PinSource::User, None)
            .await
            .expect("add");
        let e = entry(store.clone());
        let out = (e.handler)(json!({"keyword": "foo"}), empty_context())
            .await
            .expect("ok");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("unpinned"));
        assert_eq!(v.get("removed").and_then(|x| x.as_u64()), Some(1));
        let remaining = store.list(PinScope::Project, None).await.expect("list");
        assert!(remaining.is_empty(), "FooBar should have been removed");
    }

    #[tokio::test]
    async fn unpin_memory_zero_matches_returns_status_not_found() {
        let (store, _g) = test_store();
        store
            .add("alpha", PinScope::Project, PinSource::User, None)
            .await
            .expect("add");
        let e = entry(store);
        let out = (e.handler)(json!({"keyword": "nope"}), empty_context())
            .await
            .expect("ok (zero matches must not error)");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("not_found"));
        assert_eq!(v.get("removed").and_then(|x| x.as_u64()), Some(0));
    }

    #[tokio::test]
    async fn unpin_memory_searches_both_scopes() {
        let (store, _g) = test_store();
        store
            .add(
                "common-needle (global)",
                PinScope::Global,
                PinSource::User,
                None,
            )
            .await
            .expect("add global");
        store
            .add(
                "common-needle (project)",
                PinScope::Project,
                PinSource::User,
                None,
            )
            .await
            .expect("add project");
        let e = entry(store.clone());
        let out = (e.handler)(json!({"keyword": "common-needle"}), empty_context())
            .await
            .expect("ok");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v.get("removed").and_then(|x| x.as_u64()), Some(2));
        assert_eq!(
            store
                .list(PinScope::Global, None)
                .await
                .expect("list")
                .len(),
            0,
            "global pin should be gone"
        );
        assert_eq!(
            store
                .list(PinScope::Project, None)
                .await
                .expect("list")
                .len(),
            0,
            "project pin should be gone"
        );
    }
}
