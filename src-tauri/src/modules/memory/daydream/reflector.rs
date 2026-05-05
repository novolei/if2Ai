//! DayDream reflection step — integrates the evolution module's
//! [`SelfReflector`] and [`ProceduralMemoryManager`] into the DayDream
//! consolidation cycle.
//!
//! This is "Step 4: Reflect" — executed after Prune → Merge → Refresh.
//! It analyzes recent trajectories, extracts insights, and internalizes
//! them as procedural memories for prompt injection.

use crate::modules::memory::evolution::{
    ProceduralMemoryManager, SelfReflector, TrajectoryCollector,
};
use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryError, SharedMemoryProvider};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// Report produced by the DayDream reflection step.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DayDreamReflectionReport {
    /// Number of trajectories that were analyzed.
    pub trajectories_analyzed: usize,
    /// Number of insights extracted from trajectories.
    pub insights_extracted: usize,
    /// Number of new procedural memories created.
    pub procedures_created: usize,
    /// Number of existing procedural memories updated.
    pub procedures_updated: usize,
    /// Number of cross-trajectory patterns found.
    pub patterns_found: usize,
}

/// DayDream reflection step — bridges the evolution subsystem into the
/// DayDream consolidation pipeline.
///
/// When executed, it:
/// 1. Fetches recent trajectories from the [`TrajectoryCollector`].
/// 2. Runs [`SelfReflector::run_reflection_cycle`] to extract insights.
/// 3. Internalizes each insight via [`ProceduralMemoryManager`].
/// 4. Returns a summary [`DayDreamReflectionReport`].
pub struct DayDreamReflector {
    reflector: SelfReflector,
    procedural_mgr: ProceduralMemoryManager,
    collector: Arc<TrajectoryCollector>,
    /// Minimum number of completed trajectories before reflection triggers.
    min_trajectories: usize,
}

impl DayDreamReflector {
    /// Create a new DayDream reflector.
    ///
    /// - `provider`: shared memory backend for persistence.
    /// - `collector`: trajectory collector holding recent execution traces.
    #[must_use]
    pub fn new(provider: SharedMemoryProvider, collector: Arc<TrajectoryCollector>) -> Self {
        Self {
            reflector: SelfReflector::new(Arc::clone(&provider)),
            procedural_mgr: ProceduralMemoryManager::new(Arc::clone(&provider)),
            collector,
            min_trajectories: 5,
        }
    }

    /// Attach a [`UtilityLlm`] for LLM-assisted reflection.
    /// The LLM is forwarded to the inner [`SelfReflector`] for deeper
    /// trajectory analysis.  When `None`, reflection falls back to
    /// pure rule-based logic.
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn UtilityLlm>) -> Self {
        self.reflector = self.reflector.with_llm(llm);
        self
    }

    /// Execute the DayDream reflection step.
    ///
    /// 1. Retrieves the most recent trajectories from the collector.
    /// 2. If fewer than `min_trajectories` are available, returns an
    ///    empty report (reflection is skipped).
    /// 3. Runs the full reflection cycle to extract insights.
    /// 4. For each insight, calls [`ProceduralMemoryManager::internalize_insight`].
    /// 5. Returns a [`DayDreamReflectionReport`] summarizing the step.
    pub async fn run(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<DayDreamReflectionReport, MemoryError> {
        let trajectories = self.collector.recent_trajectories(50).await;

        if trajectories.len() < self.min_trajectories {
            tracing::debug!(
                count = trajectories.len(),
                min = self.min_trajectories,
                "DayDream reflect: skipping, insufficient trajectories"
            );
            return Ok(DayDreamReflectionReport::default());
        }

        // Run the reflection cycle
        let report = self.reflector.run_reflection_cycle(&self.collector).await?;

        let mut procedures_created: usize = 0;
        let mut procedures_updated: usize = 0;

        // Snapshot existing procedural keys BEFORE internalization so we can
        // correctly distinguish creates from updates.
        let existing_keys: HashSet<String> = self
            .procedural_mgr
            .active_procedures(scope)
            .await
            .unwrap_or_default()
            .iter()
            .map(|p| p.key.clone())
            .collect();

        // Internalize each insight as a procedural memory
        for insight in &report.insights {
            match self
                .procedural_mgr
                .internalize_insight(insight, scope)
                .await
            {
                Ok(key) => {
                    if existing_keys.contains(&key) {
                        procedures_updated += 1;
                    } else {
                        procedures_created += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        insight_id = %insight.id,
                        error = %e,
                        "Failed to internalize insight as procedural memory"
                    );
                }
            }
        }

        Ok(DayDreamReflectionReport {
            trajectories_analyzed: report.trajectory_count,
            insights_extracted: report.insights_extracted,
            procedures_created,
            procedures_updated,
            patterns_found: report.patterns_found,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::evolution::trajectory::*;
    use chrono::Utc;

    #[allow(deprecated)]
    fn test_provider() -> SharedMemoryProvider {
        Arc::new(crate::modules::memory::InMemoryMemoryProvider::new())
    }

    fn make_tool_call(name: &str, success: bool) -> ToolCallRecord {
        ToolCallRecord {
            tool_name: name.to_string(),
            args_summary: String::new(),
            success,
            duration_ms: 100,
            error_message: if success {
                None
            } else {
                Some("err".to_string())
            },
        }
    }

    fn make_turn(id: u32, success: bool, tools: Vec<ToolCallRecord>) -> TurnRecord {
        TurnRecord {
            turn_id: id,
            timestamp: Utc::now(),
            user_input_summary: None,
            agent_action: AgentAction::ToolUse,
            tool_calls: tools,
            success,
            self_assessment: None,
        }
    }

    #[tokio::test]
    async fn test_daydream_reflector_skip_few_trajectories() {
        let provider = test_provider();
        let collector = Arc::new(TrajectoryCollector::new());

        // Add only 2 trajectories (below default min of 5)
        for i in 0..2 {
            let sid = format!("s{i}");
            collector.start_trajectory(&sid, &format!("task {i}")).await;
            collector
                .record_turn(&sid, make_turn(1, true, vec![make_tool_call("read", true)]))
                .await;
            collector
                .finish_trajectory(&sid, TaskOutcome::Success { quality_score: 0.9 })
                .await;
        }

        let reflector = DayDreamReflector::new(provider, collector);
        let scope = MemoryExecutionScope::global();
        let report = reflector.run(&scope).await;

        assert!(report.is_ok());
        let report = report.unwrap();
        // Should be skipped
        assert_eq!(report.trajectories_analyzed, 0);
        assert_eq!(report.insights_extracted, 0);
    }

    #[tokio::test]
    async fn test_daydream_reflector_run() {
        let provider = test_provider();
        let collector = Arc::new(TrajectoryCollector::new());

        // Add 6 trajectories (above min of 5), mix of success and failure
        for i in 0..3 {
            let sid = format!("fail-{i}");
            collector
                .start_trajectory(&sid, &format!("failing task {i}"))
                .await;
            collector
                .record_turn(
                    &sid,
                    make_turn(1, false, vec![make_tool_call("bad_tool", false)]),
                )
                .await;
            collector
                .record_turn(
                    &sid,
                    make_turn(2, false, vec![make_tool_call("bad_tool", false)]),
                )
                .await;
            collector
                .record_turn(
                    &sid,
                    make_turn(3, false, vec![make_tool_call("bad_tool", false)]),
                )
                .await;
            collector
                .finish_trajectory(
                    &sid,
                    TaskOutcome::Failure {
                        error_category: "tool_error".to_string(),
                        root_cause: "bad args".to_string(),
                    },
                )
                .await;
        }

        for i in 0..3 {
            let sid = format!("ok-{i}");
            collector
                .start_trajectory(&sid, &format!("good task {i}"))
                .await;
            collector
                .record_turn(
                    &sid,
                    make_turn(1, true, vec![make_tool_call("search", true)]),
                )
                .await;
            collector
                .record_turn(&sid, make_turn(2, true, vec![make_tool_call("edit", true)]))
                .await;
            collector
                .finish_trajectory(&sid, TaskOutcome::Success { quality_score: 0.9 })
                .await;
        }

        let reflector = DayDreamReflector::new(provider, collector);
        let scope = MemoryExecutionScope::global();
        let report = reflector.run(&scope).await;

        assert!(report.is_ok());
        let report = report.unwrap();
        assert_eq!(report.trajectories_analyzed, 6);
        assert!(
            report.insights_extracted > 0,
            "should have extracted at least one insight"
        );
    }
}
