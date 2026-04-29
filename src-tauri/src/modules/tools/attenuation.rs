//! Steward-aligned tool attenuation primitives — protected-name guard +
//! per-iteration `attenuate_tools(min_trust)` API.
//!
//! # Relationship to `modules::skills::attenuation`
//!
//! if2Ai already ships a richer per-skill attenuator
//! (`crate::modules::skills::attenuation::attenuate_tool_definitions`) that
//! is wired into `turn_service::work_loop::build_canonical_tool_pool`. It
//! resolves trust **per active skill** via the hub `lock.json`, which is
//! strictly more granular than collapsing the active set down to a single
//! `min_trust`.
//!
//! This module provides the **simpler 3-level API** mirrored from Steward
//! for two reasons:
//!
//! 1. **`PROTECTED_TOOL_NAMES` registration guard** — a brand-new
//!    capability not previously present in if2Ai. After
//!    `ToolRegistry::lock_protected_names()` is called (end of
//!    `register_builtin_tools`), no dynamically registered tool (MCP /
//!    WASM / external) can shadow a security-critical builtin name.
//! 2. **API parity** with the Steward-Alignment plan, exposing
//!    `SkillTrustLevel` + `attenuate_tools(defs, min_trust)` for callers
//!    that already have a single resolved `min_trust` in hand.
//!
//! The Steward 3-level model maps onto if2Ai's existing 4-level
//! [`crate::modules::skills::guard::policy::TrustLevel`] via
//! [`SkillTrustLevel::from`]:
//! `Builtin → System`, `Trusted → Trusted`,
//! `Community | AgentCreated → Installed`.

use crate::modules::api::ToolDefinition;
use crate::modules::skills::guard::policy::TrustLevel;

/// Steward's 3-level trust collapse used for per-iteration attenuation.
///
/// `Ord`: `Installed < Trusted < System`. `.min()` over the active set
/// yields the most restrictive level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillTrustLevel {
    /// Built into the binary — full trust.
    System,
    /// User-pinned or signed by a trusted publisher — full trust.
    Trusted,
    /// Sideloaded / community / agent-created — restricted to read-only tools.
    Installed,
}

impl PartialOrd for SkillTrustLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SkillTrustLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(s: SkillTrustLevel) -> u8 {
            match s {
                SkillTrustLevel::Installed => 0,
                SkillTrustLevel::Trusted => 1,
                SkillTrustLevel::System => 2,
            }
        }
        rank(*self).cmp(&rank(*other))
    }
}

impl From<TrustLevel> for SkillTrustLevel {
    fn from(t: TrustLevel) -> Self {
        match t {
            TrustLevel::Builtin => SkillTrustLevel::System,
            TrustLevel::Trusted => SkillTrustLevel::Trusted,
            TrustLevel::Community | TrustLevel::AgentCreated => SkillTrustLevel::Installed,
        }
    }
}

/// Tool names that may NEVER be shadowed by dynamically registered (MCP /
/// WASM / external) tools after [`crate::modules::tools::registry::ToolRegistry::lock_protected_names`]
/// has been called.
///
/// Defends against an extension hijacking a security-critical builtin name.
/// Names match if2Ai's actual builtin tool registrations
/// (see [`crate::modules::tools::register_builtin_tools`]); `shell` is
/// included for forward-compat with any future shell tool variants.
pub const PROTECTED_TOOL_NAMES: &[&str] = &[
    "bash",
    "shell",
    "memory_store",
    "memory_forget",
    "memory_purge",
    "file_write",
    "file_edit",
    "http_request",
    "REPL",
    "PowerShell",
];

/// Tool names safe to expose under low-trust (`Installed`) skills. Reads
/// only — no side effects. Mirrors
/// [`crate::modules::skills::attenuation::LOW_TRUST_TOOL_ALLOWLIST`] using
/// if2Ai's actual builtin tool names.
pub const READ_ONLY_TOOL_NAMES: &[&str] = &[
    "read_file",
    "grep_search",
    "glob_search",
    "json_parse",
    "content_search",
    "tool_search",
    "skill_search",
    "skill_find",
    "skill_view",
    "skills_list",
    "skills_categories",
    "memory_recall",
    "memory_recall_explicit",
    "cron_list",
    "cron_runs",
    "conversation_search",
    "web_search",
    "web_fetch",
];

/// Filter LLM-visible tool definitions according to the lowest active skill trust.
///
/// `min_skill_trust = System | Trusted` → returns all definitions unchanged.
/// `min_skill_trust = Installed` → returns only [`READ_ONLY_TOOL_NAMES`].
///
/// **Empty active-skill list**: callers should pass [`SkillTrustLevel::System`]
/// (no attenuation), reflecting that running with no skill context is the
/// baseline trust.
///
/// Note: production callers in `turn_service::work_loop` use the richer
/// per-skill [`crate::modules::skills::attenuation::attenuate_tool_definitions`]
/// instead. This function is the simpler Steward-shaped API and is provided
/// for callers that already have a single resolved min-trust in hand.
#[must_use]
pub fn attenuate_tools(
    defs: Vec<ToolDefinition>,
    min_skill_trust: SkillTrustLevel,
) -> Vec<ToolDefinition> {
    match min_skill_trust {
        SkillTrustLevel::System | SkillTrustLevel::Trusted => defs,
        SkillTrustLevel::Installed => defs
            .into_iter()
            .filter(|d| READ_ONLY_TOOL_NAMES.contains(&d.name.as_str()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty() {
        let out = attenuate_tools(Vec::new(), SkillTrustLevel::Installed);
        assert!(out.is_empty());
    }

    #[test]
    fn protected_and_readonly_disjoint() {
        for p in PROTECTED_TOOL_NAMES {
            assert!(
                !READ_ONLY_TOOL_NAMES.contains(p),
                "{p} appears in both PROTECTED and READ_ONLY"
            );
        }
    }

    #[test]
    fn from_trustlevel_collapse() {
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
}
