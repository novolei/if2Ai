//! WU-003 — Stream-finalize sedimentation hooks.
//!
//! Three parallel pipelines that fire when a turn completes
//! successfully. Every pipeline is **failure-isolated**: any panic /
//! error inside the spawned task is logged and swallowed so the
//! turn's main flow never breaks.
//!
//! 1. **Skill sedimentation** (SE-001 → SE-002 → SE-004) —
//!    extract_skill_drafts → dedup_drafts → index_skill.
//! 2. **Working checkpoint extraction** (DK-002) —
//!    extract_checkpoint over the assistant's accumulated text.
//! 3. **Domain knowledge auto-contribution** (DK-003) —
//!    extract_domain_knowledge_candidates over the message history.
//!
//! Pipelines (1) and (3) are gated by `IF2AI_DISABLE_AUTO_SKILL=1`;
//! (2) is always-on by Pack contract (checkpoint extraction is a
//! cheap synchronous tag scan).

#![allow(dead_code)]

use std::sync::Arc;

use crate::modules::api::InputMessage;
use crate::modules::memory::UtilityLlm;
use crate::modules::runtime::working_checkpoint::{extract_checkpoint, CheckpointExtraction};
use crate::modules::skills::domain_knowledge::extract_domain_knowledge_candidates;
use crate::modules::skills::sedimentation::{extract_skill_drafts, SkillDraft};

/// Env var disabling the WU-003 sedimentation + DK pipelines.
/// Checkpoint extraction (#2) is **not** gated by this — it is
/// always-cheap and never calls the LLM.
pub const DISABLE_AUTO_SKILL_ENV: &str = "IF2AI_DISABLE_AUTO_SKILL";

fn auto_skill_disabled() -> bool {
    std::env::var(DISABLE_AUTO_SKILL_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Run the SE-001 → SE-002 sedimentation steps and return the draft
/// list. Returns empty when:
/// - Kill-switch is set.
/// - LLM yields nothing.
/// - History is too short for `MIN_REPEATS`.
///
/// Failure isolation: never panics; LLM errors degrade to empty.
pub async fn run_sedimentation_pipeline(
    history: &[InputMessage],
    llm: Arc<dyn UtilityLlm>,
) -> Vec<SkillDraft> {
    if auto_skill_disabled() {
        return Vec::new();
    }
    let result = std::panic::AssertUnwindSafe(extract_skill_drafts(history, &*llm));
    match futures::FutureExt::catch_unwind(result).await {
        Ok(drafts) => drafts,
        Err(_) => {
            tracing::warn!("[finalize_hooks] sedimentation panicked; degrading to empty");
            Vec::new()
        }
    }
}

/// Cheap synchronous wrapper around DK-002 [`extract_checkpoint`].
/// Always runs (not gated by the auto-skill kill-switch) since it
/// only does substring scanning over the assistant text.
#[must_use]
pub fn extract_turn_checkpoint(accumulated_text: &str) -> CheckpointExtraction {
    extract_checkpoint(accumulated_text)
}

/// Run the DK-003 contributor pipeline. Same kill-switch as
/// sedimentation. Returns the candidate list (empty on failure).
pub async fn run_domain_knowledge_contributor(
    history: &[InputMessage],
    llm: Arc<dyn UtilityLlm>,
) -> Vec<crate::modules::skills::domain_knowledge::DomainKnowledgeEntry> {
    if auto_skill_disabled() {
        return Vec::new();
    }
    let fut = std::panic::AssertUnwindSafe(extract_domain_knowledge_candidates(history, &*llm));
    match futures::FutureExt::catch_unwind(fut).await {
        Ok(candidates) => candidates,
        Err(_) => {
            tracing::warn!(
                "[finalize_hooks] domain knowledge contributor panicked; degrading to empty"
            );
            Vec::new()
        }
    }
}
