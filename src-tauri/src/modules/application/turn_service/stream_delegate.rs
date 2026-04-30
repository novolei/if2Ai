//! StreamDelegate adapter for `run_agentic_loop`.
//!
//! Bridges the streaming agent loop body (factored into per-phase helpers in
//! `stream_iteration.rs`) into the `LoopDelegate` trait so the unified loop
//! algorithm can drive iteration. State lives in `tokio::sync::Mutex`-wrapped
//! fields because `LoopDelegate` methods take `&self`.
//!
//! ## T5 sub-commit lineage
//!
//! - **T5a (this file at this commit)**: struct + `new()` + stub trait methods
//!   that `unimplemented!()`. No runtime adoption — production
//!   `run_stream_task_body` is untouched.
//! - **T5b (next commit)**: real trait method bodies routing through
//!   `iteration_preflight` / `iteration_run_stream` / `handle_no_tool_calls`
//!   / `iteration_execute_tools`. Drive via a mock-provider integration test.
//!   Still no production change.
//! - **T5c (final commit)**: replace the inline loop in `run_stream_task_body`
//!   with `run_agentic_loop(&delegate, &inputs.loop_config)` + LoopOutcome
//!   translation match + delegate destructure-back ceremony.
//!
//! ## Field shape (T5a baseline)
//!
//! T5a holds only `inputs: &'a StreamTaskInputs` plus the `tokio::sync::Mutex`
//! cells the audit (see prior subagent escalation) flagged as needing
//! `&self`-method access:
//!
//! - `state` — main per-turn mutable state.
//! - `cancel_rx` — `Option`-wrapped so `iteration_preflight` /
//!   `iteration_run_stream` can `take()` and restore it across the
//!   `RetryAfterSleep` boundary (the borrow shape they require today).
//! - `pending_stream` / `pending_force_final` — produced by
//!   `before_llm_call` (which wraps `iteration_preflight`), consumed by
//!   `call_llm` (which wraps `iteration_run_stream`).
//! - `tool_executor_slot` — consumed-and-restored by
//!   `iteration_execute_tools`, same `Option`-take pattern as `cancel_rx`.
//! - `pending_calls` — id → `(tool_use_id, tool_name, input_json)` lookup
//!   so the loop’s opaque-`String` call IDs can be translated back into
//!   the 3-tuples `iteration_execute_tools` expects.
//! - `next_text_continues` — sentinel set when `call_llm` routes a
//!   `NoToolOutcome::Continue` (retry/nudge) through `RespondResult::Text("")`,
//!   so `handle_text_response` knows to `Continue` instead of `Return`.
//!
//! Several other refs (stream_emitter, run_event_logger, harness_bus,
//! permission policy, app_session, etc.) are reachable via `inputs`. T5b
//! will decide whether to thread them through additional explicit borrow
//! fields or reach into `inputs` directly at each call site. T5a deliberately
//! does not pre-shape those — the audit said “let T5b figure out exact
//! borrow shape; for T5a just declare `inputs: &'a StreamTaskInputs`.”

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};

use crate::modules::api::MessageStream;
use crate::modules::application::tool_executor::ToolRegistryExecutor;

use super::agentic_loop::{
    LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult, TextAction,
};
use super::stream_loop_state::StreamLoopState;
use super::stream_task::StreamTaskInputs;

pub(super) struct StreamDelegate<'a> {
    pub(super) inputs: &'a StreamTaskInputs,
    pub(super) state: Mutex<StreamLoopState>,
    pub(super) cancel_rx: Mutex<Option<oneshot::Receiver<()>>>,
    pub(super) tool_executor_slot: Mutex<Option<ToolRegistryExecutor>>,

    /// Produced by `before_llm_call`, consumed by `call_llm`.
    pub(super) pending_stream: Mutex<Option<MessageStream>>,

    /// Produced by `before_llm_call` alongside `pending_stream`, consumed by
    /// `call_llm`. Carries the `force_final_response` decision from
    /// `iteration_preflight`'s `PreflightOutcome::Continue`.
    pub(super) pending_force_final: Mutex<Option<bool>>,

    /// Lookup by call_id for `execute_tool_calls`. Populated by `call_llm`
    /// when it translates `pending_tool_uses` (returned by
    /// `iteration_run_stream`) into `RespondResult::ToolCalls { calls }`.
    /// Tuple is `(tool_use_id, tool_name, input_json)`.
    pub(super) pending_calls: Mutex<HashMap<String, (String, String, String)>>,

    /// Sentinel set by `call_llm` when it routes a `NoToolOutcome::Continue`
    /// (retry/nudge case) through `RespondResult::Text("")`.
    /// `handle_text_response` observes this and returns `TextAction::Continue`
    /// (after clearing the flag) instead of `TextAction::Return`.
    pub(super) next_text_continues: Mutex<bool>,
}

impl<'a> StreamDelegate<'a> {
    /// Build a delegate from the values currently held as locals inside
    /// `run_stream_task_body`. T5c will use this constructor at the call site.
    pub(super) fn new(
        inputs: &'a StreamTaskInputs,
        state: StreamLoopState,
        cancel_rx: oneshot::Receiver<()>,
        tool_executor: ToolRegistryExecutor,
    ) -> Self {
        Self {
            inputs,
            state: Mutex::new(state),
            cancel_rx: Mutex::new(Some(cancel_rx)),
            tool_executor_slot: Mutex::new(Some(tool_executor)),
            pending_stream: Mutex::new(None),
            pending_force_final: Mutex::new(None),
            pending_calls: Mutex::new(HashMap::new()),
            next_text_continues: Mutex::new(false),
        }
    }

    /// Reclaim owned values after `run_agentic_loop` returns. Used by T5c at
    /// the post-loop site so the orchestrator can keep using `state` /
    /// `tool_executor` for finalize.
    pub(super) fn into_parts(self) -> (StreamLoopState, ToolRegistryExecutor) {
        let state = self.state.into_inner();
        let executor = self
            .tool_executor_slot
            .into_inner()
            .expect("tool_executor must be restored by iteration_execute_tools");
        (state, executor)
    }
}

#[async_trait]
impl<'a> LoopDelegate for StreamDelegate<'a> {
    // T5b will implement: poll cancel_rx; on cancel, set
    // state.terminal_status = Some("cancelled_by_user") and return Stop;
    // else Continue.
    #[allow(clippy::unimplemented)]
    async fn check_signals(&self) -> LoopSignal {
        unimplemented!("T5b: poll cancel_rx; map to LoopSignal Stop or Continue")
    }

    // T5b will implement: call iteration_preflight; on Continue store
    // stream + force_final_response; on RetryAfterSleep sleep then return
    // None; on BreakTerminal return Some(LoopOutcome::Failure(...)).
    #[allow(clippy::unimplemented)]
    async fn before_llm_call(&self, _ctx: &mut LoopContext, _iter: usize) -> Option<LoopOutcome> {
        unimplemented!(
            "T5b: dispatch iteration_preflight; stash pending_stream/pending_force_final; \
             map PreflightOutcome → Option<LoopOutcome>"
        )
    }

    // T5b will implement: take pending_stream; call iteration_run_stream;
    // if pending_tool_uses non-empty (or recovered via FallThrough)
    // populate pending_calls and return ToolCalls; else route
    // NoToolOutcome::{Break, Continue, FallThrough} per outcome map; if
    // force_final_response set state.terminal_status='max_iterations_reached'
    // and return Text(accumulated).
    #[allow(clippy::unimplemented)]
    async fn call_llm(&self, _ctx: &mut LoopContext) -> Result<RespondResult, String> {
        unimplemented!(
            "T5b: take pending_stream; iteration_run_stream → handle_no_tool_calls; \
             populate pending_calls and return ToolCalls/Text per outcome map"
        )
    }

    // T5b will implement: lookup 3-tuples from pending_calls; take
    // tool_executor; call iteration_execute_tools; restore tool_executor;
    // return ids of completed calls.
    #[allow(clippy::unimplemented)]
    async fn execute_tool_calls(&self, _ids: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        unimplemented!(
            "T5b: drain pending_calls by ids; iteration_execute_tools; restore executor; \
             return completed call ids"
        )
    }

    // T5b will implement: if next_text_continues sentinel is true, return
    // Continue (after clearing); else store text in state.accumulated_text
    // and return Return(LoopOutcome::Response(text)).
    #[allow(clippy::unimplemented)]
    async fn handle_text_response(&self, _text: String, _ctx: &mut LoopContext) -> TextAction {
        unimplemented!(
            "T5b: honor next_text_continues sentinel; otherwise capture text and \
             return TextAction::Return(LoopOutcome::Response(text))"
        )
    }

    async fn after_iteration(&self, _ctx: &mut LoopContext, _iter: usize) {
        // Audit confirmed: no-op. Post-batch logic lives entirely inside
        // iteration_execute_tools, which call_llm/execute_tool_calls already
        // dispatch to. Documented for T5c reviewers.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T5a smoke test: the struct compiles, the constructor signature matches
    /// what T5c will need at the call site, and the trait impl is wired (even
    /// though every method panics). Runtime invocation is intentionally avoided
    /// to keep T5a runtime-safe.
    #[test]
    fn struct_signatures_compile() {
        // Compile-time witness only. Constructing a real `StreamDelegate`
        // requires a fully-built `StreamTaskInputs`, which is heavy to fake
        // in a unit test; T5b adds the integration fixture once the methods
        // do something.
        fn _witness_construct<'a>(
            inputs: &'a StreamTaskInputs,
            state: StreamLoopState,
            cancel_rx: oneshot::Receiver<()>,
            tool_executor: ToolRegistryExecutor,
        ) -> StreamDelegate<'a> {
            StreamDelegate::new(inputs, state, cancel_rx, tool_executor)
        }

        fn _witness_into_parts(d: StreamDelegate<'_>) -> (StreamLoopState, ToolRegistryExecutor) {
            d.into_parts()
        }

        // Trait-object coercion proves the impl satisfies `LoopDelegate`
        // (Send + Sync + dyn-compatible).
        fn _witness_trait_object<'a>(d: &'a StreamDelegate<'a>) -> &'a dyn LoopDelegate {
            d
        }
    }
}
