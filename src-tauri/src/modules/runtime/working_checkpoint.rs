//! FEAT-DK-002 — Working checkpoint extraction + injection.
//!
//! Two pure functions implementing the GenericAgent `<key_info>`
//! pattern:
//!
//! 1. [`extract_checkpoint`] parses agent-emitted text for
//!    `<key_info>...</key_info>` and `<related_sop>...</related_sop>`
//!    + the standalone `<task_complete/>` sentinel.
//! 2. [`inject_checkpoint`] inserts a `[checkpoint] ...` text block
//!    into a `Vec<InputMessage>` at the configured
//!    [`InjectionPosition`], hard-capped at
//!    [`DEFAULT_CHECKPOINT_MAX_TOKENS`] (= 200) using
//!    [`crate::modules::runtime::budget::estimate_tokens`].
//!
//! Both functions are sync, allocation-light, never call any async
//! context, and never touch disk. The actual orchestration (calling
//! `extract_checkpoint` after each turn, holding the
//! `WorkingCheckpoint` per-session, calling `inject_checkpoint` in
//! `stream_preflight`) is left for a future wiring Pack — Pack
//! contract `OOS` for FEAT-DK-002 explicitly excludes those hooks.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::runtime::budget::estimate_tokens;

/// Default token ceiling for the injected checkpoint block.
///
/// Keeps the spend predictable: even when the agent stuffs a long
/// `<key_info>` blob, the prompt grows by at most ~200 tokens.
pub const DEFAULT_CHECKPOINT_MAX_TOKENS: usize = 200;

/// One persistent checkpoint per active session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCheckpoint {
    pub session_id: String,
    pub key_info: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_sop: Option<String>,
    pub turn_created: u64,
    pub turn_updated: u64,
}

/// Result of [`extract_checkpoint`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckpointExtraction {
    pub key_info: Option<String>,
    pub related_sop: Option<String>,
    /// `true` when the agent signalled `<task_complete/>` so the
    /// orchestrator should drop the per-session checkpoint.
    pub should_clear: bool,
}

/// Where the injected checkpoint block is placed in the message
/// stream. Both positions are valid; choose per scenario:
///
/// - [`InjectionPosition::BeforeLastUserMessage`] mirrors the
///   GenericAgent pattern (model sees checkpoint *just before*
///   reading the user's question).
/// - [`InjectionPosition::AsSystemBlock`] prepends as a system-role
///   block — fits If2Ai's `prompt_planner` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPosition {
    BeforeLastUserMessage,
    AsSystemBlock,
}

/// Configuration for [`inject_checkpoint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointInjectionConfig {
    pub max_tokens: usize,
    pub injection_position: InjectionPosition,
    /// Reserved for the future wiring Pack: when `false`, the
    /// orchestrator MAY skip calling `extract_checkpoint` even if a
    /// `<key_info>` block appears in the agent output.
    pub auto_extract: bool,
}

impl Default for CheckpointInjectionConfig {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_CHECKPOINT_MAX_TOKENS,
            injection_position: InjectionPosition::BeforeLastUserMessage,
            auto_extract: true,
        }
    }
}

const KEY_INFO_OPEN: &str = "<key_info>";
const KEY_INFO_CLOSE: &str = "</key_info>";
const RELATED_SOP_OPEN: &str = "<related_sop>";
const RELATED_SOP_CLOSE: &str = "</related_sop>";
const TASK_COMPLETE_TAG: &str = "<task_complete/>";

/// Pull `<key_info>` / `<related_sop>` / `<task_complete/>` markers
/// from the agent's accumulated text.  Pure & cheap — safe to call
/// after every streamed token chunk.
#[must_use]
pub fn extract_checkpoint(accumulated_text: &str) -> CheckpointExtraction {
    let key_info = extract_first_tagged_block(accumulated_text, KEY_INFO_OPEN, KEY_INFO_CLOSE);
    let related_sop =
        extract_first_tagged_block(accumulated_text, RELATED_SOP_OPEN, RELATED_SOP_CLOSE);
    let should_clear = accumulated_text.contains(TASK_COMPLETE_TAG);
    CheckpointExtraction {
        key_info,
        related_sop,
        should_clear,
    }
}

/// Inject a `[checkpoint] ...` text block into `messages` at
/// `config.injection_position`. The block content is truncated so
/// `estimate_tokens(content) <= config.max_tokens`.
///
/// Mutates `messages` in place. No-op when the checkpoint's
/// `key_info` is empty.
pub fn inject_checkpoint(
    checkpoint: &WorkingCheckpoint,
    messages: &mut Vec<InputMessage>,
    config: &CheckpointInjectionConfig,
) {
    if checkpoint.key_info.trim().is_empty() {
        return;
    }
    let mut payload = format_payload(checkpoint);
    truncate_to_token_budget(&mut payload, config.max_tokens);
    let role = match config.injection_position {
        InjectionPosition::BeforeLastUserMessage => "user".to_string(),
        InjectionPosition::AsSystemBlock => "system".to_string(),
    };
    let block = InputMessage {
        role,
        content: vec![InputContentBlock::Text { text: payload }],
        thinking: None,
    };

    match config.injection_position {
        InjectionPosition::AsSystemBlock => {
            messages.insert(0, block);
        }
        InjectionPosition::BeforeLastUserMessage => {
            let insert_at = match messages.iter().rposition(|m| m.role == "user") {
                Some(idx) => idx,
                None => messages.len(),
            };
            messages.insert(insert_at, block);
        }
    }
}

fn format_payload(checkpoint: &WorkingCheckpoint) -> String {
    match checkpoint.related_sop.as_deref() {
        Some(sop) if !sop.is_empty() => format!(
            "[checkpoint] {}\n[related_sop] {sop}",
            checkpoint.key_info.trim()
        ),
        _ => format!("[checkpoint] {}", checkpoint.key_info.trim()),
    }
}

fn truncate_to_token_budget(text: &mut String, max_tokens: usize) {
    if estimate_tokens(text) <= max_tokens {
        return;
    }
    // estimate_tokens ≈ chars / 4; pick a char ceiling that lands
    // safely inside the budget after the suffix marker.
    let target_chars = max_tokens.saturating_mul(4);
    let suffix = "…[truncated]";
    let suffix_chars = suffix.chars().count();
    let ceil = target_chars.saturating_sub(suffix_chars);
    let truncated: String = text.chars().take(ceil).collect();
    *text = format!("{truncated}{suffix}");
    if estimate_tokens(text) <= max_tokens {
        return;
    }
    // Defensive second pass — should never trigger in practice.
    let truncated2: String = text.chars().take(ceil / 2).collect();
    *text = truncated2;
}

fn extract_first_tagged_block(text: &str, open: &str, close: &str) -> Option<String> {
    let start = text.find(open)?;
    let after_open = start + open.len();
    let end = text[after_open..].find(close)?;
    let body = text[after_open..after_open + end].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}
