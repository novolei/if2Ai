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

use std::sync::Arc;

use crate::modules::api::InputMessage;
use crate::modules::memory::UtilityLlm;
use crate::modules::runtime::context_compression::{apply_digest, MessageDigester};
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
