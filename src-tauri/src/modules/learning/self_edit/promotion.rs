//! FEAT-AE-003 — Self-edit promotion state machine.
//!
//! Pure-function rollout decisions. No side effects, no I/O, no
//! `strategy_registry` mutations — by Pack contract this Pack only
//! ships the *rules*; a future wiring Pack will route the
//! [`StageTransition`] outputs into `strategy_rollout::activate` /
//! `rollback`.
//!
//! Rules:
//! - Promote one stage when `sample_size ≥ MIN_SAMPLE_FOR_DECISION`
//!   AND `failure_rate < PROMOTE_FAILURE_THRESHOLD`.
//! - Demote one stage when `failure_rate ≥ DEMOTE_FAILURE_THRESHOLD`
//!   regardless of sample size (one bad burst is enough to roll back).
//! - Hold otherwise (insufficient evidence or already at boundary).
//! - Shadow can never demote (it's the floor); Production can never
//!   promote (it's the ceiling).

use serde::{Deserialize, Serialize};

/// Minimum number of executions in the rolling window before either
/// promotion or demotion is even considered.
pub const MIN_SAMPLE_FOR_DECISION: usize = 50;

/// Strict upper bound on the failure rate to allow promotion.
pub const PROMOTE_FAILURE_THRESHOLD: f32 = 0.05;

/// Lower bound on the failure rate that forces demotion (overrides
/// sample-size guard so a sudden incident can roll back fast).
pub const DEMOTE_FAILURE_THRESHOLD: f32 = 0.10;

/// 4-stage rollout ladder. Ordered: lower variants are safer (less
/// traffic). `Shadow` is the floor, `Production` the ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionStage {
    /// 0% live traffic — runs in shadow against production behavior.
    Shadow,
    /// ~1% live traffic.
    Canary1Pct,
    /// ~10% live traffic.
    Canary10Pct,
    /// 100% live traffic.
    Production,
}

impl PromotionStage {
    /// Stable label used in tracing + frontend projections.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PromotionStage::Shadow => "shadow",
            PromotionStage::Canary1Pct => "canary_1pct",
            PromotionStage::Canary10Pct => "canary_10pct",
            PromotionStage::Production => "production",
        }
    }

    fn next_higher(self) -> Option<Self> {
        match self {
            PromotionStage::Shadow => Some(PromotionStage::Canary1Pct),
            PromotionStage::Canary1Pct => Some(PromotionStage::Canary10Pct),
            PromotionStage::Canary10Pct => Some(PromotionStage::Production),
            PromotionStage::Production => None,
        }
    }

    fn next_lower(self) -> Option<Self> {
        match self {
            PromotionStage::Shadow => None,
            PromotionStage::Canary1Pct => Some(PromotionStage::Shadow),
            PromotionStage::Canary10Pct => Some(PromotionStage::Canary1Pct),
            PromotionStage::Production => Some(PromotionStage::Canary10Pct),
        }
    }
}

/// One transition decision from the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageTransition {
    /// Move up to the wrapped stage.
    Promote(PromotionStage),
    /// Stay where we are.
    Hold,
    /// Move down to the wrapped stage.
    Demote(PromotionStage),
}

/// Decide the next stage given the current rollout level + recent
/// rolling-window stats. Pure / deterministic / unit-testable.
#[must_use]
pub fn next_stage(
    current: PromotionStage,
    recent_failure_rate: f32,
    sample_size: usize,
) -> StageTransition {
    let rate = recent_failure_rate.clamp(0.0, 1.0);

    // 1. Demotion fires regardless of sample size — one bad burst is
    //    enough to pull traffic. Floor-bounded at Shadow.
    if rate >= DEMOTE_FAILURE_THRESHOLD {
        if let Some(lower) = current.next_lower() {
            return StageTransition::Demote(lower);
        }
        return StageTransition::Hold;
    }

    // 2. Promotion requires both signals: enough samples AND a clean
    //    rate. Ceiling-bounded at Production.
    if sample_size >= MIN_SAMPLE_FOR_DECISION && rate < PROMOTE_FAILURE_THRESHOLD {
        if let Some(higher) = current.next_higher() {
            return StageTransition::Promote(higher);
        }
        return StageTransition::Hold;
    }

    // 3. Anything else → keep the current stage.
    StageTransition::Hold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_is_stable() {
        assert_eq!(PromotionStage::Shadow.label(), "shadow");
        assert_eq!(PromotionStage::Production.label(), "production");
    }

    #[test]
    fn ordering_is_safer_first() {
        assert!(PromotionStage::Shadow < PromotionStage::Canary1Pct);
        assert!(PromotionStage::Canary10Pct < PromotionStage::Production);
    }
}
