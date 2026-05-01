//! Unified agentic loop. Mirrors Steward's `run_agentic_loop`.
//!
//! For Sub-commit 5a the loop uses minimal owned types (`String` for tool calls
//! and tool results) so the trait contract can be tested in isolation. Concrete
//! delegates (5b: StreamDelegate, 5c: RunDelegate) carry full `ToolCall` /
//! `ToolResult` values inside their own state and route them by opaque-id
//! Strings through this loop. The loop never inspects call/result contents.
//!
//! Relocated from `application::turn_service::agentic_loop` in Phase 3 T1 so
//! `runtime/run_delegate.rs` and the streaming delegate can both depend on it
//! without inverting the runtime → application layering.

use async_trait::async_trait;

use super::config::{AgenticLoopConfig, IterationStrategy};
use super::iteration_tracker::IterationTracker;

/// Maximum number of context-compression retries before surfacing the error.
const MAX_CONTEXT_COMPRESSION_RETRIES: u8 = 3;

/// Escalating compression level passed to [`LoopDelegate::compress_context`]
/// when an LLM call fails with a context-length-exceeded class error.
///
/// The loop tries each level in order; if all three fail to make the request
/// fit, the original error is surfaced as `LoopOutcome::Failure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCompressionLevel {
    /// Level 1: invoke the standard `compress_for_request()` tier-budget
    /// compression on the message stream.
    StandardCompression,
    /// Level 2: shrink the WorkingMemory recency window (e.g. 8 → 4 turns).
    ReduceWindowSize,
    /// Level 3: strip non-critical system prompt segments.
    StripNonCriticalSystemPrompt,
}

/// Per-iteration scratch state passed by reference to delegate callbacks.
/// Delegates may inject messages, append tool results, and read `force_text`
/// (set by the loop after `force_text_after_truncations` consecutive
/// truncations).
#[derive(Debug, Default)]
pub struct LoopContext {
    pub injected: Vec<String>,
    pub tool_results: Vec<String>,
    pub force_text: bool,
    /// Delegates populate this when a tool call fails so the iteration
    /// tracker can drive failure-escalation hints. Tuple is
    /// `(tool_name, error_message)`. Cleared each iteration by the loop.
    pub tool_failures: Vec<(String, String)>,
    /// Delegates set this to `true` after any successful tool execution
    /// within the current iteration to reset the failure streak.
    pub had_successful_tool: bool,
}

impl LoopContext {
    /// Inject a system-level message for the next LLM call.
    pub fn inject(&mut self, msg: impl Into<String>) {
        self.injected.push(msg.into());
    }
    /// Append opaque tool-result ids returned by `execute_tool_calls`.
    pub fn append_tool_results(&mut self, mut r: Vec<String>) {
        self.tool_results.append(&mut r);
    }
    /// Report a tool failure for escalation tracking.
    pub fn report_tool_failure(&mut self, tool_name: impl Into<String>, error: impl Into<String>) {
        self.tool_failures.push((tool_name.into(), error.into()));
    }
}

#[derive(Debug, Clone)]
pub enum LoopSignal {
    Continue,
    Stop,
    InjectMessage { content: String },
}

/// What the LLM call returned. `finish_reason` is a free-form String so
/// concrete delegates can map their own provider-specific finish-reason
/// enums onto well-known sentinels: at minimum, "length" (or anything that
/// matches `is_length_truncation` below) triggers force_text bookkeeping.
#[derive(Debug, Clone)]
pub enum RespondResult {
    Text(String),
    ToolCalls {
        calls: Vec<String>,
        finish_reason: String,
    },
}

/// Delegate's verdict on a text response: terminate the loop with `outcome`,
/// or continue iterating (e.g. multi-turn flows where text is a non-final
/// chain-of-thought).
#[derive(Debug)]
pub enum TextAction {
    Return(LoopOutcome),
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopOutcome {
    Response(String),
    Stopped,
    MaxIterations,
    Failure(String),
}

#[async_trait]
pub trait LoopDelegate: Send + Sync {
    async fn check_signals(&self) -> LoopSignal;
    async fn before_llm_call(&self, ctx: &mut LoopContext, iteration: usize)
        -> Option<LoopOutcome>;
    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String>;
    async fn execute_tool_calls(&self, calls: Vec<String>, ctx: &mut LoopContext) -> Vec<String>;
    async fn handle_text_response(&self, text: String, ctx: &mut LoopContext) -> TextAction;
    async fn after_iteration(&self, ctx: &mut LoopContext, iteration: usize);

    /// Attempt to compress context at the given escalation level.
    ///
    /// Called by [`run_agentic_loop`] when `call_llm` fails with a
    /// context-length-exceeded class error. Returns `true` if compression
    /// was applied (the loop will re-invoke `before_llm_call` + `call_llm`);
    /// `false` if the delegate cannot compress further at this level.
    ///
    /// Default implementation returns `false` (no compression support).
    async fn compress_context(&self, _level: ContextCompressionLevel) -> bool {
        false
    }
}

/// Strict provider-finish-reason match for known truncation sentinels.
/// OpenAI emits `"length"`, Anthropic emits `"max_tokens"`. Substring
/// matching here would false-positive on benign reasons like
/// `"content_length_filter"` or `"input_length_exceeded"`.
fn is_length_truncation(finish_reason: &str) -> bool {
    let s = finish_reason.trim().to_ascii_lowercase();
    matches!(s.as_str(), "length" | "max_tokens" | "max_token")
}

/// Detect whether an LLM call error is a context-length-exceeded class error
/// that can potentially be resolved by compressing the request context.
///
/// Checks for well-known error patterns from OpenAI, Anthropic, and other
/// providers. The match is intentionally broad (case-insensitive substring)
/// because provider error messages are not standardized.
fn is_context_length_error(error_msg: &str) -> bool {
    let lower = error_msg.to_ascii_lowercase();
    lower.contains("context_length_exceeded")
        || lower.contains("context length exceeded")
        || lower.contains("maximum context length")
        || lower.contains("token limit")
        || lower.contains("too many tokens")
        || lower.contains("request too large")
        || lower.contains("input too long")
        || lower.contains("prompt is too long")
        || (lower.contains("max_tokens") && lower.contains("exceed"))
        || (lower.contains("too long") && lower.contains("context"))
}

pub async fn run_agentic_loop(
    delegate: &dyn LoopDelegate,
    config: &AgenticLoopConfig,
) -> LoopOutcome {
    let mut ctx = LoopContext::default();
    let mut truncation_count: u32 = 0;
    // Effective cap starts at the configured `max_iterations`; it may be
    // adjusted after the first LLM response when using `Adaptive` strategy.
    let mut effective_max = config.max_iterations;
    let mut strategy_resolved = !matches!(config.iteration_strategy, IterationStrategy::Adaptive { .. });
    let mut tracker = IterationTracker::new(
        config.max_iterations,
        &config.progress_check,
    );

    let mut iteration: usize = 0;
    while iteration < effective_max {
        match delegate.check_signals().await {
            LoopSignal::Stop => return LoopOutcome::Stopped,
            LoopSignal::InjectMessage { content } => ctx.inject(content),
            LoopSignal::Continue => {}
        }

        // Set ctx.force_text BEFORE before_llm_call so delegates that build
        // the request inside before_llm_call (e.g. StreamDelegate via
        // iteration_preflight) can honor it on the iteration when the
        // threshold was crossed, not the next one. RunDelegate reads it
        // inside call_llm, which still runs after this assignment.
        ctx.force_text = truncation_count >= config.force_text_after_truncations;

        if let Some(outcome) = delegate.before_llm_call(&mut ctx, iteration).await {
            return outcome;
        }

        let respond = match delegate.call_llm(&mut ctx).await {
            Err(e) if is_context_length_error(&e) => {
                // Context-length-exceeded: attempt escalating compression.
                let levels = [
                    ContextCompressionLevel::StandardCompression,
                    ContextCompressionLevel::ReduceWindowSize,
                    ContextCompressionLevel::StripNonCriticalSystemPrompt,
                ];
                let mut recovered = None;
                for (attempt, &level) in levels.iter().enumerate() {
                    if attempt as u8 >= MAX_CONTEXT_COMPRESSION_RETRIES {
                        break;
                    }
                    tracing::warn!(
                        "[agentic_loop] context_length_exceeded at iteration {iteration}, \
                         attempting compression level {attempt}: {level:?}"
                    );
                    if !delegate.compress_context(level).await {
                        tracing::warn!(
                            "[agentic_loop] delegate declined compression at level {level:?}"
                        );
                        continue;
                    }
                    // Re-run before_llm_call so delegates rebuild the request
                    // with the compressed context.
                    if let Some(outcome) = delegate.before_llm_call(&mut ctx, iteration).await {
                        return outcome;
                    }
                    match delegate.call_llm(&mut ctx).await {
                        Ok(r) => {
                            tracing::info!(
                                "[agentic_loop] compression level {level:?} succeeded"
                            );
                            recovered = Some(r);
                            break;
                        }
                        Err(ref retry_err) if is_context_length_error(retry_err) => {
                            tracing::warn!(
                                "[agentic_loop] still exceeded after {level:?}, escalating"
                            );
                            continue;
                        }
                        Err(other) => return LoopOutcome::Failure(other),
                    }
                }
                match recovered {
                    Some(r) => r,
                    None => return LoopOutcome::Failure(e),
                }
            }
            Err(e) => return LoopOutcome::Failure(e),
            Ok(r) => r,
        };

        match respond {
            RespondResult::Text(text) => {
                truncation_count = 0;
                // A text response on the first iteration means 0 tool calls.
                if !strategy_resolved {
                    effective_max = config.iteration_strategy.resolve(0);
                    strategy_resolved = true;
                }
                match delegate.handle_text_response(text, &mut ctx).await {
                    TextAction::Return(o) => return o,
                    TextAction::Continue => {}
                }
            }
            RespondResult::ToolCalls {
                calls,
                finish_reason,
            } => {
                // Resolve adaptive strategy on the first tool-call response.
                if !strategy_resolved {
                    effective_max = config.iteration_strategy.resolve(calls.len());
                    strategy_resolved = true;
                    tracing::debug!(
                        "[agentic_loop] adaptive strategy resolved: \
                         tool_calls={}, effective_max_iterations={effective_max}",
                        calls.len(),
                    );
                }
                if is_length_truncation(&finish_reason) {
                    truncation_count += 1;
                    continue;
                }
                if calls.is_empty() {
                    // Empty tool_calls would have been short-circuited by
                    // delegates already (StreamDelegate has debug_assert!;
                    // RunDelegate routes via Text). Reaching this branch
                    // indicates a delegate contract violation — fail loud.
                    return LoopOutcome::Failure(
                        "delegate returned empty ToolCalls (call_llm contract violation)".into(),
                    );
                }
                let results = delegate.execute_tool_calls(calls, &mut ctx).await;
                ctx.append_tool_results(results);
            }
        }

        delegate.after_iteration(&mut ctx, iteration).await;

        // --- Iteration tracker: failure escalation --------------------------
        if ctx.had_successful_tool {
            tracker.reset_failures();
        }
        let failures: Vec<(String, String)> = ctx.tool_failures.drain(..).collect();
        for (tool_name, error_msg) in failures {
            if let Some(hint) = tracker.record_failure(&tool_name, &error_msg) {
                ctx.inject(hint.content);
            }
        }
        ctx.had_successful_tool = false;

        // --- Iteration tracker: periodic progress checks --------------------
        for hint in tracker.check_progress(iteration) {
            ctx.inject(hint.content);
        }
        iteration += 1;
    }

    LoopOutcome::MaxIterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_finish_reason_recognized() {
        // True positives.
        assert!(is_length_truncation("length"));
        assert!(is_length_truncation("Length"));
        assert!(is_length_truncation("max_tokens"));
        assert!(is_length_truncation("max_token"));
        assert!(is_length_truncation(" length "));
        // True negatives — these previously matched as bypass false-positives.
        assert!(!is_length_truncation("stop"));
        assert!(!is_length_truncation(""));
        assert!(!is_length_truncation("content_length_filter"));
        assert!(!is_length_truncation("input_length_exceeded"));
        assert!(!is_length_truncation("max_token_limit"));
    }

    #[test]
    fn context_length_error_detected() {
        // True positives.
        assert!(is_context_length_error("context_length_exceeded"));
        assert!(is_context_length_error(
            "This model's maximum context length is 4097 tokens"
        ));
        assert!(is_context_length_error("token limit exceeded"));
        assert!(is_context_length_error("too many tokens in request"));
        assert!(is_context_length_error("request too large for model"));
        assert!(is_context_length_error("input too long"));
        assert!(is_context_length_error("prompt is too long"));
        assert!(is_context_length_error("max_tokens exceeded for this model"));
        assert!(is_context_length_error("context is too long"));
        // True negatives.
        assert!(!is_context_length_error("network timeout"));
        assert!(!is_context_length_error("rate limit exceeded"));
        assert!(!is_context_length_error("invalid api key"));
        assert!(!is_context_length_error(""));
    }
}
