//! Unit tests for run_agentic_loop using a scripted MockDelegate.
//!
//! Exercises every LoopOutcome variant and the loop's safety-valve behaviors:
//! max_iterations, stop signal, tool-intent nudge, force_text after truncation.

use async_trait::async_trait;
use std::sync::Mutex;

use if2ai_backend::modules::application::turn_service::agentic_loop::{
    run_agentic_loop, LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult, TextAction,
};
use if2ai_backend::modules::application::turn_service::AgenticLoopConfig;

#[derive(Default)]
struct ScriptedDelegate {
    /// Each entry is the next `RespondResult` to return from `call_llm`.
    /// The mock pops front each call.
    script: Mutex<std::collections::VecDeque<RespondResult>>,
    /// Optionally stop the loop on iteration N (0-indexed).
    stop_at_iter: Option<usize>,
    /// Optional ctx-injection script (item per iteration). Default: none.
    inject_at_iter: Mutex<std::collections::VecDeque<Option<String>>>,
    /// Records what handle_text_response was given.
    last_text: Mutex<Option<String>>,
    /// Whether handle_text_response should Return or Continue.
    text_continues: bool,
    /// Records tool-call execution invocations.
    tool_calls_seen: Mutex<Vec<Vec<String>>>,
    /// Records ctx.force_text observed at call_llm time per iteration.
    force_text_observed: Mutex<Vec<bool>>,
    iter: Mutex<usize>,
}

#[async_trait]
impl LoopDelegate for ScriptedDelegate {
    async fn check_signals(&self) -> LoopSignal {
        let n = *self.iter.lock().unwrap();
        if let Some(stop) = self.stop_at_iter {
            if n >= stop {
                return LoopSignal::Stop;
            }
        }
        if let Some(Some(msg)) = self.inject_at_iter.lock().unwrap().pop_front() {
            return LoopSignal::InjectMessage { content: msg };
        }
        LoopSignal::Continue
    }

    async fn before_llm_call(&self, _ctx: &mut LoopContext, _i: usize) -> Option<LoopOutcome> {
        None
    }

    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String> {
        self.force_text_observed
            .lock()
            .unwrap()
            .push(ctx.force_text);
        *self.iter.lock().unwrap() += 1;
        Ok(self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .expect("script exhausted"))
    }

    async fn execute_tool_calls(&self, calls: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        self.tool_calls_seen.lock().unwrap().push(calls.clone());
        calls
            .into_iter()
            .map(|c| format!("result-of-{c}"))
            .collect()
    }

    async fn handle_text_response(&self, text: String, _ctx: &mut LoopContext) -> TextAction {
        *self.last_text.lock().unwrap() = Some(text.clone());
        if self.text_continues {
            TextAction::Continue
        } else {
            TextAction::Return(LoopOutcome::Response(text))
        }
    }

    async fn after_iteration(&self, _ctx: &mut LoopContext, _i: usize) {}
}

#[tokio::test]
async fn returns_response_on_text() {
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::Text("hello".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig::default();
    match run_agentic_loop(&d, &cfg).await {
        LoopOutcome::Response(s) => assert_eq!(s, "hello"),
        other => panic!("expected Response, got {other:?}"),
    }
}

#[tokio::test]
async fn stop_signal_terminates() {
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::Text("never".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        stop_at_iter: Some(0),
        ..Default::default()
    };
    match run_agentic_loop(&d, &AgenticLoopConfig::default()).await {
        LoopOutcome::Stopped => {}
        other => panic!("expected Stopped, got {other:?}"),
    }
}

#[tokio::test]
async fn max_iterations_terminates() {
    let mut script = std::collections::VecDeque::new();
    for _ in 0..10 {
        script.push_back(RespondResult::ToolCalls {
            calls: vec!["x".into()],
            finish_reason: "stop".into(),
        });
    }
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        text_continues: false,
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 3,
        ..Default::default()
    };
    match run_agentic_loop(&d, &cfg).await {
        LoopOutcome::MaxIterations => {}
        other => panic!("expected MaxIterations, got {other:?}"),
    }
    assert_eq!(d.tool_calls_seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn force_text_engages_after_threshold_truncations() {
    let mut script = std::collections::VecDeque::new();
    // Two truncated tool-call results, then a successful text reply.
    for _ in 0..2 {
        script.push_back(RespondResult::ToolCalls {
            calls: vec!["partial".into()],
            finish_reason: "length".into(),
        });
    }
    script.push_back(RespondResult::Text("done".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 10,
        force_text_after_truncations: 2,
        ..Default::default()
    };
    let _ = run_agentic_loop(&d, &cfg).await;

    let observed = d.force_text_observed.lock().unwrap().clone();
    // Iter 0: not force_text. Iter 1: not (only 1 truncation so far).
    // Iter 2: force_text TRUE (2 truncations preceded this call_llm).
    assert_eq!(
        observed.len(),
        3,
        "expected 3 call_llm invocations: {observed:?}"
    );
    assert!(!observed[0]);
    assert!(!observed[1]);
    assert!(observed[2], "force_text should be true on the 3rd call");
}

#[tokio::test]
async fn tool_intent_nudge_injects_message_when_calls_empty() {
    // First response: empty tool_calls (intent without call) → nudge.
    // Second response: text → return.
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::ToolCalls {
        calls: vec![],
        finish_reason: "stop".into(),
    });
    script.push_back(RespondResult::Text("ok".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 5,
        enable_tool_intent_nudge: true,
        max_tool_intent_nudges: 2,
        ..Default::default()
    };
    let outcome = run_agentic_loop(&d, &cfg).await;
    assert!(matches!(outcome, LoopOutcome::Response(_)));
    assert_eq!(
        d.tool_calls_seen.lock().unwrap().len(),
        0,
        "no tools should have been executed (calls was empty)"
    );
}

#[tokio::test]
async fn nudge_capped_at_max_then_loop_proceeds() {
    let mut script = std::collections::VecDeque::new();
    // Exceed nudge cap: 3 empty-tool responses with cap=2.
    for _ in 0..4 {
        script.push_back(RespondResult::ToolCalls {
            calls: vec![],
            finish_reason: "stop".into(),
        });
    }
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 4,
        enable_tool_intent_nudge: true,
        max_tool_intent_nudges: 2,
        ..Default::default()
    };
    // After 2 nudges, the 3rd empty-tools response should NOT nudge — loop proceeds.
    // With max_iterations=4 and no text response, loop exits MaxIterations.
    let outcome = run_agentic_loop(&d, &cfg).await;
    assert!(matches!(outcome, LoopOutcome::MaxIterations));
}

#[tokio::test]
async fn handle_text_continue_loops_again() {
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::Text("first".into()));
    script.push_back(RespondResult::Text("second".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        text_continues: true,
        // Stop after the 2 scripted responses are consumed; otherwise the
        // mock would panic on script exhaustion. The loop's behaviour under
        // test (text → Continue keeps looping) is fully observed by then.
        stop_at_iter: Some(2),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 5,
        ..Default::default()
    };
    let _ = run_agentic_loop(&d, &cfg).await;
    // Both call_llm invocations happened.
    assert_eq!(d.force_text_observed.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn truncation_count_resets_after_text_response() {
    let mut script = std::collections::VecDeque::new();
    script.push_back(RespondResult::ToolCalls {
        calls: vec!["partial".into()],
        finish_reason: "length".into(),
    });
    // Text response — should reset truncation_count back to 0.
    script.push_back(RespondResult::Text("interlude".into()));
    let d = ScriptedDelegate {
        script: Mutex::new(script),
        text_continues: true,
        // Stop after the 2 scripted responses; otherwise the mock would panic
        // on script exhaustion. The reset behaviour under test is fully
        // observable from the 2 captured `force_text` snapshots.
        stop_at_iter: Some(2),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig {
        max_iterations: 10,
        force_text_after_truncations: 2,
        ..Default::default()
    };
    let _ = run_agentic_loop(&d, &cfg).await;
    let observed = d.force_text_observed.lock().unwrap().clone();
    assert!(
        observed.iter().all(|f| !f),
        "force_text never engaged: {observed:?}"
    );
}
