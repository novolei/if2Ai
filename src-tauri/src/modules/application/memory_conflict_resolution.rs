//! Memory conflict resolution rules (Phase M3.4 P4 skeleton).
//!
//! Lives next to [`super::memory_quality_gate`].  The quality gate
//! catches single-candidate problems (duplicates, weak evidence,
//! ambiguous kind).  Conflict resolution catches problems **between
//! candidates** or **between a candidate and an existing record**:
//!
//!   1. same fact, different value
//!   2. same preference, different polarity
//!   3. global vs project/session scope collision
//!   4. old stable fact vs new weak-evidence fact
//!
//! Honest scope (M3-B skeleton):
//!
//! - The `existing_record` parameter is **caller-supplied** today.
//!   The persistence wiring in M3-B+ will hand the resolver real
//!   prior records pulled from the memory store.  Until then the
//!   resolver is exercised purely against synthetic fixtures in
//!   tests; production callers may simply not invoke the resolver
//!   if they cannot supply a prior record.
//! - The four built-in rules below cover the file-level plan
//!   §5.5 P4 spec.  Adding rules is non-breaking; renaming
//!   [`ConflictResolutionOutcome`] variants is a contract break.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::runtime::contracts::memory::{
    MemoryObjectKind, MemoryScope, MemoryWriteCandidate,
};

/// Stable resolver version for harness eval pinning.
pub const MEMORY_CONFLICT_RESOLVER_VERSION: &str = "memory-conflict-resolver@m3.4-skeleton";

/// Reason codes the resolver may attach to its outcome.
pub mod reason_codes {
    pub const SAME_FACT_DIFFERENT_VALUE: &str = "same_fact_different_value";
    pub const SAME_PREFERENCE_DIFFERENT_POLARITY: &str = "same_preference_different_polarity";
    pub const SCOPE_COLLISION_NARROWER_WINS: &str = "scope_collision_narrower_wins";
    pub const STABLE_FACT_VS_WEAK_EVIDENCE: &str = "stable_fact_vs_weak_evidence";
    pub const NO_CONFLICT: &str = "no_conflict";
}

/// One resolution outcome.  Maps to file-level plan §5.5 P4
/// "处理结果至少要区分" 4 buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionOutcome {
    /// Replace the existing record with the new candidate.
    AcceptReplacement,
    /// Keep the existing record; reject the new candidate.
    KeepExisting,
    /// Defer to the user via a permission prompt.
    RequirePrompt,
    /// Reject the new candidate without prompting.
    RejectCandidate,
    /// No conflict detected; the candidate may persist as-is.
    NoConflict,
}

/// Caller-supplied "what is currently on file" descriptor.
///
/// In the M3-B persistence wiring this is built from the memory
/// store; the M3.4 skeleton accepts it from tests / synthetic
/// fixtures.
#[derive(Debug, Clone)]
pub struct ExistingRecordRef {
    pub object_kind: MemoryObjectKind,
    pub scope: MemoryScope,
    pub content_preview: String,
    /// `true` if the existing record is considered stable
    /// (multiple confirmations or backed by reliable evidence).
    pub stable: bool,
    /// `true` if the existing record carries an `evidence_id`.
    pub has_evidence: bool,
    /// Polarity hint (for preferences): `Some(true)` = positive,
    /// `Some(false)` = negative; `None` = N/A or unknown.
    pub polarity: Option<bool>,
}

/// Resolver result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub outcome: ConflictResolutionOutcome,
    pub reason_codes: Vec<String>,
    pub policy_version: String,
}

/// Polarity hint extracted from a candidate (heuristic — production
/// callers should classify upstream and pass an explicit polarity).
fn candidate_polarity(c: &MemoryWriteCandidate) -> Option<bool> {
    let lower = c.content_preview.to_lowercase();
    if lower.contains("doesn't ") || lower.contains("does not ") || lower.contains("hates ") {
        Some(false)
    } else if lower.contains("prefers ") || lower.contains("likes ") || lower.contains("wants ") {
        Some(true)
    } else {
        None
    }
}

/// Run the resolver.
///
/// `existing` is `None` when the caller has no prior record to
/// compare against, in which case the result is always
/// `NoConflict` (the candidate proceeds via the normal write
/// pipeline).
#[must_use]
pub fn resolve_conflict(
    candidate: &MemoryWriteCandidate,
    existing: Option<&ExistingRecordRef>,
) -> ConflictResolution {
    let Some(existing) = existing else {
        return ConflictResolution {
            outcome: ConflictResolutionOutcome::NoConflict,
            reason_codes: vec![reason_codes::NO_CONFLICT.to_string()],
            policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
        };
    };

    // Rule 4 (highest priority) — old stable fact vs new weak
    // evidence: keep existing.
    if existing.stable
        && existing.has_evidence
        && candidate.evidence_id.is_none()
        && candidate.object_kind == MemoryObjectKind::Fact
    {
        return ConflictResolution {
            outcome: ConflictResolutionOutcome::KeepExisting,
            reason_codes: vec![reason_codes::STABLE_FACT_VS_WEAK_EVIDENCE.to_string()],
            policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
        };
    }

    // Rule 3 — scope collision: narrower scope (Session < Project < Global) wins.
    if scope_rank(candidate.scope) < scope_rank(existing.scope) {
        return ConflictResolution {
            outcome: ConflictResolutionOutcome::AcceptReplacement,
            reason_codes: vec![reason_codes::SCOPE_COLLISION_NARROWER_WINS.to_string()],
            policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
        };
    }
    if scope_rank(candidate.scope) > scope_rank(existing.scope) {
        return ConflictResolution {
            outcome: ConflictResolutionOutcome::KeepExisting,
            reason_codes: vec![reason_codes::SCOPE_COLLISION_NARROWER_WINS.to_string()],
            policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
        };
    }

    // Rule 2 — same preference, different polarity → prompt.
    if candidate.object_kind == MemoryObjectKind::Preference
        && existing.object_kind == MemoryObjectKind::Preference
    {
        let cand_pol = candidate_polarity(candidate);
        if cand_pol.is_some() && existing.polarity.is_some() && cand_pol != existing.polarity {
            return ConflictResolution {
                outcome: ConflictResolutionOutcome::RequirePrompt,
                reason_codes: vec![reason_codes::SAME_PREFERENCE_DIFFERENT_POLARITY.to_string()],
                policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
            };
        }
    }

    // Rule 1 — same fact kind + same scope + different content
    // preview → prompt the user to confirm replacement.
    if candidate.object_kind == MemoryObjectKind::Fact
        && existing.object_kind == MemoryObjectKind::Fact
        && candidate.scope == existing.scope
        && candidate.content_preview != existing.content_preview
    {
        return ConflictResolution {
            outcome: ConflictResolutionOutcome::RequirePrompt,
            reason_codes: vec![reason_codes::SAME_FACT_DIFFERENT_VALUE.to_string()],
            policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
        };
    }

    ConflictResolution {
        outcome: ConflictResolutionOutcome::NoConflict,
        reason_codes: vec![reason_codes::NO_CONFLICT.to_string()],
        policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
    }
}

fn scope_rank(scope: MemoryScope) -> u8 {
    match scope {
        MemoryScope::Session => 0,
        MemoryScope::Project => 1,
        MemoryScope::Global => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact_candidate(
        scope: MemoryScope,
        content: &str,
        evidence: Option<&str>,
    ) -> MemoryWriteCandidate {
        MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Fact,
            scope,
            content_preview: content.into(),
            evidence_id: evidence.map(str::to_string),
            source: "test".into(),
        }
    }

    fn pref_candidate(content: &str) -> MemoryWriteCandidate {
        MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Preference,
            scope: MemoryScope::Global,
            content_preview: content.into(),
            evidence_id: Some("turn-1".into()),
            source: "test".into(),
        }
    }

    fn fact_existing(
        scope: MemoryScope,
        content: &str,
        stable: bool,
        has_evidence: bool,
    ) -> ExistingRecordRef {
        ExistingRecordRef {
            object_kind: MemoryObjectKind::Fact,
            scope,
            content_preview: content.into(),
            stable,
            has_evidence,
            polarity: None,
        }
    }

    #[test]
    fn no_existing_record_yields_no_conflict() {
        let c = fact_candidate(MemoryScope::Session, "user lives in NYC", Some("t-1"));
        let r = resolve_conflict(&c, None);
        assert_eq!(r.outcome, ConflictResolutionOutcome::NoConflict);
    }

    #[test]
    fn old_stable_fact_beats_new_weak_evidence_fact() {
        let candidate = fact_candidate(MemoryScope::Session, "user lives in LA", None);
        let existing = fact_existing(MemoryScope::Session, "user lives in NYC", true, true);
        let r = resolve_conflict(&candidate, Some(&existing));
        assert_eq!(r.outcome, ConflictResolutionOutcome::KeepExisting);
        assert!(r
            .reason_codes
            .iter()
            .any(|c| c == reason_codes::STABLE_FACT_VS_WEAK_EVIDENCE));
    }

    #[test]
    fn scope_collision_narrower_wins() {
        // Candidate in Session, existing in Global → Session is narrower.
        let candidate = fact_candidate(MemoryScope::Session, "X", Some("t-1"));
        let existing = fact_existing(MemoryScope::Global, "Y", false, true);
        let r = resolve_conflict(&candidate, Some(&existing));
        assert_eq!(r.outcome, ConflictResolutionOutcome::AcceptReplacement);
    }

    #[test]
    fn scope_collision_wider_loses() {
        // Candidate in Global, existing in Session → keep narrower existing.
        let candidate = fact_candidate(MemoryScope::Global, "X", Some("t-1"));
        let existing = fact_existing(MemoryScope::Session, "Y", false, true);
        let r = resolve_conflict(&candidate, Some(&existing));
        assert_eq!(r.outcome, ConflictResolutionOutcome::KeepExisting);
    }

    #[test]
    fn same_preference_opposite_polarity_requires_prompt() {
        let candidate = pref_candidate("user prefers dark mode");
        let existing = ExistingRecordRef {
            object_kind: MemoryObjectKind::Preference,
            scope: MemoryScope::Global,
            content_preview: "user does not like dark mode".into(),
            stable: true,
            has_evidence: true,
            polarity: Some(false),
        };
        let r = resolve_conflict(&candidate, Some(&existing));
        assert_eq!(r.outcome, ConflictResolutionOutcome::RequirePrompt);
    }

    #[test]
    fn same_fact_different_value_requires_prompt() {
        let candidate = fact_candidate(MemoryScope::Session, "user lives in LA", Some("t-2"));
        let existing = fact_existing(MemoryScope::Session, "user lives in NYC", false, true);
        let r = resolve_conflict(&candidate, Some(&existing));
        assert_eq!(r.outcome, ConflictResolutionOutcome::RequirePrompt);
    }
}
