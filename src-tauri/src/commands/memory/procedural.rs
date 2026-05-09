//! A.4 — list procedural memory entries for the Memory Settings UI.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::evolution::procedural::extract_category_label;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProceduralEntryDto {
    /// The full storage key (e.g. `proc:heuristic:abc12345`).
    pub key: String,
    /// Human-readable category label (`HeuristicRule`, `AntiPattern`, ...).
    /// Derived from the key.
    pub category: String,
    /// Rule text body.
    pub content: String,
    /// Trust score in `[0, 1]`. Bumped each time the same procedure is
    /// re-encountered; doubles as a confidence proxy in the UI.
    pub trust_score: f64,
    /// Number of times the agent has acted on this procedure (or had it
    /// recalled in a turn). 0 for entries that have never been hit since
    /// internalization.
    pub access_count: u64,
    /// RFC-3339 string. Latest internalization or re-encounter.
    pub updated_at: String,
}

/// List all procedural memory entries (category = `Procedural`).
/// Newest-first.
#[tauri::command]
pub async fn procedural_memory_list(
    state: State<'_, AppState>,
) -> Result<Vec<ProceduralEntryDto>, String> {
    let entries = state
        .memory_provider
        .export(Some("procedural"))
        .await
        .map_err(|e| format!("export failed: {e}"))?;

    let mut dtos: Vec<ProceduralEntryDto> = entries
        .into_iter()
        .map(|e| ProceduralEntryDto {
            category: extract_category_label(&e.key).to_string(),
            key: e.key,
            content: e.content,
            trust_score: e.trust_score,
            access_count: e.access_count,
            updated_at: e.updated_at.to_rfc3339(),
        })
        .collect();
    dtos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(dtos)
}
