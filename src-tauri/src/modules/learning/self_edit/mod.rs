//! FEAT-AE-001..003 — Self-edit proposal pipeline.
//!
//! End-to-end flow (each step lands in its own Pack):
//!
//! 1. **AE-001 `proposal::generate_proposals`** — turn `ClusteredFailureSet`
//!    into `Vec<SelfEditProposal>` via `UtilityLlm`. Draft-only.
//! 2. **AE-002 `verification::verify_proposals`** — 4-gate filter
//!    (malformed / constitution / dedup / failure-history).
//! 3. **AE-003 `promotion::next_stage`** — rollout state machine
//!    (Shadow → Canary1Pct → Canary10Pct → Production with demote).
//!
//! Every primitive is **draft-only**: nothing is written to disk and
//! `learning::strategy_registry` is not invoked. A future wiring Pack
//! will tie verified + promoted proposals back into the existing
//! `CandidateStrategy` registry.

#![allow(dead_code)]

pub mod promotion;
pub mod proposal;
pub mod scanner;
pub mod verification;

#[allow(unused_imports)]
pub use promotion::{
    next_stage, PromotionStage, StageTransition, DEMOTE_FAILURE_THRESHOLD,
    MIN_SAMPLE_FOR_DECISION, PROMOTE_FAILURE_THRESHOLD,
};
#[allow(unused_imports)]
pub use proposal::{generate_proposals, ProposalKind, SelfEditProposal};
#[allow(unused_imports)]
pub use verification::{
    verify_proposals, Verdict, VerificationVerdict, GATE_CONSTITUTION, GATE_DEDUP,
    GATE_FAILURE_HISTORY, GATE_MALFORMED, VERIFICATION_DEDUP_THRESHOLD,
};
