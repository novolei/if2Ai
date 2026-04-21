//! Harness graders skeleton (Phase M4.3).
//!
//! Each grader is a **pure function** over the
//! [`super::run_report::HarnessRunReport`] that produces one typed
//! [`GraderVerdict`].  The full grader registry / weighted-score /
//! recommendation pipeline lands in M4.5+; this module only
//! defines the contract + the first five canonical graders the
//! M4 runbook lists:
//!
//!   - [`task_success::grade`] — REAL input
//!   - [`permission_compliance::grade`] — PARTIAL input today (no
//!     `permission_resolved` event yet)
//!   - [`memory_alignment::grade`] — REAL input (M4-A)
//!   - [`recovery_resilience::grade`] — SKELETON (no resume /
//!     retry events yet)
//!   - [`resource_efficiency::grade`] — REAL input (token + duration)
//!
//! Honest scope rule (`Severity::Skeleton`):
//!
//! Graders that depend on traces the codebase has not yet emitted
//! MUST return `Severity::Skeleton` instead of `Pass`.  Absence of
//! signal is itself signal — the M4.5 compare flow will surface
//! `Skeleton` distinctly so reviewers know the verdict was not
//! actually computed.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::run_report::{HarnessRunReport, Severity, TaskOutcome};

/// Stable grader contract version.  Bumping is breaking.
pub const HARNESS_GRADERS_VERSION: &str = "harness-graders@m4.3-skeleton";

/// Closed-set grader id alphabet.  Adding a variant is a breaking
/// contract bump that the M4.5 compare must honor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraderId {
    TaskSuccess,
    PermissionCompliance,
    MemoryAlignment,
    RecoveryResilience,
    ResourceEfficiency,
}

impl GraderId {
    /// Stable wire label suitable for governance UIs.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::TaskSuccess => "task_success",
            Self::PermissionCompliance => "permission_compliance",
            Self::MemoryAlignment => "memory_alignment",
            Self::RecoveryResilience => "recovery_resilience",
            Self::ResourceEfficiency => "resource_efficiency",
        }
    }
}

/// One grader's typed verdict.  Held by value; held in the future
/// `HarnessRunReport.grades` extension when M4.5 lands the
/// recommendation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraderVerdict {
    pub grader_id: GraderId,
    pub severity: Severity,
    /// Closed-ish reason codes the grader attached.  Open string
    /// here; M4.8 gate may pin a closed catalogue.
    pub reason_codes: Vec<String>,
    /// Short human-readable summary.  Reviewers / future UI render
    /// this verbatim.
    pub message: String,
    /// Optional pointers into the report's evidence bundle.  Same
    /// scheme as
    /// [`super::run_report::BlockingFailure::evidence_ref`].
    pub evidence_refs: Vec<String>,
    /// Stable grader contract version (`HARNESS_GRADERS_VERSION`).
    pub grader_version: String,
}

impl GraderVerdict {
    fn new(
        id: GraderId,
        severity: Severity,
        reason_codes: Vec<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            grader_id: id,
            severity,
            reason_codes,
            message: message.into(),
            evidence_refs: Vec::new(),
            grader_version: HARNESS_GRADERS_VERSION.to_string(),
        }
    }

    fn with_evidence(mut self, refs: Vec<String>) -> Self {
        self.evidence_refs = refs;
        self
    }
}

/// Run all M4.3 graders over `report` and return the verdicts in
/// stable order (matching [`GraderId`]'s declaration order).
#[must_use]
pub fn run_all(report: &HarnessRunReport) -> Vec<GraderVerdict> {
    vec![
        task_success::grade(report),
        permission_compliance::grade(report),
        memory_alignment::grade(report),
        recovery_resilience::grade(report),
        resource_efficiency::grade(report),
    ]
}

// ───────────────────────── TaskSuccess (REAL) ────────────────────

pub mod task_success {
    //! Grader source: REAL.  Reads
    //! [`super::HarnessRunReport::task::outcome`] +
    //! [`super::HarnessRunReport::aggregate::turns_completed`].
    //!
    //! Severity mapping:
    //!
    //!   - `Pass`     → `TaskOutcome::Success`
    //!   - `Warning`  → `TaskOutcome::PartialSuccess`
    //!   - `Blocking` → `TaskOutcome::Failed`
    //!   - `Skeleton` → `TaskOutcome::Incomplete` (no turn finished
    //!                  → grader cannot judge)

    use super::*;

    pub fn grade(report: &HarnessRunReport) -> GraderVerdict {
        match report.task.outcome {
            TaskOutcome::Success => GraderVerdict::new(
                GraderId::TaskSuccess,
                Severity::Pass,
                vec!["task_success".to_string()],
                "Task completed; last turn succeeded; no warnings observed.",
            ),
            TaskOutcome::PartialSuccess => GraderVerdict::new(
                GraderId::TaskSuccess,
                Severity::Warning,
                vec!["task_partial_success".to_string()],
                "Task completed but at least one warning was observed.",
            ),
            TaskOutcome::Failed => GraderVerdict::new(
                GraderId::TaskSuccess,
                Severity::Blocking,
                vec!["task_failed".to_string()],
                "Task ended in failure (last turn failed or blocking failure observed).",
            ),
            TaskOutcome::Incomplete => GraderVerdict::new(
                GraderId::TaskSuccess,
                Severity::Skeleton,
                vec!["task_incomplete_no_input".to_string()],
                "Task did not produce a TurnFinished event; cannot judge.",
            ),
        }
    }
}

// ───────────────── PermissionCompliance (PARTIAL) ────────────────

pub mod permission_compliance {
    //! Grader source: PARTIAL.  We see `PermissionPrompted` events
    //! today (`prompt fired`), but the **resolution** (`allow /
    //! deny`, `once / session`) is not yet on the EventBus —
    //! `permission_resolved` lives only in the frontend
    //! projection.  Until that lands on the bus we can only:
    //!
    //!   - count prompts (counter is real)
    //!   - report `Skeleton` when no prompts fired (we honestly
    //!     don't know if approvals were correct)
    //!   - report `Info` when prompts fired (review still required)
    //!
    //! Refinement plan: M4.4 P4 lifts `permission_resolved` onto
    //! the bus → this grader upgrades from PARTIAL to REAL with no
    //! contract change.

    use super::*;

    pub fn grade(report: &HarnessRunReport) -> GraderVerdict {
        let prompt_count = report.aggregate.permission_prompts;
        if prompt_count == 0 {
            return GraderVerdict::new(
                GraderId::PermissionCompliance,
                Severity::Skeleton,
                vec!["no_permission_traffic".to_string()],
                "No permission prompts observed and no resolution events on EventBus; \
                 cannot verify compliance from current trace inputs.",
            );
        }
        let evidence_refs: Vec<String> = (0..report.evidence.permission_prompts.len())
            .map(|i| format!("permission_prompts:{i}"))
            .collect();
        GraderVerdict::new(
            GraderId::PermissionCompliance,
            Severity::Info,
            vec!["permission_prompts_observed".to_string()],
            format!(
                "{prompt_count} permission prompt(s) fired; resolution events not yet on \
                 EventBus, manual review required."
            ),
        )
        .with_evidence(evidence_refs)
    }
}

// ───────────────────── MemoryAlignment (REAL) ────────────────────

pub mod memory_alignment {
    //! Grader source: REAL (Phase M4-A).  Reads the
    //! `memory_after_turn` envelopes off the report's
    //! [`super::HarnessRunReport::evidence::memory_after_turn`]
    //! plus the matching aggregate counters.
    //!
    //! Severity mapping:
    //!
    //!   - `Blocking` → any `MemoryWriteDisposition::Deny` decision
    //!                  (already mirrored as a `BlockingFailure`)
    //!                  OR any `RejectCandidate` conflict
    //!   - `Warning`  → at least one `RequirePrompt` conflict OR
    //!                  any quality-gate `rejected` (without Deny)
    //!   - `Info`     → quality-gate `warnings` exist but no
    //!                  rejections / conflicts
    //!   - `Pass`     → at least one decision observed AND no
    //!                  rejections / warnings / conflicts that
    //!                  required a prompt
    //!   - `Skeleton` → no `MemoryAfterTurn` envelopes observed
    //!                  at all (run produced zero candidates)

    use super::*;
    use crate::modules::application::ConflictResolutionOutcome;
    use crate::modules::runtime::contracts::memory::MemoryWriteDisposition;

    pub fn grade(report: &HarnessRunReport) -> GraderVerdict {
        if report.evidence.memory_after_turn.is_empty() {
            return GraderVerdict::new(
                GraderId::MemoryAlignment,
                Severity::Skeleton,
                vec!["no_memory_after_turn_evidence".to_string()],
                "No MemoryAfterTurn envelopes observed (run produced zero memory candidates); \
                 cannot judge alignment.",
            );
        }
        let mut deny = 0usize;
        let mut require_prompt = 0usize;
        let mut reject_candidate = 0usize;
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        let mut warnings = 0usize;
        let mut evidence_refs = Vec::new();
        for (idx, env) in report.evidence.memory_after_turn.iter().enumerate() {
            for d in &env.decisions {
                if matches!(d.disposition, MemoryWriteDisposition::Deny) {
                    deny += 1;
                }
            }
            for c in &env.conflicts {
                match c.outcome {
                    ConflictResolutionOutcome::RequirePrompt => require_prompt += 1,
                    ConflictResolutionOutcome::RejectCandidate => reject_candidate += 1,
                    _ => {}
                }
            }
            accepted += env.quality.accepted.len();
            rejected += env.quality.rejected.len();
            warnings += env.quality.warnings.len();
            evidence_refs.push(format!("memory_after_turn:{idx}"));
        }
        let mut codes = Vec::new();
        if deny > 0 {
            codes.push("memory_write_denied".to_string());
        }
        if reject_candidate > 0 {
            codes.push("conflict_reject_candidate".to_string());
        }
        if require_prompt > 0 {
            codes.push("conflict_require_prompt".to_string());
        }
        if rejected > 0 {
            codes.push("quality_rejected".to_string());
        }
        if warnings > 0 {
            codes.push("quality_warnings".to_string());
        }

        let severity = if deny > 0 || reject_candidate > 0 {
            Severity::Blocking
        } else if require_prompt > 0 || rejected > 0 {
            Severity::Warning
        } else if warnings > 0 {
            Severity::Info
        } else {
            Severity::Pass
        };
        if codes.is_empty() {
            codes.push("memory_alignment_clean".to_string());
        }
        let message = format!(
            "{accepted} accepted, {rejected} rejected, {warnings} warning(s), \
             {require_prompt} prompt-conflict(s), {reject_candidate} reject-conflict(s), \
             {deny} hard-deny."
        );
        GraderVerdict::new(GraderId::MemoryAlignment, severity, codes, message)
            .with_evidence(evidence_refs)
    }
}

// ────────────────── RecoveryResilience (SKELETON) ───────────────

pub mod recovery_resilience {
    //! Grader source: SKELETON.  Recovery / resume / retry signals
    //! (`stream_error -> resume_available` cycle, mid-run rollback,
    //! tool-retry success after first failure) are not yet on the
    //! harness EventBus today.  This grader currently approximates
    //! using only `tool_failure` blocking failures vs subsequent
    //! tool successes — a coarse heuristic that may produce
    //! noisy verdicts.  It honestly degrades to `Skeleton` when
    //! there are no failures to evaluate against.
    //!
    //! Refinement plan: M4.4 P5 lifts `stream_error` /
    //! `resume_invoked` onto the EventBus → this grader graduates
    //! from SKELETON to PARTIAL with no contract change.

    use super::*;

    pub fn grade(report: &HarnessRunReport) -> GraderVerdict {
        let tool_failures = report
            .blocking_failures
            .iter()
            .filter(|f| f.code == "tool_failure")
            .count();
        if tool_failures == 0 {
            return GraderVerdict::new(
                GraderId::RecoveryResilience,
                Severity::Skeleton,
                vec!["no_recovery_signal".to_string()],
                "No tool failures and no resume / retry events on EventBus; \
                 cannot assess recovery resilience.",
            );
        }
        // Coarse heuristic: did the agent end with a successful
        // last turn after observing failures?  If so, treat as
        // Info (recovered); else Warning (failures uncovered).
        if report.task.last_turn_succeeded {
            GraderVerdict::new(
                GraderId::RecoveryResilience,
                Severity::Info,
                vec!["recovered_after_tool_failure".to_string()],
                format!(
                    "Observed {tool_failures} tool failure(s) but last turn succeeded; \
                     coarse-recovery only — refine when stream_error / resume are bus-attached."
                ),
            )
        } else {
            GraderVerdict::new(
                GraderId::RecoveryResilience,
                Severity::Warning,
                vec!["unrecovered_tool_failure".to_string()],
                format!(
                    "Observed {tool_failures} tool failure(s) and last turn did not succeed; \
                     resilience signal partial."
                ),
            )
        }
    }
}

// ────────────────── ResourceEfficiency (REAL) ────────────────────

pub mod resource_efficiency {
    //! Grader source: REAL.  Reads
    //! [`super::HarnessRunReport::aggregate::input_tokens_total`]
    //! / `output_tokens_total` / `total_turn_duration_ms` /
    //! `turns_completed`.
    //!
    //! M4.3 thresholds (intentionally conservative; M4.8 gate
    //! will pin tighter values per corpus tier):
    //!
    //!   - `> 8000` avg input tokens per turn → `Warning`
    //!   - `> 16000` avg input tokens per turn → `Blocking`
    //!   - `> 30 000 ms` avg turn duration → `Warning`
    //!   - `> 90 000 ms` avg turn duration → `Blocking`
    //!   - `0` turns completed → `Skeleton`

    use super::*;

    const WARN_AVG_INPUT_TOKENS: u64 = 8_000;
    const BLOCK_AVG_INPUT_TOKENS: u64 = 16_000;
    const WARN_AVG_DURATION_MS: u64 = 30_000;
    const BLOCK_AVG_DURATION_MS: u64 = 90_000;

    pub fn grade(report: &HarnessRunReport) -> GraderVerdict {
        if report.aggregate.turns_completed == 0 {
            return GraderVerdict::new(
                GraderId::ResourceEfficiency,
                Severity::Skeleton,
                vec!["no_turn_input".to_string()],
                "Run produced no completed turns; cannot judge resource efficiency.",
            );
        }
        let avg_input =
            report.aggregate.input_tokens_total / report.aggregate.turns_completed.max(1);
        let avg_duration = report.aggregate.avg_turn_duration_ms();
        let mut codes = Vec::new();
        let mut severity = Severity::Pass;
        if avg_input > BLOCK_AVG_INPUT_TOKENS {
            severity = Severity::Blocking;
            codes.push("avg_input_tokens_blocking".to_string());
        } else if avg_input > WARN_AVG_INPUT_TOKENS {
            severity = upgrade(severity, Severity::Warning);
            codes.push("avg_input_tokens_warning".to_string());
        }
        if avg_duration > BLOCK_AVG_DURATION_MS {
            severity = upgrade(severity, Severity::Blocking);
            codes.push("avg_turn_duration_blocking".to_string());
        } else if avg_duration > WARN_AVG_DURATION_MS {
            severity = upgrade(severity, Severity::Warning);
            codes.push("avg_turn_duration_warning".to_string());
        }
        if codes.is_empty() {
            codes.push("resource_efficiency_pass".to_string());
        }
        let message = format!(
            "avg_input_tokens_per_turn={avg_input}, avg_turn_duration_ms={avg_duration}, \
             total_input_tokens={total_in}, total_output_tokens={total_out}, \
             turns_completed={turns}",
            total_in = report.aggregate.input_tokens_total,
            total_out = report.aggregate.output_tokens_total,
            turns = report.aggregate.turns_completed,
        );
        GraderVerdict::new(GraderId::ResourceEfficiency, severity, codes, message)
    }

    fn upgrade(prev: Severity, new: Severity) -> Severity {
        // Severity precedence (high to low):
        // Blocking > Warning > Info > Pass > Skeleton
        fn rank(s: Severity) -> u8 {
            match s {
                Severity::Blocking => 4,
                Severity::Warning => 3,
                Severity::Info => 2,
                Severity::Pass => 1,
                Severity::Skeleton => 0,
            }
        }
        if rank(new) > rank(prev) {
            new
        } else {
            prev
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run_report::{HarnessRunReport, MemoryAfterTurnTrace, Severity, TaskOutcome};
    use super::*;
    use crate::modules::application::{
        ConflictResolution, ConflictResolutionOutcome, QualityGateResult,
        MEMORY_CONFLICT_RESOLVER_VERSION, MEMORY_QUALITY_GATE_VERSION,
    };
    use crate::modules::runtime::contracts::memory::{
        MemoryObjectKind, MemoryScope, MemoryWriteDecision, MemoryWriteDisposition,
    };
    use chrono::Utc;

    fn empty_report() -> HarnessRunReport {
        HarnessRunReport::new_empty("run-x", Utc::now())
    }

    fn synth_decision(disp: MemoryWriteDisposition) -> MemoryWriteDecision {
        MemoryWriteDecision {
            disposition: disp,
            reason_codes: Vec::new(),
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            evidence_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: "2026-04-20T00:00:00Z".to_string(),
        }
    }

    fn synth_envelope(
        decisions: Vec<MemoryWriteDecision>,
        conflicts: Vec<ConflictResolution>,
    ) -> MemoryAfterTurnTrace {
        MemoryAfterTurnTrace {
            trace_version: "memory-after-turn-trace@m4.1".to_string(),
            caller: "test".to_string(),
            session_id: None,
            project_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: Utc::now(),
            decisions,
            quality: QualityGateResult {
                accepted: Vec::new(),
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
            },
            conflicts,
        }
    }

    #[test]
    fn task_success_skeleton_when_incomplete() {
        let r = empty_report();
        let v = task_success::grade(&r);
        assert_eq!(v.severity, Severity::Skeleton);
    }

    #[test]
    fn memory_alignment_skeleton_when_no_evidence() {
        let r = empty_report();
        let v = memory_alignment::grade(&r);
        assert_eq!(v.severity, Severity::Skeleton);
    }

    #[test]
    fn memory_alignment_blocks_on_deny() {
        let mut r = empty_report();
        r.evidence.memory_after_turn.push(synth_envelope(
            vec![synth_decision(MemoryWriteDisposition::Deny)],
            Vec::new(),
        ));
        let v = memory_alignment::grade(&r);
        assert_eq!(v.severity, Severity::Blocking);
        assert!(v.reason_codes.contains(&"memory_write_denied".to_string()));
    }

    #[test]
    fn memory_alignment_warns_on_require_prompt_conflict() {
        let mut r = empty_report();
        r.evidence.memory_after_turn.push(synth_envelope(
            vec![synth_decision(MemoryWriteDisposition::Allow)],
            vec![ConflictResolution {
                outcome: ConflictResolutionOutcome::RequirePrompt,
                reason_codes: vec!["same_fact_different_value".to_string()],
                policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
            }],
        ));
        let v = memory_alignment::grade(&r);
        assert_eq!(v.severity, Severity::Warning);
    }

    #[test]
    fn memory_alignment_passes_on_clean_envelope() {
        let mut r = empty_report();
        r.evidence.memory_after_turn.push(synth_envelope(
            vec![synth_decision(MemoryWriteDisposition::Allow)],
            vec![ConflictResolution {
                outcome: ConflictResolutionOutcome::NoConflict,
                reason_codes: vec!["no_conflict".to_string()],
                policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
            }],
        ));
        let v = memory_alignment::grade(&r);
        assert_eq!(v.severity, Severity::Pass);
    }

    #[test]
    fn permission_compliance_skeleton_when_no_prompts() {
        let r = empty_report();
        let v = permission_compliance::grade(&r);
        assert_eq!(v.severity, Severity::Skeleton);
    }

    #[test]
    fn resource_efficiency_skeleton_when_no_turns() {
        let r = empty_report();
        let v = resource_efficiency::grade(&r);
        assert_eq!(v.severity, Severity::Skeleton);
    }

    #[test]
    fn resource_efficiency_warns_on_high_input_tokens() {
        let mut r = empty_report();
        r.aggregate.turns_completed = 1;
        r.aggregate.input_tokens_total = 9_000;
        let v = resource_efficiency::grade(&r);
        assert_eq!(v.severity, Severity::Warning);
        assert!(v
            .reason_codes
            .contains(&"avg_input_tokens_warning".to_string()));
    }

    #[test]
    fn run_all_emits_one_verdict_per_grader_in_stable_order() {
        let r = empty_report();
        let verdicts = run_all(&r);
        assert_eq!(verdicts.len(), 5);
        assert_eq!(verdicts[0].grader_id, GraderId::TaskSuccess);
        assert_eq!(verdicts[1].grader_id, GraderId::PermissionCompliance);
        assert_eq!(verdicts[2].grader_id, GraderId::MemoryAlignment);
        assert_eq!(verdicts[3].grader_id, GraderId::RecoveryResilience);
        assert_eq!(verdicts[4].grader_id, GraderId::ResourceEfficiency);
        for v in &verdicts {
            assert_eq!(v.grader_version, HARNESS_GRADERS_VERSION);
            // Every M4.3 grader honestly reports Skeleton on the
            // empty report.
            assert_eq!(
                v.severity,
                Severity::Skeleton,
                "grader {:?} should be Skeleton on empty report",
                v.grader_id
            );
        }
    }

    #[test]
    fn task_success_passes_on_success_outcome() {
        let mut r = empty_report();
        r.aggregate.turns_completed = 1;
        r.aggregate.turns_succeeded = 1;
        r.task.outcome = TaskOutcome::Success;
        r.task.last_turn_succeeded = true;
        let v = task_success::grade(&r);
        assert_eq!(v.severity, Severity::Pass);
    }
}
