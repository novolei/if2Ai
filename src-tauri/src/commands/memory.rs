//! Memory Tauri commands — expose memory provider operations to the frontend.
//!
//! Provides IPC commands for the Memory Browser UI:
//! - `memory_recall` — search and list memory entries
//! - `memory_delete` — delete a memory entry by key
//! - `memory_export` — export all entries, optionally filtered by category

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::promotion::{
    target_scope_for, MemoryPromotionEngine, PromotionRecommendation, ScopeTier,
};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryEntry;

/// Serialisable memory entry for the frontend.
///
/// Includes the persisted `session_id` / `project_id` scope tags so the UI can
/// distinguish global / project / session entries in scoped views.
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
    pub session_id: Option<String>,
    pub project_id: Option<String>,
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
        session_id: entry.session_id.clone(),
        project_id: entry.project_id.clone(),
    }
}

/// Optional scope kind requested by the frontend.
///
/// - `Global`  → only globally-visible entries (no session, no project tag).
/// - `Project` → entries belonging to `project_id` plus globals; never session
///   entries from any session.
/// - `Session` → entries belonging to `session_id` (tied to its project) plus
///   that project's project-level entries plus globals.
///
/// When the request omits `scope_kind`, all command paths fall back to the
/// legacy unscoped view (every entry in the shared library) for backward
/// compatibility with existing callers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScopeKind {
    Global,
    Project,
    Session,
}

/// Build a `MemoryExecutionScope` from optional Tauri command inputs.
///
/// Returns `Some(scope)` only when the caller explicitly requested a scope and
/// supplied the matching identifier(s).  Missing identifiers degrade
/// gracefully (e.g. `Session` without a `session_id` falls back to the
/// `project_id` if supplied, otherwise `global`) so partially-configured
/// frontends never panic the backend.
fn build_scope(
    scope_kind: Option<MemoryScopeKind>,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Option<MemoryExecutionScope> {
    let kind = scope_kind?;
    let scope = match kind {
        MemoryScopeKind::Global => MemoryExecutionScope::global(),
        MemoryScopeKind::Project => MemoryExecutionScope {
            session_id: None,
            project_id,
            workdir: None,
        },
        MemoryScopeKind::Session => MemoryExecutionScope {
            session_id,
            project_id,
            workdir: None,
        },
    };
    Some(scope)
}

/// Search memory entries.
///
/// Returns entries matching the query, optionally filtered by category.
///
/// When `scope_kind` is provided the search is restricted to the matching
/// three-tier scope (see [`MemoryScopeKind`]).  When omitted, the legacy
/// unscoped recall is used so existing callers (Memory Browser without a
/// scope selector) keep their current behaviour.
#[tauri::command]
pub async fn memory_recall(
    state: State<'_, AppState>,
    query: String,
    category: Option<String>,
    limit: Option<usize>,
    scope_kind: Option<MemoryScopeKind>,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Result<Vec<MemoryEntryDto>, String> {
    let limit = limit.unwrap_or(50);
    let scope = build_scope(scope_kind, session_id, project_id);
    let entries = match scope {
        Some(scope) => {
            state
                .memory_provider
                .recall_scoped(&query, category.as_deref(), limit, &scope)
                .await
        }
        None => {
            state
                .memory_provider
                .recall(&query, category.as_deref(), limit)
                .await
        }
    }
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

/// Export all memory entries, optionally filtered by category and scope.
///
/// When `scope_kind` is provided the export is pushed down to
/// [`MemoryProvider::export_scoped`], which applies the three-tier visibility
/// rules at the storage layer (SQL `WHERE` clause for SQLite).  When omitted,
/// the legacy unscoped export is used for backward compatibility.
#[tauri::command]
pub async fn memory_export(
    state: State<'_, AppState>,
    category: Option<String>,
    scope_kind: Option<MemoryScopeKind>,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Result<Vec<MemoryEntryDto>, String> {
    let scope = build_scope(scope_kind, session_id, project_id);
    let entries = match scope {
        Some(scope) => {
            state
                .memory_provider
                .export_scoped(category.as_deref(), &scope)
                .await
        }
        None => state.memory_provider.export(category.as_deref()).await,
    }
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

/// Frontend-facing DTO for a promotion recommendation.
///
/// Mirrors [`PromotionRecommendation`] one-to-one so the Memory Browser can
/// surface candidates without translating between Rust and JSON conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPromotionCandidateDto {
    pub key: String,
    pub category: String,
    pub current_tier: String,
    pub target_tier: String,
    pub access_count: u64,
    pub importance: f64,
    pub reason: String,
}

fn rec_to_dto(rec: &PromotionRecommendation) -> MemoryPromotionCandidateDto {
    MemoryPromotionCandidateDto {
        key: rec.key.clone(),
        category: rec.category.clone(),
        current_tier: rec.current_tier.label().to_string(),
        target_tier: rec.target_tier.label().to_string(),
        access_count: rec.access_count,
        importance: rec.importance,
        reason: rec.reason.clone(),
    }
}

/// Scan the memory library for entries that meet the
/// `session → project` / `project → global` promotion thresholds.
///
/// The recommendation is **non-destructive**: it returns suggestions ordered
/// by descending importance.  The Memory Browser shows them in a "promotion"
/// drawer; the user opts in to apply by calling [`memory_promote`].
#[tauri::command]
pub async fn memory_promotion_candidates(
    state: State<'_, AppState>,
) -> Result<Vec<MemoryPromotionCandidateDto>, String> {
    let engine = MemoryPromotionEngine::new(state.memory_provider.as_ref());
    let recs = engine.evaluate_all().await.map_err(|e| e.to_string())?;
    Ok(recs.iter().map(rec_to_dto).collect())
}

/// Apply a single promotion to the entry identified by `key`.
///
/// `target_scope_kind` accepts `"project"` or `"global"`.  When promoting to
/// `project`, `project_id` is required so the target scope is well-defined.
/// Emits a `memory_promoted` audit event mirroring the storage write so
/// downstream observers (Telemetry Drawer, log scrapers) see the transition.
#[tauri::command]
pub async fn memory_promote(
    state: State<'_, AppState>,
    key: String,
    target_scope_kind: MemoryScopeKind,
    project_id: Option<String>,
) -> Result<(), String> {
    let target_tier = match target_scope_kind {
        MemoryScopeKind::Project => ScopeTier::Project,
        MemoryScopeKind::Global => ScopeTier::Global,
        MemoryScopeKind::Session => {
            // Session is the lowest tier; promoting *into* session is a
            // demotion and the engine never produces such recommendations.
            return Err("cannot promote into session scope (session is the leaf tier)".to_string());
        }
    };

    if matches!(target_tier, ScopeTier::Project) && project_id.is_none() {
        return Err("project_id is required when promoting to project scope".to_string());
    }

    // Resolve the entry's current state so the audit event can record the
    // before-tier accurately.  We re-read via export to avoid widening the
    // `MemoryProvider` trait with a "get one entry" method.
    let entries = state
        .memory_provider
        .export(None)
        .await
        .map_err(|e| e.to_string())?;
    let entry = entries
        .iter()
        .find(|e| e.key == key)
        .ok_or_else(|| format!("memory key not found: {key}"))?;

    let from_tier = ScopeTier::from_entry(entry);
    let target_scope = target_scope_for(target_tier, project_id.as_deref());

    state
        .memory_provider
        .promote_scope(&key, &target_scope)
        .await
        .map_err(|e| e.to_string())?;

    // Audit + event emission.  We thread the entry's *new* scope into the
    // audit context so listeners see "where the entry now lives".
    let audit_ctx = AuditContext::from_scope(&target_scope);
    MemoryAuditEmitter::memory_promoted(&audit_ctx, &key, from_tier.label(), target_tier.label());

    Ok(())
}
