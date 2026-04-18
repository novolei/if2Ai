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
/// `Skipped` — either the fingerprint was unchanged (cache hit) or
/// there was no input to summarise.  In both cases the existing
/// artifact is left untouched and no LLM tokens are spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileResult {
    /// LLM ran; output `*.md` rewritten.
    Compiled,
    /// Cache hit or empty input; output left as-is.
    Skipped,
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
    fn compile_result_serializes_snake_case() {
        // The TauriCommand surface for memory_compile_now (8B.5) ships
        // `CompileResult` to the frontend; lock the wire-format down
        // here so a future rename of the variants is caught early.
        let compiled = serde_json::to_string(&CompileResult::Compiled).expect("serialize");
        let skipped = serde_json::to_string(&CompileResult::Skipped).expect("serialize");
        assert_eq!(compiled, "\"compiled\"");
        assert_eq!(skipped, "\"skipped\"");
    }

    #[tokio::test]
    async fn wired_compile_today_compiles_empty_then_skips() {
        // 8B.3 — with the NullSessionSummaryStore returning zero rows
        // the function takes the empty-input fast path: writes an
        // empty today.md + sentinel fingerprint and returns Compiled.
        // A second call hits the cache and returns Skipped.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_today(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::Compiled);
        assert!(paths.today_md.exists());
        let second = compiler
            .compile_today(&scope, &paths)
            .await
            .expect("second compile must not error");
        assert_eq!(second, CompileResult::Skipped);
    }

    #[tokio::test]
    async fn wired_compile_week_compiles_empty_then_skips() {
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_week(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::Compiled);
        let second = compiler
            .compile_week(&scope, &paths)
            .await
            .expect("second compile must not error");
        assert_eq!(second, CompileResult::Skipped);
    }

    #[tokio::test]
    async fn stub_compile_longterm_returns_skipped() {
        // 8B.3 wired the real implementation; with no week.md on disk
        // the function still returns Skipped, preserving the original
        // contract of this test.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let paths = CompilePaths::from_scope_root(Path::new("/tmp/if2ai-stub-longterm-8b3"));
        let out = compiler
            .compile_longterm(&scope, &paths)
            .await
            .expect("must not error");
        assert_eq!(out, CompileResult::Skipped);
    }

    #[tokio::test]
    async fn wired_compile_facts_compiles_empty_then_skips() {
        // 8B.4 — with the NullSessionSummaryStore returning zero rows
        // the function takes the small-corpus fast path: writes an
        // empty facts.md + sentinel fingerprint and returns Compiled.
        // A second call hits the cache and returns Skipped.
        let compiler = make_compiler();
        let scope = MemoryExecutionScope::global();
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let first = compiler
            .compile_facts(&scope, &paths)
            .await
            .expect("first compile must not error");
        assert_eq!(first, CompileResult::Compiled);
        assert!(paths.facts_md.exists());
        let second = compiler
            .compile_facts(&scope, &paths)
            .await
            .expect("second compile must not error");
        assert_eq!(second, CompileResult::Skipped);
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
