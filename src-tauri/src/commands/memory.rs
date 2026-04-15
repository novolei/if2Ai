//! Memory Tauri commands — expose memory provider operations to the frontend.
//!
//! Provides IPC commands for the Memory Browser UI:
//! - `memory_recall` — search and list memory entries
//! - `memory_delete` — delete a memory entry by key
//! - `memory_export` — export all entries, optionally filtered by category

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::MemoryEntry;

/// Serialisable memory entry for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntryDto {
    pub key: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
    pub importance: f64,
    pub access_count: u64,
    pub trust_score: f64,
}

fn entry_to_dto(entry: &MemoryEntry) -> MemoryEntryDto {
    MemoryEntryDto {
        key: entry.key.clone(),
        content: entry.content.clone(),
        category: entry.category.as_str().to_string(),
        created_at: entry.created_at.to_rfc3339(),
        updated_at: entry.updated_at.to_rfc3339(),
        importance: entry.importance,
        access_count: entry.access_count,
        trust_score: entry.trust_score,
    }
}

/// Search memory entries.
///
/// Returns entries matching the query, optionally filtered by category.
#[tauri::command]
pub async fn memory_recall(
    state: State<'_, AppState>,
    query: String,
    category: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<MemoryEntryDto>, String> {
    let entries = state
        .memory_provider
        .recall(&query, category.as_deref(), limit.unwrap_or(50))
        .await
        .map_err(|e| e.to_string())?;

    Ok(entries.iter().map(entry_to_dto).collect())
}

/// Delete a memory entry by key.
#[tauri::command]
pub async fn memory_delete(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state
        .memory_provider
        .delete(&key)
        .await
        .map_err(|e| e.to_string())
}

/// Export all memory entries, optionally filtered by category.
#[tauri::command]
pub async fn memory_export(
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<Vec<MemoryEntryDto>, String> {
    let entries = state
        .memory_provider
        .export(category.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    Ok(entries.iter().map(entry_to_dto).collect())
}

/// Purge all entries in a category.
#[tauri::command]
pub async fn memory_purge(state: State<'_, AppState>, category: String) -> Result<(), String> {
    state
        .memory_provider
        .purge_category(&category)
        .await
        .map_err(|e| e.to_string())
}
