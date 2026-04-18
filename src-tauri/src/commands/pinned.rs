//! Pinned-memory Tauri commands (Phase 8A.10 / T-F3).
//!
//! UI surface for the future PinnedMemoryEditor component (8A.12) — the
//! agent loop itself uses the `pin_memory` / `unpin_memory` builtin
//! tools (Phase 8A.10 / T-F2), not these commands.
//!
//! Per v2 §0.5 Δ-11 the four commands are registered three places:
//! 1. `tauri::generate_handler!` in `main.rs`.
//! 2. `core:default` in `capabilities/default.json` (auto-grants invoke
//!    access to every command in `generate_handler!`).
//! 3. `src/lib/tauri.ts` typed wrappers consumed by React.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::pinned::{PinScope, PinSource, PinnedItem};
use crate::modules::memory::scope::MemoryExecutionScope;

/// Wire-shape mirror of [`PinnedItem`] used at the Tauri IPC boundary.
///
/// Keeping a DTO (rather than re-using [`PinnedItem`] directly) ensures
/// the JSON schema seen by the React layer stays stable even if the
/// internal struct gains fields, and gives [`PinSource`] a JS-friendly
/// flat representation (`created_by_kind` + optional `tool_name` /
/// `session_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedItemDto {
    /// ULID — sortable + monotonic.
    pub id: String,
    /// Pin content (PII-scrubbed at write time).
    pub content: String,
    /// `"project"` or `"global"`.
    pub scope: String,
    /// `Some(project_id)` for project-scoped pins; `None` for global.
    pub project_id: Option<String>,
    /// RFC3339 UTC timestamp of first insertion.
    pub created_at: String,
    /// `"user"` (UI add) or `"tool"` (`pin_memory` invocation).
    pub created_by_kind: String,
    /// Originating tool name when `created_by_kind == "tool"`.
    pub tool_name: Option<String>,
    /// Originating session id when `created_by_kind == "tool"`.
    pub session_id: Option<String>,
}

impl From<PinnedItem> for PinnedItemDto {
    fn from(item: PinnedItem) -> Self {
        let (kind, tool_name, session_id) = match item.created_by {
            PinSource::User => ("user".to_string(), None, None),
            PinSource::Tool {
                tool_name,
                session_id,
            } => ("tool".to_string(), Some(tool_name), Some(session_id)),
        };
        Self {
            id: item.id,
            content: item.content,
            scope: item.scope.as_str().to_string(),
            project_id: item.project_id,
            created_at: item.created_at.to_rfc3339(),
            created_by_kind: kind,
            tool_name,
            session_id,
        }
    }
}

/// List pinned memories for the requested scope.
///
/// `scope` accepts `"project"`, `"global"`, or `"both"` — the latter
/// returns the union used by the system-prompt injector (8A.11) so the
/// PinnedMemoryEditor can preview exactly what the agent will see.
#[tauri::command]
#[allow(dead_code)]
pub async fn pinned_get(
    state: State<'_, AppState>,
    scope: String,
    project_id: Option<String>,
) -> Result<Vec<PinnedItemDto>, String> {
    let project_id_ref = project_id.as_deref();
    let items = match scope.as_str() {
        "project" => state
            .pinned_store
            .list(PinScope::Project, project_id_ref)
            .await
            .map_err(|e| e.to_string())?,
        "global" => state
            .pinned_store
            .list(PinScope::Global, None)
            .await
            .map_err(|e| e.to_string())?,
        "both" => {
            let exec = MemoryExecutionScope {
                session_id: None,
                project_id: project_id.clone(),
                workdir: None,
            };
            state
                .pinned_store
                .list_all_for_prompt(&exec)
                .await
                .map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!(
                "invalid scope {other:?}; expected 'project' | 'global' | 'both'"
            ));
        }
    };
    Ok(items.into_iter().map(PinnedItemDto::from).collect())
}

/// Add a user-initiated pin from the PinnedMemoryEditor UI.
#[tauri::command]
#[allow(dead_code)]
pub async fn pinned_add(
    state: State<'_, AppState>,
    content: String,
    scope: String,
    project_id: Option<String>,
) -> Result<PinnedItemDto, String> {
    let pin_scope = match scope.as_str() {
        "project" => PinScope::Project,
        "global" => PinScope::Global,
        other => {
            return Err(format!(
                "invalid scope {other:?}; expected 'project' | 'global'"
            ));
        }
    };
    let project_id_ref = match pin_scope {
        PinScope::Project => project_id.as_deref(),
        PinScope::Global => None,
    };
    state
        .pinned_store
        .add(&content, pin_scope, PinSource::User, project_id_ref)
        .await
        .map(PinnedItemDto::from)
        .map_err(|e| e.to_string())
}

/// Delete a pin by id (idempotent — returns `false` when the id was not
/// present).
#[tauri::command]
#[allow(dead_code)]
pub async fn pinned_delete(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    state
        .pinned_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Re-stamp `created_at` for each id in `ids` so the natural
/// `ORDER BY created_at ASC` matches the supplied order.  Ids not in
/// the store are silently skipped.
#[tauri::command]
#[allow(dead_code)]
pub async fn pinned_reorder(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    state
        .pinned_store
        .reorder(&ids)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::pinned::SqlitePinnedStore;
    use std::sync::Arc;

    /// Smoke test for the DTO conversion + the underlying store wiring
    /// the four Tauri commands all delegate to.  We exercise the store
    /// directly (rather than spinning up a fake `AppState` / `tauri::test`
    /// harness) because the commands are thin pass-throughs and the
    /// store layer is already covered by 8A.9's 15 store-side tests.
    #[tokio::test]
    async fn pinned_get_returns_added_items_via_dto_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("memory.db");
        let store =
            SqlitePinnedStore::open(&db_path, dir.path().to_path_buf(), None).expect("open store");
        let store: Arc<dyn crate::modules::memory::pinned::PinnedStore> = Arc::new(store);

        let item = store
            .add("hello", PinScope::Project, PinSource::User, None)
            .await
            .expect("add");
        let dto: PinnedItemDto = item.into();
        assert_eq!(dto.scope, "project");
        assert_eq!(dto.created_by_kind, "user");
        assert_eq!(dto.content, "hello");

        let listed = store.list(PinScope::Project, None).await.expect("list");
        let dtos: Vec<PinnedItemDto> = listed.into_iter().map(PinnedItemDto::from).collect();
        assert_eq!(dtos.len(), 1);
        assert_eq!(dtos[0].id, dto.id);
    }
}
