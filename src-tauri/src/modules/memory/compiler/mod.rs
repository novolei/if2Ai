//! `MemoryCompiler` — Sprint 2 / Phase C orchestrator.
//!
//! Coordinates the four daily compile pipelines
//! (`compile_today` / `compile_week` / `compile_longterm` /
//! `compile_facts`) plus the `assemble` step that concatenates their
//! outputs into `<scope_root>/memory.md`.
//!
//! This slice (8B.1) intentionally lays only the module skeleton —
//! types, `CompilePaths`, the `MemoryCompiler` constructor, and five
//! `Ok(CompileResult::Skipped)` stub methods.  Real LLM-driven
//! compilation lands in subsequent slices:
//!   - 8B.2 → `fingerprint.rs`     (MD5 cache + `*.md.fingerprint`)
//!   - 8B.3 → `today.rs` / `week.rs` / `longterm.rs`
//!   - 8B.4 → `facts.rs` / `assemble.rs`
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §0.5:
//!   - Δ-1: `llm` MUST be `Arc<dyn UtilityLlm>`; no direct
//!     dependency on `crate::modules::api::providers::*`.
//!   - Δ-3: audit emission is via the free `MemoryAuditEmitter::*`
//!     associated functions — `MemoryCompiler` does NOT hold an
//!     `Arc<MemoryAuditEmitter>` field.
//!   - Δ-6: `CompilePaths::from_scope_root` accepts the leaf
//!     `<data_local_dir>/.if2ai/memory` directory directly, mirroring
//!     [`crate::modules::memory::summary::store::SqliteSessionSummaryStore::open`].

#![allow(dead_code)] // first real consumer lands in 8B.3 (compile_today)

pub mod assemble;
pub mod facts;
pub mod fingerprint;
pub mod longterm;
pub mod today;
pub mod week;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::modules::memory::job_runner::JobRunner;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::{MemoryError, UtilityLlm};
use crate::modules::runtime::config::CompilerConfig;

/// Outcome of a single `compile_*` invocation.
///
/// `Compiled` — the input fingerprint changed (or there was no prior
/// fingerprint), the LLM was invoked, and the corresponding `*.md`
/// artifact has been rewritten on disk.
///
/// `Skipped { reason }` — output untouched, no LLM tokens spent. The
/// `reason` payload disambiguates **why** (cache hit, empty input,
/// degraded LLM, missing upstream file) so the Memory Debug UI can
/// render a precise diagnosis instead of an opaque "SKIPPED".
///
/// Wire-format (serde, internally tagged):
/// ```json
/// { "kind": "compiled" }
/// { "kind": "skipped", "reason": "cache_hit" }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompileResult {
    /// LLM ran; output `*.md` rewritten.
    Compiled,
    /// Output left as-is; reason describes the trigger.
    Skipped { reason: SkipReason },
}

impl CompileResult {
    /// Convenience constructor: `Skipped { reason: CacheHit }` is the
    /// hottest path so callers get a tiny shorthand.
    #[must_use]
    pub const fn skipped(reason: SkipReason) -> Self {
        Self::Skipped { reason }
    }
}

/// Why a [`CompileResult::Skipped`] was emitted.  The variants form a
/// fixed alphabet — every `return Ok(CompileResult::Skipped { … })`
/// site in `compiler/*.rs` MUST pick one of these so the UI never
/// receives an unlabelled skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Fingerprint sidecar matched current inputs → no recomputation
    /// needed.  This is the *expected* reason on the second click of
    /// the Memory Debug "一键全套测试" cache-verification step.
    CacheHit,
    /// Required upstream artifact is missing on disk (e.g.
    /// `compile_longterm` skipped because `week.md` does not exist).
    UpstreamMissing,
    /// Upstream artifact / `session_summaries` query returned empty
    /// content — there is literally nothing to compile.  This is the
    /// "fresh-install / no conversations yet" path users hit first.
    EmptyInput,
    /// `JobRunner` short-circuited because retries / quota are
    /// exhausted, or the LLM call itself returned an unrecoverable
    /// error after exhausting backoff.  Indicates a real degradation
    /// — the LLM endpoint may be misconfigured.
    LlmDegraded,
}

impl SkipReason {
    /// Stable wire label (matches the `serde(rename_all)` slug) used
    /// by the audit emitter and by front-end copy generators.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            SkipReason::CacheHit => "cache_hit",
            SkipReason::UpstreamMissing => "upstream_missing",
            SkipReason::EmptyInput => "empty_input",
            SkipReason::LlmDegraded => "llm_degraded",
        }
    }
}

/// On-disk paths for one scope's compiled-memory artifacts.
///
/// All five files live directly inside `<root>` — typically
/// `dirs::data_local_dir().join(".if2ai/memory")` per v2 §0.5 Δ-6.
/// Fingerprint sidecars are derived per-file by appending
/// `.fingerprint` to the `.md` extension; that derivation lives in
/// [`fingerprint`] and lands in slice 8B.2.
#[derive(Debug, Clone)]
pub struct CompilePaths {
    /// Filesystem root used to derive every other path in this bundle.
    pub root: PathBuf,
    /// Path to today's compiled summary.
    pub today_md: PathBuf,
    /// Path to the week's compiled summary.
    pub week_md: PathBuf,
    /// Path to the long-term compiled summary.
    pub longterm_md: PathBuf,
    /// Path to the compiled `## 重要事实` extraction.
    pub facts_md: PathBuf,
    /// Path to the assembled top-level `memory.md`.
    pub memory_md: PathBuf,
}

impl CompilePaths {
    /// Build the path bundle from a scope root.
    ///
    /// `root` is normally `dirs::data_local_dir().join(".if2ai/memory")`
    /// (v2 §0.5 Δ-6); it is the leaf directory itself, not a parent.
    #[must_use]
    pub fn from_scope_root(root: &Path) -> Self {
        let root = root.to_path_buf();
        Self {
            today_md: root.join("today.md"),
            week_md: root.join("week.md"),
            longterm_md: root.join("longterm.md"),
            facts_md: root.join("facts.md"),
            memory_md: root.join("memory.md"),
            root,
        }
    }
}

/// Sprint 2 compile orchestrator.
///
/// Held as `Arc<MemoryCompiler>` on `AppState` from this slice so the
/// future Phase 8B ticker (8B.6+) can grab a ready collaborator and
/// invoke `compile_today` after every `notify_session_end` plus the
/// full daily set inside `do_daily`.
pub struct MemoryCompiler {
    summary_store: Arc<dyn SessionSummaryStore>,
    llm: Arc<dyn UtilityLlm>,
    job_runner: Arc<JobRunner>,
    config: CompilerConfig,
}

impl std::fmt::Debug for MemoryCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Collaborators are trait objects without their own `Debug`;
        // surface the cheap-to-format config so `dbg!(&compiler)` in
        // future slices still yields useful output.
        f.debug_struct("MemoryCompiler")
            .field("config", &self.config)
            .finish()
    }
}

impl MemoryCompiler {
    /// Construct a [`MemoryCompiler`] from its four collaborators.
    ///
    /// All four are held behind `Arc` so the same compiler can be
    /// shared across the ticker, manual-trigger Tauri commands, and
    /// recovery routines.
    #[must_use]
    pub fn new(
        summary_store: Arc<dyn SessionSummaryStore>,
        llm: Arc<dyn UtilityLlm>,
        job_runner: Arc<JobRunner>,
        config: CompilerConfig,
    ) -> Self {
        Self {
            summary_store,
            llm,
            job_runner,
            config,
        }
    }

    /// Read-only access to the active [`CompilerConfig`].
    #[must_use]
    pub fn config(&self) -> &CompilerConfig {
        &self.config
    }

    /// Compile today's session summaries → `today.md`.
    ///
    /// Delegates to [`today::compile_today`] with the compiler's
    /// stored collaborators; `is_zh` is read from
    /// [`crate::modules::runtime::locale::is_zh`] per v2 §0.5 Δ-13.
    pub async fn compile_today(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<CompileResult, MemoryError> {
        let is_zh = crate::modules::runtime::locale::is_zh();
        today::compile_today(
            self.summary_store.clone(),
            scope,
            &paths.today_md,
            self.llm.clone(),
            self.job_runner.clone(),
            self.config.today_max_chars,
            is_zh,
        )
        .await
    }

    /// Compile the trailing-7-day session summaries → `week.md`.
    ///
    /// Delegates to [`week::compile_week`] with the compiler's stored
    /// collaborators; `is_zh` is read from
    /// [`crate::modules::runtime::locale::is_zh`].
    pub async fn compile_week(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<CompileResult, MemoryError> {
        let is_zh = crate::modules::runtime::locale::is_zh();
        week::compile_week(
            self.summary_store.clone(),
            scope,
            &paths.week_md,
            self.llm.clone(),
            self.job_runner.clone(),
            self.config.week_max_chars,
            is_zh,
        )
        .await
    }

    /// Compile the long-term summary by folding new `week.md` content
    /// into the existing `longterm.md`.
    ///
    /// Delegates to [`longterm::compile_longterm`].  Returns
    /// `Ok(Skipped)` when `paths.week_md` does not exist yet — the
    /// daily ticker (8B.8) is responsible for sequencing
    /// `compile_week` before `compile_longterm`.
    pub async fn compile_longterm(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<CompileResult, MemoryError> {
        let is_zh = crate::modules::runtime::locale::is_zh();
        longterm::compile_longterm(
            &paths.longterm_md,
            &paths.week_md,
            self.config.longterm_max_chars,
            self.llm.clone(),
            self.job_runner.clone(),
            scope,
            is_zh,
        )
        .await
    }

    /// Extract the cumulative `## 重要事实` block → `facts.md`.
    ///
    /// Delegates to [`facts::compile_facts`] with the compiler's
    /// stored collaborators; `is_zh` is read from
    /// [`crate::modules::runtime::locale::is_zh`].
    pub async fn compile_facts(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<CompileResult, MemoryError> {
        let is_zh = crate::modules::runtime::locale::is_zh();
        facts::compile_facts(
            self.summary_store.clone(),
            scope,
            &paths.facts_md,
            self.llm.clone(),
            self.job_runner.clone(),
            self.config.facts_max_chars,
            is_zh,
        )
        .await
    }

    /// Concatenate the four `*.md` artifacts into `memory.md`.
    ///
    /// Synchronous (no LLM, no async) — delegates to
    /// [`assemble::assemble`].  The ticker (8B.7+) passes the real
    /// scope; until then the orchestrator wires the global scope so
    /// `memory_assembled` audit events still carry consistent
    /// project/session fields.
    pub fn assemble(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<(), MemoryError> {
        let is_zh = crate::modules::runtime::locale::is_zh();
        assemble::assemble(paths, scope, is_zh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::job_runner::JobRunner;
    use crate::modules::memory::llm::MockUtilityLlm;
    use crate::modules::memory::scope::MemoryExecutionScope;
    use crate::modules::memory::summary::store::NullSessionSummaryStore;

    fn make_compiler() -> MemoryCompiler {
        let summary_store: Arc<dyn SessionSummaryStore> = Arc::new(NullSessionSummaryStore::new());
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
        let job_runner =
            Arc::new(JobRunner::open_in_memory_for_tests(3, 3).expect("in-mem job runner"));
        MemoryCompiler::new(summary_store, llm, job_runner, CompilerConfig::default())
    }

    #[test]
    fn compile_paths_from_scope_root() {
        let root = Path::new("/tmp/if2ai-compile-paths-test/.if2ai/memory");
        let paths = CompilePaths::from_scope_root(root);
        assert_eq!(paths.root, root);
        assert_eq!(paths.today_md, root.join("today.md"));
        assert_eq!(paths.week_md, root.join("week.md"));
        assert_eq!(paths.longterm_md, root.join("longterm.md"));
        assert_eq!(paths.facts_md, root.join("facts.md"));
        assert_eq!(paths.memory_md, root.join("memory.md"));
    }

    #[test]
    fn compile_result_serializes_with_kind_and_reason() {
        // MEM-MOD-WIRE-FIX-3 — Wire-format locked down here so a
        // future variant rename is caught early.  The shape is the
        // same one MemoryDebugTab.tsx parses (`{kind, reason?}`),
        // which is itself why we picked `tag = "kind"` over a bare
        // string union.
        let compiled = serde_json::to_string(&CompileResult::Compiled).expect("serialize");
        let cache_hit = serde_json::to_string(&CompileResult::skipped(SkipReason::CacheHit))
            .expect("serialize");
        let empty = serde_json::to_string(&CompileResult::skipped(SkipReason::EmptyInput))
            .expect("serialize");
        let degraded = serde_json::to_string(&CompileResult::skipped(SkipReason::LlmDegraded))
            .expect("serialize");
        let upstream = serde_json::to_string(&CompileResult::skipped(SkipReason::UpstreamMissing))
            .expect("serialize");
        assert_eq!(compiled, r#"{"kind":"compiled"}"#);
        assert_eq!(cache_hit, r#"{"kind":"skipped","reason":"cache_hit"}"#);
        assert_eq!(empty, r#"{"kind":"skipped","reason":"empty_input"}"#);
        assert_eq!(degraded, r#"{"kind":"skipped","reason":"llm_degraded"}"#);
        assert_eq!(
            upstream,
            r#"{"kind":"skipped","reason":"upstream_missing"}"#
        );
    }

    #[tokio::test]
    async fn wired_compile_today_skips_when_no_summaries() {
        // MEM-MOD-WIRE-FIX-3 — fresh install (zero summaries) returns
        // Skipped { EmptyInput }, NOT a fake Compiled-with-empty-body.
        // The fingerprint is intentionally NOT written so the next
        // call after summaries arrive re-triggers the LLM path.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_today(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::skipped(SkipReason::EmptyInput));
        assert!(!paths.today_md.exists(), "must NOT write empty today.md");
        let second = compiler
            .compile_today(&scope, &paths)
            .await
            .expect("second compile must not error");
        // No fingerprint written → second call ALSO sees EmptyInput,
        // not CacheHit.  Once real summaries land, fp diverges and
        // a real Compiled fires.
        assert_eq!(second, CompileResult::skipped(SkipReason::EmptyInput));
    }

    #[tokio::test]
    async fn wired_compile_week_skips_when_no_summaries() {
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_week(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::skipped(SkipReason::EmptyInput));
    }

    #[tokio::test]
    async fn stub_compile_longterm_returns_upstream_missing() {
        // MEM-MOD-WIRE-FIX-3 — explicit reason now: week.md doesn't
        // exist, so the longterm pipeline can't fold anything → it's
        // an UpstreamMissing skip, not the generic "Skipped" of yore.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let paths = CompilePaths::from_scope_root(Path::new("/tmp/if2ai-stub-longterm-8b3"));
        let out = compiler
            .compile_longterm(&scope, &paths)
            .await
            .expect("must not error");
        assert_eq!(out, CompileResult::skipped(SkipReason::UpstreamMissing));
    }

    #[tokio::test]
    async fn wired_compile_facts_skips_when_no_summaries() {
        // MEM-MOD-WIRE-FIX-3 — fresh install (zero summaries) =>
        // chars_in == 0 in compile_facts → EmptyInput skip, no
        // file written.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_facts(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::skipped(SkipReason::EmptyInput));
        assert!(!paths.facts_md.exists(), "must NOT write empty facts.md");
    }

    #[tokio::test]
    async fn wired_assemble_writes_memory_md() {
        // 8B.4 — assemble() is now real synchronous file I/O.  With
        // empty *.md inputs it still produces a memory.md skeleton
        // populated with the four bilingual section placeholders.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        compiler
            .assemble(&scope, &paths)
            .expect("assemble must not error");
        assert!(paths.memory_md.exists());
        let body = std::fs::read_to_string(&paths.memory_md).expect("read memory.md");
        assert!(body.contains("## "));
    }

    #[test]
    fn debug_impl_includes_config() {
        let compiler = make_compiler();
        let rendered = format!("{compiler:?}");
        assert!(rendered.contains("MemoryCompiler"));
        assert!(rendered.contains("config"));
    }
}
