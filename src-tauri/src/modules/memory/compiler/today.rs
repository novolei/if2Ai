//! Compile today's session summaries → `today.md` (Phase 8B.3 / T-C3).
//!
//! Reads every [`crate::modules::memory::summary::schema::SessionSummaryRecord`]
//! whose `updated_at` falls inside today's logical range
//! `[04:00 local, +24h-1ms]` (per `crate::modules::runtime::logical_day::get_today`),
//! computes a content fingerprint (`session_id:updated_at` per row),
//! returns [`CompileResult::Skipped`] when the fingerprint matches
//! the on-disk sidecar, otherwise asks
//! [`crate::modules::memory::UtilityLlm`] to consolidate the joined
//! summary text into `today_max_chars` and atomically writes both
//! `today.md` and its `.md.fingerprint` sidecar.
//!
//! Mirrors openhanako `lib/memory/compile.js::compileToday`.
//!
//! v2 §0.5:
//!   - Δ-1 — `llm` is `Arc<dyn UtilityLlm>` (no direct ProviderManager
//!     dependency).
//!   - Δ-3 — audit emission goes through the free
//!     `MemoryAuditEmitter::memory_compiled` associated function.
//!   - Δ-13 — `is_zh` is passed in by the caller (read once from
//!     `crate::modules::runtime::locale::is_zh`), not re-read inside
//!     this function.

#![allow(dead_code)] // first production caller is MemoryCompiler::compile_today (8B.5+)

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::{
    compute_fingerprint, is_unchanged, write_fingerprint, EMPTY_FINGERPRINT,
};
use crate::modules::memory::compiler::CompileResult;
use crate::modules::memory::job_runner::{JobError, JobRunner};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::{MemoryError, UtilityLlm};
use crate::modules::runtime::logical_day::get_today;

/// Sampling temperature for the consolidation prompt.  Low value keeps
/// the model concise and deterministic across re-runs.
const COMPILE_TEMPERATURE: f32 = 0.3;
/// `JobRunner` job_kind discriminator for retry / skip bookkeeping.
const JOB_KIND: &str = "compile_today";
/// Token budget guardrails — the LLM may receive between 150 and 1500
/// tokens regardless of the configured `today_max_chars` so a
/// misconfigured value cannot collapse the output to nothing or
/// blow the provider's per-request cap.
const MIN_MAX_TOKENS: u32 = 150;
const MAX_MAX_TOKENS: u32 = 1500;

/// Run one `compile_today` cycle.
///
/// Returns:
///   - `Ok(CompileResult::Compiled)` — fingerprint changed and
///     `today.md` was rewritten (LLM may or may not have run; in the
///     empty-input fast path no tokens are spent).
///   - `Ok(CompileResult::Skipped)` — fingerprint cache hit OR
///     `JobRunner` short-circuited because the (kind, target) pair is
///     in `Skipped` state.
///   - `Err(MemoryError)` — infrastructure failure (filesystem,
///     `JobRunner`, LLM error past retry budget).
pub async fn compile_today(
    summary_store: Arc<dyn SessionSummaryStore>,
    scope: &MemoryExecutionScope,
    output_path: &Path,
    llm: Arc<dyn UtilityLlm>,
    job_runner: Arc<JobRunner>,
    today_max_chars: usize,
    is_zh: bool,
) -> Result<CompileResult, MemoryError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MemoryError::Generic(format!("compile_today: mkdir {parent:?}: {e}")))?;
    }

    let logical = get_today();
    let summaries = summary_store
        .list_in_range(scope, logical.range_start, logical.range_end)
        .await?;

    let fp_keys: Vec<String> = if summaries.is_empty() {
        vec![EMPTY_FINGERPRINT.to_string()]
    } else {
        summaries
            .iter()
            .map(|s| format!("{}:{}", s.session_id, s.updated_at.to_rfc3339()))
            .collect()
    };
    let fp = compute_fingerprint(&fp_keys);
    if is_unchanged(output_path, &fp) {
        tracing::debug!(
            target = ?output_path,
            "compile_today: fingerprint unchanged, skipping"
        );
        return Ok(CompileResult::Skipped);
    }

    let audit_ctx = AuditContext::from_scope(scope);

    if summaries.is_empty() {
        atomic_write(output_path, "")?;
        write_fingerprint(output_path, &fp)?;
        MemoryAuditEmitter::memory_compiled(&audit_ctx, "today", "compiled", 0, 0, 0);
        return Ok(CompileResult::Compiled);
    }

    let input = summaries
        .iter()
        .map(|s| s.summary.as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    let chars_in = input.chars().count();
    let prompt = build_compile_today_prompt(is_zh, today_max_chars);
    let max_tokens = budget_max_tokens(today_max_chars);

    let started = Instant::now();
    let llm_for_job = llm.clone();
    let prompt_system = prompt.clone();
    let input_owned = input.clone();
    let llm_result = job_runner
        .run(JOB_KIND, "today", &audit_ctx, move || async move {
            llm_for_job
                .complete(
                    &prompt_system,
                    &input_owned,
                    max_tokens,
                    COMPILE_TEMPERATURE,
                )
                .await
                .map_err(|e| anyhow::anyhow!("UtilityLlm.complete failed: {e}"))
        })
        .await;

    let result_text = match llm_result {
        Ok(Some(s)) => s,
        Ok(None) => {
            tracing::warn!("compile_today: skipped — JobRunner exhausted retries or quota");
            return Ok(CompileResult::Skipped);
        }
        Err(JobError::Generic(msg)) => {
            return Err(MemoryError::Generic(format!(
                "compile_today LLM failed: {msg}"
            )));
        }
        Err(other) => return Err(MemoryError::Generic(other.to_string())),
    };

    atomic_write(output_path, &result_text)?;
    write_fingerprint(output_path, &fp)?;
    let chars_out = result_text.chars().count();
    let latency_ms = started.elapsed().as_millis() as u64;
    MemoryAuditEmitter::memory_compiled(
        &audit_ctx, "today", "compiled", chars_in, chars_out, latency_ms,
    );
    Ok(CompileResult::Compiled)
}

/// Compute a safe `max_tokens` budget from `max_chars` — roughly 1.5
/// tokens per char with hard min/max guardrails.
fn budget_max_tokens(max_chars: usize) -> u32 {
    let raw = ((max_chars as f32) * 1.5).round() as i64;
    let clamped = raw.clamp(i64::from(MIN_MAX_TOKENS), i64::from(MAX_MAX_TOKENS));
    clamped as u32
}

/// Build the system prompt for `compile_today`.  Two flavours per
/// v2 §0.5 Δ-13: zh-CN uses a char budget directly; English uses an
/// approximate words budget (≈ 0.6 word per char).
fn build_compile_today_prompt(is_zh: bool, max_chars: usize) -> String {
    if is_zh {
        format!(
            "将以下今天的对话摘要整合成一段概要（{max_chars} 字以内）。\n\
             重点突出，抓关键事件和决策，保留时间标注（HH:MM）。\n\
             直接输出概要文本，不要寒暄、不要标题。"
        )
    } else {
        let max_words = ((max_chars as f32) * 0.6).round() as usize;
        format!(
            "Consolidate the following conversation summaries from today into a single overview \
             (≤ {max_words} words). Highlight key events and decisions, preserve time stamps \
             (HH:MM). Output the overview text directly — no greeting, no title."
        )
    }
}

/// Atomic file write: `tmp + rename`.  On Windows `rename` performs a
/// `ReplaceFile`, on POSIX a single `rename(2)` system call — both are
/// atomic w.r.t. concurrent readers.
fn atomic_write(output_path: &Path, content: &str) -> Result<(), MemoryError> {
    let mut tmp = output_path.to_path_buf().into_os_string();
    tmp.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp_path, content)
        .map_err(|e| MemoryError::Generic(format!("atomic_write tmp {tmp_path:?}: {e}")))?;
    std::fs::rename(&tmp_path, output_path).map_err(|e| {
        MemoryError::Generic(format!(
            "atomic_write rename {tmp_path:?} -> {output_path:?}: {e}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::modules::memory::llm::MockUtilityLlm;
    use crate::modules::memory::summary::schema::{SessionSummaryRecord, SummarySource};
    use crate::modules::memory::summary::store::NullSessionSummaryStore;
    use chrono::{DateTime, Utc};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex as TokioMutex;

    /// Tiny in-memory `SessionSummaryStore` used only by the tests in
    /// this and sibling compiler modules — kept here (not in
    /// `summary/store.rs`) because it is pure test scaffolding.
    pub(crate) struct InMemSummaryStore {
        pub(crate) rows: TokioMutex<Vec<SessionSummaryRecord>>,
    }

    impl InMemSummaryStore {
        pub(crate) fn new(rows: Vec<SessionSummaryRecord>) -> Self {
            Self {
                rows: TokioMutex::new(rows),
            }
        }
    }

    #[async_trait::async_trait]
    impl SessionSummaryStore for InMemSummaryStore {
        async fn get(&self, session_id: &str) -> Result<Option<SessionSummaryRecord>, MemoryError> {
            Ok(self
                .rows
                .lock()
                .await
                .iter()
                .find(|r| r.session_id == session_id)
                .cloned())
        }
        async fn save(&self, rec: &SessionSummaryRecord) -> Result<(), MemoryError> {
            let mut g = self.rows.lock().await;
            if let Some(slot) = g.iter_mut().find(|r| r.session_id == rec.session_id) {
                *slot = rec.clone();
            } else {
                g.push(rec.clone());
            }
            Ok(())
        }
        async fn list_in_range(
            &self,
            _scope: &MemoryExecutionScope,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
        ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
            Ok(self
                .rows
                .lock()
                .await
                .iter()
                .filter(|r| r.updated_at >= start && r.updated_at <= end)
                .cloned()
                .collect())
        }
        async fn list_dirty(
            &self,
            _scope: &MemoryExecutionScope,
        ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
            Ok(vec![])
        }
        async fn mark_processed(&self, _id: &str) -> Result<(), MemoryError> {
            Ok(())
        }
    }

    pub(crate) fn make_record(sid: &str, summary: &str, ts: DateTime<Utc>) -> SessionSummaryRecord {
        SessionSummaryRecord {
            session_id: sid.into(),
            project_id: None,
            created_at: ts,
            updated_at: ts,
            summary: summary.into(),
            snapshot: String::new(),
            snapshot_at: None,
            message_count: 0,
            source: SummarySource::Rolling,
        }
    }

    pub(crate) fn job_runner() -> Arc<JobRunner> {
        Arc::new(JobRunner::open_in_memory_for_tests(3, 2).expect("jobrunner"))
    }

    #[tokio::test]
    async fn empty_input_writes_empty_md_and_fingerprint() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        let store: Arc<dyn SessionSummaryStore> = Arc::new(NullSessionSummaryStore::new());
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::new(vec!["unused".into()]));
        let result = compile_today(
            store,
            &MemoryExecutionScope::global(),
            &out,
            llm,
            job_runner(),
            500,
            true,
        )
        .await
        .expect("compile_today must succeed");
        assert_eq!(result, CompileResult::Compiled);
        assert!(out.exists());
        assert!(std::fs::read_to_string(&out).expect("read").is_empty());
    }

    #[tokio::test]
    async fn fingerprint_cache_hit_skips_llm() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        let now = Utc::now();
        let store: Arc<dyn SessionSummaryStore> =
            Arc::new(InMemSummaryStore::new(vec![make_record(
                "s1",
                "summary text",
                now,
            )]));
        let llm = Arc::new(MockUtilityLlm::new(vec![
            "compiled body".into(),
            "second call".into(),
        ]));
        let llm_ref: Arc<dyn UtilityLlm> = llm.clone();
        compile_today(
            store.clone(),
            &MemoryExecutionScope::global(),
            &out,
            llm_ref.clone(),
            job_runner(),
            500,
            true,
        )
        .await
        .expect("first compile");
        let calls_after_first = llm.call_count();
        let res = compile_today(
            store,
            &MemoryExecutionScope::global(),
            &out,
            llm_ref,
            job_runner(),
            500,
            true,
        )
        .await
        .expect("second compile");
        assert_eq!(res, CompileResult::Skipped);
        assert_eq!(
            llm.call_count(),
            calls_after_first,
            "no LLM call on cache hit"
        );
    }

    #[tokio::test]
    async fn changed_input_recompiles() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        let now = Utc::now();
        let store = Arc::new(InMemSummaryStore::new(vec![make_record("s1", "v1", now)]));
        let llm = Arc::new(MockUtilityLlm::new(vec![
            "compiled v1".into(),
            "compiled v2".into(),
        ]));
        let store_dyn: Arc<dyn SessionSummaryStore> = store.clone();
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        compile_today(
            store_dyn.clone(),
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn.clone(),
            job_runner(),
            500,
            true,
        )
        .await
        .expect("first compile");
        {
            let mut g = store.rows.lock().await;
            g[0].summary = "v2".into();
            g[0].updated_at = now + chrono::Duration::seconds(1);
        }
        let res = compile_today(
            store_dyn,
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn,
            job_runner(),
            500,
            true,
        )
        .await
        .expect("second compile");
        assert_eq!(res, CompileResult::Compiled);
        assert!(std::fs::read_to_string(&out).expect("read").contains("v2"));
    }

    #[test]
    fn prompt_zh_contains_max_chars() {
        let p = build_compile_today_prompt(true, 500);
        assert!(p.contains("500"));
        assert!(p.contains("今天"));
    }

    #[test]
    fn prompt_en_uses_words_budget() {
        let p = build_compile_today_prompt(false, 500);
        assert!(p.to_lowercase().contains("words"));
        assert!(p.contains("today"));
    }

    #[test]
    fn budget_max_tokens_clamps_low_and_high() {
        assert_eq!(budget_max_tokens(0), MIN_MAX_TOKENS);
        assert_eq!(budget_max_tokens(10), MIN_MAX_TOKENS);
        assert_eq!(budget_max_tokens(500), 750);
        assert_eq!(budget_max_tokens(10_000), MAX_MAX_TOKENS);
    }
}
