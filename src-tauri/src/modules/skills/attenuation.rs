//! Skill trust attenuation — when low-trust skills are active, shrink the
//! tool definitions exposed to the LLM (Steward-style conservative surface).
//!
//! See plan: Steward → if2Ai P1-5. Trust resolution uses hub `lock.json` when
//! available, otherwise [`TrustLevel::from_source_identifier`] on the raw id.

use std::collections::HashSet;

use crate::modules::api::ToolDefinition;
use crate::modules::skills::guard::policy::{is_trusted_repo, TrustLevel};
use crate::modules::skills::hub::state::{HubLock, HubLockEntry, HubPaths};

/// Tools considered read-only / discovery-only for low-trust skill sessions.
///
/// Names must match OpenAI `function.name` on each [`ToolEntry`](crate::modules::tools::registry::ToolEntry).
pub const LOW_TRUST_TOOL_ALLOWLIST: &[&str] = &[
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
];

fn env_force_attenuation() -> bool {
    std::env::var("IF2AI_FORCE_TOOL_ATTENUATION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn env_disable_attenuation() -> bool {
    std::env::var("IF2AI_DISABLE_TOOL_ATTENUATION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn trust_for_hub_entry(entry: &HubLockEntry) -> TrustLevel {
    // P1-5 — manifest-declared trustTier wins over source inference.
    if let Some(tier) = entry.trust_tier.as_deref() {
        match tier.to_ascii_lowercase().as_str() {
            "high" | "trusted" => return TrustLevel::Trusted,
            "builtin" => return TrustLevel::Builtin,
            "low" | "community" => return TrustLevel::Community,
            "agent" | "agent-created" | "agent_created" => return TrustLevel::AgentCreated,
            // unknown value — fall through to identifier inference and
            // log so reviewers can spot the typo.
            _ => tracing::warn!(
                tier = %tier,
                skill = %entry.skill_name,
                "[skill_attenuation] unknown trustTier in lock entry; falling back to identifier inference"
            ),
        }
    }
    let src = entry.source.to_lowercase();
    if src == "builtin" {
        return TrustLevel::Builtin;
    }
    if src.contains("agent") {
        return TrustLevel::AgentCreated;
    }
    let parts: Vec<&str> = entry
        .identifier
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        let repo = format!("{}/{}", parts[0], parts[1]);
        if is_trusted_repo(&repo) {
            return TrustLevel::Trusted;
        }
    }
    TrustLevel::Community
}

fn trust_for_active_skill_id(skill_id: &str, hub_lock: &HubLock) -> TrustLevel {
    if let Ok(Some(entry)) = hub_lock.get(skill_id) {
        return trust_for_hub_entry(&entry);
    }
    TrustLevel::from_source_identifier(skill_id)
}

fn active_skill_set_requires_attenuation(active_skill_ids: &[String]) -> bool {
    if active_skill_ids.is_empty() {
        return false;
    }
    if env_disable_attenuation() {
        return false;
    }
    if env_force_attenuation() {
        return true;
    }
    let paths = HubPaths::default();
    let hub_lock_opt = match HubLock::load(paths.lock_file) {
        Ok(l) => Some(l),
        Err(e) => {
            tracing::warn!(
                "[skill_attenuation] failed to load hub lock; using identifier-only trust: {}",
                e
            );
            None
        }
    };
    active_skill_ids.iter().any(|id| {
        let trust = if let Some(ref lock) = hub_lock_opt {
            trust_for_active_skill_id(id, lock)
        } else {
            TrustLevel::from_source_identifier(id)
        };
        matches!(trust, TrustLevel::Community | TrustLevel::AgentCreated)
    })
}

/// `Some(allowlist)` when the model should only see allowlisted tools; `None` = full registry.
#[must_use]
pub fn tool_allowlist_for_active_skills(active_skill_ids: &[String]) -> Option<HashSet<String>> {
    if !active_skill_set_requires_attenuation(active_skill_ids) {
        return None;
    }
    Some(
        LOW_TRUST_TOOL_ALLOWLIST
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    )
}

/// Filter tool definitions passed to the LLM (streaming path).
#[must_use]
pub fn attenuate_tool_definitions(
    defs: Vec<ToolDefinition>,
    active_skill_ids: &[String],
) -> Vec<ToolDefinition> {
    let Some(allow) = tool_allowlist_for_active_skills(active_skill_ids) else {
        return defs;
    };
    let original_len = defs.len();
    let filtered: Vec<ToolDefinition> = defs
        .into_iter()
        .filter(|d| allow.contains(&d.name))
        .collect();
    tracing::info!(
        "[skill_attenuation] low-trust active skills → LLM tools {} → {}",
        original_len,
        filtered.len()
    );
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_active_never_attenuates() {
        assert!(tool_allowlist_for_active_skills(&[]).is_none());
    }
}
