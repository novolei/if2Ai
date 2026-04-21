//! Failure taxonomy (Phase M5 m5.2, closeout).
//!
//! Closed alphabet of failure categories the M5 reflection +
//! candidate-generation pipeline operates on.  Distinct from
//! the harness-side `BlockingFailure.code` (which is open
//! free-form): the taxonomy provides a typed normalisation so
//! reflection notes can carry stable issue types rather than
//! re-classifying free-form strings each time.
//!
//! Honest scope:
//!
//! - **Closed catalogue**.  Adding a category is a contract
//!   bump.  M5 covers: `intent_miss / policy_issue /
//!   memory_issue / recovery_issue / tool_misuse / other`.
//! - **Typed evidence mapping** —
//!   [`classify_blocking_failure`] does keyword + severity
//!   matching against `BlockingFailure.code`.  Heuristic for
//!   the open code space; reviewers should treat it as a
//!   normalisation hint, not ground truth.
//! - **No NLU / no model calls**.  Pure mapping function so
//!   tests are deterministic.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::harness::{BlockingFailure, Severity};

/// Stable contract version.
pub const FAILURE_TAXONOMY_VERSION: &str = "failure-taxonomy@m5.2";

/// Closed alphabet of failure categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    /// Agent misunderstood the user intent / picked the wrong
    /// task framing.
    IntentMiss,
    /// Policy violation — boundary/permission/sandbox decision
    /// said deny / escalate.
    PolicyIssue,
    /// Memory write rejected, scope violation, or stale recall.
    MemoryIssue,
    /// Tool failed and recovery did not complete the task.
    RecoveryIssue,
    /// Wrong tool chosen / repeated mis-invocation.
    ToolMisuse,
    /// Anything not yet covered by the closed catalogue.
    Other,
}

impl FailureCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::IntentMiss => "intent_miss",
            Self::PolicyIssue => "policy_issue",
            Self::MemoryIssue => "memory_issue",
            Self::RecoveryIssue => "recovery_issue",
            Self::ToolMisuse => "tool_misuse",
            Self::Other => "other",
        }
    }
}

/// Classify one [`BlockingFailure`] into a [`FailureCategory`].
/// Combines the failure code + severity into a stable bucket.
/// Caller-supplied context (e.g. tool retry count) can override
/// at the cluster layer.
#[must_use]
pub fn classify_blocking_failure(failure: &BlockingFailure) -> FailureCategory {
    let code = failure.code.to_ascii_lowercase();
    if code.contains("permission")
        || code.contains("policy")
        || code.contains("denied")
        || code.contains("escalat")
        || code.contains("boundary")
    {
        return FailureCategory::PolicyIssue;
    }
    if code.contains("memory")
        || code.contains("recall")
        || code.contains("scope_violation")
        || code.contains("conflict")
    {
        return FailureCategory::MemoryIssue;
    }
    if code.contains("recovery")
        || code.contains("resume")
        || code.contains("recover")
        || code.contains("dead_loop")
    {
        return FailureCategory::RecoveryIssue;
    }
    if code.contains("tool") || code.contains("misuse") {
        return FailureCategory::ToolMisuse;
    }
    if code.contains("intent") || code.contains("misunderstood") {
        return FailureCategory::IntentMiss;
    }
    // Severity hint: blocking-level unknowns lean Recovery
    // (something hard-failed); warning-level unknowns lean
    // ToolMisuse (a soft signal the agent did something
    // suboptimal).
    match failure.severity {
        Severity::Blocking => FailureCategory::RecoveryIssue,
        Severity::Warning => FailureCategory::ToolMisuse,
        _ => FailureCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make(code: &str, sev: Severity) -> BlockingFailure {
        BlockingFailure {
            code: code.into(),
            severity: sev,
            message: String::new(),
            evidence_ref: None,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn classifies_known_codes() {
        assert_eq!(
            classify_blocking_failure(&make("memory_rejected", Severity::Blocking)),
            FailureCategory::MemoryIssue
        );
        assert_eq!(
            classify_blocking_failure(&make("permission_denied", Severity::Blocking)),
            FailureCategory::PolicyIssue
        );
        assert_eq!(
            classify_blocking_failure(&make("tool_failure", Severity::Warning)),
            FailureCategory::ToolMisuse
        );
        assert_eq!(
            classify_blocking_failure(&make("recovery_failure", Severity::Blocking)),
            FailureCategory::RecoveryIssue
        );
    }

    #[test]
    fn falls_back_via_severity() {
        assert_eq!(
            classify_blocking_failure(&make("unknown_code", Severity::Blocking)),
            FailureCategory::RecoveryIssue
        );
        assert_eq!(
            classify_blocking_failure(&make("unknown_code", Severity::Info)),
            FailureCategory::Other
        );
    }
}
