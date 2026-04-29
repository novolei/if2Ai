//! WU-005 — Self-edit background scanner.
//!
//! Periodic tokio task that walks the AE-001 → AE-002 → AE-003
//! pipeline:
//!
//! 1. `cluster_failures(reports)` — collect cross-run blocking failures.
//! 2. `generate_proposals(failures, history, llm)` — draft proposals.
//! 3. `verify_proposals(...)` — 4-gate filter.
//! 4. `next_stage(...)` — promotion state machine input.
//!
//! Every step is **failure-isolated**: any error / panic logs at warn
//! and the task continues with the next tick. The scanner runs only
//! when `IF2AI_DISABLE_SELF_EDIT=1` is unset; production callers
//! decide whether to actually `spawn` the loop.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use crate::modules::api::InputMessage;
use crate::modules::harness::run_report::HarnessRunReport;
use crate::modules::learning::failure_clustering::{cluster_failures, ClusteredFailureSet};
use crate::modules::memory::UtilityLlm;

use super::{
    generate_proposals, next_stage, verify_proposals, PromotionStage, SelfEditProposal,
    StageTransition, Verdict, VerificationVerdict,
};
use crate::modules::skills::sedimentation::Embedder;

/// Env var disabling the WU-005 background scanner.
pub const DISABLE_SELF_EDIT_ENV: &str = "IF2AI_DISABLE_SELF_EDIT";

/// Default tick interval between full scanner passes.
pub const DEFAULT_SCANNER_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// `true` when the kill-switch is set.
#[must_use]
pub fn self_edit_scanner_disabled() -> bool {
    std::env::var(DISABLE_SELF_EDIT_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// One pass of the scanner pipeline. Pure-async helper exposed so the
/// production spawn site + tests run identical logic.
pub async fn run_scanner_once(
    reports: &[&HarnessRunReport],
    history: &[InputMessage],
    llm: Arc<dyn UtilityLlm>,
    embedder: &dyn Embedder,
    current_stage: PromotionStage,
    failure_rate: f32,
    sample_size: usize,
) -> ScannerOutcome {
    if self_edit_scanner_disabled() {
        return ScannerOutcome {
            cluster: ClusteredFailureSet::empty(),
            proposals: Vec::new(),
            verdicts: Vec::new(),
            transition: StageTransition::Hold,
            skipped_reason: Some("kill-switch set".to_string()),
        };
    }

    let cluster = cluster_failures(reports);
    if cluster.clusters.is_empty() {
        return ScannerOutcome {
            cluster,
            proposals: Vec::new(),
            verdicts: Vec::new(),
            transition: StageTransition::Hold,
            skipped_reason: Some("no clusters yet".to_string()),
        };
    }

    let proposals = generate_proposals(&cluster, history, &*llm).await;
    let verdicts = verify_proposals(proposals.clone(), &[], embedder);
    let transition = next_stage(current_stage, failure_rate, sample_size);

    ScannerOutcome {
        cluster,
        proposals,
        verdicts,
        transition,
        skipped_reason: None,
    }
}

/// Output of one scanner pass — captured by the spawn site so it can
/// emit the per-step evolution events without re-running the
/// pipeline.
#[derive(Debug)]
pub struct ScannerOutcome {
    pub cluster: ClusteredFailureSet,
    pub proposals: Vec<SelfEditProposal>,
    pub verdicts: Vec<(SelfEditProposal, VerificationVerdict)>,
    pub transition: StageTransition,
    pub skipped_reason: Option<String>,
}

impl ScannerOutcome {
    /// `true` when at least one verified proposal passed the 4-gate.
    #[must_use]
    pub fn has_passing_proposal(&self) -> bool {
        self.verdicts
            .iter()
            .any(|(_, v)| v.verdict == Verdict::Pass)
    }
}
