//! WU-004 — Preflight wire-up helpers (TE-002 digester, TE-003 mini
//! index, DK-002 checkpoint).
//!
//! All helpers are pure / async-friendly and **fail closed**: any
//! error path falls back to the legacy preflight behavior so the
//! agent loop never breaks because of an opt-in evolution feature.
//!
//! Production callers (currently a future deeper-wire Pack) plug
//! these between the streaming orchestrator and
//! [`super::stream_preflight::build_iteration_request`]:
//!
//! 1. [`digest_messages_for_preflight`] async — passes the result
//!    into `PreflightContext.digested_messages`.
//! 2. [`maybe_inject_checkpoint`] sync — mutates the message vec
//!    in place, broadcasts a `CheckpointUpdated` envelope.
//! 3. [`maybe_render_mini_index_block`] sync — returns
//!    `Some(PromptBlock)` for the planner to push at priority 95.

#![allow(dead_code)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::memory::UtilityLlm;
use crate::modules::runtime::context_compression::{apply_digest, CompressedMessage, MessageDigester};
use crate::modules::runtime::working_checkpoint::{
    inject_checkpoint, CheckpointInjectionConfig, WorkingCheckpoint,
};

/// Env var disabling the WU-004 digester injection.
pub const DISABLE_DIGESTER_ENV: &str = "IF2AI_DISABLE_DIGESTER";

/// Env var disabling the WU-004 mini-index block push.
pub const DISABLE_MINI_INDEX_ENV: &str = "IF2AI_DISABLE_MINI_INDEX";

fn flag_set(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

// ---- Digest caching for critical-path optimization ----

/// Cached digest results from a previous iteration. Allows incremental
/// digest: only new/changed messages at the tail are sent through the
/// expensive LLM summarization path; the unchanged prefix is reused.
#[derive(Debug, Clone)]
pub struct DigestCache {
    /// Per-message content fingerprints from the last digest run.
    pub fingerprints: Vec<u64>,
    /// Compressed results from the last digest run (same length as
    /// `fingerprints`).
    pub results: Vec<CompressedMessage>,
}

/// Compute a cheap content fingerprint for one message. Two messages
/// with identical role + content blocks + thinking produce the same
/// hash. Used to detect unchanged messages across iterations so the
/// digester can skip re-processing them.
fn message_fingerprint(msg: &InputMessage) -> u64 {
    let mut hasher = DefaultHasher::new();
    msg.role.hash(&mut hasher);
    if let Some(ref thinking) = msg.thinking {
        thinking.hash(&mut hasher);
    }
    for block in &msg.content {
        match block {
            InputContentBlock::Text { text } => {
                0u8.hash(&mut hasher);
                text.hash(&mut hasher);
            }
            InputContentBlock::ToolUse { id, name, input } => {
                1u8.hash(&mut hasher);
                id.hash(&mut hasher);
                name.hash(&mut hasher);
                input.to_string().hash(&mut hasher);
            }
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                2u8.hash(&mut hasher);
                tool_use_id.hash(&mut hasher);
                is_error.hash(&mut hasher);
                for tr in content {
                    use crate::modules::api::ToolResultContentBlock;
                    match tr {
                        ToolResultContentBlock::Text { text } => text.hash(&mut hasher),
                        ToolResultContentBlock::Json { value } => {
                            value.to_string().hash(&mut hasher)
                        }
                        ToolResultContentBlock::Image { .. } => 3u8.hash(&mut hasher),
                    }
                }
            }
        }
    }
    hasher.finish()
}

/// Run the TE-002 digester over `messages` and materialize the
/// resulting `Vec<InputMessage>`. Returns `None` when the digester
/// is disabled, when no compression actually happened (digest is
/// identical to input → caller should pass the original through),
/// or — implicitly — when the digester degrades internally (it
/// already preserves originals on LLM failure, so the returned
/// vec is *always* the same length as `messages`).
pub async fn digest_messages_for_preflight(
    messages: &[InputMessage],
    llm: Arc<dyn UtilityLlm>,
) -> Option<Vec<InputMessage>> {
    if flag_set(DISABLE_DIGESTER_ENV) {
        return None;
    }
    if messages.is_empty() {
        return None;
    }
    let digester = MessageDigester::new(llm);
    let digest = digester.digest(messages).await;
    let materialized = apply_digest(&digest);
    if materialized.is_empty() {
        return None;
    }
    Some(materialized)
}

/// Incremental variant of [`digest_messages_for_preflight`] that reuses
/// cached digest results for an unchanged message prefix.
///
/// On the first call (`cache` is `None`), behaves identically to the
/// non-cached version but populates the cache for subsequent calls.
/// On subsequent calls, computes per-message fingerprints and finds
/// the longest unchanged prefix. Only messages after that prefix are
/// sent through the (expensive, LLM-backed) digester — the rest are
/// served from the cache.
///
/// Returns `(materialized_messages, updated_cache)`. The cache should
/// be stored on the loop state and threaded back on the next iteration.
pub async fn digest_messages_for_preflight_cached(
    messages: &[InputMessage],
    llm: Arc<dyn UtilityLlm>,
    cache: Option<DigestCache>,
) -> (Option<Vec<InputMessage>>, Option<DigestCache>) {
    if flag_set(DISABLE_DIGESTER_ENV) || messages.is_empty() {
        return (None, cache);
    }

    let fingerprints: Vec<u64> = messages.iter().map(message_fingerprint).collect();

    // Find the longest matching prefix with the cached fingerprints.
    let reuse_count = cache
        .as_ref()
        .map(|c| {
            c.fingerprints
                .iter()
                .zip(fingerprints.iter())
                .take_while(|(a, b)| a == b)
                .count()
        })
        .unwrap_or(0);

    // Fast path: all messages unchanged → reuse entire cached result.
    if reuse_count == messages.len() {
        if let Some(ref cached) = cache {
            let materialized = apply_digest(&cached.results);
            if materialized.is_empty() {
                return (None, cache);
            }
            tracing::debug!(
                "[digest_cache] full HIT: reusing {} cached digest results",
                reuse_count
            );
            return (Some(materialized), cache);
        }
    }

    // Slow path: digest only the new/changed tail.
    let digester = MessageDigester::new(llm);
    let results = if reuse_count > 0 {
        let cached = cache.as_ref().expect("reuse_count > 0 implies cache exists");
        let mut results = cached.results[..reuse_count].to_vec();
        tracing::debug!(
            "[digest_cache] partial HIT: reusing {} of {} messages, digesting {} new",
            reuse_count,
            messages.len(),
            messages.len() - reuse_count,
        );
        let new_digest = digester.digest(&messages[reuse_count..]).await;
        results.extend(new_digest);
        results
    } else {
        tracing::debug!(
            "[digest_cache] MISS: digesting all {} messages",
            messages.len()
        );
        digester.digest(messages).await
    };

    let materialized = apply_digest(&results);
    let new_cache = DigestCache {
        fingerprints,
        results,
    };
    if materialized.is_empty() {
        (None, Some(new_cache))
    } else {
        (Some(materialized), Some(new_cache))
    }
}

/// Inject `checkpoint` into `messages` using the DK-002 helper.
/// Returns `true` when injection actually happened (callers can use
/// the boolean to decide whether to broadcast a `CheckpointUpdated`
/// envelope). No-op + `false` when `checkpoint` is `None` or its
/// `key_info` is empty.
pub fn maybe_inject_checkpoint(
    checkpoint: Option<&WorkingCheckpoint>,
    messages: &mut Vec<InputMessage>,
    config: &CheckpointInjectionConfig,
) -> bool {
    let Some(cp) = checkpoint else {
        return false;
    };
    if cp.key_info.trim().is_empty() {
        return false;
    }
    let before = messages.len();
    inject_checkpoint(cp, messages, config);
    messages.len() > before
}

/// Wrap TE-003's `render_mini_index_block` with the WU-004 disable
/// flag. Returns `None` when the kill-switch is set, when the input
/// is empty, or when the mini-index text is empty (TE-003's helper
/// already returns `None` for empty input).
pub fn maybe_render_mini_index_block(
    messages: &[InputMessage],
) -> Option<crate::modules::application::prompt_planner::PromptBlock> {
    if flag_set(DISABLE_MINI_INDEX_ENV) {
        return None;
    }
    let text = crate::modules::runtime::context_compression::build_mini_index(messages);
    if text.trim().is_empty() {
        return None;
    }
    crate::modules::application::prompt_planner::render_mini_index_block(text)
}
