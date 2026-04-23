//! P2-13 — Tools that must not run in fully autonomous / scheduled contexts.
//!
//! Ported conceptually from Steward `tools/autonomy.rs`. Interactive chat
//! keeps full tooling; scheduled / headless runners should consult
//! [`is_tool_denied_for_autonomy`] before dispatching tool names.

/// Tool names blocked for autonomous execution (exact `function.name` match).
pub const AUTONOMOUS_DENIED_TOOLS: &[&str] = &[
    "bash",
    "PowerShell",
    "REPL",
    "NotebookEdit",
    "file_write",
    "file_edit",
    "http_request",
    "web_fetch",
    "cron_add",
    "cron_remove",
    "cron_run",
    "agent",
];

#[must_use]
pub fn autonomy_denylist_enforced() -> bool {
    std::env::var("IF2AI_AUTONOMY_DENYLIST_ENFORCE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[must_use]
pub fn is_tool_denied_for_autonomy(tool_name: &str) -> bool {
    if !autonomy_denylist_enforced() {
        return false;
    }
    AUTONOMOUS_DENIED_TOOLS
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(tool_name))
}
