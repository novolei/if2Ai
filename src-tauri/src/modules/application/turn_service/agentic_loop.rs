//! Unified agentic loop. Mirrors Steward's `run_agentic_loop`.
//!
//! For Sub-commit 5a the loop uses minimal owned types (`String` for tool calls
//! and tool results) so the trait contract can be tested in isolation. Concrete
//! delegates (5b: StreamDelegate, 5c: RunDelegate) carry full `ToolCall` /
//! `ToolResult` values inside their own state and route them by opaque-id
//! Strings through this loop. The loop never inspects call/result contents.

use async_trait::async_trait;

use crate::modules::application::turn_service::AgenticLoopConfig;

/// Per-iteration scratch state passed by reference to delegate callbacks.
/// Delegates may inject messages, append tool results, and read `force_text`
/// (set by the loop after `force_text_after_truncations` consecutive
/// truncations).
#[derive(Debug, Default)]
pub struct LoopContext {
    pub injected: Vec<String>,
    pub tool_results: Vec<String>,
    pub force_text: bool,
}

impl LoopContext {
    pub fn inject(&mut self, msg: impl Into<String>) {
        self.injected.push(msg.into());
    }
    pub fn append_tool_results(&mut self, mut r: Vec<String>) {
        self.tool_results.append(&mut r);
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
}

/// Lower-cased substring check: any finish_reason containing "length" or
/// "max_token" is treated as a truncation. Adjust if your provider uses a
/// different sentinel — but keep this conservative.
fn is_length_truncation(finish_reason: &str) -> bool {
    let s = finish_reason.to_ascii_lowercase();
    s.contains("length") || s.contains("max_token")
}

pub async fn run_agentic_loop(
    delegate: &dyn LoopDelegate,
    config: &AgenticLoopConfig,
) -> LoopOutcome {
    let mut ctx = LoopContext::default();
    let mut nudge_count: u32 = 0;
    let mut truncation_count: u32 = 0;

    for iteration in 0..config.max_iterations {
        match delegate.check_signals().await {
            LoopSignal::Stop => return LoopOutcome::Stopped,
            LoopSignal::InjectMessage { content } => ctx.inject(content),
            LoopSignal::Continue => {}
        }

        if let Some(outcome) = delegate.before_llm_call(&mut ctx, iteration).await {
            return outcome;
        }

        ctx.force_text = truncation_count >= config.force_text_after_truncations;

        let respond = match delegate.call_llm(&mut ctx).await {
            Err(e) => return LoopOutcome::Failure(e),
            Ok(r) => r,
        };

        match respond {
            RespondResult::Text(text) => {
                truncation_count = 0;
                match delegate.handle_text_response(text, &mut ctx).await {
                    TextAction::Return(o) => return o,
                    TextAction::Continue => {}
                }
            }
            RespondResult::ToolCalls {
                calls,
                finish_reason,
            } => {
                if is_length_truncation(&finish_reason) {
                    truncation_count += 1;
                    continue;
                }
                if calls.is_empty()
                    && config.enable_tool_intent_nudge
                    && nudge_count < config.max_tool_intent_nudges
                {
                    ctx.inject(
                        "(You signaled tool intent but made no tool calls. \
                         Please call a tool now or respond with text.)",
                    );
                    nudge_count += 1;
                    continue;
                }
                let results = delegate.execute_tool_calls(calls, &mut ctx).await;
                ctx.append_tool_results(results);
            }
        }

        delegate.after_iteration(&mut ctx, iteration).await;
    }

    LoopOutcome::MaxIterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_finish_reason_recognized() {
        assert!(is_length_truncation("length"));
        assert!(is_length_truncation("Length"));
        assert!(is_length_truncation("max_tokens"));
        assert!(is_length_truncation("max_token"));
        assert!(!is_length_truncation("stop"));
        assert!(!is_length_truncation(""));
    }
}
