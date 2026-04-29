//! WU-006 — Browser wire-up integration tests for BR-001 simplify +
//! BR-002 coordinate strategy at the runtime dispatch layer.

use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{emit_evolution_event, DISABLE_EMIT_ENV};
use if2ai_backend::modules::smart_browser::coordinate_strategy::{
    IdentifiedElement, InteractionStrategy, ScreenshotAnalysis,
};
use if2ai_backend::modules::smart_browser::runtime::{
    decide_browser_click_strategy, simplify_browser_result_text, BROWSER_SIMPLIFY_DEFAULT_TOKENS,
    DISABLE_BROWSER_SIMPLIFY_ENV,
};

const HTML_FIXTURE: &str =
    "<html><body><main><h1>Title</h1><p>content</p><script>alert(1)</script></main></body></html>";

#[test]
fn html_result_is_simplified() {
    let prev = std::env::var(DISABLE_BROWSER_SIMPLIFY_ENV).ok();
    std::env::remove_var(DISABLE_BROWSER_SIMPLIFY_ENV);
    let out = simplify_browser_result_text(HTML_FIXTURE, BROWSER_SIMPLIFY_DEFAULT_TOKENS);
    assert!(
        !out.contains("<script"),
        "script tag must be filtered, got: {out}"
    );
    assert!(out.contains("<main") || out.contains("Title"));
    if let Some(v) = prev {
        std::env::set_var(DISABLE_BROWSER_SIMPLIFY_ENV, v);
    }
}

#[test]
fn simplify_failure_falls_back() {
    // Non-HTML input must pass through unchanged (failure-mode = "not
    // HTML, leave as-is" per WU-006 contract).
    let prev = std::env::var(DISABLE_BROWSER_SIMPLIFY_ENV).ok();
    std::env::remove_var(DISABLE_BROWSER_SIMPLIFY_ENV);
    let plain = "browser-use MCP completed without textual output";
    let out = simplify_browser_result_text(plain, 1_000);
    assert_eq!(out, plain, "non-HTML input must pass through verbatim");

    // Empty input → empty output (no panic).
    assert_eq!(simplify_browser_result_text("", 100), "");
    if let Some(v) = prev {
        std::env::set_var(DISABLE_BROWSER_SIMPLIFY_ENV, v);
    }
}

#[test]
fn click_calls_decide_interaction() {
    let screenshot = ScreenshotAnalysis {
        viewport_size: (1280, 800),
        identified_elements: vec![IdentifiedElement {
            label: "Submit button".to_string(),
            bounding_box: (100.0, 200.0, 80.0, 30.0),
            center: (140.0, 215.0),
            element_type: "button".to_string(),
            confidence: 0.95,
        }],
        scroll_position: (0, 0),
    };
    let decision =
        decide_browser_click_strategy("submit button", Some(&screenshot), &["#submit".to_string()]);
    match decision.strategy {
        InteractionStrategy::CoordinateClick { x, y, .. } => {
            assert!((x - 140.0).abs() < 1e-6);
            assert!((y - 215.0).abs() < 1e-6);
        }
        other => panic!("expected CoordinateClick, got {other:?}"),
    }
    assert_eq!(decision.fallback_chain.len(), 3);
}

#[test]
fn simplify_emits_content_simplified_event() {
    // Use the WU-001 kill-switch so we don't need an AppHandle.
    let prev_emit = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let payload = serde_json::json!({
        "originalChars": HTML_FIXTURE.len(),
        "simplifiedChars": 64,
        "tokenEstimate": 16,
        "compressionRatio": 0.5,
        "keyElementCount": 0,
    });
    let env = emit_evolution_event(
        None,
        RuntimeEventType::ContentSimplified,
        "web_scan",
        CorrelationIds::default(),
        &payload,
        None,
    )
    .expect("WU-001 emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::ContentSimplified);
    match prev_emit {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn env_flag_disables_simplify() {
    let prev = std::env::var(DISABLE_BROWSER_SIMPLIFY_ENV).ok();
    std::env::set_var(DISABLE_BROWSER_SIMPLIFY_ENV, "1");
    let out = simplify_browser_result_text(HTML_FIXTURE, BROWSER_SIMPLIFY_DEFAULT_TOKENS);
    assert_eq!(out, HTML_FIXTURE, "kill-switch must pass HTML unchanged");
    std::env::set_var(DISABLE_BROWSER_SIMPLIFY_ENV, "true");
    let out2 = simplify_browser_result_text(HTML_FIXTURE, 100);
    assert_eq!(out2, HTML_FIXTURE);
    match prev {
        Some(v) => std::env::set_var(DISABLE_BROWSER_SIMPLIFY_ENV, v),
        None => std::env::remove_var(DISABLE_BROWSER_SIMPLIFY_ENV),
    }
}
