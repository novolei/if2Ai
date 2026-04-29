//! WU-008 — Domain knowledge lookup hook.
//!
//! Exposes `lookup_for_skill_resolution(query, store)` that:
//!
//! 1. Honors the `IF2AI_DISABLE_DK_LOOKUP=1` kill-switch.
//! 2. Calls DK-001's `KnowledgeStore::lookup` (async).
//! 3. Maps each hit to a `PromptContribution` at priority 70 with
//!    `source = "domain_knowledge"`.
//! 4. Returns an empty Vec on any failure so the caller's
//!    `resolve_skill_plan` keeps working.

#![allow(dead_code)]

use crate::modules::application::prompt_planner::{
    PromptBlockKind, PromptBlockSource, PromptContribution,
};
use crate::modules::skills::domain_knowledge::{
    DomainKnowledgeEntry, DomainKnowledgeKind, KnowledgeStore,
};

/// Env var disabling the WU-008 domain knowledge lookup.
pub const DISABLE_DK_LOOKUP_ENV: &str = "IF2AI_DISABLE_DK_LOOKUP";

/// Source label written on every DK-derived `PromptContribution`.
pub const DK_CONTRIBUTION_SOURCE: &str = "domain_knowledge";

fn dk_lookup_disabled() -> bool {
    std::env::var(DISABLE_DK_LOOKUP_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Look up domain knowledge entries that match `query`, returning
/// one `PromptContribution` per hit. Empty result on disable / no
/// match / store error.
pub async fn lookup_for_skill_resolution(
    query: &str,
    store: &dyn KnowledgeStore,
) -> Vec<PromptContribution> {
    if dk_lookup_disabled() {
        return Vec::new();
    }
    let entries = store.lookup(query, None).await;
    entries
        .into_iter()
        .map(entry_to_contribution)
        .collect()
}

fn entry_to_contribution(entry: DomainKnowledgeEntry) -> PromptContribution {
    let title = match &entry.kind {
        DomainKnowledgeKind::WebsiteDomain { domain, .. } => format!("dk:{domain}"),
        DomainKnowledgeKind::InteractionPrimitive { category, .. } => format!("dk:{category}"),
        DomainKnowledgeKind::TaskSOP { task_type, .. } => format!("dk:{task_type}"),
    };
    let body = serde_json::to_string_pretty(&entry.kind)
        .unwrap_or_else(|_| String::from("(domain knowledge entry serialization failed)"));
    PromptContribution {
        kind: PromptBlockKind::Skill,
        title,
        body,
        source: PromptBlockSource {
            subsystem: DK_CONTRIBUTION_SOURCE.to_string(),
            reference: Some(entry.id.clone()),
        },
    }
}
