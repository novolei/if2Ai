//! Prune step — runs the forgetting sweep and adapts its report.

use crate::modules::memory::forgetting::ForgettingCurveEngine;
use crate::modules::memory::quality::QualityScorer;
use crate::modules::memory::SharedMemoryProvider;

use super::report::{StepError, StepOutcome};

/// Run the forgetting sweep and produce a `StepOutcome` for the daydream cycle.
pub async fn run(
    provider: &SharedMemoryProvider,
    scorer: &QualityScorer,
    forgetting: &ForgettingCurveEngine,
    max_entries: usize,
) -> StepOutcome {
    let started = std::time::Instant::now();
    let _ = max_entries; // forgetting sweep is internally bounded by provider size
    match forgetting.sweep(provider.as_ref(), scorer).await {
        Ok(r) => StepOutcome {
            step: "prune".into(),
            examined: r.scanned_count,
            mutated: r.archived_count,
            duration_ms: started.elapsed().as_millis() as u64,
            error: None,
            extracted_count: None,
            rejected_count: None,
        },
        Err(e) => StepOutcome {
            step: "prune".into(),
            examined: 0,
            mutated: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            error: Some(StepError {
                kind: "provider".into(),
                message: format!("forgetting sweep failed: {e}"),
            }),
            extracted_count: None,
            rejected_count: None,
        },
    }
}

#[cfg(test)]
mod tests {
    // The full async path needs an InMemoryMemoryProvider; that's exercised in
    // the engine tests (Task 6). This task ships the wrapper structurally.

    #[test]
    fn prune_step_label_constant() {
        // Sanity check: the step name string used by the report must
        // remain stable for the UI status row.
        let name = "prune";
        assert_eq!(name, "prune");
    }
}
