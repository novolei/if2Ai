//! FEAT-AE-002 — Self-edit verification gate (4 dimensions).
//!
//! Pure-sync filter over [`SelfEditProposal`] streams:
//!
//! 1. **malformed** — `after` non-empty after trim.
//! 2. **constitution** — build a temp `SkillDraft` from the proposal's
//!    `after` and run [`evaluate_constitution`]; any violation fails.
//! 3. **dedup** — cosine similarity vs every `prior_proposals` entry;
//!    `>= 0.85` fails (sourced from FEAT-SE-002 contract).
//! 4. **failure_history** — proposal must be tied to a real cluster
//!    (`source_cluster_id.is_some()`) so we know it isn't a phantom.
//!
//! Each proposal returns paired with a [`VerificationVerdict`] so
//! callers can route Pass through to AE-003 promotion and surface
//! Fail reasons in tracing / UI.

use serde::{Deserialize, Serialize};

use super::proposal::SelfEditProposal;
use crate::modules::skills::guard::evaluate_constitution;
use crate::modules::skills::sedimentation::{cosine_similarity, Embedder, SkillDraft};

/// Cosine threshold above which two proposals are treated as
/// duplicates. Mirrors `DEDUP_SIMILARITY_THRESHOLD` from SE-002.
pub const VERIFICATION_DEDUP_THRESHOLD: f32 = 0.85;

/// Stable gate identifiers — used in `failed_gates` so callers can
/// programmatically filter without parsing the human reason.
pub const GATE_MALFORMED: &str = "malformed";
pub const GATE_CONSTITUTION: &str = "constitution";
pub const GATE_DEDUP: &str = "dedup";
pub const GATE_FAILURE_HISTORY: &str = "failure_history";

/// Pass / Fail outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
}

/// Verdict + diagnostic detail for one [`SelfEditProposal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationVerdict {
    pub proposal_id: String,
    pub verdict: Verdict,
    pub failed_gates: Vec<String>,
    pub reasons: Vec<String>,
}

/// Run all 4 gates against every proposal. Returns `(proposal, verdict)`
/// pairs in input order (no filtering — caller decides what to do
/// with Fail entries, e.g. record into a "rejected drafts" trace).
#[must_use]
pub fn verify_proposals(
    proposals: Vec<SelfEditProposal>,
    prior_proposals: &[SelfEditProposal],
    embedder: &dyn Embedder,
) -> Vec<(SelfEditProposal, VerificationVerdict)> {
    let prior_embeddings: Vec<Vec<f32>> = prior_proposals
        .iter()
        .map(|p| embedder.embed(&render_for_embedding(p)))
        .collect();

    proposals
        .into_iter()
        .map(|proposal| {
            let mut failed: Vec<String> = Vec::new();
            let mut reasons: Vec<String> = Vec::new();

            if proposal.after.trim().is_empty() {
                failed.push(GATE_MALFORMED.to_string());
                reasons.push("after is empty after trim".to_string());
            }

            let surrogate = proposal_as_skill_draft(&proposal);
            let violations = evaluate_constitution(&surrogate);
            if !violations.is_empty() {
                failed.push(GATE_CONSTITUTION.to_string());
                let summary = violations
                    .iter()
                    .map(|v| v.rule_id)
                    .collect::<Vec<_>>()
                    .join(", ");
                reasons.push(format!("constitution rules fired: [{summary}]"));
            }

            if !prior_proposals.is_empty() {
                let this_vec = embedder.embed(&render_for_embedding(&proposal));
                if let Some((idx, sim)) = max_similarity(&this_vec, &prior_embeddings) {
                    if sim >= VERIFICATION_DEDUP_THRESHOLD {
                        failed.push(GATE_DEDUP.to_string());
                        reasons.push(format!(
                            "cosine similarity {:.3} ≥ {} vs prior proposal[{}]",
                            sim, VERIFICATION_DEDUP_THRESHOLD, idx
                        ));
                    }
                }
            }

            if proposal.source_cluster_id.is_none() {
                failed.push(GATE_FAILURE_HISTORY.to_string());
                reasons.push("proposal has no source_cluster_id".to_string());
            }

            let verdict = if failed.is_empty() {
                Verdict::Pass
            } else {
                Verdict::Fail
            };
            let v = VerificationVerdict {
                proposal_id: proposal.id.clone(),
                verdict,
                failed_gates: failed,
                reasons,
            };
            (proposal, v)
        })
        .collect()
}

fn proposal_as_skill_draft(p: &SelfEditProposal) -> SkillDraft {
    let body = format!(
        "---\nname: \"{}\"\ndescription: \"{}\"\n---\n{}",
        p.target.replace('"', "\\\""),
        p.justification.replace('"', "\\\""),
        p.after,
    );
    SkillDraft {
        name: p.target.clone(),
        description: p.justification.clone(),
        body,
        source_turns: Vec::new(),
        tool_sequence: Vec::new(),
    }
}

fn render_for_embedding(p: &SelfEditProposal) -> String {
    format!("{}\n{}\n{}", p.kind.as_str(), p.target, p.after)
}

fn max_similarity(query: &[f32], pool: &[Vec<f32>]) -> Option<(usize, f32)> {
    pool.iter()
        .enumerate()
        .map(|(idx, v)| (idx, cosine_similarity(query, v)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}
