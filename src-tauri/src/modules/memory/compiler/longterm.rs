//! Compile the long-term memory by folding new `week.md` content into
//! the existing `longterm.md` (Phase 8B.3 / T-C3).
//!
//! Unlike `compile_today` / `compile_week`, this function reads no
//! `SessionSummaryStore`; its sole input is `week.md` (produced by
//! `compile_week`).  The fingerprint is therefore the MD5 of the
//! current `week.md` content alone — a fresh weekly digest forces a
//! re-fold; an unchanged week is a Skipped no-op.
//!
//! When a prior `longterm.md` exists we send the LLM the previous
//! long-term digest plus this week's new content under two labelled
//! sections so the model can decide what to keep, merge, or supersede.
//!
//! Mirrors openhanako `lib/memory/compile.js::compileLongterm`.

#![allow(dead_code)] // first production caller is MemoryCompiler::compile_longterm (8B.5+)

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::{
    compute_fingerprint, is_unchanged, write_fingerprint,
};
use crate::modules::memory::compiler::{CompileResult, SkipReason};
use crate::modules::memory::job_runner::{JobError, JobRunner};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryError, UtilityLlm};

const COMPILE_TEMPERATURE: f32 = 0.3;
const JOB_KIND: &str = "compile_longterm";
const MIN_MAX_TOKENS: u32 = 200;
const MAX_MAX_TOKENS: u32 = 1500;

/// Run one `compile_longterm` cycle.
///
/// `week_md_path` MUST exist (produced by a previous `compile_week`)
/// for any work to happen — when missing or empty the function returns
/// `Ok(CompileResult::Skipped)` without touching `output_path`.
pub async fn compile_longterm(
    output_path: &Path,
    week_md_path: &Path,
    longterm_max_chars: usize,
    llm: Arc<dyn UtilityLlm>,
    job_runner: Arc<JobRunner>,
    scope: &MemoryExecutionScope,
    is_zh: bool,
) -> Result<CompileResult, MemoryError> {
    if !week_md_path.exists() {
        tracing::debug!(week = ?week_md_path, "compile_longterm: week.md missing, skipping");
        return Ok(CompileResult::skipped(SkipReason::UpstreamMissing));
    }
    let week_content = std::fs::read_to_string(week_md_path).map_err(|e| {
        MemoryError::Generic(format!(
            "compile_longterm: read week.md {week_md_path:?}: {e}"
        ))
    })?;
    if week_content.trim().is_empty() {
        tracing::debug!("compile_longterm: week.md empty, skipping");
        return Ok(CompileResult::skipped(SkipReason::EmptyInput));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            MemoryError::Generic(format!("compile_longterm: mkdir {parent:?}: {e}"))
        })?;
    }

    // Fingerprint depends ONLY on week content — longterm is a fold
    // of week → longterm, so a fresh week always invalidates.
    let fp = compute_fingerprint(std::slice::from_ref(&week_content));
    if is_unchanged(output_path, &fp) {
        tracing::debug!(
            target = ?output_path,
            "compile_longterm: fingerprint unchanged, skipping"
        );
        return Ok(CompileResult::skipped(SkipReason::CacheHit));
    }

    let prev_longterm = std::fs::read_to_string(output_path).unwrap_or_default();
    let combined = if prev_longterm.trim().is_empty() {
        week_content.clone()
    } else if is_zh {
        format!("## 上一版长期记忆\n\n{prev_longterm}\n\n## 本周新增\n\n{week_content}")
    } else {
        format!(
            "## Previous long-term memory\n\n{prev_longterm}\n\n## New this week\n\n{week_content}"
        )
    };
    let chars_in = combined.chars().count();
    let prompt = build_compile_longterm_prompt(is_zh, longterm_max_chars);
    let max_tokens = budget_max_tokens(longterm_max_chars);

    let started = Instant::now();
    let audit_ctx = AuditContext::from_scope(scope);
    let llm_for_job = llm.clone();
    let prompt_system = prompt.clone();
    let input_owned = combined.clone();
    let llm_result = job_runner
        .run(JOB_KIND, "longterm", &audit_ctx, move || async move {
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
            tracing::warn!("compile_longterm: skipped — JobRunner exhausted retries or quota");
            return Ok(CompileResult::skipped(SkipReason::LlmDegraded));
        }
        Err(JobError::Generic(msg)) => {
            return Err(MemoryError::Generic(format!(
                "compile_longterm LLM failed: {msg}"
            )));
        }
        Err(other) => return Err(MemoryError::Generic(other.to_string())),
    };

    atomic_write(output_path, &result_text)?;
    write_fingerprint(output_path, &fp)?;
    let chars_out = result_text.chars().count();
    let latency_ms = started.elapsed().as_millis() as u64;
    MemoryAuditEmitter::memory_compiled(
        &audit_ctx, "longterm", "compiled", chars_in, chars_out, latency_ms,
    );
    Ok(CompileResult::Compiled)
}

fn budget_max_tokens(max_chars: usize) -> u32 {
    let raw = ((max_chars as f32) * 1.5).round() as i64;
    let clamped = raw.clamp(i64::from(MIN_MAX_TOKENS), i64::from(MAX_MAX_TOKENS));
    clamped as u32
}

fn build_compile_longterm_prompt(is_zh: bool, max_chars: usize) -> String {
    if is_zh {
        format!(
            "将以下两部分（旧的长期记忆 + 本周新增内容）合并成一份新的长期记忆\
             （{max_chars} 字以内）。\n\
             去重、合并相似主题，保留长期有效的事实、偏好和未完成的承诺；\
             丢弃已经过期或被新事实取代的内容。\n\
             直接输出新的长期记忆文本，不要寒暄、不要标题。"
        )
    } else {
        let max_words = ((max_chars as f32) * 0.6).round() as usize;
        format!(
            "Merge the following two sections (previous long-term memory + new content from this \
             week) into a single updated long-term memory (≤ {max_words} words). Deduplicate, \
             collapse related topics, keep durable facts / preferences / open commitments; drop \
             content that is stale or superseded by newer facts. Output the new long-term memory \
             directly — no greeting, no title."
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
    use crate::modules::memory::compiler::today::tests::job_runner as mk_job_runner;
    use crate::modules::memory::llm::MockUtilityLlm;
    use tempfile::tempdir;

    #[tokio::test]
    async fn week_md_missing_returns_skipped() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("longterm.md");
        let week = dir.path().join("week.md"); // not created
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
        let res = compile_longterm(
            &out,
            &week,
            300,
            llm,
            mk_job_runner(),
            &MemoryExecutionScope::global(),
            true,
        )
        .await
        .expect("must not error");
        assert_eq!(res, CompileResult::skipped(SkipReason::UpstreamMissing),);
        assert!(!out.exists(), "longterm.md must not be created");
    }

    #[tokio::test]
    async fn week_md_empty_returns_skipped() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("longterm.md");
        let week = dir.path().join("week.md");
        std::fs::write(&week, "   \n  ").expect("write empty week");
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
        let res = compile_longterm(
            &out,
            &week,
            300,
            llm,
            mk_job_runner(),
            &MemoryExecutionScope::global(),
            false,
        )
        .await
        .expect("must not error");
        assert_eq!(res, CompileResult::skipped(SkipReason::EmptyInput));
    }

    #[tokio::test]
    async fn compiles_when_week_md_present() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("longterm.md");
        let week = dir.path().join("week.md");
        std::fs::write(&week, "weekly digest body").expect("write week");
        let llm = Arc::new(MockUtilityLlm::new(vec!["new long-term".into()]));
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        let res = compile_longterm(
            &out,
            &week,
            300,
            llm_dyn,
            mk_job_runner(),
            &MemoryExecutionScope::global(),
            true,
        )
        .await
        .expect("must compile");
        assert_eq!(res, CompileResult::Compiled);
        assert_eq!(llm.call_count(), 1);
        let body = std::fs::read_to_string(&out).expect("read");
        assert!(body.contains("new long-term"));
    }

    #[tokio::test]
    async fn unchanged_week_is_skipped_on_second_run() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("longterm.md");
        let week = dir.path().join("week.md");
        std::fs::write(&week, "stable weekly digest").expect("write week");
        let llm = Arc::new(MockUtilityLlm::new(vec![
            "first longterm".into(),
            "second longterm".into(),
        ]));
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        compile_longterm(
            &out,
            &week,
            300,
            llm_dyn.clone(),
            mk_job_runner(),
            &MemoryExecutionScope::global(),
            true,
        )
        .await
        .expect("first compile");
        let calls_after = llm.call_count();
        let res = compile_longterm(
            &out,
            &week,
            300,
            llm_dyn,
            mk_job_runner(),
            &MemoryExecutionScope::global(),
            true,
        )
        .await
        .expect("second compile");
        assert_eq!(res, CompileResult::skipped(SkipReason::CacheHit));
        assert_eq!(llm.call_count(), calls_after, "no LLM call on cache hit");
    }

    #[test]
    fn prompt_zh_contains_chars_budget() {
        let p = build_compile_longterm_prompt(true, 300);
        assert!(p.contains("300"));
        assert!(p.contains("长期"));
    }

    #[test]
    fn prompt_en_mentions_long_term_words() {
        let p = build_compile_longterm_prompt(false, 300);
        assert!(p.to_lowercase().contains("long-term"));
        assert!(p.to_lowercase().contains("words"));
    }
}
