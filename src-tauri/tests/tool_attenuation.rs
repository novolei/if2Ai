//! Verifies `attenuate_tools` filters dangerous tools when low-trust skills
//! are active, plus the `PROTECTED_TOOL_NAMES` registration guard.
//!
//! Adapted from the Steward plan to use if2Ai's actual builtin tool names
//! (e.g. `read_file` rather than Steward's `file_read`, `grep_search` rather
//! than `grep`). See `src/modules/tools/attenuation.rs` for the rationale.

use serde_json::json;

use if2ai_backend::modules::api::ToolDefinition;
use if2ai_backend::modules::skills::guard::policy::TrustLevel;
use if2ai_backend::modules::tools::attenuation::{
    attenuate_tools, SkillTrustLevel, PROTECTED_TOOL_NAMES, READ_ONLY_TOOL_NAMES,
};

fn td(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: None,
        input_schema: json!({"type": "object"}),
    }
}

#[test]
fn system_trust_keeps_all() {
    let defs = vec![td("bash"), td("read_file"), td("memory_store")];
    let out = attenuate_tools(defs, SkillTrustLevel::System);
    assert_eq!(out.len(), 3);
}

#[test]
fn trusted_trust_keeps_all() {
    let defs = vec![td("bash"), td("read_file"), td("memory_store")];
    let out = attenuate_tools(defs, SkillTrustLevel::Trusted);
    assert_eq!(out.len(), 3);
}

#[test]
fn installed_trust_keeps_only_read_only() {
    let defs = vec![
        td("bash"),
        td("read_file"),
        td("memory_store"),
        td("grep_search"),
        td("file_write"),
    ];
    let out = attenuate_tools(defs, SkillTrustLevel::Installed);
    let names: Vec<_> = out.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read_file"), "read_file missing: {names:?}");
    assert!(
        names.contains(&"grep_search"),
        "grep_search missing: {names:?}"
    );
    assert!(!names.contains(&"bash"), "bash should be filtered");
    assert!(
        !names.contains(&"memory_store"),
        "memory_store should be filtered"
    );
    assert!(
        !names.contains(&"file_write"),
        "file_write should be filtered"
    );
}

#[test]
fn read_only_baseline_includes_expected() {
    for n in &[
        "read_file",
        "glob_search",
        "grep_search",
        "skill_find",
        "skill_view",
        "skills_list",
        "web_search",
        "web_fetch",
    ] {
        assert!(
            READ_ONLY_TOOL_NAMES.contains(n),
            "READ_ONLY_TOOL_NAMES missing: {n}"
        );
    }
}

#[test]
fn protected_set_disjoint_from_readonly() {
    for p in PROTECTED_TOOL_NAMES {
        assert!(
            !READ_ONLY_TOOL_NAMES.contains(p),
            "{p} appears in both PROTECTED and READ_ONLY"
        );
    }
}

#[test]
fn min_trust_ordering_installed_is_lowest() {
    let levels = [
        SkillTrustLevel::System,
        SkillTrustLevel::Installed,
        SkillTrustLevel::Trusted,
    ];
    let min = levels.iter().min().unwrap();
    assert_eq!(*min, SkillTrustLevel::Installed);
}

#[test]
fn from_trustlevel_maps_4level_to_3level() {
    assert_eq!(
        SkillTrustLevel::from(TrustLevel::Builtin),
        SkillTrustLevel::System
    );
    assert_eq!(
        SkillTrustLevel::from(TrustLevel::Trusted),
        SkillTrustLevel::Trusted
    );
    assert_eq!(
        SkillTrustLevel::from(TrustLevel::Community),
        SkillTrustLevel::Installed
    );
    assert_eq!(
        SkillTrustLevel::from(TrustLevel::AgentCreated),
        SkillTrustLevel::Installed
    );
}

#[test]
fn protected_names_includes_security_critical_builtins() {
    for n in &[
        "bash",
        "memory_store",
        "memory_forget",
        "memory_purge",
        "file_write",
        "file_edit",
        "http_request",
    ] {
        assert!(
            PROTECTED_TOOL_NAMES.contains(n),
            "PROTECTED_TOOL_NAMES missing security-critical builtin: {n}"
        );
    }
}
