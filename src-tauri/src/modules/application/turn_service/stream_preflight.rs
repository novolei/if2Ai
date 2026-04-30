//! Preflight + iteration request builder extracted from
//! [`crate::modules::application::turn_service::stream_task::run_stream_task`].
//!
//! Owns: effective input-token budget computation, ContextGovernor preflight
//! admission, sanitize_messages_for_provider pass, force-final-response
//! injection, and the double-admit pattern (trimmed then final request).
//!
//! Lifted out of `stream_task.rs` per GAP-005.

use crate::modules::api::{InputMessage, MessageRequest, ToolChoice, ToolDefinition};
use crate::modules::application::prompt_planner::{
    extend_sample_ids, sanitize_messages_for_provider, ContextGovernor,
};
use crate::modules::runtime::budget::{
    MAX_REQUEST_CHAR_BUDGET, MAX_REQUEST_MESSAGE_COUNT, MAX_REQUEST_TOKEN_BUDGET_ESTIMATE,
};
use crate::modules::runtime::context_compression::{compress_for_request, TierBudgetAllocation};

/// All context needed to build one API iteration's request.
pub(super) struct PreflightContext<'a> {
    pub session_messages: &'a [InputMessage],
    /// FEAT-TE-002: optional pre-computed digest of `session_messages`.
    /// When `Some`, each entry replaces the corresponding original
    /// message (typically a long message → its LLM summary). Length
    /// MUST equal `session_messages.len()`; mismatched inputs are
    /// silently ignored and the original messages are used.
    /// Computed asynchronously by the orchestrator (see
    /// [`crate::modules::runtime::context_compression::MessageDigester`])
    /// before entering this synchronous preflight.
    pub digested_messages: Option<&'a [InputMessage]>,
    pub tool_defs: &'a [ToolDefinition],
    pub system_prompt: &'a str,
    pub model: &'a str,
    pub context_window: u64,
    pub force_final_response: bool,
    pub finalization_reason: &'a str,
    pub force_tool_choice: bool,
    /// Phase 3 T3 (N2-β): when `true`, drop all tool definitions from the
    /// outgoing request so the provider is forced to produce a text reply.
    /// Set by `run_agentic_loop` after `force_text_after_truncations`
    /// consecutive `length`-truncation finishes — a safety valve for
    /// runaway tool-call loops that keep getting cut off mid-output.
    pub force_text: bool,
    pub tool_loop_iter: usize,
    pub max_iterations: usize,
    pub stream_id: &'a str,
    pub session_id: &'a str,
}

/// Results of building an iteration request, including updated stats.
pub(super) struct PreflightResult {
    pub request: MessageRequest,
    pub session_messages: Vec<InputMessage>,
    pub preflight_trim_rounds_added: usize,
    pub preflight_dropped_messages_added: usize,
    pub preflight_trimmed_chars_added: usize,
    pub sanitize_rounds_added: usize,
    pub sanitized_dropped_empty_messages_added: usize,
    pub sanitized_dropped_orphan_tool_results_added: usize,
    pub sanitized_dropped_unmatched_tool_uses_added: usize,
    pub sanitized_dropped_invalid_tool_use_inputs_added: usize,
    pub sanitize_orphan_samples_added: Vec<String>,
    pub sanitize_unmatched_samples_added: Vec<String>,
    pub sanitize_invalid_tool_use_samples_added: Vec<String>,
}

const PREFLIGHT_OUTPUT_RESERVE: u64 = 4_096;

/// Build the [`MessageRequest`] for one outer-loop iteration.
///
/// Applies context-governor preflight admission + sanitize in the
/// same double-admit pattern the original closure used. Returns
/// the built request alongside incremental stat deltas so the
/// orchestrator can sum them into its turn-wide accumulators.
pub(super) fn build_iteration_request(ctx: PreflightContext<'_>) -> PreflightResult {
    let PreflightContext {
        session_messages,
        digested_messages,
        tool_defs,
        system_prompt,
        model,
        context_window,
        force_final_response,
        finalization_reason,
        force_tool_choice,
        force_text,
        tool_loop_iter,
        max_iterations,
        stream_id,
        session_id: _session_id,
    } = ctx;

    let mut preflight_trim_rounds_added = 0usize;
    let mut preflight_dropped_messages_added = 0usize;
    let mut preflight_trimmed_chars_added = 0usize;
    let mut sanitize_rounds_added = 0usize;
    let mut sanitized_dropped_empty_messages_added = 0usize;
    let mut sanitized_dropped_orphan_tool_results_added = 0usize;
    let mut sanitized_dropped_unmatched_tool_uses_added = 0usize;
    let mut sanitized_dropped_invalid_tool_use_inputs_added = 0usize;
    let mut sanitize_orphan_samples_added: Vec<String> = Vec::new();
    let mut sanitize_unmatched_samples_added: Vec<String> = Vec::new();
    let mut sanitize_invalid_tool_use_samples_added: Vec<String> = Vec::new();

    let preflight_input_budget = (context_window.saturating_sub(PREFLIGHT_OUTPUT_RESERVE))
        .max(MAX_REQUEST_TOKEN_BUDGET_ESTIMATE as u64) as usize;

    // FEAT-TE-001: enforce the 5-tier hard budget *before* the existing
    // ContextGovernor preflight. The governor still runs (char + message
    // count caps) so legacy diagnostics keep emitting, but the tier cap
    // guarantees we never feed it more than `TierBudgetAllocation::total()`
    // tokens of history regardless of upstream behaviour.
    // FEAT-TE-002: when the orchestrator pre-computed an LLM digest
    // (long messages collapsed to summaries), feed *that* slice into
    // the tier cap; otherwise the original session messages are used.
    let digest_owned: Vec<InputMessage>;
    let messages_for_tier: &[InputMessage] = match digested_messages {
        Some(d) if d.len() == session_messages.len() => {
            digest_owned = d.to_vec();
            &digest_owned
        }
        Some(d) => {
            tracing::warn!(
                "[start_agent_stream] digest length mismatch (digested={}, session={}); ignoring digest",
                d.len(),
                session_messages.len(),
            );
            session_messages
        }
        None => session_messages,
    };
    let tier_allocation = TierBudgetAllocation::default();
    let tier_outcome = compress_for_request(messages_for_tier, &tier_allocation);
    if tier_outcome.dropped > 0 {
        tracing::warn!(
            "[start_agent_stream] tier hard-cap dropped {} oldest message(s): stream_id={}, session_id={}, kept={}, kept_tokens={}, tier_total={}",
            tier_outcome.dropped,
            stream_id,
            _session_id,
            tier_outcome.kept.len(),
            tier_outcome.kept_tokens,
            tier_allocation.total(),
        );
    }
    let tier_capped_messages: Vec<InputMessage> = if tier_outcome.passthrough {
        messages_for_tier.to_vec()
    } else {
        tier_outcome.kept
    };

    let (trimmed_session_messages, preflight_stats) = ContextGovernor.admit(
        &tier_capped_messages,
        MAX_REQUEST_MESSAGE_COUNT,
        MAX_REQUEST_CHAR_BUDGET,
        preflight_input_budget,
    );
    if preflight_stats.has_changes() {
        preflight_trim_rounds_added += 1;
        preflight_dropped_messages_added += preflight_stats.dropped_messages;
        preflight_trimmed_chars_added += preflight_stats.trimmed_chars;
        tracing::warn!(
            "[start_agent_stream] preflight request trim: stream_id={}, session_id={}, before_messages={}, after_messages={}, before_chars={}, after_chars={}, dropped_messages={}, trimmed_chars={}",
            stream_id,
            _session_id,
            preflight_stats.before_messages,
            preflight_stats.after_messages,
            preflight_stats.before_chars,
            preflight_stats.after_chars,
            preflight_stats.dropped_messages,
            preflight_stats.trimmed_chars,
        );
    }
    let (sanitized_session_messages, sanitize_stats) =
        sanitize_messages_for_provider(&trimmed_session_messages);
    if sanitize_stats.has_changes() {
        sanitize_rounds_added += 1;
        sanitized_dropped_empty_messages_added += sanitize_stats.dropped_empty_messages;
        sanitized_dropped_orphan_tool_results_added += sanitize_stats.dropped_orphan_tool_results;
        sanitized_dropped_unmatched_tool_uses_added += sanitize_stats.dropped_unmatched_tool_uses;
        sanitized_dropped_invalid_tool_use_inputs_added +=
            sanitize_stats.dropped_invalid_tool_use_inputs;
        let mut orphan_ids: Vec<String> = Vec::new();
        let mut unmatched_ids: Vec<String> = Vec::new();
        let mut invalid_ids: Vec<String> = Vec::new();
        extend_sample_ids(&mut orphan_ids, &sanitize_stats.orphan_tool_result_ids, 12);
        extend_sample_ids(
            &mut unmatched_ids,
            &sanitize_stats.unmatched_tool_use_ids,
            12,
        );
        extend_sample_ids(
            &mut invalid_ids,
            &sanitize_stats.invalid_tool_use_input_ids,
            12,
        );
        sanitize_orphan_samples_added.extend(orphan_ids);
        sanitize_unmatched_samples_added.extend(unmatched_ids);
        sanitize_invalid_tool_use_samples_added.extend(invalid_ids);
        tracing::warn!(
            "[start_agent_stream] sanitized malformed tool history before request: stream_id={}, session_id={}, before_messages={}, after_messages={}, dropped_empty_messages={}, dropped_orphan_tool_results={}, dropped_unmatched_tool_uses={}, dropped_invalid_tool_use_inputs={}, orphan_tool_result_ids={:?}, unmatched_tool_use_ids={:?}, invalid_tool_use_input_ids={:?}",
            stream_id,
            _session_id,
            session_messages.len(),
            sanitized_session_messages.len(),
            sanitize_stats.dropped_empty_messages,
            sanitize_stats.dropped_orphan_tool_results,
            sanitize_stats.dropped_unmatched_tool_uses,
            sanitize_stats.dropped_invalid_tool_use_inputs,
            sanitize_stats.orphan_tool_result_ids,
            sanitize_stats.unmatched_tool_use_ids,
            sanitize_stats.invalid_tool_use_input_ids,
        );
    }
    let mut request_messages = sanitized_session_messages.clone();
    if preflight_stats.has_changes() || sanitize_stats.has_changes() {
        request_messages.insert(
            0,
            InputMessage::user_text(format!(
                "[context_trim_notice] dropped_messages={}, dropped_empty_messages={}, dropped_orphan_tool_results={}, dropped_unmatched_tool_uses={}, dropped_invalid_tool_use_inputs={}",
                preflight_stats.dropped_messages,
                sanitize_stats.dropped_empty_messages,
                sanitize_stats.dropped_orphan_tool_results,
                sanitize_stats.dropped_unmatched_tool_uses,
                sanitize_stats.dropped_invalid_tool_use_inputs
            )),
        );
    }
    let (final_request_messages, final_preflight_stats) = ContextGovernor.admit(
        &request_messages,
        MAX_REQUEST_MESSAGE_COUNT,
        MAX_REQUEST_CHAR_BUDGET,
        preflight_input_budget,
    );
    if final_preflight_stats.has_changes() {
        preflight_trim_rounds_added += 1;
        preflight_dropped_messages_added += final_preflight_stats.dropped_messages;
        preflight_trimmed_chars_added += final_preflight_stats.trimmed_chars;
        tracing::warn!(
            "[start_agent_stream] final preflight trim: stream_id={}, session_id={}, before_messages={}, after_messages={}, before_chars={}, after_chars={}, dropped_messages={}, trimmed_chars={}",
            stream_id,
            _session_id,
            final_preflight_stats.before_messages,
            final_preflight_stats.after_messages,
            final_preflight_stats.before_chars,
            final_preflight_stats.after_chars,
            final_preflight_stats.dropped_messages,
            final_preflight_stats.trimmed_chars,
        );
    }
    let session_messages = final_request_messages;

    let request_messages_for_iteration = if force_final_response {
        let mut messages = session_messages.clone();
        messages.push(InputMessage::user_text(format!(
            "[agent_loop_control] Stop calling tools now. Produce a concise final user-facing summary in the user's language. Include: completed work, last successful tool evidence, what remains, and how to continue if needed. reason={finalization_reason}; iteration={tool_loop_iter}/{max_iterations}"
        )));
        messages
    } else {
        session_messages.clone()
    };

    // Phase 3 T3 (N2-β): `force_text` collapses tools and tool_choice the
    // same way `force_final_response` does, so the provider must emit text.
    let drop_tools = force_final_response || force_text || tool_defs.is_empty();
    let tool_choice = if drop_tools {
        None
    } else if force_tool_choice {
        Some(ToolChoice::Any)
    } else {
        None
    };

    let request = MessageRequest {
        model: model.to_string(),
        max_tokens: 4096,
        messages: request_messages_for_iteration,
        system: if system_prompt.is_empty() {
            None
        } else {
            Some(system_prompt.to_string())
        },
        tools: if drop_tools {
            None
        } else {
            Some(tool_defs.to_vec())
        },
        tool_choice,
        stream: true,
    };

    PreflightResult {
        request,
        session_messages,
        preflight_trim_rounds_added,
        preflight_dropped_messages_added,
        preflight_trimmed_chars_added,
        sanitize_rounds_added,
        sanitized_dropped_empty_messages_added,
        sanitized_dropped_orphan_tool_results_added,
        sanitized_dropped_unmatched_tool_uses_added,
        sanitized_dropped_invalid_tool_use_inputs_added,
        sanitize_orphan_samples_added,
        sanitize_unmatched_samples_added,
        sanitize_invalid_tool_use_samples_added,
    }
}
