//! FEAT-BR-002 — Coordinate-first browser interaction strategy.
//!
//! Pure functions over screenshot analysis + selector hints. Returns
//! an [`InteractionDecision`] whose `strategy` is the most reliable
//! option for the requested target, plus a `fallback_chain` ordered
//! by priority (Coordinate → Label → Selector).
//!
//! Pack contract: this module does NOT call CDP
//! `Input.dispatchMouseEvent`; the wire-up Pack will translate
//! `InteractionStrategy` into actual chromiumoxide calls.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorAction {
    Click,
    Type(String),
    ScrollIntoView,
}

/// Three-tier interaction strategy. Lower variants fall back as the
/// previous tier becomes infeasible.
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionStrategy {
    /// Tier 1 — most reliable; pierces iframes / shadow DOM.
    CoordinateClick {
        x: f64,
        y: f64,
        button: MouseButton,
        verify_screenshot: bool,
    },
    /// Tier 2 — accessible-name based.
    LabelReference {
        label_text: String,
        element_type: Option<String>,
    },
    /// Tier 3 — DOM selector. Most fragile.
    CssSelector {
        selector: String,
        action: SelectorAction,
    },
}

/// One element identified inside the screenshot.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifiedElement {
    pub label: String,
    pub bounding_box: (f64, f64, f64, f64),
    pub center: (f64, f64),
    pub element_type: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenshotAnalysis {
    pub viewport_size: (u32, u32),
    pub identified_elements: Vec<IdentifiedElement>,
    pub scroll_position: (i32, i32),
}

/// Top-level decision the daemon / runtime should execute next.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionDecision {
    pub strategy: InteractionStrategy,
    pub fallback_chain: Vec<InteractionStrategy>,
    pub reason: String,
}

/// Pure decision function. Pick the highest-priority strategy whose
/// preconditions are satisfied; emit a fallback chain so the executor
/// can retry the next tier on failure.
#[must_use]
pub fn decide_interaction(
    target_description: &str,
    screenshot: Option<&ScreenshotAnalysis>,
    available_selectors: &[String],
) -> InteractionDecision {
    let coord = best_screenshot_match(target_description, screenshot);
    let label = build_label_strategy(target_description);
    let selector = available_selectors
        .iter()
        .find(|s| !s.trim().is_empty())
        .map(|s| InteractionStrategy::CssSelector {
            selector: s.clone(),
            action: SelectorAction::Click,
        });

    let mut chain: Vec<InteractionStrategy> = Vec::new();
    if let Some(c) = coord.clone() {
        chain.push(c);
    }
    chain.push(label.clone());
    if let Some(s) = selector.clone() {
        chain.push(s);
    }

    let (chosen, reason) = if let Some(c) = coord {
        (
            c,
            "screenshot match found — coordinate click selected".to_string(),
        )
    } else if let Some(s) = selector {
        (
            s,
            "no screenshot — falling back to CSS selector".to_string(),
        )
    } else {
        (
            label,
            "no screenshot and no selector — using label reference".to_string(),
        )
    };

    InteractionDecision {
        strategy: chosen,
        fallback_chain: chain,
        reason,
    }
}

fn best_screenshot_match(
    target: &str,
    screenshot: Option<&ScreenshotAnalysis>,
) -> Option<InteractionStrategy> {
    let analysis = screenshot?;
    if analysis.identified_elements.is_empty() {
        return None;
    }
    let target_lower = target.trim().to_lowercase();
    let pick = if target_lower.is_empty() {
        analysis.identified_elements.first()?
    } else {
        analysis
            .identified_elements
            .iter()
            .find(|e| e.label.to_lowercase().contains(&target_lower))
            .or_else(|| analysis.identified_elements.first())?
    };
    Some(InteractionStrategy::CoordinateClick {
        x: pick.center.0,
        y: pick.center.1,
        button: MouseButton::Left,
        verify_screenshot: true,
    })
}

fn build_label_strategy(target: &str) -> InteractionStrategy {
    InteractionStrategy::LabelReference {
        label_text: target.trim().to_string(),
        element_type: None,
    }
}

/// Outcome of comparing two screenshots taken before / after a click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickVerification {
    pub changed: bool,
    pub similarity: f32,
}

/// Compare two screenshot byte streams. Identity → `changed = false,
/// similarity = 1.0`. Different lengths → conservatively report
/// `changed = true, similarity = 0.0`. Otherwise compute a Hamming
/// fraction over the raw bytes (cheap dHash stand-in).
#[must_use]
pub fn verify_click_effect(before: &[u8], after: &[u8]) -> ClickVerification {
    if before.is_empty() && after.is_empty() {
        return ClickVerification {
            changed: false,
            similarity: 1.0,
        };
    }
    if before.len() != after.len() {
        return ClickVerification {
            changed: true,
            similarity: 0.0,
        };
    }
    let mut equal = 0usize;
    for i in 0..before.len() {
        if before[i] == after[i] {
            equal += 1;
        }
    }
    let similarity = equal as f32 / before.len().max(1) as f32;
    ClickVerification {
        changed: similarity < 1.0,
        similarity,
    }
}
