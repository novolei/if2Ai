//! Memory Tauri commands — expose memory provider operations to the frontend.
//!
//! ## Module layout (Memory Audit P3 god-file split)
//!
//! Originally a single 1005-LOC file; split into three sibling modules
//! that each own one IPC surface group:
//!
//! - this `mod.rs` — shared DTOs (`MemoryEntryDto`,
//!   `MemoryPromotionCandidateDto`) + scope helpers + the read /
//!   write / lifecycle IPCs
//! - [`compile`] — `memory_compile_now` / `memory_compiled_read` /
//!   `memory_compiled_clear` plus their DTOs
//! - [`narrative`] — `memory_summaries_list` plus `SessionSummaryDto`
//!
//! Public API is unchanged — every command is re-exported from this
//! `mod.rs` so `commands/mod.rs::pub use memory::{...}` keeps working
//! and `tauri::generate_handler!` continues to resolve every name at
//! the same `crate::commands::*` path.

pub mod compile;
pub mod daydream;
pub mod narrative;

pub use compile::{
    memory_compile_now, memory_compiled_clear, memory_compiled_read, CompileReport,
    CompiledMemoryDto, CompiledSection,
};
pub use daydream::{daydream_get_config, daydream_run_cycle, daydream_set_config};
pub use narrative::{memory_summaries_list, SessionSummaryDto};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::promotion::{
    target_scope_for, MemoryPromotionEngine, PromotionRecommendation, ScopeTier,
};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::security::ScrubResult;
use crate::modules::memory::MemoryEntry;

/// Phase 8A — thin wrapper that delegates to
/// [`crate::modules::memory::security::ThreatScanner::scan_and_redact`] on
/// the shared `state.threat_scanner` and emits the
/// `memory_pii_redacted` audit event when hits are found.
///
/// This is **not** a parallel scrub implementation (per v2 §0.5 Δ-2 the
/// scrub logic lives entirely on [`ThreatScanner`]); it only centralises
/// the audit-emission boilerplate so every Tauri command that ingests
/// user-supplied content emits the same payload shape.
///
/// Callers must persist `result.cleaned`, never the original content.
#[allow(dead_code)] // consumed by Phase 8A.9 + 8A.10 commands; wired here so the
                    // helper review-checks the shared signature in this slice.
pub(crate) fn scan_and_emit_pii_audit(
    state: &AppState,
    scope: &MemoryExecutionScope,
    key: &str,
    content: &str,
) -> ScrubResult {
    let result = state.threat_scanner.scan_and_redact(key, content);
    if result.flagged {
        let ctx = AuditContext::from_scope(scope);
        MemoryAuditEmitter::memory_pii_redacted(&ctx, key, &result.detected);
    }
    result
}

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

/// MEM-MOD-P6 — One historical snapshot of a memory entry returned
/// by [`memory_history`] to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHistoryEntryDto {
    pub key: String,
    pub content: String,
    pub category: String,
    pub importance: f64,
    pub trust_score: f64,
    pub valid_from: String,
    pub valid_to: String,
    pub source: String,
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

/// MEM-MOD-P6 — Return the temporal history of a memory key
/// (newest snapshot first).  Each item describes a value the entry
/// held BEFORE the snapshot's `valid_to` timestamp.  Empty when the
/// key has never been updated / consolidated.
#[tauri::command]
pub async fn memory_history(
    state: State<'_, AppState>,
    key: String,
) -> Result<Vec<MemoryHistoryEntryDto>, String> {
    let entries = state
        .memory_provider
        .list_history(&key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|h| MemoryHistoryEntryDto {
            key: h.key,
            content: h.content,
            category: h.category,
            importance: h.importance,
            trust_score: h.trust_score,
            valid_from: h.valid_from.to_rfc3339(),
            valid_to: h.valid_to.to_rfc3339(),
            source: h.source,
        })
        .collect())
}

/// MEM-MOD-P7 — DTO returned by `learned_traits_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedTraitDto {
    pub id: i64,
    pub trait_text: String,
    pub evidence_count: i64,
    pub confidence: f64,
    pub first_seen_at: String,
    pub last_updated_at: String,
    pub source_session: Option<String>,
}

/// MEM-MOD-P7 — list active (non-disagreed) cross-session traits the
/// agent has accumulated about the user.  Newest first.  `limit`
/// defaults to 50.
#[tauri::command]
pub async fn learned_traits_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<LearnedTraitDto>, String> {
    let store = match &state.learned_traits {
        Some(s) => s.clone(),
        None => return Ok(Vec::new()),
    };
    let traits = tokio::task::spawn_blocking(move || store.list_active(limit.unwrap_or(50)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(traits
        .into_iter()
        .map(|t| LearnedTraitDto {
            id: t.id,
            trait_text: t.trait_text,
            evidence_count: t.evidence_count,
            confidence: t.confidence,
            first_seen_at: t.first_seen_at.to_rfc3339(),
            last_updated_at: t.last_updated_at.to_rfc3339(),
            source_session: t.source_session,
        })
        .collect())
}

/// MEM-MOD-P7 — user-facing "I don't agree" button. Marks the trait
/// as retired so it stops appearing in the prompt block.  The row is
/// kept for audit; idempotent re-disagree on an already-disagreed row
/// returns `KeyNotFound` (UI can ignore).
#[tauri::command]
pub async fn learned_traits_disagree(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let store = match &state.learned_traits {
        Some(s) => s.clone(),
        None => return Err("learned_traits store not initialised".into()),
    };
    tokio::task::spawn_blocking(move || store.disagree(id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a memory entry by key.
#[tauri::command]
pub async fn memory_delete(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state
        .memory_provider
        .delete(&key)
        .await
        .map_err(|e| e.to_string())?;
    // Memory Audit P1 #5 — broadcast invalidation so any open frontend
    // view (Browser, Narrative, etc.) refetches without polling.
    let audit_ctx = AuditContext {
        trace_id: None,
        session_id: None,
        project_id: None,
        effective_workdir: None,
    };
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "entries");
    Ok(())
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
        .map_err(|e| e.to_string())?;
    let audit_ctx = AuditContext {
        trace_id: None,
        session_id: None,
        project_id: None,
        effective_workdir: None,
    };
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "entries");
    Ok(())
}

/// Wipe **every** memory entry across all categories and scopes.
///
/// Backs the "Clear all memories" affordance in the Memory Settings page.
/// Returns the number of rows removed (best effort — providers without a
/// fast bulk delete report the iteration count from the trait fallback).
///
/// The frontend is expected to present a confirmation dialog before
/// invoking this; the backend deliberately performs no extra
/// confirmation so it remains scriptable from harness tooling.  A
/// `memory_cleared` audit event is always emitted, even when zero rows
/// were removed, so operators can correlate the action in the Telemetry
/// Drawer.
#[tauri::command]
pub async fn memory_clear_all(state: State<'_, AppState>) -> Result<usize, String> {
    let removed = state
        .memory_provider
        .clear_all()
        .await
        .map_err(|e| e.to_string())?;
    let audit_ctx = AuditContext {
        trace_id: None,
        session_id: None,
        project_id: None,
        effective_workdir: None,
    };
    MemoryAuditEmitter::memory_cleared(&audit_ctx, removed);
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "all");
    Ok(removed)
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
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "entries");

    Ok(())
}

/// Reverse of [`memory_promote`] — narrow an entry's visibility back down.
///
/// `target_scope_kind` accepts `"project"` or `"session"`.  When demoting to
/// `project`, `project_id` is required; when demoting to `session`, both
/// `session_id` and `project_id` (the entry's owning project) are required so
/// the entry doesn't disappear from the user's current view.
///
/// Emits a `memory_demoted` audit event so the Telemetry Drawer can render
/// the round-trip.  Refuses to demote into a higher tier (e.g. `session →
/// project` request) — call [`memory_promote`] for that direction.
#[tauri::command]
pub async fn memory_demote(
    state: State<'_, AppState>,
    key: String,
    target_scope_kind: MemoryScopeKind,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    let target_tier = match target_scope_kind {
        MemoryScopeKind::Session => ScopeTier::Session,
        MemoryScopeKind::Project => ScopeTier::Project,
        MemoryScopeKind::Global => {
            return Err("cannot demote into global scope (global is the top tier)".to_string());
        }
    };

    if matches!(target_tier, ScopeTier::Project) && project_id.is_none() {
        return Err("project_id is required when demoting to project scope".to_string());
    }
    if matches!(target_tier, ScopeTier::Session) && session_id.is_none() {
        return Err("session_id is required when demoting to session scope".to_string());
    }

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

    // Reject same-or-upward "demotion" calls so the UI cannot accidentally
    // launder a promote through this command.
    let from_rank = scope_tier_rank(from_tier);
    let to_rank = scope_tier_rank(target_tier);
    if to_rank <= from_rank {
        return Err(format!(
            "demote_scope refused: {from} is already at or below {to}",
            from = from_tier.label(),
            to = target_tier.label(),
        ));
    }

    let target_scope = MemoryExecutionScope {
        session_id: session_id.clone(),
        project_id: project_id.clone().or_else(|| entry.project_id.clone()),
        workdir: None,
    };

    state
        .memory_provider
        .demote_scope(&key, &target_scope)
        .await
        .map_err(|e| e.to_string())?;

    let audit_ctx = AuditContext::from_scope(&target_scope);
    MemoryAuditEmitter::memory_demoted(&audit_ctx, &key, from_tier.label(), target_tier.label());
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "entries");

    Ok(())
}

/// Tier ordering for [`memory_demote`] — lower rank = broader visibility.
/// Global is the most-visible tier (rank 0); session is the most-restricted
/// (rank 2).  Demote requests must move from a lower rank to a higher rank.
fn scope_tier_rank(tier: ScopeTier) -> u8 {
    match tier {
        ScopeTier::Global => 0,
        ScopeTier::Project => 1,
        ScopeTier::Session => 2,
    }
}
