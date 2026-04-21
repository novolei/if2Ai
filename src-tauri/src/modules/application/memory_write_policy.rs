//! Memory write-policy seam (Phase M3.3 skeleton).
//!
//! Defines the trait the [`MemoryCoordinator`] uses to render a
//! pre-write `MemoryWriteDecision` for every
//! [`MemoryWriteCandidate`].
//!
//! Honest scope today:
//!
//! - The default implementation [`DefaultMemoryWritePolicy`] is a
//!   **skeleton**. It always returns `Allow` with a single
//!   `default_skeleton_allow` reason code. It does NOT yet check
//!   sensitive content, scope completeness, evidence completeness,
//!   duplicates or conflicts. Those rules land in M3.3 P2 + M3.4
//!   quality gate.
//! - The legacy [`crate::modules::memory::policy::MemoryPolicyEngine`]
//!   stays in place and continues to govern the existing
//!   `memory_store` tool path. M3.3 P2 will refactor that engine
//!   into a `WritePolicy` impl behind this trait; until then the
//!   coordinator's seam is **advisory**: callers may run the
//!   policy here for trace / explainability while the legacy path
//!   keeps making the actual persistence decision.
//!
//! Hard rules:
//! 1. The trait MUST stay synchronous and pure: no IO, no `await`.
//!    Quality-gate rules that need IO (duplicate lookup, conflict
//!    resolution against existing entries) live in M3.4
//!    `quality_gate.rs` and run *after* the policy.
//! 2. Reason codes are open strings today; M3.4 will pin a closed
//!    catalogue. Adding a code is non-breaking; renaming is a
//!    contract break (audit consumers reference them).

#![allow(dead_code)]

use crate::modules::runtime::contracts::memory::{
    MemoryWriteCandidate, MemoryWriteDecision, MemoryWriteDisposition,
};

/// Stable policy version emitted by the M3.3 skeleton.  Pin this
/// when wiring [`crate::modules::application::memory_coordinator::MemoryCoordinator`]
/// so harness eval can compare two runs.
pub const MEMORY_WRITE_POLICY_VERSION: &str = "memory-write-policy@m3.3-skeleton";

/// Stable reason-code constants. Open enum: M3.4 quality gate
/// will add more; renaming any of these is a contract break.
pub mod reason_codes {
    pub const DEFAULT_SKELETON_ALLOW: &str = "default_skeleton_allow";
    /// Reserved — populated by M3.3 P2 when the M0 `policy.rs`
    /// engine is folded behind this trait.
    pub const SENSITIVE_CONTENT: &str = "sensitive_content";
    /// Reserved — M3.4 quality gate.
    pub const SCOPE_MISSING: &str = "scope_missing";
    /// Reserved — M3.4 quality gate.
    pub const EVIDENCE_MISSING: &str = "evidence_missing";
    /// Reserved — M3.4 quality gate.
    pub const DUPLICATE_CANDIDATE: &str = "duplicate_candidate";
    /// Reserved — M3.4 quality gate.
    pub const CONFLICTING_CANDIDATE: &str = "conflicting_candidate";
}

/// Synchronous, pure write-policy gate.
///
/// Implementations evaluate one candidate at a time so the
/// coordinator can fan out a stream of candidates without locking
/// any persistent state.
pub trait MemoryWritePolicy: Send + Sync {
    /// Render a typed pre-write decision for `candidate`.
    fn evaluate(&self, candidate: &MemoryWriteCandidate) -> MemoryWriteDecision;

    /// Stable policy version. Pinned so the harness eval surface
    /// can compare two runs even when the trait impl swaps.
    fn policy_version(&self) -> &str;
}

/// Skeleton write policy used by the M3-A coordinator.
///
/// Always returns `Allow` with `default_skeleton_allow` so the
/// coordinator API is fully exercised in M3-A. Real rules
/// (sensitive content, scope/evidence completeness, dedup,
/// conflict) land progressively in M3.3 P2 → M3.4 quality gate.
pub struct DefaultMemoryWritePolicy {
    version: &'static str,
}

impl Default for DefaultMemoryWritePolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultMemoryWritePolicy {
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: MEMORY_WRITE_POLICY_VERSION,
        }
    }
}

impl MemoryWritePolicy for DefaultMemoryWritePolicy {
    fn evaluate(&self, candidate: &MemoryWriteCandidate) -> MemoryWriteDecision {
        MemoryWriteDecision {
            disposition: MemoryWriteDisposition::Allow,
            reason_codes: vec![reason_codes::DEFAULT_SKELETON_ALLOW.to_string()],
            object_kind: candidate.object_kind,
            scope: candidate.scope,
            evidence_id: candidate.evidence_id.clone(),
            policy_version: self.version.to_string(),
            decided_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn policy_version(&self) -> &str {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::memory::{MemoryObjectKind, MemoryScope};

    fn fact_candidate() -> MemoryWriteCandidate {
        MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            content_preview: "user prefers TypeScript".into(),
            evidence_id: Some("turn-1".into()),
            source: "after_turn_extract".into(),
        }
    }

    #[test]
    fn default_policy_always_allows_in_skeleton() {
        let policy = DefaultMemoryWritePolicy::new();
        let decision = policy.evaluate(&fact_candidate());
        assert_eq!(decision.disposition, MemoryWriteDisposition::Allow);
        assert_eq!(
            decision.reason_codes,
            vec![reason_codes::DEFAULT_SKELETON_ALLOW.to_string()]
        );
        assert_eq!(decision.object_kind, MemoryObjectKind::Fact);
        assert_eq!(decision.scope, MemoryScope::Session);
        assert_eq!(decision.evidence_id.as_deref(), Some("turn-1"));
        assert_eq!(decision.policy_version, MEMORY_WRITE_POLICY_VERSION);
    }

    #[test]
    fn policy_version_is_pinned() {
        let policy = DefaultMemoryWritePolicy::new();
        assert_eq!(policy.policy_version(), MEMORY_WRITE_POLICY_VERSION);
    }
}
