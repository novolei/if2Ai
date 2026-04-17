//! DOM AXTree snapshot script.
//!
//! This module embeds the JavaScript that traverses the live DOM, annotates
//! every interactive element with a `data-if2ai-ref` attribute, and returns a
//! compact text tree the LLM can read. The full algorithm is implemented in
//! `snapshot.js` and loaded here via `include_str!`.
//!
//! The implementation is completed in slice 7B.3. This stub exposes the
//! constant so that [`super::session`] can compile.

/// JavaScript source for the AXTree snapshot. Injected into the page via
/// `page.evaluate()`. Returns `{ text: string, currentUrl: string, title: string }`.
///
/// Full implementation arrives in slice 7B.3 (snapshot.js).
pub const SNAPSHOT_SCRIPT: &str = r#"
(function() {
  return {
    text: '[Browser snapshot not yet implemented — slice 7B.3]',
    currentUrl: window.location.href,
    title: document.title
  };
})()
"#;
