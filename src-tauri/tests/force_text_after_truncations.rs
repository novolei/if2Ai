//! Verifies that `force_text_after_truncations` triggers `ctx.force_text`
//! after N consecutive `length` truncations, and that delegates can observe
//! the flag during their `call_llm` invocation (the production wiring then
//! drops tool definitions from the outgoing request).
//!
//! Phase 3 T3 (N2-β) — closes the safety-valve gap identified in Phase 2 T12 I-1.

use std::sync::Mutex;

use async_trait::async_trait;
use if2ai_backend::modules::runtime::agent_loop::{
    run_agentic_loop, AgenticLoopConfig, LoopContext, LoopDelegate, LoopOutcome, LoopSignal,
    RespondResult, TextAction,
};

#[derive(Default)]
struct TruncationDelegate {
    script: Mutex<std::collections::VecDeque<RespondResult>>,
    /// Records `ctx.force_text` observed at each `call_llm` invocation.
    force_text_seen: Mutex<Vec<bool>>,
}

#[async_trait]
impl LoopDelegate for TruncationDelegate {
    async fn check_signals(&self) -> LoopSignal {
        LoopSignal::Continue
    }
    async fn before_llm_call(&self, _ctx: &mut LoopContext, _iter: usize) -> Option<LoopOutcome> {
        None
    }
    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String> {
        self.force_text_seen.lock().unwrap().push(ctx.force_text);
        Ok(self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .expect("script exhausted"))
    }
    async fn execute_tool_calls(&self, _calls: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        Vec::new()
    }
    async fn handle_text_response(&self, text: String, _ctx: &mut LoopContext) -> TextAction {
        TextAction::Return(LoopOutcome::Response(text))
    }
    async fn after_iteration(&self, _ctx: &mut LoopContext, _iter: usize) {}
}

#[tokio::test]
async fn force_text_engages_after_threshold_truncations() {
    let mut script = std::collections::VecDeque::new();
    for _ in 0..2 {
        script.push_back(RespondResult::ToolCalls {
            calls: vec!["partial-id".into()],
            finish_reason: "length".into(),
        });
    }
    script.push_back(RespondResult::Text("done".into()));
    let d = TruncationDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 10,
        force_text_after_truncations: 2,
    };
    let outcome = run_agentic_loop(&d, &cfg).await;
    assert!(
        matches!(outcome, LoopOutcome::Response(ref s) if s == "done"),
        "got {outcome:?}"
    );

    let observed = d.force_text_seen.lock().unwrap().clone();
    assert_eq!(
        observed.len(),
        3,
        "expected 3 call_llm invocations: {observed:?}"
    );
    assert!(!observed[0], "iter 0: no truncations yet");
    assert!(!observed[1], "iter 1: only 1 truncation");
    assert!(
        observed[2],
        "iter 2: 2 truncations preceded — force_text should be true"
    );
}

#[tokio::test]
async fn force_text_does_not_engage_below_threshold() {
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::ToolCalls {
        calls: vec!["partial-id".into()],
        finish_reason: "length".into(),
    });
    script.push_back(RespondResult::Text("ok".into()));
    let d = TruncationDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 10,
        force_text_after_truncations: 2,
    };
    let _ = run_agentic_loop(&d, &cfg).await;
    let observed = d.force_text_seen.lock().unwrap().clone();
    assert!(
        observed.iter().all(|f| !f),
        "force_text should not engage below threshold: {observed:?}"
    );
}
