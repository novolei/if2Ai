//! Memory quality gate (Phase M3.4 skeleton).
//!
//! Runs **after** the [`crate::modules::application::memory_write_policy::MemoryWritePolicy`]
//! has rendered a [`MemoryWriteDecision`].  The quality gate's job
//! is to catch the candidates that the write policy permitted but
//! that the persistence layer should still reject or annotate:
//!
//!   - duplicates (same content_preview within a recency window),
//!   - weak-evidence candidates (no `evidence_id` and stable kind),
//!   - ambiguous candidates (`MemoryObjectKind::Unknown` for stable
//!     scopes like `Global` / `Project`).
//!
//! Honest scope:
//!
//! - The gate is **deterministic + in-memory only**.  It does NOT
//!   query the existing memory store for cross-session duplicates
//!   — that requires a per-scope index, which lands with the
//!   M3-B+ persistence wiring.  The duplicate check today catches
//!   only intra-batch duplicates within a single `after_turn`
//!   call.  Cross-batch dedup will plug in via
//!   [`QualityGateContext`] in a follow-up slice.
//! - Conflict resolution is delegated to the
//!   [`super::memory_conflict_resolution`] module.  This file does
//!   not implement conflict logic.
//! - Audit emission is **not** done here (per file-level plan §5.4
//!   "不要把 audit emission 写进 gate 本身").  Callers
//!   ([`super::memory_coordinator::MemoryCoordinator::after_turn`])
//!   own the audit.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::runtime::contracts::memory::{
    MemoryObjectKind, MemoryScope, MemoryWriteCandidate, MemoryWriteDecision,
    MemoryWriteDisposition,
};

/// Stable policy version emitted by the M3.4 skeleton gate.
pub const MEMORY_QUALITY_GATE_VERSION: &str = "memory-quality-gate@m3.4-skeleton";

/// Stable warning / rejection reason codes (open string).
pub mod reason_codes {
    pub const DUPLICATE_IN_BATCH: &str = "duplicate_in_batch";
    pub const WEAK_EVIDENCE_STABLE_KIND: &str = "weak_evidence_stable_kind";
    pub const AMBIGUOUS_KIND_FOR_STABLE_SCOPE: &str = "ambiguous_kind_for_stable_scope";
    pub const SHORT_CONTENT: &str = "short_content";
    pub const QUALITY_PASSED: &str = "quality_passed";
}

/// Per-call context the gate may use to look up cross-batch state.
/// Today empty; extending it (with e.g. recent persisted hashes) is
/// the M3-B persistence wiring's job.
#[derive(Debug, Default, Clone)]
pub struct QualityGateContext;

/// Outcome of running the gate over a list of candidates + their
/// pre-write decisions.  Mirrors the file-level plan §5.4 P3 shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateResult {
    /// Decisions that survived the gate.  Disposition was either
    /// `Allow` going in and stayed `Allow`, or it was downgraded
    /// from `Allow` → `Prompt` due to a `weak_evidence_stable_kind`
    /// / `ambiguous_kind_for_stable_scope` warning.
    pub accepted: Vec<QualityGateAccepted>,
    /// Candidates rejected by the gate (or already `Deny` going in).
    pub rejected: Vec<QualityGateRejected>,
    /// Non-blocking annotations attached to accepted candidates.
    pub warnings: Vec<QualityGateWarning>,
    /// Stable gate version that produced this result.
    pub policy_version: String,
}

/// Accepted candidate, paired with its (possibly-downgraded) decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateAccepted {
    pub candidate: MemoryWriteCandidate,
    pub decision: MemoryWriteDecision,
}

/// Rejected candidate, with the decision that drove the rejection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateRejected {
    pub candidate: MemoryWriteCandidate,
    pub decision: MemoryWriteDecision,
    pub gate_reason: String,
}

/// Non-blocking warning annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateWarning {
    pub candidate_index: usize,
    pub code: String,
    pub message: String,
}

/// Minimum content-preview length below which a candidate is
/// flagged with `short_content`.  Below this, even an `Allow`
/// decision is downgraded to `Prompt` so the user can confirm.
const MIN_CONTENT_PREVIEW_CHARS: usize = 8;

/// Run the gate.
///
/// Inputs MUST be parallel: `candidates[i]` corresponds to
/// `decisions[i]`.  Mismatched length is treated as an internal
/// caller bug and panics in debug, returns an empty result in
/// release.
#[must_use]
pub fn evaluate_quality_gate(
    candidates: &[MemoryWriteCandidate],
    decisions: &[MemoryWriteDecision],
    _ctx: &QualityGateContext,
) -> QualityGateResult {
    debug_assert_eq!(
        candidates.len(),
        decisions.len(),
        "candidate/decision length mismatch"
    );
    if candidates.len() != decisions.len() {
        return QualityGateResult {
            accepted: Vec::new(),
            rejected: Vec::new(),
            warnings: Vec::new(),
            policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
        };
    }

    let mut seen_previews: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut accepted: Vec<QualityGateAccepted> = Vec::new();
    let mut rejected: Vec<QualityGateRejected> = Vec::new();
    let mut warnings: Vec<QualityGateWarning> = Vec::new();

    for (idx, (candidate, decision)) in candidates.iter().zip(decisions.iter()).enumerate() {
        // Pass-through deny: gate respects upstream policy.
        if decision.disposition == MemoryWriteDisposition::Deny {
            rejected.push(QualityGateRejected {
                candidate: candidate.clone(),
                decision: decision.clone(),
                gate_reason: "upstream_policy_denied".to_string(),
            });
            continue;
        }

        // Duplicate within this batch.
        let preview_key = format!("{}::{}", candidate.scope_str(), candidate.content_preview);
        if !seen_previews.insert(preview_key) {
            let mut downgraded = decision.clone();
            downgraded
                .reason_codes
                .push(reason_codes::DUPLICATE_IN_BATCH.into());
            rejected.push(QualityGateRejected {
                candidate: candidate.clone(),
                decision: downgraded,
                gate_reason: reason_codes::DUPLICATE_IN_BATCH.to_string(),
            });
            continue;
        }

        // Mutating warnings build up here.
        let mut decision_with_warnings = decision.clone();

        // Short content → downgrade to Prompt for user confirmation.
        if candidate.content_preview.chars().count() < MIN_CONTENT_PREVIEW_CHARS {
            decision_with_warnings.disposition = MemoryWriteDisposition::Prompt;
            decision_with_warnings
                .reason_codes
                .push(reason_codes::SHORT_CONTENT.into());
            warnings.push(QualityGateWarning {
                candidate_index: idx,
                code: reason_codes::SHORT_CONTENT.into(),
                message: format!(
                    "content preview is {} chars; minimum {}",
                    candidate.content_preview.chars().count(),
                    MIN_CONTENT_PREVIEW_CHARS
                ),
            });
        }

        // Stable kind without evidence_id → downgrade to Prompt.
        if is_stable_kind(candidate.object_kind) && candidate.evidence_id.is_none() {
            decision_with_warnings.disposition = MemoryWriteDisposition::Prompt;
            decision_with_warnings
                .reason_codes
                .push(reason_codes::WEAK_EVIDENCE_STABLE_KIND.into());
            warnings.push(QualityGateWarning {
                candidate_index: idx,
                code: reason_codes::WEAK_EVIDENCE_STABLE_KIND.into(),
                message: format!(
                    "candidate of kind {:?} lacks evidence_id",
                    candidate.object_kind
                ),
            });
        }

        // Unknown kind for global / project scope → downgrade.
        if candidate.object_kind == MemoryObjectKind::Unknown
            && matches!(candidate.scope, MemoryScope::Global | MemoryScope::Project)
        {
            decision_with_warnings.disposition = MemoryWriteDisposition::Prompt;
            decision_with_warnings
                .reason_codes
                .push(reason_codes::AMBIGUOUS_KIND_FOR_STABLE_SCOPE.into());
            warnings.push(QualityGateWarning {
                candidate_index: idx,
                code: reason_codes::AMBIGUOUS_KIND_FOR_STABLE_SCOPE.into(),
                message: "Unknown kind for stable scope; classification needed".into(),
            });
        }

        accepted.push(QualityGateAccepted {
            candidate: candidate.clone(),
            decision: decision_with_warnings,
        });
    }

    QualityGateResult {
        accepted,
        rejected,
        warnings,
        policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
    }
}

fn is_stable_kind(kind: MemoryObjectKind) -> bool {
    matches!(
        kind,
        MemoryObjectKind::Fact | MemoryObjectKind::Preference | MemoryObjectKind::Strategy
    )
}

trait ScopeStr {
    fn scope_str(&self) -> &'static str;
}

impl ScopeStr for MemoryWriteCandidate {
    fn scope_str(&self) -> &'static str {
        match self.scope {
            MemoryScope::Session => "session",
            MemoryScope::Project => "project",
            MemoryScope::Global => "global",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_decision(candidate: &MemoryWriteCandidate) -> MemoryWriteDecision {
        MemoryWriteDecision {
            disposition: MemoryWriteDisposition::Allow,
            reason_codes: vec!["default_skeleton_allow".into()],
            object_kind: candidate.object_kind,
            scope: candidate.scope,
            evidence_id: candidate.evidence_id.clone(),
            policy_version: "memory-write-policy@test".into(),
            decided_at: "2026-04-20T00:00:00+00:00".into(),
        }
    }

    fn fact(content: &str, evidence: Option<&str>) -> MemoryWriteCandidate {
        MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            content_preview: content.into(),
            evidence_id: evidence.map(str::to_string),
            source: "test".into(),
        }
    }

    #[test]
    fn duplicate_in_batch_is_rejected() {
        let c1 = fact("user prefers TypeScript over JS", Some("turn-1"));
        let c2 = fact("user prefers TypeScript over JS", Some("turn-2"));
        let candidates = vec![c1.clone(), c2.clone()];
        let decisions = vec![allow_decision(&c1), allow_decision(&c2)];
        let result = evaluate_quality_gate(&candidates, &decisions, &QualityGateContext::default());
        assert_eq!(result.accepted.len(), 1);
        assert_eq!(result.rejected.len(), 1);
        assert!(result.rejected[0]
            .gate_reason
            .contains(reason_codes::DUPLICATE_IN_BATCH));
    }

    #[test]
    fn weak_evidence_stable_kind_downgrades_to_prompt() {
        let c = fact("user prefers Rust", None);
        let decisions = vec![allow_decision(&c)];
        let result = evaluate_quality_gate(&[c], &decisions, &QualityGateContext::default());
        assert_eq!(result.accepted.len(), 1);
        assert_eq!(
            result.accepted[0].decision.disposition,
            MemoryWriteDisposition::Prompt
        );
        assert!(result.accepted[0]
            .decision
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::WEAK_EVIDENCE_STABLE_KIND));
    }

    #[test]
    fn unknown_kind_for_global_scope_downgrades_to_prompt() {
        let c = MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Unknown,
            scope: MemoryScope::Global,
            content_preview: "important note here".into(),
            evidence_id: Some("turn-1".into()),
            source: "test".into(),
        };
        let decisions = vec![allow_decision(&c)];
        let result = evaluate_quality_gate(&[c], &decisions, &QualityGateContext::default());
        assert_eq!(result.accepted.len(), 1);
        assert_eq!(
            result.accepted[0].decision.disposition,
            MemoryWriteDisposition::Prompt
        );
    }

    #[test]
    fn deny_passes_through_to_rejected() {
        let c = fact("anything", Some("turn-1"));
        let mut d = allow_decision(&c);
        d.disposition = MemoryWriteDisposition::Deny;
        d.reason_codes = vec!["sensitive_content".into()];
        let result = evaluate_quality_gate(&[c], &[d], &QualityGateContext::default());
        assert_eq!(result.rejected.len(), 1);
        assert_eq!(result.rejected[0].gate_reason, "upstream_policy_denied");
    }

    #[test]
    fn good_candidate_passes_clean() {
        let c = fact("this is enough content for a fact", Some("turn-1"));
        let decisions = vec![allow_decision(&c)];
        let result = evaluate_quality_gate(&[c], &decisions, &QualityGateContext::default());
        assert_eq!(result.accepted.len(), 1);
        assert_eq!(result.rejected.len(), 0);
        assert_eq!(
            result.accepted[0].decision.disposition,
            MemoryWriteDisposition::Allow
        );
    }
}
