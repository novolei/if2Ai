//! FEAT-BR-001..003 — Browser & tool refinement integration tests.
//!
//! Each Pack appends its own marked section.

use if2ai_backend::modules::smart_browser::content_simplifier::{
    adaptive_simplify, simplify_html, SimplifierConfig, DEFAULT_MAX_OUTPUT_CHARS,
};

const SAMPLE_HTML: &str = r#"
<html>
  <head>
    <script>alert('hi');</script>
    <style>body { color: red; }</style>
    <title>Sample Title</title>
  </head>
  <body>
    <nav>top nav links</nav>
    <main id="content">
      <article>
        <h1>Main Heading</h1>
        <p>Body paragraph with content.</p>
      </article>
      <form id="search">
        <input type="text" name="q" placeholder="Search" />
        <button type="submit">Go</button>
      </form>
    </main>
    <aside>side bar content</aside>
    <footer>copyright stuff</footer>
  </body>
</html>
"#;

// ---------------------------------------------------------------------------
// FEAT-BR-001: Browser Content Simplifier
// ---------------------------------------------------------------------------

#[test]
fn test_filter_script_style_removed() {
    let cfg = SimplifierConfig::default();
    let out = simplify_html(SAMPLE_HTML, &cfg);
    assert!(
        !out.html.contains("<script"),
        "<script> tag must be filtered, got: {}",
        out.html
    );
    assert!(!out.html.contains("<style"), "<style> tag must be filtered");
    assert!(!out.html.contains("alert("));
}

#[test]
fn test_preserve_main_and_form() {
    let cfg = SimplifierConfig::default();
    let out = simplify_html(SAMPLE_HTML, &cfg);
    assert!(
        out.html.contains("<main"),
        "main must be preserved: {}",
        out.html
    );
    assert!(out.html.contains("<form"), "form must be preserved");
    assert!(out.html.contains("<input"), "input must be preserved");
    assert!(
        !out.key_elements.is_empty(),
        "key_elements must include the form / input / button"
    );
}

#[test]
fn test_35k_hard_limit() {
    let mut huge = String::from("<html><body><main>");
    for i in 0..5000 {
        huge.push_str(&format!(
            "<p>paragraph number {i} with some real text content</p>"
        ));
    }
    huge.push_str("</main></body></html>");
    let cfg = SimplifierConfig::default();
    let out = simplify_html(&huge, &cfg);
    assert!(
        out.simplified_chars <= DEFAULT_MAX_OUTPUT_CHARS,
        "simplified_chars {} must be ≤ {}",
        out.simplified_chars,
        DEFAULT_MAX_OUTPUT_CHARS
    );
    assert!(out.original_chars > out.simplified_chars);
    assert!(out.compression_ratio < 1.0);
}

#[test]
fn test_empty_html_no_panic() {
    let cfg = SimplifierConfig::default();
    let out = simplify_html("", &cfg);
    assert_eq!(out.original_chars, 0);
    assert_eq!(out.simplified_chars, 0);
    assert!((out.compression_ratio - 1.0).abs() < 1e-9);
    assert!(out.html.is_empty());
}

#[test]
fn test_adaptive_simplify_token_budget() {
    let mut huge = String::from("<html><body><main>");
    for i in 0..3000 {
        huge.push_str(&format!("<p>text-block-{i} more padding here</p>"));
    }
    huge.push_str("</main></body></html>");
    let out = adaptive_simplify(&huge, 2_000);
    assert!(
        out.token_estimate <= 2_000,
        "adaptive_simplify(2000) → token_estimate {} must be ≤ 2000",
        out.token_estimate
    );
}

// ---------------------------------------------------------------------------
// FEAT-BR-002: Coordinate-First Browser Strategy
// ---------------------------------------------------------------------------

use if2ai_backend::modules::smart_browser::coordinate_strategy::{
    decide_interaction, verify_click_effect, IdentifiedElement, InteractionStrategy,
    ScreenshotAnalysis,
};

fn screenshot_with_button() -> ScreenshotAnalysis {
    ScreenshotAnalysis {
        viewport_size: (1280, 800),
        identified_elements: vec![IdentifiedElement {
            label: "Submit button".to_string(),
            bounding_box: (100.0, 200.0, 80.0, 30.0),
            center: (140.0, 215.0),
            element_type: "button".to_string(),
            confidence: 0.95,
        }],
        scroll_position: (0, 0),
    }
}

#[test]
fn test_coordinate_click_preferred_with_screenshot() {
    let screenshot = screenshot_with_button();
    let decision = decide_interaction("submit button", Some(&screenshot), &["#submit".to_string()]);
    match decision.strategy {
        InteractionStrategy::CoordinateClick { x, y, .. } => {
            assert!((x - 140.0).abs() < 1e-6);
            assert!((y - 215.0).abs() < 1e-6);
        }
        other => panic!("expected CoordinateClick, got {other:?}"),
    }
}

#[test]
fn test_css_selector_fallback_no_screenshot() {
    let decision = decide_interaction("submit button", None, &["#submit".to_string()]);
    match decision.strategy {
        InteractionStrategy::CssSelector { selector, .. } => {
            assert_eq!(selector, "#submit");
        }
        other => panic!("expected CssSelector, got {other:?}"),
    }
}

#[test]
fn test_fallback_chain_order() {
    let screenshot = screenshot_with_button();
    let decision = decide_interaction("submit button", Some(&screenshot), &["#submit".to_string()]);
    assert_eq!(decision.fallback_chain.len(), 3);
    assert!(matches!(
        decision.fallback_chain[0],
        InteractionStrategy::CoordinateClick { .. }
    ));
    assert!(matches!(
        decision.fallback_chain[1],
        InteractionStrategy::LabelReference { .. }
    ));
    assert!(matches!(
        decision.fallback_chain[2],
        InteractionStrategy::CssSelector { .. }
    ));
}

#[test]
fn test_empty_input_non_empty_fallback() {
    let decision = decide_interaction("", None, &[]);
    assert!(
        !decision.fallback_chain.is_empty(),
        "fallback chain must not be empty even on empty input"
    );
    // Always at least the LabelReference fallback present.
    assert!(decision
        .fallback_chain
        .iter()
        .any(|s| matches!(s, InteractionStrategy::LabelReference { .. })));
}

#[test]
fn test_verify_click_effect_identical() {
    let bytes = vec![1u8, 2, 3, 4, 5];
    let v = verify_click_effect(&bytes, &bytes);
    assert!(!v.changed);
    assert!((v.similarity - 1.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// FEAT-BR-003: Tool Atomicity Consolidation
// ---------------------------------------------------------------------------

use if2ai_backend::modules::tools::builtin::atomic_consolidation::{
    list_atomic_tools, resolve_alias, ATOMIC_MEMORY_READ, ATOMIC_MEMORY_SEARCH,
    ATOMIC_MEMORY_WRITE, ATOMIC_SCHEDULE_MANAGE, ATOMIC_SKILL_FIND, ATOMIC_SKILL_USE,
};

#[test]
fn test_alias_memory_recall() {
    assert_eq!(resolve_alias("memory_recall"), Some(ATOMIC_MEMORY_READ));
    assert_eq!(resolve_alias("memory_export"), Some(ATOMIC_MEMORY_READ));
    assert_eq!(resolve_alias("memory_pin"), Some(ATOMIC_MEMORY_WRITE));
    assert_eq!(resolve_alias("memory_search"), Some(ATOMIC_MEMORY_SEARCH));
}

#[test]
fn test_alias_cron_create() {
    for legacy in [
        "cron_create",
        "cron_list",
        "cron_pause",
        "cron_resume",
        "cron_delete",
    ] {
        assert_eq!(
            resolve_alias(legacy),
            Some(ATOMIC_SCHEDULE_MANAGE),
            "{legacy} must map to schedule_manage"
        );
    }
}

#[test]
fn test_alias_skill_list() {
    assert_eq!(resolve_alias("skill_list"), Some(ATOMIC_SKILL_FIND));
    assert_eq!(resolve_alias("skill_search"), Some(ATOMIC_SKILL_FIND));
    assert_eq!(resolve_alias("skill_export"), Some(ATOMIC_SKILL_FIND));
    assert_eq!(resolve_alias("skill_load"), Some(ATOMIC_SKILL_USE));
    assert_eq!(resolve_alias("skill_install"), Some(ATOMIC_SKILL_USE));
}

#[test]
fn test_alias_unknown_returns_none() {
    assert_eq!(resolve_alias("unknown_tool_xyz"), None);
    assert_eq!(resolve_alias(""), None);
    assert_eq!(resolve_alias("file_read"), None);
}

#[test]
fn test_list_atomic_tools_count_and_uniqueness() {
    let tools = list_atomic_tools();
    assert_eq!(tools.len(), 6, "must have exactly 6 atomic tools");
    let set: std::collections::HashSet<&'static str> = tools.iter().copied().collect();
    assert_eq!(set.len(), 6, "all atomic tool names must be unique");
    for expected in [
        ATOMIC_MEMORY_READ,
        ATOMIC_MEMORY_WRITE,
        ATOMIC_MEMORY_SEARCH,
        ATOMIC_SCHEDULE_MANAGE,
        ATOMIC_SKILL_FIND,
        ATOMIC_SKILL_USE,
    ] {
        assert!(
            set.contains(expected),
            "atomic tool {expected} missing from list"
        );
    }
}
