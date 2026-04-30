//! Mutable per-turn state for the streaming agent loop.
//!
//! Extracted from `run_stream_task_body` (sub-commit 5b-1) to make the
//! ~957-LOC function tractable and to provide a clean handle for future
//! `StreamDelegate` adoption (5b-5). **No behavior change vs the prior
//! inline locals — this is a pure refactor.**
//!
//! Scope: holds the ~38 mutable per-turn scalars, accumulators, flags,
//! status fields, and the stream-circuit breaker that previously lived
//! as `let mut` declarations at the top of `run_stream_task_body`.
//!
//! Intentionally NOT held here:
//! - `cancel_rx` and `tool_executor` — these are moved into per-iteration
//!   helpers and restored from their results, so they remain locals in
//!   the orchestrator.
//! - Read-only inputs (`max_iterations`, `mode`, `permission_policy`,
//!   `execution_context`, etc.) — those are not mutable state.
//!
//! Future commits (5b-2..5b-5) will shrink this struct as helper
//! extraction folds related fields into smaller scoped containers and
//! as `StreamDelegate` adopts the per-iteration boundary.

use crate::modules::api::InputMessage;
use crate::modules::provider::resilience::StreamCircuitState;
use crate::modules::runtime::contracts::agent_loop::PendingOperationMetadata;
use crate::modules::runtime::session::ConversationMessage;
use crate::modules::runtime::usage::TokenUsage;

/// Mutable per-turn state for `run_stream_task_body`.
///
/// Field-level docs match the inline locals they replaced; see the
/// MIG-001-d module header in `stream_task.rs` for the historical
/// rationale of each accumulator/flag.
#[allow(clippy::struct_excessive_bools)] // bag intentionally collects existing flags; will shrink in 5b-2..5b-5.
#[derive(Debug)]
pub(super) struct StreamLoopState {
    // === Iteration counters ===
    pub tool_loop_iter: usize,
    pub stream_event_retry_count: usize,
    pub stream_start_retry_count: usize,
    pub tool_required_no_tool_retry_count: usize,
    pub tool_intent_nudge_retry_count: usize,
    pub repeated_tool_batch_count: usize,
    pub invalid_tool_args_streak: usize,

    // === Accumulators (consumed/restored across the inner SSE loop
    // and tool-batch helper each iteration) ===
    pub session_messages: Vec<InputMessage>,
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub token_count: u32,
    pub accumulated_usage: TokenUsage,
    pub current_call_usage: TokenUsage,
    pub timeline_session_messages: Vec<ConversationMessage>,
    pub diagnostic_warnings: Vec<String>,
    pub sanitize_orphan_samples: Vec<String>,
    pub sanitize_unmatched_samples: Vec<String>,
    pub sanitize_invalid_tool_use_samples: Vec<String>,

    // === Sanitize / preflight running totals ===
    pub sanitize_rounds: usize,
    pub sanitized_dropped_empty_messages: usize,
    pub sanitized_dropped_orphan_tool_results: usize,
    pub sanitized_dropped_unmatched_tool_uses: usize,
    pub sanitized_dropped_invalid_tool_use_inputs: usize,
    pub preflight_trim_rounds: usize,
    pub preflight_dropped_messages_total: usize,
    pub preflight_trimmed_chars_total: usize,

    // === Control flags ===
    pub force_final_response_next: bool,
    pub force_tool_choice_next: bool,
    pub stream_failed: bool,
    pub completion_already_emitted: bool,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub provider_textual_tool_markup_seen: bool,

    // === Result status ===
    pub terminal_status: Option<&'static str>,
    pub last_stream_error_reason: Option<String>,
    /// Provider-supplied finish reason from the most-recent inner SSE
    /// event-loop call (steward-align Phase 2 T2 surfaced this on
    /// `StreamEventLoopResult.finish_reason`; T4 stores it on the loop
    /// state so post-loop / future per-iteration consumers can act on
    /// `length` / `max_tokens` truncations). `None` until the first
    /// successful event-loop completion in the current turn.
    pub last_finish_reason: Option<String>,
    pub finalization_reason: Option<String>,
    pub last_tool_batch_signature: Option<String>,
    pub pending_operation_for_delegate: Option<PendingOperationMetadata>,
    pub provider_request_id: String,

    // === Provider-side resilience breaker ===
    pub stream_circuit: StreamCircuitState,
}

impl StreamLoopState {
    /// Build the initial state for one streaming turn.
    ///
    /// `session_messages`, `provider_request_id`, and
    /// `force_tool_choice_next` carry caller-computed initial values
    /// (the originals were `messages_for_stream.clone()`,
    /// `format!("stream_{}", stream_id_for_task)`, and the
    /// `requires_tool_execution_evidence(...) || requires_single_shell_command_evidence(...)`
    /// expression respectively).
    pub fn new(
        session_messages: Vec<InputMessage>,
        provider_request_id: String,
        force_tool_choice_next: bool,
    ) -> Self {
        Self {
            tool_loop_iter: 0,
            stream_event_retry_count: 0,
            stream_start_retry_count: 0,
            tool_required_no_tool_retry_count: 0,
            tool_intent_nudge_retry_count: 0,
            repeated_tool_batch_count: 0,
            invalid_tool_args_streak: 0,

            session_messages,
            accumulated_text: String::new(),
            accumulated_thinking: String::new(),
            token_count: 0,
            accumulated_usage: TokenUsage::default(),
            current_call_usage: TokenUsage::default(),
            timeline_session_messages: Vec::new(),
            diagnostic_warnings: Vec::new(),
            sanitize_orphan_samples: Vec::new(),
            sanitize_unmatched_samples: Vec::new(),
            sanitize_invalid_tool_use_samples: Vec::new(),

            sanitize_rounds: 0,
            sanitized_dropped_empty_messages: 0,
            sanitized_dropped_orphan_tool_results: 0,
            sanitized_dropped_unmatched_tool_uses: 0,
            sanitized_dropped_invalid_tool_use_inputs: 0,
            preflight_trim_rounds: 0,
            preflight_dropped_messages_total: 0,
            preflight_trimmed_chars_total: 0,

            force_final_response_next: false,
            force_tool_choice_next,
            stream_failed: false,
            completion_already_emitted: false,
            has_successful_tool: false,
            has_successful_mutating_tool: false,
            provider_textual_tool_markup_seen: false,

            terminal_status: None,
            last_stream_error_reason: None,
            last_finish_reason: None,
            finalization_reason: None,
            last_tool_batch_signature: None,
            pending_operation_for_delegate: None,
            provider_request_id,

            stream_circuit: StreamCircuitState::default(),
        }
    }
}
