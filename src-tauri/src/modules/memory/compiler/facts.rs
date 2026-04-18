//! Compile important facts → `facts.md` (Phase 8B.4 / T-C3 facts variant).
//!
//! Scans the trailing-30-day [`crate::modules::memory::summary::schema::SessionSummaryRecord`]
//! rows for the `## 重要事实` / `## Key facts` section via a regex
//! (mirroring openhanako `lib/memory/compile.js::_extractFacts`),
//! merges the extraction with the previous `facts.md` body, and either:
//!
//!   - Writes the merged corpus directly when its char count is below
//!     [`FACTS_NO_LLM_THRESHOLD_CHARS`] — saves one LLM round-trip
//!     when the cumulative facts are still small.
//!   - Otherwise asks [`crate::modules::memory::UtilityLlm`] to
//!     consolidate the corpus down to `facts_max_chars`.
//!
//! v2 §0.5:
//!   - Δ-1 — `llm` is `Arc<dyn UtilityLlm>` (no direct ProviderManager
//!     dependency).
//!   - Δ-3 — audit emission goes through the free
//!     `MemoryAuditEmitter::memory_compiled` associated function.
//!   - Δ-13 — `is_zh` is passed in by the caller (read once from
//!     `crate::modules::runtime::locale::is_zh`), not re-read here.
//!   - Δ-17 — the `facts.md` artifact is the *highest* priority
//!     section in the assembled `memory.md`; never quietly truncated
//!     here.

#![allow(dead_code)] // first production caller is MemoryCompiler::compile_facts (8B.5+)

use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use chrono::Duration;
use regex::Regex;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::fingerprint::{
    compute_fingerprint, is_unchanged, write_fingerprint, EMPTY_FINGERPRINT,
};
use crate::modules::memory::compiler::CompileResult;
use crate::modules::memory::job_runner::{JobError, JobRunner};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::{MemoryError, UtilityLlm};

/// Skip LLM consolidation when the merged facts corpus is smaller
/// than this many chars — direct writes are cheaper and lossless for
/// small corpora.
pub const FACTS_NO_LLM_THRESHOLD_CHARS: usize = 500;

/// Sampling temperature for the consolidation prompt.  Low keeps the
/// output deterministic across re-runs.
const COMPILE_TEMPERATURE: f32 = 0.3;
/// `JobRunner` job_kind discriminator for retry/skip bookkeeping.
const JOB_KIND: &str = "compile_facts";
/// Token budget guardrails — same band as `compile_today` so a
/// misconfigured `facts_max_chars` cannot collapse output to nothing
/// or blow the provider per-request cap.
const MIN_MAX_TOKENS: u32 = 150;
const MAX_MAX_TOKENS: u32 = 1500;
/// Trailing window scanned for `## 重要事实` / `## Key facts`
/// sections.  Mirrors openhanako compile.js (30 days).
const FACTS_LOOKBACK_DAYS: i64 = 30;

/// Lazy regex matching the *header* of a `## 重要事实` or
/// `## Key facts` section.  We can't use a single body-capturing
/// regex because the `regex` crate does not support lookahead, so
/// [`extract_facts_sections`] uses this header matcher to locate
/// section starts and then manually slices the body up to the next
/// `\n## ` or end-of-input.  Cached in a `OnceLock` so we never
/// recompile per-call.
fn facts_header_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Static-pattern Regex compilation cannot fail at runtime; the
    // panic message below documents the invariant.  `expect_used` is
    // explicitly allowed so the harness review gate accepts the
    // standard OnceLock<Regex> idiom.
    #[allow(clippy::expect_used)]
    RE.get_or_init(|| {
        Regex::new(r"(?m)^##[ \t]*(?:重要事实|Key facts)[ \t]*\n")
            .expect("facts header regex must compile")
    })
}

/// Regex matching any subsequent `## ` header at the start of a line —
/// used to find the end boundary of a facts section body.
fn next_header_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    #[allow(clippy::expect_used)]
    RE.get_or_init(|| Regex::new(r"(?m)^## ").expect("next header regex must compile"))
}

/// Run one `compile_facts` cycle.
///
/// Returns:
///   - `Ok(CompileResult::Compiled)` — fingerprint changed and
///     `facts.md` was rewritten (LLM may or may not have run; small
///     corpora skip the LLM call entirely).
///   - `Ok(CompileResult::Skipped)` — fingerprint cache hit OR
///     `JobRunner` short-circuited because the (kind, target) pair is
///     in `Skipped` state.
///   - `Err(MemoryError)` — infrastructure failure (filesystem,
///     `JobRunner`, LLM error past retry budget).
pub async fn compile_facts(
    summary_store: Arc<dyn SessionSummaryStore>,
    scope: &MemoryExecutionScope,
    output_path: &Path,
    llm: Arc<dyn UtilityLlm>,
    job_runner: Arc<JobRunner>,
    facts_max_chars: usize,
    is_zh: bool,
) -> Result<CompileResult, MemoryError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MemoryError::Generic(format!("compile_facts: mkdir {parent:?}: {e}")))?;
    }

    let now = chrono::Utc::now();
    let lookback_start = now - Duration::days(FACTS_LOOKBACK_DAYS);
    let summaries = summary_store
        .list_in_range(scope, lookback_start, now)
        .await?;

    let summaries_blob = summaries
        .iter()
        .map(|s| s.summary.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let extracted = extract_facts_sections(&summaries_blob);

    // Fingerprint over the *extracted* corpus alone (not the merged
    // prev+extracted) so re-running with the same set of summaries is
    // a deterministic cache hit even after the previous run wrote its
    // own merged body back to facts.md.
    let fp_keys: Vec<String> = if extracted.trim().is_empty() {
        vec![EMPTY_FINGERPRINT.to_string()]
    } else {
        vec![extracted.clone()]
    };
    let fp = compute_fingerprint(&fp_keys);
    if is_unchanged(output_path, &fp) {
        tracing::debug!(
            target = ?output_path,
            "compile_facts: fingerprint unchanged, skipping"
        );
        return Ok(CompileResult::Skipped);
    }

    let prev = std::fs::read_to_string(output_path).unwrap_or_default();
    let merged = merge_facts(prev.trim(), extracted.trim());
    let chars_in = merged.chars().count();
    let audit_ctx = AuditContext::from_scope(scope);

    // Small corpus → direct write, no LLM call.  The empty-input case
    // also lands here (chars_in == 0) and is recorded as Compiled so
    // the fingerprint sidecar gets written.
    if chars_in < FACTS_NO_LLM_THRESHOLD_CHARS {
        atomic_write(output_path, &merged)?;
        write_fingerprint(output_path, &fp)?;
        MemoryAuditEmitter::memory_compiled(&audit_ctx, "facts", "compiled", chars_in, chars_in, 0);
        return Ok(CompileResult::Compiled);
    }

    let prompt = build_compile_facts_prompt(is_zh, facts_max_chars);
    let max_tokens = budget_max_tokens(facts_max_chars);

    let started = Instant::now();
    let llm_for_job = llm.clone();
    let prompt_system = prompt.clone();
    let input_owned = merged.clone();
    let llm_result = job_runner
        .run(JOB_KIND, "facts", &audit_ctx, move || async move {
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
            tracing::warn!("compile_facts: skipped — JobRunner exhausted retries or quota");
            return Ok(CompileResult::Skipped);
        }
        Err(JobError::Generic(msg)) => {
            return Err(MemoryError::Generic(format!(
                "compile_facts LLM failed: {msg}"
            )));
        }
        Err(other) => return Err(MemoryError::Generic(other.to_string())),
    };

    atomic_write(output_path, &result_text)?;
    write_fingerprint(output_path, &fp)?;
    let chars_out = result_text.chars().count();
    let latency_ms = started.elapsed().as_millis() as u64;
    MemoryAuditEmitter::memory_compiled(
        &audit_ctx, "facts", "compiled", chars_in, chars_out, latency_ms,
    );
    Ok(CompileResult::Compiled)
}

/// Extract every `## 重要事实` / `## Key facts` section body from a
/// concatenated summary blob and join them with blank lines.
///
/// Implemented with two regexes (header + next-header) plus manual
/// slicing because the `regex` crate has no lookahead support.
#[must_use]
pub fn extract_facts_sections(summary_blob: &str) -> String {
    let header_re = facts_header_regex();
    let next_re = next_header_regex();
    let mut bodies: Vec<String> = Vec::new();
    for header_match in header_re.find_iter(summary_blob) {
        let body_start = header_match.end();
        let remainder = &summary_blob[body_start..];
        let body_end = next_re
            .find(remainder)
            .map_or(summary_blob.len(), |m| body_start + m.start());
        let body = summary_blob[body_start..body_end].trim();
        if !body.is_empty() {
            bodies.push(body.to_string());
        }
    }
    bodies.join("\n\n")
}

/// Merge previous `facts.md` body with newly-extracted sections —
/// a blank line separator when both sides are non-empty, otherwise
/// just whichever is non-empty (or "" when both are empty).
fn merge_facts(prev: &str, extracted: &str) -> String {
    match (prev.is_empty(), extracted.is_empty()) {
        (true, true) => String::new(),
        (true, false) => extracted.to_string(),
        (false, true) => prev.to_string(),
        (false, false) => format!("{prev}\n\n{extracted}"),
    }
}

/// Compute a safe `max_tokens` budget from `max_chars` — roughly 1.5
/// tokens per char with hard min/max guardrails (mirrors `today.rs`).
fn budget_max_tokens(max_chars: usize) -> u32 {
    let raw = ((max_chars as f32) * 1.5).round() as i64;
    let clamped = raw.clamp(i64::from(MIN_MAX_TOKENS), i64::from(MAX_MAX_TOKENS));
    clamped as u32
}

/// Build the system prompt for `compile_facts`.  Two flavours per
/// v2 §0.5 Δ-13: zh-CN uses a char budget directly; English uses an
/// approximate words budget (≈ 0.6 word per char).
fn build_compile_facts_prompt(is_zh: bool, max_chars: usize) -> String {
    if is_zh {
        format!(
            "以下是从最近 30 天对话中抽取的「重要事实」原始集合。\n\
             请合并去重，去掉过时或冲突的条目，按主题分组，控制在 {max_chars} 字以内。\n\
             直接输出 markdown 列表（每条以 `- ` 开头），不要寒暄、不要标题。"
        )
    } else {
        let max_words = ((max_chars as f32) * 0.6).round() as usize;
        format!(
            "Below is the raw set of \"## Key facts\" extracted from the past 30 days of \
             conversations. Merge duplicates, drop stale or contradicted items, group by topic, \
             keep ≤ {max_words} words. Output a markdown bullet list (each line `- `). No \
             greeting, no header."
        )
    }
}

/// Atomic file write: tmp + rename.  Mirrors
/// [`crate::modules::memory::compiler::today`]'s helper so a crash
/// mid-write cannot corrupt `facts.md`.
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
        job_runner, make_record, InMemSummaryStore,
    };
    use crate::modules::memory::llm::MockUtilityLlm;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn extract_section_zh() {
        let input = "## 重要事实\n- A\n- B\n\n## 事情经过\n后续...";
        let r = extract_facts_sections(input);
        assert!(r.contains("- A") && r.contains("- B"));
        assert!(!r.contains("后续"));
    }

    #[test]
    fn extract_section_en() {
        let input = "## Key facts\n- foo\n- bar\n\n## What happened\nstuff";
        let r = extract_facts_sections(input);
        assert!(r.contains("- foo") && r.contains("- bar"));
        assert!(!r.contains("stuff"));
    }

    #[test]
    fn extract_section_concatenates_multiple() {
        let input = "## 重要事实\n- A\n\n## 事情经过\n...\n\n## 重要事实\n- B\n";
        let r = extract_facts_sections(input);
        assert!(r.contains("- A") && r.contains("- B"));
    }

    #[test]
    fn extract_returns_empty_when_no_match() {
        assert!(extract_facts_sections("## Random\nfoo").is_empty());
    }

    #[test]
    fn prompt_zh_contains_max_chars() {
        let p = build_compile_facts_prompt(true, 200);
        assert!(p.contains("200"));
        assert!(p.contains("重要事实"));
    }

    #[test]
    fn prompt_en_uses_words_budget() {
        let p = build_compile_facts_prompt(false, 200);
        assert!(p.to_lowercase().contains("words"));
        assert!(p.to_lowercase().contains("key facts"));
    }

    #[tokio::test]
    async fn small_facts_no_llm_call() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("facts.md");
        let now = Utc::now();
        let store = Arc::new(InMemSummaryStore::new(vec![make_record(
            "s1",
            "## 重要事实\n- short fact A\n- short fact B\n",
            now,
        )]));
        let store_dyn: Arc<dyn SessionSummaryStore> = store;
        let llm = Arc::new(MockUtilityLlm::new(vec!["should-not-be-used".into()]));
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        let res = compile_facts(
            store_dyn,
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn,
            job_runner(),
            200,
            true,
        )
        .await
        .expect("compile_facts must not error");
        assert_eq!(res, CompileResult::Compiled);
        assert!(out.exists());
        let body = std::fs::read_to_string(&out).expect("read");
        assert!(body.contains("short fact A"));
        assert_eq!(
            llm.call_count(),
            0,
            "LLM must not be called for small corpus"
        );
    }

    #[tokio::test]
    async fn large_facts_invokes_llm_and_writes_output() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("facts.md");
        let now = Utc::now();
        // Build a summary whose facts section exceeds the
        // FACTS_NO_LLM_THRESHOLD_CHARS so the LLM path is taken.
        let big_fact = "- ".to_string() + &"x".repeat(FACTS_NO_LLM_THRESHOLD_CHARS + 100);
        let summary = format!("## 重要事实\n{big_fact}\n");
        let store = Arc::new(InMemSummaryStore::new(vec![make_record(
            "s1", &summary, now,
        )]));
        let store_dyn: Arc<dyn SessionSummaryStore> = store;
        let llm = Arc::new(MockUtilityLlm::new(vec!["compressed facts body".into()]));
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        let res = compile_facts(
            store_dyn,
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn,
            job_runner(),
            200,
            true,
        )
        .await
        .expect("compile_facts must not error");
        assert_eq!(res, CompileResult::Compiled);
        assert_eq!(llm.call_count(), 1, "LLM must be called for large corpus");
        let body = std::fs::read_to_string(&out).expect("read");
        assert_eq!(body, "compressed facts body");
    }

    #[tokio::test]
    async fn fingerprint_cache_hit_skips_on_second_run() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("facts.md");
        let now = Utc::now();
        let store = Arc::new(InMemSummaryStore::new(vec![make_record(
            "s1",
            "## 重要事实\n- stable fact\n",
            now,
        )]));
        let store_dyn: Arc<dyn SessionSummaryStore> = store;
        let llm = Arc::new(MockUtilityLlm::new(vec!["unused".into()]));
        let llm_dyn: Arc<dyn UtilityLlm> = llm.clone();
        compile_facts(
            store_dyn.clone(),
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn.clone(),
            job_runner(),
            200,
            true,
        )
        .await
        .expect("first compile");
        let res = compile_facts(
            store_dyn,
            &MemoryExecutionScope::global(),
            &out,
            llm_dyn,
            job_runner(),
            200,
            true,
        )
        .await
        .expect("second compile");
        assert_eq!(res, CompileResult::Skipped);
    }

    #[test]
    fn merge_facts_handles_all_corners() {
        assert_eq!(merge_facts("", ""), "");
        assert_eq!(merge_facts("prev", ""), "prev");
        assert_eq!(merge_facts("", "new"), "new");
        assert_eq!(merge_facts("prev", "new"), "prev\n\nnew");
    }
}
