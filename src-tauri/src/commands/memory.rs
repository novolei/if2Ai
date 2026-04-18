//! Memory Tauri commands — expose memory provider operations to the frontend.
//!
//! Provides IPC commands for the Memory Browser UI:
//! - `memory_recall` — search and list memory entries
//! - `memory_delete` — delete a memory entry by key
//! - `memory_export` — export all entries, optionally filtered by category

use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::fingerprint_path;
use crate::modules::memory::compiler::{CompilePaths, CompileResult};
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

// ─── Phase 8B.5 / T-C5 — memory compile commands ────────────────────────────
//
// Three Tauri commands surface the [`MemoryCompiler`] pipeline (8B.1–8B.4)
// to the frontend so the CompiledMemoryViewer (8B.10) and operator
// scripting can manually trigger / inspect / clear the compiled memory
// without waiting for the daily ticker (8B.6+).
//
// Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
// §0.5 Δ-11 every command must be registered in three places: this file
// (function definition), `commands/mod.rs` (re-export) and `main.rs`
// (`tauri::generate_handler!`).  Capabilities live in
// `src-tauri/capabilities/default.json` (description bump only — the
// `core:default` permission already grants invoke access to every
// `#[tauri::command]` registered through the handler macro).

/// One section of [`CompiledMemoryDto`].
///
/// Sources its `last_compiled_at` from the on-disk file's modification
/// time — this lets the UI render "compiled 5 minutes ago" without the
/// backend having to thread a separate timestamp store.
#[derive(Debug, Clone, Serialize)]
pub struct CompiledSection {
    /// Raw markdown contents of the section file.  Empty string when
    /// the file is missing (first run before any compile).
    pub content: String,
    /// File modification time (UTC).  `None` when the file is missing
    /// or the platform refuses to surface mtime.
    pub last_compiled_at: Option<DateTime<Utc>>,
    /// Character count of `content` (Unicode scalar values, not bytes).
    pub chars: usize,
}

impl CompiledSection {
    /// Read `path` and build the DTO.  Missing / unreadable files are
    /// reported as empty content with `last_compiled_at = None`; this
    /// keeps the contract compatible with the "first-run, never
    /// compiled" state which is normal, not an error.
    fn from_path(path: &Path) -> Self {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let chars = content.chars().count();
        let last_compiled_at = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| {
                let secs = i64::try_from(d.as_secs()).ok()?;
                DateTime::<Utc>::from_timestamp(secs, d.subsec_nanos())
            });
        Self {
            content,
            last_compiled_at,
            chars,
        }
    }
}

/// Outcome of a single [`memory_compile_now`] invocation.
///
/// Carries the four `CompileResult` per-section verdicts plus the
/// overall `assembled` flag (true when the final `memory.md`
/// concatenation succeeded) and `elapsed_ms` for the UI to render a
/// "compiled in 1.2 s" hint.
#[derive(Debug, Clone, Serialize)]
pub struct CompileReport {
    /// Verdict for `today.md`.
    pub today: CompileResult,
    /// Verdict for `week.md`.
    pub week: CompileResult,
    /// Verdict for `longterm.md` (Skipped when `week.md` missing).
    pub longterm: CompileResult,
    /// Verdict for `facts.md`.
    pub facts: CompileResult,
    /// `true` when the four section files were assembled into
    /// `memory.md`; `false` when assemble itself failed (the four
    /// per-section results above still reflect their individual
    /// verdicts).
    pub assembled: bool,
    /// End-to-end wall time in milliseconds.
    pub elapsed_ms: u64,
}

/// Returned by [`memory_compiled_read`] — a snapshot of every compiled
/// `*.md` for the requested scope, plus the assembled `memory.md`.
#[derive(Debug, Clone, Serialize)]
pub struct CompiledMemoryDto {
    /// The assembled top-level `memory.md` body (4 sections joined).
    pub memory_md: String,
    /// `today.md` section.
    pub today: CompiledSection,
    /// `week.md` section.
    pub week: CompiledSection,
    /// `longterm.md` section.
    pub longterm: CompiledSection,
    /// `facts.md` section.
    pub facts: CompiledSection,
}

/// Resolve a frontend `scope` string to the execution scope + path
/// bundle used by [`MemoryCompiler`].
///
/// Until multi-scope sidecars land in 8B.x we collapse every accepted
/// label (`current` / `all` / `project` / `global`) onto the single
/// shared `<data_local_dir>/.if2ai/memory/` root.  This keeps the
/// frontend wire-shape stable — once project / session sidecars
/// arrive (post-8B) the resolver gains real branches without touching
/// the Tauri command surface.
fn resolve_paths(scope: &str) -> Result<(MemoryExecutionScope, CompilePaths), String> {
    match scope {
        "current" | "all" | "project" | "global" => {
            let root = dirs::data_local_dir()
                .ok_or_else(|| "data_local_dir unavailable".to_string())?
                .join(".if2ai")
                .join("memory");
            Ok((
                MemoryExecutionScope::global(),
                CompilePaths::from_scope_root(&root),
            ))
        }
        other => Err(format!(
            "invalid scope {other:?}; expected 'current'|'all'|'project'|'global'"
        )),
    }
}

/// Manually trigger the full Phase 8B compile pipeline for `scope`.
///
/// Runs the four `compile_*` stages sequentially (`today` → `week` →
/// `longterm` → `facts`) then the synchronous `assemble`.  Each stage
/// honours its own fingerprint cache so a no-op run is cheap (no LLM
/// tokens are spent when the inputs haven't changed).
///
/// Returns a [`CompileReport`] with the per-stage verdicts + total
/// elapsed time.  Errors from any stage propagate as the command's
/// `Err`; the report is only built on full success so the UI never
/// sees a partial state.
#[tauri::command]
pub async fn memory_compile_now(
    state: State<'_, AppState>,
    scope: String,
) -> Result<CompileReport, String> {
    let (exec_scope, paths) = resolve_paths(&scope)?;
    let started = Instant::now();
    let today = state
        .memory_compiler
        .compile_today(&exec_scope, &paths)
        .await
        .map_err(|e| e.to_string())?;
    let week = state
        .memory_compiler
        .compile_week(&exec_scope, &paths)
        .await
        .map_err(|e| e.to_string())?;
    let longterm = state
        .memory_compiler
        .compile_longterm(&exec_scope, &paths)
        .await
        .map_err(|e| e.to_string())?;
    let facts = state
        .memory_compiler
        .compile_facts(&exec_scope, &paths)
        .await
        .map_err(|e| e.to_string())?;
    let assembled = state.memory_compiler.assemble(&exec_scope, &paths).is_ok();
    Ok(CompileReport {
        today,
        week,
        longterm,
        facts,
        assembled,
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Snapshot every compiled `*.md` for the requested scope.
///
/// Read-only: never invokes the LLM and never mutates the cache.
/// Used by the CompiledMemoryViewer (8B.10) to render the four
/// section panels + the assembled `memory.md` without forcing a fresh
/// compile cycle.
#[tauri::command]
pub async fn memory_compiled_read(
    _state: State<'_, AppState>,
    scope: String,
) -> Result<CompiledMemoryDto, String> {
    let (_exec_scope, paths) = resolve_paths(&scope)?;
    Ok(CompiledMemoryDto {
        memory_md: std::fs::read_to_string(&paths.memory_md).unwrap_or_default(),
        today: CompiledSection::from_path(&paths.today_md),
        week: CompiledSection::from_path(&paths.week_md),
        longterm: CompiledSection::from_path(&paths.longterm_md),
        facts: CompiledSection::from_path(&paths.facts_md),
    })
}

/// Drop the compiled cache for `scope`.
///
/// Truncates the four `*.md` section files plus the assembled
/// `memory.md` (writes empty content) and then unlinks every
/// `.fingerprint` sidecar so the next [`memory_compile_now`] is
/// guaranteed to re-run the LLM.  The sequence is intentional:
/// content is cleared first so a crash mid-clear cannot leave a
/// fingerprint pointing at obsolete content (i.e. there is never a
/// half-clear state where the cache says "fresh" but the file is
/// stale).
#[tauri::command]
pub async fn memory_compiled_clear(
    _state: State<'_, AppState>,
    scope: String,
) -> Result<(), String> {
    let (_exec_scope, paths) = resolve_paths(&scope)?;
    for p in [
        &paths.today_md,
        &paths.week_md,
        &paths.longterm_md,
        &paths.facts_md,
        &paths.memory_md,
    ] {
        if p.exists() {
            std::fs::write(p, "").map_err(|e| format!("clear {p:?}: {e}"))?;
        }
    }
    for md_path in [
        &paths.today_md,
        &paths.week_md,
        &paths.longterm_md,
        &paths.facts_md,
    ] {
        let fp_path = fingerprint_path(md_path);
        if fp_path.exists() {
            let _ = std::fs::remove_file(&fp_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod compile_command_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn compile_report_serializes_snake_case_results() {
        // Locks the wire-shape: variants must serialise as
        // "compiled" / "skipped" so the frontend `CompileResultKind`
        // string union stays in sync without a translation layer.
        let report = CompileReport {
            today: CompileResult::Compiled,
            week: CompileResult::Skipped,
            longterm: CompileResult::Skipped,
            facts: CompileResult::Compiled,
            assembled: true,
            elapsed_ms: 42,
        };
        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["today"], "compiled");
        assert_eq!(json["week"], "skipped");
        assert_eq!(json["assembled"], true);
        assert_eq!(json["elapsed_ms"], 42);
    }

    #[test]
    fn compiled_section_from_missing_path() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("today.md");
        let section = CompiledSection::from_path(&missing);
        assert_eq!(section.content, "");
        assert_eq!(section.chars, 0);
        assert!(section.last_compiled_at.is_none());
    }

    #[test]
    fn compiled_section_from_existing_path_has_mtime_and_char_count() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("today.md");
        // Use multibyte content to verify chars counts scalars not bytes.
        std::fs::write(&path, "你好 hello").expect("write");
        let section = CompiledSection::from_path(&path);
        assert_eq!(section.content, "你好 hello");
        assert_eq!(section.chars, 8);
        assert!(
            section.last_compiled_at.is_some(),
            "mtime must be surfaced from existing files",
        );
    }

    #[test]
    fn resolve_paths_rejects_unknown_scope() {
        let err = resolve_paths("session-only").expect_err("unknown scope must error");
        assert!(err.contains("invalid scope"));
        assert!(err.contains("session-only"));
    }

    #[test]
    fn resolve_paths_accepts_canonical_scopes() {
        for scope in ["current", "all", "project", "global"] {
            let (exec_scope, paths) =
                resolve_paths(scope).unwrap_or_else(|e| panic!("{scope}: {e}"));
            assert!(exec_scope.is_global());
            assert!(paths.root.ends_with("memory"));
            assert!(paths.memory_md.ends_with("memory.md"));
        }
    }

    #[test]
    fn clear_truncates_files_then_removes_fingerprints() {
        // Build a self-contained CompilePaths under a tempdir, seed
        // every artefact + sidecar, then exercise the same clear
        // sequence the Tauri command uses.  Verifies (a) content is
        // emptied (not deleted, so future writes don't have to
        // recreate parent dirs), (b) fingerprint sidecars are
        // unlinked, (c) no half-state where a fingerprint outlives
        // its content.
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        for p in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
            &paths.memory_md,
        ] {
            std::fs::write(p, "stale body").expect("seed md");
        }
        for md in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
        ] {
            std::fs::write(fingerprint_path(md), "deadbeef").expect("seed fp");
        }

        // Replay the body of memory_compiled_clear (sans State).
        for p in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
            &paths.memory_md,
        ] {
            std::fs::write(p, "").expect("clear");
        }
        for md in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
        ] {
            let fp = fingerprint_path(md);
            if fp.exists() {
                std::fs::remove_file(&fp).expect("rm fp");
            }
        }

        for p in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
            &paths.memory_md,
        ] {
            assert!(p.exists(), "{p:?} must remain after clear");
            assert_eq!(std::fs::read_to_string(p).expect("read"), "");
        }
        for md in [
            &paths.today_md,
            &paths.week_md,
            &paths.longterm_md,
            &paths.facts_md,
        ] {
            assert!(
                !fingerprint_path(md).exists(),
                "fingerprint sidecar for {md:?} must be unlinked",
            );
        }
    }
}
