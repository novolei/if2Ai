//! Memory compile pipeline IPC commands (Memory Audit P3 #13).
//!
//! Three Tauri commands surface the [`MemoryCompiler`] pipeline
//! (8B.1–8B.4) to the frontend so the CompiledMemoryViewer (8B.10)
//! and operator scripting can manually trigger / inspect / clear the
//! compiled memory without waiting for the daily ticker (8B.6+).
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §0.5 Δ-11 every command must be registered in three places:
//! - this file (function definition)
//! - `commands/mod.rs` (re-export, picks up the `pub use compile::*`
//!   from `commands/memory/mod.rs`)
//! - `main.rs` (`tauri::generate_handler!`)
//! Capabilities live in `src-tauri/capabilities/default.json`.
//!
//! Extracted from the legacy single-file `commands/memory.rs` during
//! the Memory System Audit P3 god-file split — content unchanged from
//! the previous Phase 8B.5 / T-C5 implementation.

use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::fingerprint_path;
use crate::modules::memory::compiler::{CompilePaths, CompileResult};
use crate::modules::memory::scope::MemoryExecutionScope;

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
/// shared `<data_local_dir>/.if2ai/memory/` root.
fn resolve_paths(scope: &str) -> Result<(MemoryExecutionScope, CompilePaths), String> {
    match scope {
        "current" | "all" | "project" | "global" => {
            // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
            let root = crate::modules::config::store::if2ai_data_root().join("memory");
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
    let audit_ctx = AuditContext::from_scope(&exec_scope);
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "compiled");
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
/// fingerprint pointing at obsolete content.
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
    let audit_ctx = AuditContext {
        trace_id: None,
        session_id: None,
        project_id: None,
        effective_workdir: None,
    };
    MemoryAuditEmitter::memory_invalidated(&audit_ctx, "compiled");
    Ok(())
}

#[cfg(test)]
mod compile_command_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn compile_report_serializes_snake_case_results() {
        // MEM-MOD-WIRE-FIX-3 — `Skipped` now carries a reason payload;
        // wire shape is `{kind:"skipped",reason:"cache_hit"}` instead
        // of the bare `"skipped"` string of yore.
        let report = CompileReport {
            today: CompileResult::Compiled,
            week: CompileResult::skipped(crate::modules::memory::SkipReason::CacheHit),
            longterm: CompileResult::skipped(
                crate::modules::memory::SkipReason::UpstreamMissing,
            ),
            facts: CompileResult::Compiled,
            assembled: true,
            elapsed_ms: 42,
        };
        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["today"]["kind"], "compiled");
        assert_eq!(json["week"]["kind"], "skipped");
        assert_eq!(json["week"]["reason"], "cache_hit");
        assert_eq!(json["longterm"]["reason"], "upstream_missing");
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
