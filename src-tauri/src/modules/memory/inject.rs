//! Memory injection for the system prompt (Phase 8A.11 / T-F4).
//!
//! Per v2 §0.5 Δ-7,
//! [`crate::modules::runtime::prompt::SystemPromptBuilder::build`] is
//! synchronous, so memory must be assembled BEFORE the builder runs.
//! This module provides the async pre-fetch that turns the live
//! [`PinnedStore`] + on-disk compiled `memory.md` into a
//! [`MemoryInjection`] payload, which the builder then renders inline
//! through
//! [`crate::modules::runtime::prompt::SystemPromptBuilder::with_memory_injection`].
//!
//! # Budget enforcement (v2 §Sprint 1 / T-F4)
//!
//! - Total chars cap = `max_tokens * 4` (rough char-per-token estimate
//!   via [`CHARS_PER_TOKEN_ESTIMATE`]).
//! - Pinned section is unconditionally first (highest user trust).
//! - Compiled section is truncated to whatever budget remains.
//! - Rules section is small and always included so the agent knows how
//!   to USE the injected memory.

#![allow(dead_code)] // first production wiring lands when commands/agent.rs un-stashes (8A.12+)

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::modules::memory::evolution::ProceduralMemoryManager;
use crate::modules::memory::pinned::{PinnedStore, MAX_PIN_CONTENT_CHARS};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryError, SharedMemoryProvider};

/// Char-per-token approximation for budget computation.
///
/// English averages ~4 chars/token; Chinese closer to ~1.8.  We pick
/// `4` to stay conservative in English; Chinese pins truncate slightly
/// later than they could.  Acceptable trade-off — the rules section
/// keeps the final prompt deterministic regardless.
pub const CHARS_PER_TOKEN_ESTIMATE: usize = 4;

/// Prompt Caching hint — instructs the provider layer on how to
/// cache a given prompt fragment.
///
/// Mapped to Anthropic `cache_control` / OpenAI prompt-caching
/// semantics downstream by the provider adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheHint {
    /// Do not cache (re-process every turn).
    None,
    /// Stable cache — content changes very rarely; provider may
    /// keep the processed prefix for a long time.
    Stable,
    /// Ephemeral cache — content is stable within a session but
    /// may change across sessions.
    Ephemeral,
}

impl Default for CacheHint {
    fn default() -> Self {
        Self::None
    }
}

/// Per-section cache hints carried inside [`MemoryInjection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCacheHints {
    /// Pinned memory — user-managed, changes rarely.
    pub pinned: CacheHint,
    /// Compiled memory — regenerated each session.
    pub compiled: CacheHint,
    /// Procedural rules — DayDream-cycle cadence.
    pub procedural: CacheHint,
    /// Usage-rules section — almost never changes.
    pub rules: CacheHint,
}

impl Default for MemoryCacheHints {
    fn default() -> Self {
        Self {
            pinned: CacheHint::Stable,
            compiled: CacheHint::Ephemeral,
            procedural: CacheHint::Stable,
            rules: CacheHint::Stable,
        }
    }
}

/// Token-level cache efficiency statistics for observability.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheEfficiencyStats {
    /// Estimated token count of fragments marked [`CacheHint::Stable`].
    pub stable_tokens: usize,
    /// Estimated token count of fragments marked [`CacheHint::Ephemeral`].
    pub ephemeral_tokens: usize,
    /// Estimated token count of fragments with [`CacheHint::None`].
    pub uncached_tokens: usize,
    /// `stable_tokens / total` — rough predicted cache-hit ratio.
    pub estimated_hit_rate: f64,
}

impl PartialEq for CacheEfficiencyStats {
    fn eq(&self, other: &Self) -> bool {
        self.stable_tokens == other.stable_tokens
            && self.ephemeral_tokens == other.ephemeral_tokens
            && self.uncached_tokens == other.uncached_tokens
            && self.estimated_hit_rate.to_bits() == other.estimated_hit_rate.to_bits()
    }
}

impl Eq for CacheEfficiencyStats {}

/// Output of [`build_memory_injection`].
///
/// The three section fields are rendered in order (pinned, compiled,
/// rules) by
/// [`crate::modules::runtime::prompt::SystemPromptBuilder::build`]
/// after the dynamic boundary.  `total_tokens_estimate` is computed
/// from the actual rendered section sizes (including header markdown)
/// so callers can log / budget without re-counting.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryInjection {
    /// Markdown rendering of pinned items, or `None` when no pins
    /// exist for the active scope.
    pub pinned_section: Option<String>,
    /// Markdown rendering of `<scope>/.if2ai/memory/memory.md`, or
    /// `None` when the file does not exist or is empty.  Compiled by
    /// the 8B+ pipeline; until then this is always `None`.
    pub compiled_section: Option<String>,
    /// Always-included instructions on how the agent should consume
    /// the pinned + compiled sections.  Locale-aware
    /// (`is_zh` → Chinese; otherwise English).
    pub rules_section: String,
    /// Procedural rules learned from experience, generated by
    /// [`crate::modules::memory::evolution::ProceduralMemoryManager::format_for_prompt_injection`].
    /// `None` when no active procedures exist or the provider is unavailable.
    pub procedural_section: Option<String>,
    /// Sum of `pinned_section + compiled_section + rules_section`
    /// chars divided by [`CHARS_PER_TOKEN_ESTIMATE`].
    pub total_tokens_estimate: usize,
    /// Per-section cache hints for downstream provider adapters.
    pub cache_hints: MemoryCacheHints,
    /// Token-level cache efficiency statistics.
    pub cache_stats: CacheEfficiencyStats,
}

/// Cross-turn compiled-memory hash comparison.
/// When two consecutive calls produce the same compiled section,
/// the hint is promoted from Ephemeral → Stable.
///
/// Uses `AtomicU64` instead of `thread_local!` because Tokio's
/// multi-threaded runtime may schedule consecutive `.await` points
/// on different OS threads, causing a `thread_local!` cell to lose
/// the previous value and making the Stable promotion unreachable.
static LAST_COMPILED_HASH: AtomicU64 = AtomicU64::new(0);

/// Assemble the [`MemoryInjection`] for one agent turn.
///
/// `scope` decides which project's pins are merged with global pins
/// (delegates to [`PinnedStore::list_all_for_prompt`]).
/// `compiled_path` points at `<scope_root>/.if2ai/memory/memory.md`;
/// the file may not exist — that is normal until the 8B compile
/// pipeline runs (missing file → `compiled_section = None`, no
/// error).
///
/// `is_zh` selects the Chinese vs English rules-section wording (use
/// [`crate::modules::runtime::locale::is_zh`] in production).
///
/// `max_tokens` is converted to a chars budget via
/// [`CHARS_PER_TOKEN_ESTIMATE`]; the pinned section gets first
/// allocation, then compiled section, then procedural section.
/// When the budget cannot even fit the rules section, returns a
/// [`MemoryInjection`] containing only `rules_section` (caller
/// decides whether to drop it entirely).
///
/// `memory_provider` is an optional [`SharedMemoryProvider`] used to
/// fetch procedural rules via [`ProceduralMemoryManager`].  Pass
/// `None` to skip procedural injection (e.g. in tests or when the
/// provider is not yet available).
///
/// # Errors
///
/// Propagates [`MemoryError`] from
/// [`PinnedStore::list_all_for_prompt`].  Filesystem errors reading
/// `compiled_path` and procedural-memory retrieval failures are
/// intentionally swallowed so a missing file / unsupported provider
/// remains the happy path.
pub async fn build_memory_injection(
    pinned_store: Arc<dyn PinnedStore>,
    scope: &MemoryExecutionScope,
    compiled_path: &Path,
    is_zh: bool,
    max_tokens: usize,
    memory_provider: Option<SharedMemoryProvider>,
) -> Result<MemoryInjection, MemoryError> {
    let chars_budget = max_tokens.saturating_mul(CHARS_PER_TOKEN_ESTIMATE);

    let rules_section = render_rules_section(is_zh);
    let rules_chars = rules_section.chars().count();

    if chars_budget <= rules_chars {
        let total = rules_chars / CHARS_PER_TOKEN_ESTIMATE.max(1);
        return Ok(MemoryInjection {
            pinned_section: None,
            compiled_section: None,
            rules_section,
            procedural_section: None,
            total_tokens_estimate: total,
            cache_hints: MemoryCacheHints::default(),
            cache_stats: CacheEfficiencyStats {
                stable_tokens: total, // rules is Stable by default
                ephemeral_tokens: 0,
                uncached_tokens: 0,
                estimated_hit_rate: 1.0,
            },
        });
    }
    let mut remaining = chars_budget - rules_chars;

    let pins = pinned_store.list_all_for_prompt(scope).await?;
    let pinned_section = render_pinned_section(&pins, is_zh, &mut remaining);

    let compiled_section = render_compiled_section(compiled_path, is_zh, remaining);

    // Subtract compiled chars from remaining budget for procedural
    let compiled_chars = compiled_section
        .as_ref()
        .map(|s| s.chars().count())
        .unwrap_or(0);
    remaining = remaining.saturating_sub(compiled_chars);

    // --- Procedural memory injection (Phase 3 Task 10) ---
    let procedural_section =
        fetch_procedural_section(memory_provider.as_ref(), scope, remaining).await;

    let procedural_chars = procedural_section
        .as_ref()
        .map(|s| s.chars().count())
        .unwrap_or(0);

    let total_chars = pinned_section
        .as_ref()
        .map(|s| s.chars().count())
        .unwrap_or(0)
        + compiled_chars
        + procedural_chars
        + rules_chars;
    let total_tokens_estimate = total_chars / CHARS_PER_TOKEN_ESTIMATE.max(1);

    // --- Cache hints computation ---
    let mut cache_hints = MemoryCacheHints::default();

    // If compiled content hash matches the previous invocation,
    // promote from Ephemeral to Stable. We use a simple content
    // hash stored in a thread-local for cross-turn comparison.
    if let Some(ref compiled_text) = compiled_section {
        let mut hasher = DefaultHasher::new();
        compiled_text.hash(&mut hasher);
        let current_hash = hasher.finish();
        let prev = LAST_COMPILED_HASH.load(Ordering::Relaxed);
        if prev != 0 && prev == current_hash {
            cache_hints.compiled = CacheHint::Stable;
        }
        LAST_COMPILED_HASH.store(current_hash, Ordering::Relaxed);
    }

    // --- Cache efficiency stats ---
    let est = CHARS_PER_TOKEN_ESTIMATE.max(1);
    let pinned_tokens = pinned_section
        .as_ref()
        .map(|s| s.chars().count() / est)
        .unwrap_or(0);
    let compiled_tokens = compiled_chars / est;
    let procedural_tokens = procedural_chars / est;
    let rules_tokens = rules_chars / est;

    let mut stable_tokens = 0usize;
    let mut ephemeral_tokens = 0usize;
    let uncached_tokens = 0usize; // no sections default to None currently

    // Accumulate per-hint bucket
    fn add_to_bucket(hint: CacheHint, tokens: usize, stable: &mut usize, ephemeral: &mut usize) {
        match hint {
            CacheHint::Stable => *stable += tokens,
            CacheHint::Ephemeral => *ephemeral += tokens,
            CacheHint::None => {} // counted as uncached
        }
    }
    add_to_bucket(
        cache_hints.pinned,
        pinned_tokens,
        &mut stable_tokens,
        &mut ephemeral_tokens,
    );
    add_to_bucket(
        cache_hints.compiled,
        compiled_tokens,
        &mut stable_tokens,
        &mut ephemeral_tokens,
    );
    add_to_bucket(
        cache_hints.procedural,
        procedural_tokens,
        &mut stable_tokens,
        &mut ephemeral_tokens,
    );
    add_to_bucket(
        cache_hints.rules,
        rules_tokens,
        &mut stable_tokens,
        &mut ephemeral_tokens,
    );

    let total_for_rate = stable_tokens + ephemeral_tokens + uncached_tokens;
    let estimated_hit_rate = if total_for_rate > 0 {
        stable_tokens as f64 / total_for_rate as f64
    } else {
        0.0
    };

    let cache_stats = CacheEfficiencyStats {
        stable_tokens,
        ephemeral_tokens,
        uncached_tokens,
        estimated_hit_rate,
    };

    Ok(MemoryInjection {
        pinned_section,
        compiled_section,
        rules_section,
        procedural_section,
        total_tokens_estimate,
        cache_hints,
        cache_stats,
    })
}

fn render_pinned_section(
    pins: &[crate::modules::memory::pinned::PinnedItem],
    is_zh: bool,
    remaining: &mut usize,
) -> Option<String> {
    if pins.is_empty() {
        return None;
    }
    let header = if is_zh {
        "## 置顶记忆\n\n"
    } else {
        "## Pinned memory\n\n"
    };
    let header_chars = header.chars().count();
    if *remaining <= header_chars {
        return None;
    }
    let mut body = String::new();
    body.push_str(header);
    *remaining -= header_chars;

    for pin in pins {
        let content = truncate_chars_with_ellipsis(&pin.content, MAX_PIN_CONTENT_CHARS);
        let line = format!("- {content}\n");
        let line_chars = line.chars().count();
        if line_chars > *remaining {
            break;
        }
        *remaining -= line_chars;
        body.push_str(&line);
    }

    if body.chars().count() > header_chars {
        Some(body)
    } else {
        None
    }
}

fn render_compiled_section(compiled_path: &Path, is_zh: bool, remaining: usize) -> Option<String> {
    let text = std::fs::read_to_string(compiled_path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let header = if is_zh {
        "## 编译记忆\n\n"
    } else {
        "## Compiled memory\n\n"
    };
    let header_chars = header.chars().count();
    if remaining <= header_chars {
        return None;
    }
    let body_budget = remaining - header_chars;
    let body = truncate_chars_with_ellipsis(trimmed, body_budget);
    if body.is_empty() {
        return None;
    }
    let mut content = String::with_capacity(header.len() + body.len() + 1);
    content.push_str(header);
    content.push_str(&body);
    content.push('\n');
    Some(content)
}

/// "Memory usage rules" — the agent-facing instructions on how to
/// consume the pinned + compiled sections above.  Adapted from
/// `openhanako` `core/agent.js:626-644` (中文 / English variants).
fn render_rules_section(is_zh: bool) -> String {
    if is_zh {
        "## 记忆使用规则\n\n\
         - **置顶记忆**是用户明确钉住的事实，每次回复都必须遵守，不要质疑或忽略\n\
         - **编译记忆**是从历史对话提炼的长期上下文，作为推理背景\n\
         - 如果用户问的事实在置顶记忆里，直接引用；不要再调用 memory_recall 重复确认\n\
         - 如果置顶记忆与本轮对话冲突，以本轮对话为准并主动询问是否更新置顶\n\
         - 不要在回复中显式提到\"置顶记忆\"或\"编译记忆\"这些字面，自然地使用即可\n"
            .to_string()
    } else {
        "## Memory usage rules\n\n\
         - **Pinned memory** is facts the user explicitly pinned; honour them every reply, never question or ignore.\n\
         - **Compiled memory** is long-term context distilled from prior conversations; use it as reasoning background.\n\
         - If a user-asked fact lives in pinned memory, cite it directly — do not call `memory_recall` to re-confirm.\n\
         - If pinned memory contradicts the current turn, defer to the current turn and proactively ask whether to update the pin.\n\
         - Never mention the literal phrases \"pinned memory\" or \"compiled memory\" to the user; just consume them naturally.\n"
            .to_string()
    }
}

/// Fetch procedural rules from the memory provider, returning `None`
/// on any failure (unsupported provider, empty result, budget exhausted).
///
/// Silently degrades: errors are logged at `warn` level and the
/// caller receives `None`.
async fn fetch_procedural_section(
    provider: Option<&SharedMemoryProvider>,
    scope: &MemoryExecutionScope,
    remaining_chars: usize,
) -> Option<String> {
    let provider = provider?;
    if remaining_chars == 0 {
        return None;
    }

    let mgr = ProceduralMemoryManager::new(Arc::clone(provider));
    match mgr.format_for_prompt_injection(scope, 10).await {
        Ok(text) if text.is_empty() => None,
        Ok(text) => {
            let text = truncate_chars_with_ellipsis(&text, remaining_chars);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[memory::inject] procedural memory retrieval failed; skipping"
            );
            None
        }
    }
}

/// Char-aware truncation (NOT byte-based, to avoid CJK panics).
///
/// Returns the input unchanged when its char count fits within `cap`.
/// Otherwise truncates to `cap - 1` chars and appends `…` (single
/// ellipsis char), so the result's char count is exactly `cap`.
/// Returns the empty string when `cap == 0`.
fn truncate_chars_with_ellipsis(text: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= cap {
        return text.to_string();
    }
    let mut out: String = text.chars().take(cap - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::pinned::types::{PinScope, PinSource, PinnedItem};
    use crate::modules::memory::pinned::NullPinnedStore;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use std::sync::Mutex;

    /// Serialises tests that depend on the global `LAST_COMPILED_HASH`
    /// atomic, preventing concurrent tests from clobbering each other's
    /// stored hash value.
    static HASH_TEST_MUTEX: Mutex<()> = Mutex::new(());
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// In-memory PinnedStore mock backed by a `Vec<PinnedItem>` —
    /// only `list_all_for_prompt` is exercised; other trait methods
    /// return defaults so tests stay focused on injection rendering.
    struct MockPinned(Vec<PinnedItem>);

    #[async_trait]
    impl PinnedStore for MockPinned {
        async fn list(
            &self,
            _scope: PinScope,
            _project_id: Option<&str>,
        ) -> Result<Vec<PinnedItem>, MemoryError> {
            Ok(self.0.clone())
        }
        async fn list_all_for_prompt(
            &self,
            _scope: &MemoryExecutionScope,
        ) -> Result<Vec<PinnedItem>, MemoryError> {
            Ok(self.0.clone())
        }
        async fn add(
            &self,
            _content: &str,
            _scope: PinScope,
            _source: PinSource,
            _project_id: Option<&str>,
        ) -> Result<PinnedItem, MemoryError> {
            Err(MemoryError::Generic("mock".to_string()))
        }
        async fn delete(&self, _id: &str) -> Result<bool, MemoryError> {
            Ok(false)
        }
        async fn reorder(&self, _ids: &[String]) -> Result<(), MemoryError> {
            Ok(())
        }
    }

    fn make_pin(id: &str, content: &str) -> PinnedItem {
        PinnedItem {
            id: id.to_string(),
            content: content.to_string(),
            scope: PinScope::Global,
            project_id: None,
            created_at: Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now),
            created_by: PinSource::User,
        }
    }

    fn empty_scope() -> MemoryExecutionScope {
        MemoryExecutionScope {
            session_id: None,
            project_id: None,
            workdir: None,
        }
    }

    fn missing_path() -> PathBuf {
        PathBuf::from("/nonexistent/path/that/should/never/exist/memory.md")
    }

    #[tokio::test]
    async fn build_with_no_pins_no_compiled_returns_rules_only() {
        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), false, 2000, None)
                .await
                .expect("build_memory_injection");
        assert!(result.pinned_section.is_none());
        assert!(result.compiled_section.is_none());
        assert!(result.rules_section.contains("## Memory usage rules"));
    }

    #[tokio::test]
    async fn build_with_pins_includes_pinned_section() {
        let store: Arc<dyn PinnedStore> = Arc::new(MockPinned(vec![
            make_pin("01", "hello world"),
            make_pin("02", "foo bar"),
        ]));
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), false, 2000, None)
                .await
                .expect("build_memory_injection");
        let pinned = result.pinned_section.expect("pinned section present");
        assert!(pinned.contains("## Pinned memory"));
        assert!(pinned.contains("- hello world"));
        assert!(pinned.contains("- foo bar"));
    }

    #[tokio::test]
    async fn build_with_compiled_file_includes_compiled_section() {
        let tmp = tempdir().expect("tempdir");
        let compiled = tmp.path().join("memory.md");
        std::fs::write(&compiled, "compiled body content").expect("write compiled");
        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let result = build_memory_injection(store, &empty_scope(), &compiled, false, 2000, None)
            .await
            .expect("build_memory_injection");
        let section = result.compiled_section.expect("compiled section present");
        assert!(section.contains("## Compiled memory"));
        assert!(section.contains("compiled body content"));
    }

    #[tokio::test]
    async fn budget_truncation_pinned_priority() {
        // One large pin (200 chars) + a large compiled body — under a
        // tight budget, the pinned section is rendered in full and
        // the compiled section is truncated (with `…`) to whatever
        // space remains.  We assert both: pinned present in full,
        // compiled present but shorter than the source.
        let large_pin = "x".repeat(200);
        let large_compiled = "y".repeat(2000);
        let store: Arc<dyn PinnedStore> = Arc::new(MockPinned(vec![make_pin("01", &large_pin)]));
        let tmp = tempdir().expect("tempdir");
        let compiled = tmp.path().join("memory.md");
        std::fs::write(&compiled, &large_compiled).expect("write compiled");

        let result = build_memory_injection(store, &empty_scope(), &compiled, false, 400, None)
            .await
            .expect("build_memory_injection");

        let pinned = result.pinned_section.expect("pinned wins budget first");
        assert!(
            pinned.contains(&large_pin),
            "pinned section must contain the full 200-char body"
        );
        let compiled_body = result.compiled_section.expect("compiled fits remainder");
        assert!(
            compiled_body.contains('…'),
            "compiled body must be truncated with ellipsis under tight budget"
        );
        assert!(
            compiled_body.chars().count() < large_compiled.chars().count(),
            "compiled body must be shorter than source after truncation"
        );
    }

    #[tokio::test]
    async fn degenerate_budget_returns_rules_only() {
        let store: Arc<dyn PinnedStore> = Arc::new(MockPinned(vec![make_pin("01", "anything")]));
        let result = build_memory_injection(store, &empty_scope(), &missing_path(), false, 1, None)
            .await
            .expect("build_memory_injection");
        assert!(result.pinned_section.is_none());
        assert!(result.compiled_section.is_none());
        assert!(result.rules_section.contains("## Memory usage rules"));
    }

    #[tokio::test]
    async fn is_zh_true_uses_chinese_rules() {
        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), true, 2000, None)
                .await
                .expect("build_memory_injection");
        assert!(
            result.rules_section.contains("## 记忆使用规则"),
            "Chinese rules header missing: {}",
            result.rules_section
        );
    }

    #[tokio::test]
    async fn is_zh_false_uses_english_rules() {
        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), false, 2000, None)
                .await
                .expect("build_memory_injection");
        assert!(result.rules_section.contains("## Memory usage rules"));
        assert!(!result.rules_section.contains("## 记忆使用规则"));
    }

    #[tokio::test]
    async fn missing_compiled_file_is_silently_handled() {
        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), false, 2000, None)
                .await
                .expect("build_memory_injection");
        assert!(result.compiled_section.is_none());
    }

    #[test]
    fn truncate_chars_handles_cjk_without_panic() {
        let cjk = "你好世界这是一段中文";
        let out = truncate_chars_with_ellipsis(cjk, 5);
        assert_eq!(out.chars().count(), 5);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_chars_returns_input_when_within_cap() {
        assert_eq!(truncate_chars_with_ellipsis("abc", 10), "abc");
    }

    #[test]
    fn truncate_chars_zero_cap_returns_empty() {
        assert_eq!(truncate_chars_with_ellipsis("abc", 0), "");
    }

    #[tokio::test]
    async fn pin_content_truncated_when_exceeds_max_pin_chars() {
        // Defensive cap: even legacy pins beyond MAX_PIN_CONTENT_CHARS
        // are rendered with ellipsis (line is `- {trunc}\n`).
        let oversize = "y".repeat(MAX_PIN_CONTENT_CHARS + 50);
        let store: Arc<dyn PinnedStore> = Arc::new(MockPinned(vec![make_pin("01", &oversize)]));
        let result =
            build_memory_injection(store, &empty_scope(), &missing_path(), false, 5000, None)
                .await
                .expect("build_memory_injection");
        let pinned = result.pinned_section.expect("pinned section present");
        // The line itself is the pin body wrapped in `- ... \n`,
        // so chars beyond MAX_PIN_CONTENT_CHARS must be elided.
        assert!(pinned.contains('…'));
    }

    // --- Prompt Caching integration tests ---

    #[test]
    fn test_default_cache_hints() {
        let hints = MemoryCacheHints::default();
        assert_eq!(hints.pinned, CacheHint::Stable);
        assert_eq!(hints.compiled, CacheHint::Ephemeral);
        assert_eq!(hints.procedural, CacheHint::Stable);
        assert_eq!(hints.rules, CacheHint::Stable);
    }

    #[test]
    fn test_cache_hint_default_is_none() {
        assert_eq!(CacheHint::default(), CacheHint::None);
    }

    #[test]
    fn test_cache_hint_serde_round_trip() {
        let hints = MemoryCacheHints::default();
        let json = serde_json::to_string(&hints).expect("serialize");
        let parsed: MemoryCacheHints = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.pinned, CacheHint::Stable);
        assert_eq!(parsed.compiled, CacheHint::Ephemeral);
    }

    #[tokio::test]
    async fn test_cache_hints_propagation() {
        let _lock = HASH_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        LAST_COMPILED_HASH.store(0, Ordering::Relaxed);
        // Build with pins + compiled → verify cache_hints are set correctly
        let store: Arc<dyn PinnedStore> = Arc::new(MockPinned(vec![make_pin("01", "test pin")]));
        let tmp = tempdir().expect("tempdir");
        let compiled = tmp.path().join("memory.md");
        std::fs::write(&compiled, "compiled content").expect("write");

        let result = build_memory_injection(store, &empty_scope(), &compiled, false, 2000, None)
            .await
            .expect("build_memory_injection");

        // Default hints: pinned=Stable, compiled=Ephemeral (first call),
        // procedural=Stable, rules=Stable
        assert_eq!(result.cache_hints.pinned, CacheHint::Stable);
        assert_eq!(result.cache_hints.rules, CacheHint::Stable);
        assert_eq!(result.cache_hints.procedural, CacheHint::Stable);
        // First call → compiled stays Ephemeral (no previous hash)
        assert_eq!(result.cache_hints.compiled, CacheHint::Ephemeral);
    }

    #[tokio::test]
    async fn test_cache_efficiency_stats() {
        let _lock = HASH_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        LAST_COMPILED_HASH.store(0, Ordering::Relaxed);
        let store: Arc<dyn PinnedStore> =
            Arc::new(MockPinned(vec![make_pin("01", "x".repeat(40).as_str())]));
        let tmp = tempdir().expect("tempdir");
        let compiled = tmp.path().join("memory.md");
        std::fs::write(&compiled, "y".repeat(80)).expect("write");

        let result = build_memory_injection(store, &empty_scope(), &compiled, false, 2000, None)
            .await
            .expect("build_memory_injection");

        // Stats should be non-zero
        let stats = &result.cache_stats;
        assert!(
            stats.stable_tokens > 0,
            "stable tokens should be positive (pinned+rules)"
        );
        assert!(
            stats.ephemeral_tokens > 0,
            "ephemeral tokens should be positive (compiled)"
        );
        let total = stats.stable_tokens + stats.ephemeral_tokens + stats.uncached_tokens;
        assert!(total > 0, "total tokens should be positive");
        assert!(
            stats.estimated_hit_rate > 0.0 && stats.estimated_hit_rate <= 1.0,
            "estimated_hit_rate should be between 0 and 1, got {}",
            stats.estimated_hit_rate
        );
    }

    #[tokio::test]
    async fn test_compiled_hash_promotion_to_stable() {
        let _lock = HASH_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Reset the global atomic hash
        LAST_COMPILED_HASH.store(0, Ordering::Relaxed);

        let store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());
        let tmp = tempdir().expect("tempdir");
        let compiled = tmp.path().join("memory.md");
        std::fs::write(&compiled, "stable compiled content").expect("write");

        // First call — no previous hash → Ephemeral
        let r1 =
            build_memory_injection(store.clone(), &empty_scope(), &compiled, false, 2000, None)
                .await
                .expect("first");
        assert_eq!(r1.cache_hints.compiled, CacheHint::Ephemeral);

        // Second call with same content → promoted to Stable
        let r2 =
            build_memory_injection(store.clone(), &empty_scope(), &compiled, false, 2000, None)
                .await
                .expect("second");
        assert_eq!(r2.cache_hints.compiled, CacheHint::Stable);
    }
}
