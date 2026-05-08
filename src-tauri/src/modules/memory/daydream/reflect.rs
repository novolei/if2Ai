//! Reflect step — SelfReflector → ProceduralMemoryManager wiring.

use crate::modules::memory::evolution::{
    procedural::ProceduralMemoryManager,
    reflector::{Insight, SelfReflector},
    trajectory::{TaskOutcome, Trajectory},
};
use crate::modules::memory::MemoryExecutionScope;

use super::report::{StepError, StepOutcome};

/// Insights below this confidence are dropped before internalization.
pub const MIN_INSIGHT_CONFIDENCE: f64 = 0.7;

fn was_successful(t: &Trajectory) -> bool {
    matches!(t.outcome, Some(TaskOutcome::Success { .. }))
}

pub async fn run(
    reflector: &SelfReflector,
    procedural: &ProceduralMemoryManager,
    trajectories: Vec<Trajectory>,
) -> StepOutcome {
    let started = std::time::Instant::now();
    let examined = trajectories.len();

    let mut insights: Vec<Insight> = Vec::new();
    for t in &trajectories {
        if was_successful(t) {
            insights.extend(reflector.reflect_on_success(t));
        } else {
            insights.extend(reflector.reflect_on_failure(t));
        }
    }
    insights.retain(|i| i.confidence >= MIN_INSIGHT_CONFIDENCE);

    let scope = MemoryExecutionScope::global();
    let mut mutated = 0;
    for insight in insights {
        match procedural.internalize_insight(&insight, &scope).await {
            Ok(_) => mutated += 1,
            Err(e) => {
                return StepOutcome {
                    step: "reflect".into(),
                    examined,
                    mutated,
                    duration_ms: started.elapsed().as_millis() as u64,
                    error: Some(StepError {
                        kind: "provider".into(),
                        message: format!("internalize_insight failed: {e}"),
                    }),
                };
            }
        }
    }

    StepOutcome {
        step: "reflect".into(),
        examined,
        mutated,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_floor_is_07() {
        assert!((MIN_INSIGHT_CONFIDENCE - 0.7).abs() < 1e-9);
    }
}
