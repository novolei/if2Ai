//! DayDreamEngine — orchestrator that dispatches steps by strategy.

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::modules::memory::evolution::{
    procedural::ProceduralMemoryManager, reflector::SelfReflector, trajectory::Trajectory,
};
use crate::modules::memory::forgetting::ForgettingCurveEngine;
use crate::modules::memory::quality::QualityScorer;
use crate::modules::memory::{SharedMemoryProvider, UtilityLlm};

use super::config::{ConsolidationStrategy, DayDreamConfig};
use super::merge::{MergeBudget, AGGRESSIVE_MERGE_TOKEN_CAP, BALANCED_MERGE_TOKEN_CAP};
use super::report::{DayDreamReport, StepError, StepOutcome};
use super::{merge, prune, reflect, refresh};

/// Source of trajectories the reflect step consumes.
#[async_trait::async_trait]
pub trait TrajectorySource: Send + Sync {
    /// Return up to `max` recent trajectories.
    async fn recent_trajectories(&self, max: usize) -> Result<Vec<Trajectory>, String>;
}

/// Orchestrator that dispatches consolidation steps according to the configured strategy.
pub struct DayDreamEngine {
    provider: SharedMemoryProvider,
    scorer: Arc<QualityScorer>,
    forgetting: Arc<ForgettingCurveEngine>,
    llm: Arc<dyn UtilityLlm>,
    reflector: Arc<SelfReflector>,
    procedural: Arc<ProceduralMemoryManager>,
    trajectories: Arc<dyn TrajectorySource>,
    config: DayDreamConfig,
    running: Mutex<bool>,
}

/// Errors returned by [`DayDreamEngine::run_cycle`].
#[derive(Debug, thiserror::Error)]
pub enum CycleError {
    #[error("daydream is disabled")]
    Disabled,
    #[error("a cycle is already running")]
    AlreadyRunning,
}

impl DayDreamEngine {
    /// Construct a new engine with the given collaborators and configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: SharedMemoryProvider,
        scorer: Arc<QualityScorer>,
        forgetting: Arc<ForgettingCurveEngine>,
        llm: Arc<dyn UtilityLlm>,
        reflector: Arc<SelfReflector>,
        procedural: Arc<ProceduralMemoryManager>,
        trajectories: Arc<dyn TrajectorySource>,
        config: DayDreamConfig,
    ) -> Self {
        Self {
            provider,
            scorer,
            forgetting,
            llm,
            reflector,
            procedural,
            trajectories,
            config,
            running: Mutex::new(false),
        }
    }

    /// Return a reference to the active configuration.
    pub fn config(&self) -> &DayDreamConfig {
        &self.config
    }

    /// Execute one consolidation cycle.
    ///
    /// `trigger` is a short label ("idle", "manual", etc.) stored in the report.
    /// Returns [`CycleError::Disabled`] when the engine is not enabled, or
    /// [`CycleError::AlreadyRunning`] when a concurrent cycle is in progress.
    pub async fn run_cycle(&self, trigger: &str) -> Result<DayDreamReport, CycleError> {
        if !self.config.enabled {
            return Err(CycleError::Disabled);
        }
        {
            let mut guard = self.running.lock().await;
            if *guard {
                return Err(CycleError::AlreadyRunning);
            }
            *guard = true;
        }
        let report = self.run_cycle_inner(trigger).await;
        *self.running.lock().await = false;
        Ok(report)
    }

    async fn run_cycle_inner(&self, trigger: &str) -> DayDreamReport {
        let started_at = chrono::Utc::now();
        let cycle_id = uuid::Uuid::new_v4().to_string();
        let strategy = self.config.strategy;
        let max = self.config.max_entries_per_cycle;
        let mut steps: Vec<StepOutcome> = Vec::new();

        // Step 1 — Prune (always).
        steps.push(prune::run(&self.provider, &self.scorer, &self.forgetting, max).await);

        // Step 2 — Merge (Balanced + Aggressive).
        if matches!(
            strategy,
            ConsolidationStrategy::Balanced | ConsolidationStrategy::Aggressive
        ) {
            let cap = if matches!(strategy, ConsolidationStrategy::Aggressive) {
                AGGRESSIVE_MERGE_TOKEN_CAP
            } else {
                BALANCED_MERGE_TOKEN_CAP
            };
            steps.push(
                merge::run(
                    &self.provider,
                    &self.scorer,
                    self.llm.as_ref(),
                    &MergeBudget {
                        max_entries: max,
                        token_cap: cap,
                    },
                )
                .await,
            );
        }

        // Step 3 — Refresh (Aggressive only).
        if matches!(strategy, ConsolidationStrategy::Aggressive) {
            steps.push(refresh::run(&self.provider, max).await);
        }

        // Step 4 — Reflect (Balanced + Aggressive).
        if matches!(
            strategy,
            ConsolidationStrategy::Balanced | ConsolidationStrategy::Aggressive
        ) {
            match self.trajectories.recent_trajectories(max).await {
                Ok(t) if !t.is_empty() => {
                    steps.push(reflect::run(&self.reflector, &self.procedural, t).await);
                }
                Ok(_) => {} // no trajectories — skip silently
                Err(e) => {
                    steps.push(StepOutcome {
                        step: "reflect".into(),
                        examined: 0,
                        mutated: 0,
                        duration_ms: 0,
                        error: Some(StepError {
                            kind: "provider".into(),
                            message: format!("trajectory source failed: {e}"),
                        }),
                        extracted_count: None,
                        rejected_count: None,
                    });
                }
            }
        }

        DayDreamReport {
            cycle_id,
            trigger: trigger.into(),
            strategy: format!("{strategy:?}").to_lowercase(),
            started_at,
            finished_at: chrono::Utc::now(),
            steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_strategies_exist() {
        let _ = ConsolidationStrategy::Conservative;
        let _ = ConsolidationStrategy::Balanced;
        let _ = ConsolidationStrategy::Aggressive;
    }

    #[test]
    fn cycle_error_disabled_message() {
        let e = CycleError::Disabled;
        assert_eq!(format!("{e}"), "daydream is disabled");
    }

    #[test]
    fn cycle_error_already_running_message() {
        let e = CycleError::AlreadyRunning;
        assert_eq!(format!("{e}"), "a cycle is already running");
    }
}
