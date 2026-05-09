//! Cycle-outcome types reported back to the UI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-step outcome counters. Each step appends one of these to the
/// report. A failed step still produces a `StepOutcome` with
/// `error: Some(_)` so the cycle is observable end-to-end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutcome {
    /// Step name, lower-snake (`prune`, `merge`, `refresh`, `reflect`).
    pub step: String,
    /// Entries the step actually examined (≤ `max_entries_per_cycle`).
    pub examined: usize,
    /// Entries the step mutated (pruned / merged / refreshed / inserted).
    pub mutated: usize,
    /// Wall-clock milliseconds the step took.
    pub duration_ms: u64,
    /// `Some(reason)` if the step short-circuited or errored.
    pub error: Option<StepError>,
    /// A.4 — number of items the step considered before any internal
    /// filter. For `reflect`: insights produced by `SelfReflector` before
    /// the confidence floor. `None` for steps where the concept doesn't
    /// apply (prune / merge / refresh).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_count: Option<usize>,
    /// A.4 — number of items the step dropped by an internal filter.
    /// For `reflect`: insights below `MIN_INSIGHT_CONFIDENCE` (0.7).
    /// `None` for steps where the concept doesn't apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepError {
    /// Machine-readable kind (`token_cap`, `provider`, `llm`, `unexpected`).
    pub kind: String,
    /// Human-readable reason for the status row.
    pub message: String,
}

/// Per-cycle report. Emitted as a `RuntimeEventType::DaydreamCycle`
/// envelope on completion (success *or* failure).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayDreamReport {
    pub cycle_id: String,
    pub trigger: String,
    pub strategy: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub steps: Vec<StepOutcome>,
}

impl DayDreamReport {
    /// `true` iff every step completed without an `error`.
    pub fn all_succeeded(&self) -> bool {
        self.steps.iter().all(|s| s.error.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_succeeded_returns_false_when_any_step_errored() {
        let now = Utc::now();
        let r = DayDreamReport {
            cycle_id: "c1".into(),
            trigger: "idle".into(),
            strategy: "balanced".into(),
            started_at: now,
            finished_at: now,
            steps: vec![
                StepOutcome { step: "prune".into(), examined: 10, mutated: 2, duration_ms: 5, error: None, extracted_count: None, rejected_count: None },
                StepOutcome { step: "merge".into(), examined: 0, mutated: 0, duration_ms: 1, error: Some(StepError { kind: "token_cap".into(), message: "exceeded budget".into() }), extracted_count: None, rejected_count: None },
            ],
        };
        assert!(!r.all_succeeded());
    }
}
