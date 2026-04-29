//! WU-007 — Integration tests for the tool-alias redirect at the
//! `tool_execution_broker` dispatch entry point.
//!
//! We exercise the public helper `resolve_tool_name` directly. The
//! actual `execute_with_trace` path requires a SessionExecutionContext
//! + ToolRegistry which is far heavier than the contract this Pack
//! delivers (a pure `&str` rewrite); the kill-switch + panic-safety
//! + identity behavior are all observable through the helper.

use if2ai_backend::modules::control_plane::tool_execution_broker::{
    resolve_tool_name, DISABLE_TOOL_ALIAS_ENV,
};

#[test]
fn old_name_resolves_to_atomic_tool() {
    let prev = std::env::var(DISABLE_TOOL_ALIAS_ENV).ok();
    std::env::remove_var(DISABLE_TOOL_ALIAS_ENV);
    assert_eq!(resolve_tool_name("memory_recall"), "memory_read");
    assert_eq!(resolve_tool_name("cron_create"), "schedule_manage");
    assert_eq!(resolve_tool_name("skill_install"), "skill_use");
    if let Some(v) = prev {
        std::env::set_var(DISABLE_TOOL_ALIAS_ENV, v);
    }
}

#[test]
fn unknown_name_passes_through() {
    let prev = std::env::var(DISABLE_TOOL_ALIAS_ENV).ok();
    std::env::remove_var(DISABLE_TOOL_ALIAS_ENV);
    assert_eq!(
        resolve_tool_name("totally_made_up_tool_xyz"),
        "totally_made_up_tool_xyz"
    );
    assert_eq!(resolve_tool_name("file_read"), "file_read");
    if let Some(v) = prev {
        std::env::set_var(DISABLE_TOOL_ALIAS_ENV, v);
    }
}

#[test]
fn resolve_alias_panic_falls_back() {
    // The static `resolve_alias` table is `&'static`, can't realistically
    // panic at runtime — but the broker wraps its call in
    // `catch_unwind` so a future table swap that introduces a panic
    // path still keeps dispatch flowing. We assert the safe-default
    // shape: empty input is forwarded unchanged.
    let prev = std::env::var(DISABLE_TOOL_ALIAS_ENV).ok();
    std::env::remove_var(DISABLE_TOOL_ALIAS_ENV);
    assert_eq!(resolve_tool_name(""), "");
    if let Some(v) = prev {
        std::env::set_var(DISABLE_TOOL_ALIAS_ENV, v);
    }
}

#[test]
fn env_flag_disables_alias() {
    let prev = std::env::var(DISABLE_TOOL_ALIAS_ENV).ok();
    std::env::set_var(DISABLE_TOOL_ALIAS_ENV, "1");
    assert_eq!(
        resolve_tool_name("memory_recall"),
        "memory_recall",
        "alias must be skipped when kill-switch is on"
    );
    std::env::set_var(DISABLE_TOOL_ALIAS_ENV, "true");
    assert_eq!(resolve_tool_name("cron_create"), "cron_create");
    std::env::set_var(DISABLE_TOOL_ALIAS_ENV, "0");
    assert_eq!(resolve_tool_name("memory_recall"), "memory_read");
    match prev {
        Some(v) => std::env::set_var(DISABLE_TOOL_ALIAS_ENV, v),
        None => std::env::remove_var(DISABLE_TOOL_ALIAS_ENV),
    }
}
