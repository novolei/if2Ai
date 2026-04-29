//! FEAT-AE-001 — Self-edit proposal generator.
//!
//! Consumes the `ClusteredFailureSet` produced by
//! [`crate::modules::learning::failure_clustering::cluster_failures`] +
//! conversation history, asks `UtilityLlm` to draft one
//! [`SelfEditProposal`] per high-frequency failure cluster
//! ([`PROPOSAL_MIN_OCCURRENCES`] = 3), and returns the drafts.
//!
//! Draft-only by Pack contract: nothing is persisted, no `strategy_registry`
//! mutations happen. Downstream Packs (AE-002 verification, AE-003
//! promotion) consume `Vec<SelfEditProposal>` from here.

use serde::{Deserialize, Serialize};

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::learning::failure_clustering::{ClusteredFailureSet, FailureCluster};
use crate::modules::memory::UtilityLlm;

/// Minimum number of failures in a cluster before we attempt to
/// draft a proposal. Lower values produce noisy false positives.
pub const PROPOSAL_MIN_OCCURRENCES: usize = 3;

/// Per-proposal `max_tokens` ceiling. Keeps the LLM honest.
const PROPOSAL_MAX_TOKENS: u32 = 360;
const PROPOSAL_TEMPERATURE: f32 = 0.2;

/// Categorical kind of self-edit being proposed. Closed enum so
/// downstream gates can pattern-match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    /// Tweak a system / scenario prompt fragment.
    PromptTweak,
    /// Add a new pre-execution guard rule for a tool.
    ToolGuardRule,
    /// Introduce a new sedimented skill draft.
    SkillDraft,
    /// Adjust a retry / backoff policy.
    RetryPolicy,
}

impl ProposalKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ProposalKind::PromptTweak => "prompt_tweak",
            ProposalKind::ToolGuardRule => "tool_guard_rule",
            ProposalKind::SkillDraft => "skill_draft",
            ProposalKind::RetryPolicy => "retry_policy",
        }
    }
}

/// One draft self-edit produced by [`generate_proposals`]. Held by
/// value (no shared state) so downstream gates can freely re-shape
/// it without breaking other consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfEditProposal {
    /// UUID v4 — globally unique even across sessions.
    pub id: String,
    pub kind: ProposalKind,
    /// Subsystem-specific target (e.g. file path, tool name, prompt block id).
    pub target: String,
    /// Existing content to replace; `None` for "create new".
    pub before: Option<String>,
    /// New content to install.
    pub after: String,
    /// Why this edit is proposed (human-readable, max ~280 chars).
    pub justification: String,
    /// Cluster category that motivated this proposal.
    pub source_cluster_id: Option<String>,
}

const PROPOSAL_SYSTEM_PROMPT: &str = concat!(
    "You propose ONE concrete self-edit that would prevent a recurring agent ",
    "failure. Output ONLY a JSON object with these exact fields:\n",
    "{\"kind\":\"prompt_tweak|tool_guard_rule|skill_draft|retry_policy\",",
    "\"target\":\"<subsystem id>\",",
    "\"before\":\"<existing or empty>\",",
    "\"after\":\"<new content>\",",
    "\"justification\":\"<one sentence>\"}",
    "\nNo markdown fences. No commentary.",
);

/// Generate zero or more self-edit proposals for the high-frequency
/// failure clusters in `failures`. Conversation `history` is
/// summarized into the prompt for context.
pub async fn generate_proposals(
    failures: &ClusteredFailureSet,
    history: &[InputMessage],
    llm: &dyn UtilityLlm,
) -> Vec<SelfEditProposal> {
    if failures.clusters.is_empty() {
        return Vec::new();
    }
    let history_summary = summarize_history(history);
    let mut out: Vec<SelfEditProposal> = Vec::new();
    for cluster in &failures.clusters {
        if cluster.total_failures < PROPOSAL_MIN_OCCURRENCES {
            continue;
        }
        let user_prompt = render_prompt_for_cluster(cluster, &history_summary);
        let raw = match llm
            .complete(
                PROPOSAL_SYSTEM_PROMPT,
                &user_prompt,
                PROPOSAL_MAX_TOKENS,
                PROPOSAL_TEMPERATURE,
            )
            .await
        {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => {
                tracing::debug!(
                    cluster = ?cluster.category,
                    "[self_edit.proposal] LLM returned empty body; skipping cluster"
                );
                continue;
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    cluster = ?cluster.category,
                    "[self_edit.proposal] LLM error; skipping cluster"
                );
                continue;
            }
        };

        match parse_llm_response(&raw, cluster) {
            Some(mut proposal) => {
                if proposal.after.trim().is_empty() {
                    tracing::debug!(
                        cluster = ?cluster.category,
                        "[self_edit.proposal] empty `after`; skipping"
                    );
                    continue;
                }
                if proposal.id.is_empty() {
                    proposal.id = uuid::Uuid::new_v4().to_string();
                }
                out.push(proposal);
            }
            None => {
                tracing::debug!(
                    cluster = ?cluster.category,
                    raw_len = raw.len(),
                    "[self_edit.proposal] failed to parse LLM JSON; skipping"
                );
            }
        }
    }
    out
}

fn render_prompt_for_cluster(cluster: &FailureCluster, history_summary: &str) -> String {
    let codes: Vec<String> = cluster
        .signatures
        .iter()
        .take(5)
        .map(|s| format!("{} (×{})", s.code, s.occurrences))
        .collect();
    format!(
        "Failure category: {:?}\nTotal failures: {}\nTop codes: [{}]\n\nRecent history:\n{}\n\nDraft the JSON now.",
        cluster.category,
        cluster.total_failures,
        codes.join(", "),
        history_summary,
    )
}

fn summarize_history(history: &[InputMessage]) -> String {
    if history.is_empty() {
        return "(no history)".to_string();
    }
    let mut out = String::new();
    let take_from = history.len().saturating_sub(8);
    for msg in &history[take_from..] {
        out.push_str(&format!("[{}] ", msg.role));
        for block in &msg.content {
            match block {
                InputContentBlock::Text { text } => {
                    let trimmed: String = text.chars().take(120).collect();
                    out.push_str(&trimmed);
                    out.push(' ');
                }
                InputContentBlock::ToolUse { name, .. } => {
                    out.push_str(&format!("→{name} "));
                }
                _ => {}
            }
        }
        out.push('\n');
    }
    out
}

fn parse_llm_response(raw: &str, cluster: &FailureCluster) -> Option<SelfEditProposal> {
    let trimmed = raw.trim();
    let json_text = extract_json_object(trimmed)?;
    let value: serde_json::Value = serde_json::from_str(&json_text).ok()?;
    let kind_str = value.get("kind")?.as_str()?;
    let kind = match kind_str {
        "prompt_tweak" => ProposalKind::PromptTweak,
        "tool_guard_rule" => ProposalKind::ToolGuardRule,
        "skill_draft" => ProposalKind::SkillDraft,
        "retry_policy" => ProposalKind::RetryPolicy,
        _ => return None,
    };
    let target = value
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let after = value
        .get("after")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let before = value
        .get("before")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let justification = value
        .get("justification")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    Some(SelfEditProposal {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        target,
        before,
        after,
        justification,
        source_cluster_id: Some(format!("{:?}", cluster.category).to_lowercase()),
    })
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (idx, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..start + idx + ch.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
