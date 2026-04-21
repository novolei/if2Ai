//! Reflection note contract (Phase M3.7 skeleton).
//!
//! Typed candidate-shaped output the
//! [`crate::modules::application::memory_coordinator::MemoryCoordinator`]
//! and the future M5 strategy-promotion pipeline both consume.
//!
//! Honest scope:
//!
//! - This module defines **contract types only**.  The legacy
//!   [`crate::modules::learning::reflection::Reflection`] continues
//!   to drive existing self-model updates; M5+ will refactor that
//!   path to emit `ReflectionNote` directly.  Until then any
//!   producer that wants to surface a candidate to the memory
//!   coordinator builds a `ReflectionNote` here.
//! - The note is a **candidate**.  It does NOT mutate active
//!   strategy / memory state; promotion happens via the (future)
//!   harness compare + gate pipeline (M4 / M5).
//! - `evidence` is a list because a reflection may pull from
//!   multiple turns / tool traces.
//!
//! Hard rules:
//!
//! 1. Every variant of [`ReflectionIssueType`] must remain a
//!    stable string on the wire (matched by harness traces).
//!    Adding a variant is non-breaking; renaming is a contract
//!    break.
//! 2. `risk_level` mirrors the [`crate::modules::runtime::contracts::execution_mode::RiskLevel`]
//!    enum so M4 governance can correlate reflection-driven
//!    candidates with classifier risk.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::runtime::contracts::execution_mode::RiskLevel;

/// Stable contract version for harness eval pinning.
pub const REFLECTION_NOTE_VERSION: &str = "reflection-note@m3.7-skeleton";

/// Closed alphabet of reflection issue kinds.  Adding a variant
/// is non-breaking; renaming is a contract break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionIssueType {
    /// The agent took a long path / used many tool calls for a
    /// task that should have been short.
    OverlongTrajectory,
    /// The agent retried an action that had already failed.
    RepeatedFailure,
    /// The agent ignored an existing memory item that was
    /// directly relevant.
    MissedRecall,
    /// The agent chose a less-effective tool when a better one
    /// was available.
    ToolMismatch,
    /// The agent produced an answer without verifying with
    /// available tools.
    UnverifiedClaim,
    /// Catch-all for issues not yet covered by the closed
    /// catalogue.
    Other,
}

/// Reference to one piece of evidence backing a reflection note.
/// Today the resolver shape stays open; M5 may narrow it to a
/// closed enum once the trace surfaces stabilise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionEvidenceRef {
    /// Turn / run / tool-trace id that backs the issue.
    pub trace_id: String,
    /// Free-form descriptor of what the trace demonstrates
    /// (`"tool_failed"`, `"answer_without_tool"`, etc.).
    pub kind: String,
    /// Optional inline excerpt for harness UI / explainability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

/// One proposed strategy change.  This is the "what would we do
/// differently next time" suggestion.  M5 promotion gate decides
/// whether to adopt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyProposal {
    /// Stable identifier the strategy registry / policy versioning
    /// will use.
    pub proposal_id: String,
    /// Human-readable summary of the change.
    pub summary: String,
    /// Optional pointer to a richer strategy definition (file
    /// path, candidate-registry entry id, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
}

/// Canonical reflection candidate.  Held as a value (not a
/// reference) so producers can hand it to the coordinator
/// `after_turn` channel without lifetime juggling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionNote {
    /// Stable note id (typically a uuid the producer assigns).
    pub note_id: String,
    /// Issue category — drives downstream routing (e.g. M4
    /// harness gate may treat `RepeatedFailure` differently from
    /// `MissedRecall`).
    pub issue_type: ReflectionIssueType,
    /// Free-form short summary.
    pub summary: String,
    /// Evidence backing the note.  At least one entry is
    /// expected; `is_well_formed` enforces this.
    pub evidence: Vec<ReflectionEvidenceRef>,
    /// Proposed strategy change.  Optional because a note may
    /// surface an issue without yet proposing a fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_strategy: Option<StrategyProposal>,
    /// Coarse expected gain — open string today
    /// (`"latency_reduction"` / `"answer_quality"` /
    /// `"tool_efficiency"` etc.); M5 may tighten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_gain: Option<String>,
    /// Mirrors classifier risk taxonomy so M4 governance can
    /// correlate.
    pub risk_level: RiskLevel,
    /// RFC3339 timestamp.
    pub created_at: String,
    /// Stable contract version that produced this note.
    pub contract_version: String,
}

impl ReflectionNote {
    /// True iff the note is structurally well-formed (has a non-
    /// empty `note_id`, a non-empty `summary`, and at least one
    /// evidence entry).  Producers should always emit well-formed
    /// notes; consumers may reject ill-formed input.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        !self.note_id.is_empty() && !self.summary.is_empty() && !self.evidence.is_empty()
    }
}

/// Phase M5-B — adapter from the legacy
/// [`crate::modules::learning::reflection::Reflection`] shape into a
/// well-formed [`ReflectionNote`].  Used by the auto producer
/// path so existing `ReflectionEngine::analyze_session` output
/// can drive M5 candidate registration without an engine
/// rewrite.
///
/// Honest scope:
///
/// - Pure pure projection.  Maps legacy `pattern` strings into
///   the closed [`ReflectionIssueType`] catalogue using a small
///   keyword heuristic; the full taxonomy remediation lives in
///   M5-C `failure_taxonomy.rs`.
/// - Returns `None` when the legacy reflection cannot be
///   converted into a well-formed note (e.g. empty pattern).
///   Callers MUST treat `None` as "skip this reflection", not as
///   failure.
/// - The resulting note carries `proposed_strategy = None` —
///   the legacy engine does not yet propose strategies (M5-C
///   territory).
#[must_use]
pub fn from_legacy_reflection(
    legacy: &crate::modules::learning::reflection::Reflection,
) -> Option<ReflectionNote> {
    if legacy.pattern.trim().is_empty() {
        return None;
    }
    let summary = if legacy.insight.trim().is_empty() {
        legacy.pattern.clone()
    } else {
        format!("{} — {}", legacy.pattern, legacy.insight)
    };
    let evidence = vec![ReflectionEvidenceRef {
        trace_id: if legacy.source_session.is_empty() {
            "unknown_session".to_string()
        } else {
            legacy.source_session.clone()
        },
        kind: classify_pattern_kind(&legacy.pattern).to_string(),
        excerpt: Some(legacy.insight.clone()).filter(|s| !s.is_empty()),
    }];
    let issue_type = classify_issue_type(&legacy.pattern);
    let risk_level = if legacy.confidence >= 0.8 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };
    Some(ReflectionNote {
        note_id: format!("legacy:{}", uuid::Uuid::new_v4()),
        issue_type,
        summary,
        evidence,
        proposed_strategy: None,
        expected_gain: None,
        risk_level,
        created_at: legacy.timestamp.to_rfc3339(),
        contract_version: REFLECTION_NOTE_VERSION.to_string(),
    })
}

/// Heuristic mapping from legacy reflection `pattern` strings to
/// the closed [`ReflectionIssueType`] catalogue.  Intentionally
/// conservative: anything not recognised falls through to
/// [`ReflectionIssueType::Other`] so the producer never silently
/// misclassifies as a hard category.
fn classify_issue_type(pattern: &str) -> ReflectionIssueType {
    let p = pattern.to_ascii_lowercase();
    if p.starts_with("tool errors") || p.contains("error") || p.contains("failure") {
        ReflectionIssueType::RepeatedFailure
    } else if p.starts_with("tool sequence") || p.contains("tool sequence:") {
        ReflectionIssueType::ToolMismatch
    } else if p.starts_with("file type") {
        ReflectionIssueType::Other
    } else {
        ReflectionIssueType::Other
    }
}

/// Stable evidence-kind label paired with the heuristic above.
fn classify_pattern_kind(pattern: &str) -> &'static str {
    let p = pattern.to_ascii_lowercase();
    if p.starts_with("tool errors") || p.contains("error") || p.contains("failure") {
        "legacy_reflection_tool_error"
    } else if p.starts_with("tool sequence") {
        "legacy_reflection_tool_sequence"
    } else if p.starts_with("file type") {
        "legacy_reflection_file_type"
    } else {
        "legacy_reflection_other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_note() -> ReflectionNote {
        ReflectionNote {
            note_id: "note-1".into(),
            issue_type: ReflectionIssueType::ToolMismatch,
            summary: "used grep where ripgrep would have been faster".into(),
            evidence: vec![ReflectionEvidenceRef {
                trace_id: "turn-3".into(),
                kind: "tool_chosen".into(),
                excerpt: Some("grep -r 'foo' .".into()),
            }],
            proposed_strategy: Some(StrategyProposal {
                proposal_id: "use-rg-by-default".into(),
                summary: "prefer rg when available".into(),
                definition_ref: None,
            }),
            expected_gain: Some("latency_reduction".into()),
            risk_level: RiskLevel::Low,
            created_at: "2026-04-20T00:00:00+00:00".into(),
            contract_version: REFLECTION_NOTE_VERSION.into(),
        }
    }

    #[test]
    fn well_formed_note_round_trips() {
        let n = sample_note();
        let s = serde_json::to_string(&n).unwrap();
        let back: ReflectionNote = serde_json::from_str(&s).unwrap();
        assert_eq!(n, back);
        assert!(back.is_well_formed());
    }

    #[test]
    fn empty_evidence_is_not_well_formed() {
        let mut n = sample_note();
        n.evidence.clear();
        assert!(!n.is_well_formed());
    }

    #[test]
    fn empty_summary_is_not_well_formed() {
        let mut n = sample_note();
        n.summary.clear();
        assert!(!n.is_well_formed());
    }

    #[test]
    fn issue_type_variants_are_serializable() {
        for kind in [
            ReflectionIssueType::OverlongTrajectory,
            ReflectionIssueType::RepeatedFailure,
            ReflectionIssueType::MissedRecall,
            ReflectionIssueType::ToolMismatch,
            ReflectionIssueType::UnverifiedClaim,
            ReflectionIssueType::Other,
        ] {
            let s = serde_json::to_string(&kind).unwrap();
            let back: ReflectionIssueType = serde_json::from_str(&s).unwrap();
            assert_eq!(kind, back);
        }
    }
}
