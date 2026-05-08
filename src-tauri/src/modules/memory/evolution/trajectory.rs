//! Trajectory collector — captures structured execution traces for
//! the Reflexion self-improvement pipeline.
//!
//! Each [`Trajectory`] represents one task-level execution: a sequence
//! of [`TurnRecord`]s bookended by `start_trajectory` / `finish_trajectory`.
//! The [`TrajectoryCollector`] is the in-memory owner of all active and
//! recently-completed trajectories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Data models
// ---------------------------------------------------------------------------

/// Record of a single tool invocation within one turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Name of the tool that was invoked.
    pub tool_name: String,
    /// Brief summary of the arguments passed.
    pub args_summary: String,
    /// Whether the call completed successfully.
    pub success: bool,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Error message when `success` is false.
    pub error_message: Option<String>,
}

/// High-level classification of an agent action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentAction {
    /// The agent produced a natural-language reply.
    Reply,
    /// The agent invoked one or more tools.
    ToolUse,
    /// The agent performed internal reasoning / chain-of-thought.
    Reasoning,
    /// An error occurred during the action.
    Error,
}

/// One turn in the conversation / task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    /// Sequential identifier within the trajectory.
    pub turn_id: u32,
    /// Timestamp of this turn.
    pub timestamp: DateTime<Utc>,
    /// Brief summary of the user input (if any).
    pub user_input_summary: Option<String>,
    /// Classification of the agent's action.
    pub agent_action: AgentAction,
    /// Tool calls made during this turn.
    pub tool_calls: Vec<ToolCallRecord>,
    /// Whether the turn as a whole succeeded.
    pub success: bool,
    /// Optional self-assessment text from the agent.
    pub self_assessment: Option<String>,
}

/// Outcome of a completed trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutcome {
    /// Task completed fully with a quality score.
    Success {
        /// Quality score in `[0.0, 1.0]`.
        quality_score: f64,
    },
    /// Task partially completed.
    PartialSuccess {
        /// Ratio of sub-tasks completed, in `[0.0, 1.0]`.
        completed_ratio: f64,
    },
    /// Task failed.
    Failure {
        /// Broad error category (e.g. "timeout", "tool_error").
        error_category: String,
        /// Root cause description.
        root_cause: String,
    },
    /// Task was abandoned by the user or agent.
    Abandoned,
}

/// Complete execution trajectory for one task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Unique trajectory identifier.
    pub trajectory_id: String,
    /// Owning session identifier.
    pub session_id: String,
    /// Human-readable task description.
    pub task_description: String,
    /// Ordered list of turns.
    pub turns: Vec<TurnRecord>,
    /// Final outcome (set on `finish_trajectory`).
    pub outcome: Option<TaskOutcome>,
    /// When the trajectory started.
    pub started_at: DateTime<Utc>,
    /// When the trajectory finished (if completed).
    pub finished_at: Option<DateTime<Utc>>,
    /// Total wall-clock seconds from start to finish.
    pub duration_secs: Option<u64>,
    /// Aggregate tool usage counts: tool_name → invocation count.
    pub tool_usage: HashMap<String, usize>,
    /// Total number of errors across all turns.
    pub error_count: usize,
}

// ---------------------------------------------------------------------------
// TrajectoryCollector
// ---------------------------------------------------------------------------

/// Maximum number of completed trajectories kept in-memory.
const MAX_COMPLETED_TRAJECTORIES: usize = 100;

/// In-memory collector that accumulates [`Trajectory`] data for active
/// sessions and retains the most recent completed trajectories for
/// downstream analysis by [`super::reflector::SelfReflector`].
pub struct TrajectoryCollector {
    /// Active (in-flight) trajectories keyed by `session_id`.
    active_trajectories: Arc<RwLock<HashMap<String, Trajectory>>>,
    /// Ring buffer of recently completed trajectories (newest last).
    completed: Arc<RwLock<Vec<Trajectory>>>,
    /// A.3.2 — per-session bucket of in-flight `ToolCallRecord`s observed
    /// during a run. Producers call [`Self::observe_tool_call`] after each
    /// tool execution; the run finalize site calls
    /// [`Self::drain_pending_tool_calls`] once to populate
    /// `TurnRecord.tool_calls` before the trajectory is finished.
    pending_tool_calls: Arc<RwLock<HashMap<String, Vec<ToolCallRecord>>>>,
}

impl TrajectoryCollector {
    /// Create a new, empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_trajectories: Arc::new(RwLock::new(HashMap::new())),
            completed: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// A.3.2 — record one tool call observed during the active run for
    /// `session_id`. Accumulated tool calls are drained by
    /// [`Self::drain_pending_tool_calls`] when the run finalizes.
    /// Calls for unknown sessions are accepted (defensive — the producer
    /// site has no atomicity contract with `start_trajectory`).
    pub async fn observe_tool_call(&self, session_id: &str, record: ToolCallRecord) {
        let mut pending = self.pending_tool_calls.write().await;
        pending
            .entry(session_id.to_string())
            .or_default()
            .push(record);
    }

    /// A.3.2 — remove + return all pending tool calls accumulated for
    /// `session_id`. Called by the finalize path right before building
    /// `TurnRecord`. Returns an empty `Vec` if no calls were observed
    /// (e.g. text-only turn).
    pub async fn drain_pending_tool_calls(&self, session_id: &str) -> Vec<ToolCallRecord> {
        let mut pending = self.pending_tool_calls.write().await;
        pending.remove(session_id).unwrap_or_default()
    }

    /// Start a new trajectory for the given session.
    ///
    /// Returns the newly created `trajectory_id`. If a trajectory
    /// already exists for this `session_id` it is silently replaced
    /// (the old one is treated as abandoned).
    pub async fn start_trajectory(&self, session_id: &str, task_description: &str) -> String {
        let trajectory_id = Uuid::new_v4().to_string();
        let trajectory = Trajectory {
            trajectory_id: trajectory_id.clone(),
            session_id: session_id.to_string(),
            task_description: task_description.to_string(),
            turns: Vec::new(),
            outcome: None,
            started_at: Utc::now(),
            finished_at: None,
            duration_secs: None,
            tool_usage: HashMap::new(),
            error_count: 0,
        };
        let mut active = self.active_trajectories.write().await;
        active.insert(session_id.to_string(), trajectory);
        trajectory_id
    }

    /// Append a turn record to the active trajectory for `session_id`.
    ///
    /// If no active trajectory exists for the session the turn is
    /// silently dropped (defensive — avoids panics on race conditions).
    pub async fn record_turn(&self, session_id: &str, turn: TurnRecord) {
        let mut active = self.active_trajectories.write().await;
        if let Some(trajectory) = active.get_mut(session_id) {
            // Update aggregate tool-usage stats.
            for tc in &turn.tool_calls {
                *trajectory
                    .tool_usage
                    .entry(tc.tool_name.clone())
                    .or_insert(0) += 1;
            }
            // Update error count — only count tool-call-level failures
            // to avoid double-counting (turn.success is a roll-up of its
            // tool_calls, so counting it separately inflates the metric).
            for tc in &turn.tool_calls {
                if !tc.success {
                    trajectory.error_count += 1;
                }
            }
            trajectory.turns.push(turn);
        }
    }

    /// Finish the active trajectory for `session_id`, set its outcome,
    /// and move it to the completed ring buffer.
    ///
    /// Does nothing if no active trajectory exists for the session.
    pub async fn finish_trajectory(&self, session_id: &str, outcome: TaskOutcome) {
        let trajectory = {
            let mut active = self.active_trajectories.write().await;
            active.remove(session_id)
        };

        if let Some(mut traj) = trajectory {
            let now = Utc::now();
            traj.outcome = Some(outcome);
            traj.finished_at = Some(now);
            traj.duration_secs = Some(
                now.signed_duration_since(traj.started_at)
                    .num_seconds()
                    .unsigned_abs(),
            );

            let mut completed = self.completed.write().await;
            completed.push(traj);
            // Evict oldest entries beyond the cap.
            if completed.len() > MAX_COMPLETED_TRAJECTORIES {
                let excess = completed.len() - MAX_COMPLETED_TRAJECTORIES;
                completed.drain(..excess);
            }
        }
    }

    /// Return the `limit` most-recently completed trajectories (newest first).
    pub async fn recent_trajectories(&self, limit: usize) -> Vec<Trajectory> {
        let completed = self.completed.read().await;
        completed.iter().rev().take(limit).cloned().collect()
    }

    /// Return up to `limit` recently completed trajectories whose outcome
    /// is [`TaskOutcome::Failure`] or [`TaskOutcome::Abandoned`].
    pub async fn failed_trajectories(&self, limit: usize) -> Vec<Trajectory> {
        let completed = self.completed.read().await;
        completed
            .iter()
            .rev()
            .filter(|t| {
                matches!(
                    &t.outcome,
                    Some(TaskOutcome::Failure { .. } | TaskOutcome::Abandoned)
                )
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Aggregate tool-usage counts across all completed trajectories.
    pub async fn tool_usage_stats(&self) -> HashMap<String, usize> {
        let completed = self.completed.read().await;
        let mut stats: HashMap<String, usize> = HashMap::new();
        for traj in completed.iter() {
            for (tool, count) in &traj.tool_usage {
                *stats.entry(tool.clone()).or_insert(0) += count;
            }
        }
        stats
    }
}

impl Default for TrajectoryCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal turn record.
    fn make_turn(id: u32, success: bool, tool_calls: Vec<ToolCallRecord>) -> TurnRecord {
        TurnRecord {
            turn_id: id,
            timestamp: Utc::now(),
            user_input_summary: Some(format!("user input {id}")),
            agent_action: if tool_calls.is_empty() {
                AgentAction::Reply
            } else {
                AgentAction::ToolUse
            },
            tool_calls,
            success,
            self_assessment: None,
        }
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
                Some("fail".to_string())
            },
        }
    }

    #[tokio::test]
    async fn test_start_and_record_trajectory() {
        let collector = TrajectoryCollector::new();
        let tid = collector.start_trajectory("s1", "test task").await;
        assert!(!tid.is_empty());

        collector
            .record_turn(
                "s1",
                make_turn(1, true, vec![make_tool_call("read_file", true)]),
            )
            .await;
        collector
            .record_turn(
                "s1",
                make_turn(2, true, vec![make_tool_call("search", true)]),
            )
            .await;

        // Verify the active trajectory state.
        let active = collector.active_trajectories.read().await;
        let traj = active.get("s1").expect("trajectory should exist");
        assert_eq!(traj.turns.len(), 2);
        assert_eq!(traj.tool_usage.get("read_file"), Some(&1));
        assert_eq!(traj.tool_usage.get("search"), Some(&1));
        assert_eq!(traj.error_count, 0);
    }

    #[tokio::test]
    async fn test_finish_trajectory() {
        let collector = TrajectoryCollector::new();
        collector.start_trajectory("s1", "task A").await;
        collector
            .record_turn("s1", make_turn(1, true, vec![]))
            .await;
        collector
            .finish_trajectory(
                "s1",
                TaskOutcome::Success {
                    quality_score: 0.95,
                },
            )
            .await;

        // Active should be empty, completed should have one.
        let active = collector.active_trajectories.read().await;
        assert!(active.is_empty());
        let completed = collector.completed.read().await;
        assert_eq!(completed.len(), 1);
        assert!(completed[0].finished_at.is_some());
        assert!(completed[0].duration_secs.is_some());
    }

    #[tokio::test]
    async fn test_recent_trajectories_limit() {
        let collector = TrajectoryCollector::new();
        for i in 0..5 {
            let sid = format!("s{i}");
            collector.start_trajectory(&sid, &format!("task {i}")).await;
            collector
                .finish_trajectory(&sid, TaskOutcome::Success { quality_score: 0.8 })
                .await;
        }

        let recent = collector.recent_trajectories(3).await;
        assert_eq!(recent.len(), 3);
        // Newest first — session ids should be s4, s3, s2
        assert_eq!(recent[0].session_id, "s4");
        assert_eq!(recent[1].session_id, "s3");
        assert_eq!(recent[2].session_id, "s2");
    }

    #[tokio::test]
    async fn test_failed_trajectories_filter() {
        let collector = TrajectoryCollector::new();

        // One success
        collector.start_trajectory("s-ok", "good task").await;
        collector
            .finish_trajectory("s-ok", TaskOutcome::Success { quality_score: 1.0 })
            .await;

        // One failure
        collector.start_trajectory("s-fail", "bad task").await;
        collector
            .finish_trajectory(
                "s-fail",
                TaskOutcome::Failure {
                    error_category: "timeout".to_string(),
                    root_cause: "network".to_string(),
                },
            )
            .await;

        // One abandoned
        collector
            .start_trajectory("s-abandon", "abandoned task")
            .await;
        collector
            .finish_trajectory("s-abandon", TaskOutcome::Abandoned)
            .await;

        let failed = collector.failed_trajectories(10).await;
        assert_eq!(failed.len(), 2);
        // Should contain s-abandon and s-fail (newest first)
        let ids: Vec<&str> = failed.iter().map(|t| t.session_id.as_str()).collect();
        assert!(ids.contains(&"s-fail"));
        assert!(ids.contains(&"s-abandon"));
    }

    // ────────────────── A.3.2 pending-tool-call tests ──────────────────

    #[tokio::test]
    async fn observe_then_drain_returns_in_order() {
        let collector = TrajectoryCollector::new();
        collector
            .observe_tool_call("s1", make_tool_call("bash", true))
            .await;
        collector
            .observe_tool_call("s1", make_tool_call("file_read", false))
            .await;

        let drained = collector.drain_pending_tool_calls("s1").await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].tool_name, "bash");
        assert_eq!(drained[1].tool_name, "file_read");
        // After drain, second drain returns empty.
        let again = collector.drain_pending_tool_calls("s1").await;
        assert!(again.is_empty());
    }

    #[tokio::test]
    async fn drain_unknown_session_returns_empty() {
        let collector = TrajectoryCollector::new();
        let v = collector.drain_pending_tool_calls("never-observed").await;
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn observe_partitions_by_session() {
        let collector = TrajectoryCollector::new();
        collector
            .observe_tool_call("a", make_tool_call("bash", true))
            .await;
        collector
            .observe_tool_call("b", make_tool_call("file_read", true))
            .await;

        let a = collector.drain_pending_tool_calls("a").await;
        let b = collector.drain_pending_tool_calls("b").await;
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].tool_name, "bash");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].tool_name, "file_read");
    }
}
