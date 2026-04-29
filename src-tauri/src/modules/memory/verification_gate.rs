//! FEAT-AE-002 — Execution-aware gate helpers for memory writes.
//!
//! Full "No Execution, No Memory" requires correlating `memory_store`
//! with prior tool success in the same agent step. The counter lives on
//! [`crate::modules::control_plane::SessionExecutionContext`] and is
//! threaded through [`crate::modules::tools::context::ToolContext`].

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Env var: when `1` / `true`, `memory_store` refuses persistence until
/// at least one non-memory tool completed successfully earlier in the
/// same outer stream iteration (see `stream_task` reset + broker bump).
pub const STRICT_MEMORY_TOOL_EVIDENCE_ENV: &str = "IF2AI_MEMORY_STORE_REQUIRE_TOOL_EVIDENCE";

/// Returns `true` when strict execution-evidence mode is enabled.
#[must_use]
pub fn strict_tool_evidence_required() -> bool {
    std::env::var(STRICT_MEMORY_TOOL_EVIDENCE_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// When strict mode is on and `evidence` is `Some`, returns an error
/// message if the counter is zero (no successful tools yet this step).
#[must_use]
pub fn tool_evidence_gate_message(evidence: Option<&Arc<AtomicU32>>) -> Option<&'static str> {
    if !strict_tool_evidence_required() {
        return None;
    }
    let arc = evidence?;
    if arc.load(Ordering::Relaxed) > 0 {
        return None;
    }
    Some(
        "memory_store blocked (FEAT-AE-002): no successful non-memory tool execution in this agent step yet; \
run a read or mutating tool first, or disable IF2AI_MEMORY_STORE_REQUIRE_TOOL_EVIDENCE",
    )
}

/// Simple volatile-content hints (spec Module K — session / pid mentions).
#[must_use]
pub fn volatile_content_hints(content: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let lower = content.to_ascii_lowercase();
    if lower.contains("session_id") || lower.contains("session id") {
        out.push("session_id");
    }
    if lower.contains("pid=") || lower.contains("process id") {
        out.push("process_id");
    }
    out
}
