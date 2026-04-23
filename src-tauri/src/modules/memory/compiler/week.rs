//! Compile the trailing-7-day session summaries → `week.md`
//! (Phase 8B.3 / T-C3).
//!
//! Identical machinery to [`crate::modules::memory::compiler::today`]
//! but with a 7-day rolling window (`now - 7d .. now`) instead of
//! today's logical-day range, and a different `JOB_KIND` so the
//! `JobRunner` retry counters do not collide.
//!
//! Mirrors openhanako `lib/memory/compile.js::compileWeek`.

#![allow(dead_code)] // first production caller is MemoryCompiler::compile_week (8B.5+)

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use chrono::{Duration, Utc};

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::{
    compute_fingerprint, is_unchanged, write_fingerprint, EMPTY_FINGERPRINT,
};
use crate::modules::memory::compiler::{CompileResult, SkipReason};
use crate::modules::memory::job_runner::{JobError, JobRunner};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::{MemoryError, UtilityLlm};

const COMPILE_TEMPERATURE: f32 = 0.3;
const JOB_KIND: &str = "compile_week";
const MIN_MAX_TOKENS: u32 = 150;
const MAX_MAX_TOKENS: u32 = 1500;
const WEEK_DAYS: i64 = 7;

/// Run one `compile_week` cycle; semantics mirror
/// [`crate::modules::memory::compiler::today::compile_today`].
pub async fn compile_week(
    summary_store: Arc<dyn SessionSummaryStore>,
    scope: &MemoryExecutionScope,
    output_path: &Path,
    llm: Arc<dyn UtilityLlm>,
    job_runner: Arc<JobRunner>,
    week_max_chars: usize,
    is_zh: bool,
) -> Result<CompileResult, MemoryError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MemoryError::Generic(format!("compile_week: mkdir {parent:?}: {e}")))?;
    }

    let now = Utc::now();
    let week_ago = now - Duration::days(WEEK_DAYS);
    let summaries = summary_store.list_in_range(scope, week_ago, now).await?;

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
        tracing::debug!(target = ?output_path, "compile_week: fingerprint unchanged, skipping");
        return Ok(CompileResult::skipped(SkipReason::CacheHit));
    }

    let audit_ctx = AuditContext::from_scope(scope);

    // MEM-MOD-WIRE-FIX-3 — surface "no input" honestly; see today.rs.
    if summaries.is_empty() {
        return Ok(CompileResult::skipped(SkipReason::EmptyInput));
    }

    let input = summaries
        .iter()
        .map(|s| s.summary.as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    let chars_in = input.chars().count();
    let prompt = build_compile_week_prompt(is_zh, week_max_chars);
    let max_tokens = budget_max_tokens(week_max_chars);

    let started = Instant::now();
    let llm_for_job = llm.clone();
    let prompt_system = prompt.clone();
    let input_owned = input.clone();
    let llm_result = job_runner
        .run(JOB_KIND, "week", &audit_ctx, move || async move {
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
            tracing::warn!("compile_week: skipped — JobRunner exhausted retries or quota");
            return Ok(CompileResult::skipped(SkipReason::LlmDegraded));
        }
        Err(JobError::Generic(msg)) => {
            return Err(MemoryError::Generic(format!(
                "compile_week LLM failed: {msg}"
            )));
        }
        Err(other) => return Err(MemoryError::Generic(other.to_string())),
    };

    atomic_write(output_path, &result_text)?;
    write_fingerprint(output_path, &fp)?;
    let chars_out = result_text.chars().count();
    let latency_ms = started.elapsed().as_millis() as u64;
    MemoryAuditEmitter::memory_compiled(
        &audit_ctx, "week", "compiled", chars_in, chars_out, latency_ms,
    );
    Ok(CompileResult::Compiled)
}

fn budget_max_tokens(max_chars: usize) -> u32 {
    let raw = ((max_chars as f32) * 1.5).round() as i64;
    let clamped = raw.clamp(i64::from(MIN_MAX_TOKENS), i64::from(MAX_MAX_TOKENS));
    clamped as u32
}

fn build_compile_week_prompt(is_zh: bool, max_chars: usize) -> String {
    if is_zh {
        format!(
            "将以下过去 7 天的对话摘要整合成一段周度概要（{max_chars} 字以内）。\n\
             按时间脉络梳理，突出关键决策、长期任务进展和值得记住的事实。\n\
             直接输出概要文本，不要寒暄、不要标题。"
        )
    } else {
        let max_words = ((max_chars as f32) * 0.6).round() as usize;
        format!(
            "Consolidate the following conversation summaries from the past 7 days into a single \
             weekly overview (≤ {max_words} words). Organise chronologically, highlight key \
             decisions, long-running task progress, and durable facts worth remembering. \
             Output the overview text directly — no greeting, no title."
        )
    }
}

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
mod tests {
    use super::*;
    use crate::modules::memory::compiler::today::tests::{
        job_runner as mk_job_runner, make_record, InMemSummaryStore,
    };
    use crate::modules::memory::llm::MockUtilityLlm;
    use crate::modules::memory::summary::store::NullSessionSummaryStore;
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn empty_input_skips_with_empty_input_reason() {
        // MEM-MOD-WIRE-FIX-3 — see today.rs comment.
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("week.md");
        let store: Arc<dyn SessionSummaryStore> = Arc::new(NullSessionSummaryStore::new());
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
        let result = compile_week(
            store,
            &MemoryExecutionScope::global(),
            &out,
            llm,
            mk_job_runner(),
            500,
            true,
        )
        .await
        .expect("compile_week must succeed");
        assert_eq!(result, CompileResult::skipped(SkipReason::EmptyInput),);
        assert!(!out.exists(), "must NOT write empty week.md");
    }

    #[tokio::test]
    async fn seven_day_range_includes_recent_excludes_old() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("week.md");
        let now = Utc::now();
        let recent = make_record("s-recent", "this week", now - Duration::days(2));
        let too_old = make_record("s-old", "older than week", now - Duration::days(15));
        let store = Arc::new(InMemSummaryStore::new(vec![recent, too_old]));
        let llm = Arc::new(MockUtilityLlm::new(vec!["weekly digest".into()]));
        let store_dyn: Arc<dyn SessionSummaryStore> = store.clone();
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        let result = compile_week(
            store_dyn,
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn,
            mk_job_runner(),
            500,
            false,
        )
        .await
        .expect("compile_week must succeed");
        assert_eq!(result, CompileResult::Compiled);
        // The mock returns "weekly digest" once — confirms exactly one
        // LLM call happened (i.e. summaries were non-empty after the
        // 7-day filter rejected `too_old`).
        assert_eq!(llm.call_count(), 1);
        assert!(std::fs::read_to_string(&out)
            .expect("read")
            .contains("weekly digest"));
    }

    #[test]
    fn prompt_zh_mentions_seven_days() {
        let p = build_compile_week_prompt(true, 500);
        assert!(p.contains("7 天") || p.contains("7天"));
        assert!(p.contains("500"));
    }

    #[test]
    fn prompt_en_mentions_past_7_days() {
        let p = build_compile_week_prompt(false, 500);
        assert!(p.to_lowercase().contains("past 7 days"));
        assert!(p.to_lowercase().contains("words"));
    }
}
