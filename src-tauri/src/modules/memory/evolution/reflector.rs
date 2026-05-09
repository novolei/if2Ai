//! SelfReflector — rule-based self-reflection engine that extracts
//! actionable [`Insight`]s from completed [`Trajectory`] data.
//!
//! The current implementation is entirely **rule-based** (no LLM calls).
//! A future Task 8 may add LLM-assisted analysis on top.

use super::trajectory::{TaskOutcome, Trajectory};
use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::{MemoryCategory, MemoryError, SharedMemoryProvider};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public data models
// ---------------------------------------------------------------------------

/// Classification of an extracted insight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InsightCategory {
    /// Actionable heuristic rule (e.g. "always check X before Y").
    HeuristicRule,
    /// Pattern that should be avoided.
    AntiPattern,
    /// Proven good practice worth repeating.
    BestPractice,
    /// Discovered user preference.
    UserPreference,
    /// Observation about tool usage effectiveness.
    ToolUsagePattern,
}

/// A single insight extracted from one or more trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    /// Unique identifier.
    pub id: String,
    /// Insight classification.
    pub category: InsightCategory,
    /// Human-readable description of the insight.
    pub content: String,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Contexts where this insight applies.
    pub applicable_contexts: Vec<String>,
    /// IDs of source trajectories that contributed to this insight.
    pub source_trajectory_ids: Vec<String>,
    /// When this insight was created.
    pub created_at: DateTime<Utc>,
}

/// Summary report produced by a full reflection cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionReport {
    /// Number of trajectories analyzed.
    pub trajectory_count: usize,
    /// Number of insights extracted.
    pub insights_extracted: usize,
    /// Number of cross-trajectory patterns found.
    pub patterns_found: usize,
    /// The insights themselves.
    pub insights: Vec<Insight>,
}

/// A recurring pattern observed across multiple trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Human-readable description.
    pub description: String,
    /// How many trajectories exhibited this pattern.
    pub frequency: usize,
    /// Contributing trajectory IDs.
    pub trajectory_ids: Vec<String>,
    /// Recommended action to take.
    pub suggested_action: String,
}

// ---------------------------------------------------------------------------
// SelfReflector
// ---------------------------------------------------------------------------

/// Rule-based self-reflection engine with optional LLM augmentation.
///
/// Analyzes completed trajectories to produce [`Insight`]s and [`Pattern`]s.
/// When an LLM is available, supplements rule-based insights with deeper
/// LLM analysis.  When unavailable, falls back to pure rule-based logic.
/// Insights are optionally persisted as `Reflection`-category memories
/// via the provided [`SharedMemoryProvider`].
pub struct SelfReflector {
    provider: SharedMemoryProvider,
    /// Minimum number of trajectories required before cross-trajectory
    /// analysis is considered meaningful. Default: `5`.
    min_trajectories_for_reflection: usize,
    /// Optional LLM for deeper trajectory analysis.
    llm: Option<Arc<dyn UtilityLlm>>,
}

impl SelfReflector {
    /// Create a new reflector backed by the given memory provider.
    #[must_use]
    pub fn new(provider: SharedMemoryProvider) -> Self {
        Self {
            provider,
            min_trajectories_for_reflection: 5,
            llm: None,
        }
    }

    /// Attach a [`UtilityLlm`] for LLM-assisted trajectory analysis.
    /// When set, `reflect_on_failure` supplements rule-based insights
    /// with deeper LLM analysis.  When `None`, only rule-based logic
    /// is used.
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn UtilityLlm>) -> Self {
        self.llm = Some(llm);
        self
    }

    // -- single-trajectory reflection ------------------------------------

    /// Extract improvement insights from a **failed** trajectory.
    ///
    /// Rules applied:
    /// 1. If error rate > 50 % → `AntiPattern` insight.
    /// 2. If any tool failed ≥ 3 times → `ToolUsagePattern` insight.
    /// 3. If duration > 2× the average turn count × 10 s baseline → `HeuristicRule`.
    pub fn reflect_on_failure(&self, trajectory: &Trajectory) -> Vec<Insight> {
        let mut insights = Vec::new();
        let total_turns = trajectory.turns.len();
        if total_turns == 0 {
            return insights;
        }

        let tid = &trajectory.trajectory_id;

        // Rule 1: high error rate
        let failed_turns = trajectory.turns.iter().filter(|t| !t.success).count();
        let error_rate = failed_turns as f64 / total_turns as f64;
        if error_rate > 0.5 {
            insights.push(Insight {
                id: Uuid::new_v4().to_string(),
                category: InsightCategory::AntiPattern,
                content: format!(
                    "High error rate ({:.0}%) detected in task '{}'. Consider breaking the task into smaller steps.",
                    error_rate * 100.0,
                    trajectory.task_description,
                ),
                confidence: (error_rate * 0.9).min(1.0),
                applicable_contexts: vec![trajectory.task_description.clone()],
                source_trajectory_ids: vec![tid.clone()],
                created_at: Utc::now(),
            });
        }

        // Rule 2: tool repeated failures
        let mut tool_fail_counts: HashMap<&str, usize> = HashMap::new();
        for turn in &trajectory.turns {
            for tc in &turn.tool_calls {
                if !tc.success {
                    *tool_fail_counts.entry(&tc.tool_name).or_insert(0) += 1;
                }
            }
        }
        for (tool, count) in &tool_fail_counts {
            if *count >= 3 {
                insights.push(Insight {
                    id: Uuid::new_v4().to_string(),
                    category: InsightCategory::ToolUsagePattern,
                    content: format!(
                        "Tool '{}' failed {} times in task '{}'. Verify arguments or consider alternative tools.",
                        tool,
                        count,
                        trajectory.task_description,
                    ),
                    confidence: 0.7,
                    applicable_contexts: vec![tool.to_string()],
                    source_trajectory_ids: vec![tid.clone()],
                    created_at: Utc::now(),
                });
            }
        }

        // Rule 3: excessive duration
        let baseline_secs = (total_turns as u64) * 10;
        if let Some(dur) = trajectory.duration_secs {
            if dur > baseline_secs * 2 {
                insights.push(Insight {
                    id: Uuid::new_v4().to_string(),
                    category: InsightCategory::HeuristicRule,
                    content: format!(
                        "Task '{}' took {}s, which is over 2× the expected {}s baseline. Consider optimizing the approach.",
                        trajectory.task_description,
                        dur,
                        baseline_secs,
                    ),
                    confidence: 0.6,
                    applicable_contexts: vec![trajectory.task_description.clone()],
                    source_trajectory_ids: vec![tid.clone()],
                    created_at: Utc::now(),
                });
            }
        }

        insights
    }

    /// Supplement rule-based failure insights with LLM deep analysis.
    ///
    /// Sends a formatted trajectory summary to the LLM and parses the
    /// response as a JSON array of insights. The results are merged with
    /// the rule-based insights (deduped by content similarity). On any
    /// failure, logs a warning and returns the original insights unchanged.
    async fn augment_failure_insights_with_llm(
        &self,
        trajectory: &Trajectory,
        mut rule_insights: Vec<Insight>,
    ) -> Vec<Insight> {
        let llm = match &self.llm {
            Some(l) => l,
            None => return rule_insights,
        };

        let summary = format_trajectory_summary(trajectory);
        let prompt = format!(
            "分析以下 Agent 任务执行轨迹的失败原因，提取可转移的改进规则。\n\n\
             轨迹摘要:\n{}\n\n\
             请以 JSON 数组格式返回洞察，每个洞察包含:\n\
             - category: HeuristicRule|AntiPattern|BestPractice\n\
             - content: 规则内容\n\
             - confidence: 0.0-1.0\n\n\
             示例: [{{\"category\":\"AntiPattern\",\"content\":\"规则内容\",\"confidence\":0.8}}]",
            summary
        );

        let response = match llm
            .complete(
                "你是一个 Agent 执行轨迹分析助手。只返回 JSON 数组，不要返回其他内容。",
                &prompt,
                1024,
                0.4,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "LLM failure analysis call failed, using rule-based only");
                return rule_insights;
            }
        };

        // Parse LLM response as JSON array of insights
        let trimmed = response.trim();
        let json_str = extract_json_array(trimmed);

        let llm_insights: Vec<LlmInsightPayload> = match serde_json::from_str(json_str) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    response_preview = &trimmed[..trimmed.len().min(200)],
                    "LLM insight response parse failed, using rule-based only"
                );
                return rule_insights;
            }
        };

        // Merge LLM insights with rule-based, deduplicating by content
        let existing_contents: std::collections::HashSet<String> = rule_insights
            .iter()
            .map(|i| i.content.to_lowercase())
            .collect();

        for payload in llm_insights {
            if existing_contents.contains(&payload.content.to_lowercase()) {
                continue;
            }
            let category = match payload.category.as_str() {
                "HeuristicRule" => InsightCategory::HeuristicRule,
                "AntiPattern" => InsightCategory::AntiPattern,
                "BestPractice" => InsightCategory::BestPractice,
                _ => InsightCategory::HeuristicRule,
            };
            rule_insights.push(Insight {
                id: Uuid::new_v4().to_string(),
                category,
                content: payload.content,
                confidence: payload.confidence.clamp(0.0, 1.0),
                applicable_contexts: vec![trajectory.task_description.clone()],
                source_trajectory_ids: vec![trajectory.trajectory_id.clone()],
                created_at: Utc::now(),
            });
        }

        rule_insights
    }

    /// Extract best-practice insights from a **successful** trajectory.
    ///
    /// Rules applied:
    /// 1. Identify the most frequently used tools → `BestPractice`.
    /// 2. If zero errors and > 1 turn → `BestPractice` (clean execution).
    pub fn reflect_on_success(&self, trajectory: &Trajectory) -> Vec<Insight> {
        let mut insights = Vec::new();
        let tid = &trajectory.trajectory_id;

        // Rule 1: frequent tool combo
        if !trajectory.tool_usage.is_empty() {
            let mut sorted_tools: Vec<(&String, &usize)> = trajectory.tool_usage.iter().collect();
            sorted_tools.sort_by(|a, b| b.1.cmp(a.1));
            let top_tools: Vec<String> = sorted_tools
                .iter()
                .take(3)
                .map(|(name, _)| (*name).clone())
                .collect();
            insights.push(Insight {
                id: Uuid::new_v4().to_string(),
                category: InsightCategory::BestPractice,
                content: format!(
                    "Effective tool combination for '{}': [{}].",
                    trajectory.task_description,
                    top_tools.join(", "),
                ),
                confidence: 0.65,
                applicable_contexts: vec![trajectory.task_description.clone()],
                source_trajectory_ids: vec![tid.clone()],
                created_at: Utc::now(),
            });
        }

        // Rule 2: clean execution.
        //
        // A.3.2-fix-2: also fire on single-turn trajectories that exercised
        // multiple successful tool calls. Wave A.3 producer creates exactly
        // one TurnRecord per run (Q1=B brainstorm decision), so the original
        // `turns.len() > 1` gate was structurally unreachable. The
        // multi-tool variant captures the same signal — "agent completed a
        // non-trivial task without errors" — at the same 0.8 confidence.
        let successful_tool_calls: usize = trajectory
            .turns
            .iter()
            .flat_map(|t| t.tool_calls.iter())
            .filter(|tc| tc.success)
            .count();
        let multi_step_clean = trajectory.error_count == 0
            && (trajectory.turns.len() > 1 || successful_tool_calls >= 2);
        if multi_step_clean {
            let qualifier = if trajectory.turns.len() > 1 {
                format!("{} turns", trajectory.turns.len())
            } else {
                format!("{successful_tool_calls} successful tool calls")
            };
            insights.push(Insight {
                id: Uuid::new_v4().to_string(),
                category: InsightCategory::BestPractice,
                content: format!(
                    "Clean execution of '{}' with {} and zero errors — replicate this approach.",
                    trajectory.task_description, qualifier,
                ),
                confidence: 0.8,
                applicable_contexts: vec![trajectory.task_description.clone()],
                source_trajectory_ids: vec![tid.clone()],
                created_at: Utc::now(),
            });
        }

        insights
    }

    // -- cross-trajectory analysis ----------------------------------------

    /// Analyze multiple trajectories to discover recurring patterns.
    ///
    /// Patterns detected:
    /// 1. Repeated failure causes across trajectories.
    /// 2. Tools that succeed across most trajectories (reliable tools).
    /// 3. Common tool-usage trends.
    pub fn cross_trajectory_analysis(&self, trajectories: &[Trajectory]) -> Vec<Pattern> {
        let mut patterns = Vec::new();

        if trajectories.len() < self.min_trajectories_for_reflection {
            return patterns;
        }

        // Pattern 1: repeated failure categories
        let mut failure_causes: HashMap<String, Vec<String>> = HashMap::new();
        for traj in trajectories {
            if let Some(TaskOutcome::Failure { error_category, .. }) = &traj.outcome {
                failure_causes
                    .entry(error_category.clone())
                    .or_default()
                    .push(traj.trajectory_id.clone());
            }
        }
        for (cause, tids) in &failure_causes {
            if tids.len() >= 2 {
                patterns.push(Pattern {
                    description: format!(
                        "Recurring failure cause '{}' observed in {} trajectories.",
                        cause,
                        tids.len(),
                    ),
                    frequency: tids.len(),
                    trajectory_ids: tids.clone(),
                    suggested_action: format!(
                        "Investigate and mitigate the '{}' failure category proactively.",
                        cause,
                    ),
                });
            }
        }

        // Pattern 2: tool reliability — tools that appear in most trajectories and rarely fail
        let mut tool_success: HashMap<String, usize> = HashMap::new();
        let mut tool_total: HashMap<String, usize> = HashMap::new();
        let mut tool_trajs: HashMap<String, Vec<String>> = HashMap::new();
        for traj in trajectories {
            for turn in &traj.turns {
                for tc in &turn.tool_calls {
                    *tool_total.entry(tc.tool_name.clone()).or_insert(0) += 1;
                    if tc.success {
                        *tool_success.entry(tc.tool_name.clone()).or_insert(0) += 1;
                    }
                    tool_trajs
                        .entry(tc.tool_name.clone())
                        .or_default()
                        .push(traj.trajectory_id.clone());
                }
            }
        }

        for (tool, total) in &tool_total {
            let success = tool_success.get(tool).copied().unwrap_or(0);
            if *total >= 5 && (success as f64 / *total as f64) > 0.9 {
                let mut tids = tool_trajs.get(tool).cloned().unwrap_or_default();
                tids.sort();
                tids.dedup();
                patterns.push(Pattern {
                    description: format!(
                        "Tool '{}' is highly reliable ({}/{} successes).",
                        tool, success, total,
                    ),
                    frequency: *total,
                    trajectory_ids: tids,
                    suggested_action: format!(
                        "Prefer '{}' when multiple tool options are available.",
                        tool,
                    ),
                });
            }
        }

        // Pattern 3: tools with high failure rate across trajectories
        for (tool, total) in &tool_total {
            let success = tool_success.get(tool).copied().unwrap_or(0);
            if *total >= 3 && (success as f64 / *total as f64) < 0.5 {
                let mut tids = tool_trajs.get(tool).cloned().unwrap_or_default();
                tids.sort();
                tids.dedup();
                patterns.push(Pattern {
                    description: format!(
                        "Tool '{}' has low success rate ({}/{}).",
                        tool, success, total,
                    ),
                    frequency: *total,
                    trajectory_ids: tids,
                    suggested_action: format!(
                        "Review usage of '{}' — consider fixing arguments or using alternatives.",
                        tool,
                    ),
                });
            }
        }

        patterns
    }

    // -- full cycle -------------------------------------------------------

    /// Run a complete reflection cycle:
    ///
    /// 1. Fetch recent trajectories from the collector.
    /// 2. Reflect on each failed trajectory.
    /// 3. Reflect on each successful trajectory.
    /// 4. Perform cross-trajectory pattern analysis.
    /// 5. Persist extracted insights as `Reflection`-category memories.
    ///
    /// Returns a [`ReflectionReport`] summarizing the cycle.
    pub async fn run_reflection_cycle(
        &self,
        collector: &super::trajectory::TrajectoryCollector,
    ) -> Result<ReflectionReport, MemoryError> {
        let trajectories = collector.recent_trajectories(50).await;
        if trajectories.is_empty() {
            return Ok(ReflectionReport {
                trajectory_count: 0,
                insights_extracted: 0,
                patterns_found: 0,
                insights: Vec::new(),
            });
        }

        let mut all_insights: Vec<Insight> = Vec::new();

        // Reflect per trajectory
        for traj in &trajectories {
            match &traj.outcome {
                Some(TaskOutcome::Failure { .. } | TaskOutcome::Abandoned) => {
                    let rule_insights = self.reflect_on_failure(traj);
                    let enriched = self
                        .augment_failure_insights_with_llm(traj, rule_insights)
                        .await;
                    all_insights.extend(enriched);
                }
                Some(TaskOutcome::Success { .. }) => {
                    all_insights.extend(self.reflect_on_success(traj));
                }
                Some(TaskOutcome::PartialSuccess { completed_ratio }) => {
                    if *completed_ratio < 0.5 {
                        let rule_insights = self.reflect_on_failure(traj);
                        let enriched = self
                            .augment_failure_insights_with_llm(traj, rule_insights)
                            .await;
                        all_insights.extend(enriched);
                    } else {
                        all_insights.extend(self.reflect_on_success(traj));
                    }
                }
                None => {}
            }
        }

        // Cross-trajectory patterns
        let patterns = self.cross_trajectory_analysis(&trajectories);

        // Persist insights as Reflection-category memories.
        for insight in &all_insights {
            let key = format!("reflection-insight-{}", insight.id);
            let content = serde_json::to_string(insight).unwrap_or_default();
            if let Err(e) = self
                .provider
                .store(&key, &content, MemoryCategory::Reflection)
                .await
            {
                tracing::warn!("[evolution] Failed to persist insight {}: {e}", insight.id);
            }
        }

        Ok(ReflectionReport {
            trajectory_count: trajectories.len(),
            insights_extracted: all_insights.len(),
            patterns_found: patterns.len(),
            insights: all_insights,
        })
    }
}

/// Deserialisation target for LLM-produced insight JSON payloads.
#[derive(Debug, Deserialize)]
struct LlmInsightPayload {
    category: String,
    content: String,
    confidence: f64,
}

/// Format a trajectory into a concise LLM-friendly text summary.
///
/// The output is capped at ~2000 characters to avoid excessive token usage.
/// Includes task description, outcome, turn count, error count, and a
/// per-turn breakdown of tool calls and success status.
pub fn format_trajectory_summary(trajectory: &Trajectory) -> String {
    let mut buf = String::with_capacity(2000);

    buf.push_str(&format!("Task: {}\n", trajectory.task_description));
    buf.push_str(&format!(
        "Turns: {}, Errors: {}\n",
        trajectory.turns.len(),
        trajectory.error_count
    ));
    if let Some(dur) = trajectory.duration_secs {
        buf.push_str(&format!("Duration: {dur}s\n"));
    }
    if let Some(ref outcome) = trajectory.outcome {
        let outcome_str = match outcome {
            TaskOutcome::Success { quality_score } => {
                format!("Success (quality={quality_score:.2})")
            }
            TaskOutcome::PartialSuccess { completed_ratio } => {
                format!("Partial ({completed_ratio:.0}%)")
            }
            TaskOutcome::Failure {
                error_category,
                root_cause,
            } => {
                format!("Failure [{error_category}]: {root_cause}")
            }
            TaskOutcome::Abandoned => "Abandoned".to_string(),
        };
        buf.push_str(&format!("Outcome: {outcome_str}\n"));
    }

    buf.push_str("\nTurn details:\n");
    for turn in &trajectory.turns {
        if buf.len() > 1800 {
            buf.push_str("... (truncated)\n");
            break;
        }
        let status = if turn.success { "OK" } else { "FAIL" };
        let tools: Vec<String> = turn
            .tool_calls
            .iter()
            .map(|tc| {
                let ts = if tc.success { "ok" } else { "err" };
                format!("{}({ts})", tc.tool_name)
            })
            .collect();
        buf.push_str(&format!(
            "  T{}: [{status}] {}\n",
            turn.turn_id,
            tools.join(", ")
        ));
    }

    // Ensure we don't exceed ~2000 chars
    if buf.len() > 2000 {
        buf.truncate(2000);
        buf.push_str("...");
    }
    buf
}

/// Try to extract a JSON array from a response that may contain markdown
/// code fences or other wrapper text.
fn extract_json_array(s: &str) -> &str {
    if let Some(start) = s.find('[') {
        if let Some(end) = s.rfind(']') {
            if end >= start {
                return &s[start..=end];
            }
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::evolution::trajectory::*;
    use std::sync::Arc;

    /// Build a minimal in-memory provider for tests.
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

    fn make_trajectory(
        tid: &str,
        sid: &str,
        turns: Vec<TurnRecord>,
        outcome: TaskOutcome,
    ) -> Trajectory {
        let error_count = turns.iter().filter(|t| !t.success).count();
        let mut tool_usage = HashMap::new();
        for turn in &turns {
            for tc in &turn.tool_calls {
                *tool_usage.entry(tc.tool_name.clone()).or_insert(0) += 1;
            }
        }
        Trajectory {
            trajectory_id: tid.to_string(),
            session_id: sid.to_string(),
            task_description: format!("task-{sid}"),
            turns,
            outcome: Some(outcome),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            duration_secs: Some(120),
            tool_usage,
            error_count,
        }
    }

    #[test]
    fn test_reflect_on_failure_high_error_rate() {
        let reflector = SelfReflector::new(test_provider());
        let turns = vec![
            make_turn(1, false, vec![make_tool_call("run_cmd", false)]),
            make_turn(2, false, vec![make_tool_call("run_cmd", false)]),
            make_turn(3, false, vec![make_tool_call("run_cmd", false)]),
            make_turn(4, true, vec![make_tool_call("read_file", true)]),
        ];
        let traj = make_trajectory(
            "t1",
            "s1",
            turns,
            TaskOutcome::Failure {
                error_category: "tool_error".to_string(),
                root_cause: "bad args".to_string(),
            },
        );

        let insights = reflector.reflect_on_failure(&traj);
        // Should have AntiPattern (75% error rate) + ToolUsagePattern (run_cmd failed 3x)
        let categories: Vec<_> = insights.iter().map(|i| &i.category).collect();
        assert!(
            categories.contains(&&InsightCategory::AntiPattern),
            "expected AntiPattern insight"
        );
        assert!(
            categories.contains(&&InsightCategory::ToolUsagePattern),
            "expected ToolUsagePattern insight"
        );
    }

    #[test]
    fn test_reflect_on_success_efficient_tools() {
        let reflector = SelfReflector::new(test_provider());
        let turns = vec![
            make_turn(1, true, vec![make_tool_call("search", true)]),
            make_turn(
                2,
                true,
                vec![
                    make_tool_call("read_file", true),
                    make_tool_call("search", true),
                ],
            ),
            make_turn(3, true, vec![make_tool_call("edit_file", true)]),
        ];
        let traj = make_trajectory(
            "t2",
            "s2",
            turns,
            TaskOutcome::Success { quality_score: 0.9 },
        );

        let insights = reflector.reflect_on_success(&traj);
        assert!(
            !insights.is_empty(),
            "should extract at least one best-practice insight"
        );
        assert!(insights
            .iter()
            .all(|i| i.category == InsightCategory::BestPractice));
    }

    /// A.3.2-fix-2: Wave A.3 producer creates single-turn trajectories.
    /// Rule 2's clean-execution insight must fire on a single-turn run
    /// that exercised at least 2 successful tool calls (otherwise the
    /// 0.8-confidence path is unreachable for streaming chat traffic).
    #[test]
    fn reflect_on_success_fires_on_single_turn_multi_tool_clean_run() {
        let reflector = SelfReflector::new(test_provider());
        let turns = vec![make_turn(
            1,
            true,
            vec![
                make_tool_call("search", true),
                make_tool_call("read_file", true),
            ],
        )];
        let traj = make_trajectory(
            "t-single",
            "s-single",
            turns,
            TaskOutcome::Success { quality_score: 1.0 },
        );

        let insights = reflector.reflect_on_success(&traj);
        let clean = insights
            .iter()
            .find(|i| i.content.contains("Clean execution"));
        assert!(
            clean.is_some(),
            "single-turn multi-tool clean run must produce a clean-execution insight"
        );
        let clean = clean.unwrap();
        assert!(
            clean.confidence >= 0.7,
            "clean-execution confidence must clear the 0.7 promotion floor (got {})",
            clean.confidence
        );
        assert_eq!(clean.category, InsightCategory::BestPractice);
        assert!(
            clean.content.contains("2 successful tool calls"),
            "qualifier should mention tool count: {}",
            clean.content
        );
    }

    /// A.3.2-fix-2: a single-turn run with only 1 tool call (the most
    /// common shape for trivial chats) should NOT trigger the clean-
    /// execution insight — otherwise procedural memory fills with
    /// noise like "Clean execution of 'what's 2+2'".
    #[test]
    fn reflect_on_success_skips_single_turn_single_tool_run() {
        let reflector = SelfReflector::new(test_provider());
        let turns = vec![make_turn(1, true, vec![make_tool_call("search", true)])];
        let traj = make_trajectory(
            "t-trivial",
            "s-trivial",
            turns,
            TaskOutcome::Success { quality_score: 1.0 },
        );

        let insights = reflector.reflect_on_success(&traj);
        assert!(
            insights
                .iter()
                .all(|i| !i.content.contains("Clean execution")),
            "trivial 1-tool run must not produce clean-execution insight"
        );
    }

    #[test]
    fn test_cross_trajectory_analysis() {
        let reflector = SelfReflector {
            provider: test_provider(),
            min_trajectories_for_reflection: 2, // lower for test
            llm: None,
        };

        let mut trajectories = Vec::new();
        // 3 failures with same cause
        for i in 0..3 {
            let turns = vec![make_turn(1, false, vec![make_tool_call("bad_tool", false)])];
            trajectories.push(make_trajectory(
                &format!("tf{i}"),
                &format!("sf{i}"),
                turns,
                TaskOutcome::Failure {
                    error_category: "timeout".to_string(),
                    root_cause: "slow network".to_string(),
                },
            ));
        }
        // 2 successes with reliable tool
        for i in 0..2 {
            let turns = vec![
                make_turn(1, true, vec![make_tool_call("good_tool", true)]),
                make_turn(2, true, vec![make_tool_call("good_tool", true)]),
                make_turn(3, true, vec![make_tool_call("good_tool", true)]),
            ];
            trajectories.push(make_trajectory(
                &format!("ts{i}"),
                &format!("ss{i}"),
                turns,
                TaskOutcome::Success {
                    quality_score: 0.95,
                },
            ));
        }

        let patterns = reflector.cross_trajectory_analysis(&trajectories);
        assert!(
            !patterns.is_empty(),
            "should find at least one cross-trajectory pattern"
        );
        // Should have the recurring "timeout" failure pattern
        assert!(
            patterns.iter().any(|p| p.description.contains("timeout")),
            "expected recurring failure pattern for 'timeout'"
        );
    }

    #[tokio::test]
    async fn test_run_reflection_cycle() {
        let provider = test_provider();
        let reflector = SelfReflector {
            provider: Arc::clone(&provider),
            min_trajectories_for_reflection: 1,
            llm: None,
        };
        let collector = TrajectoryCollector::new();

        // Add a failure trajectory
        collector
            .start_trajectory("cyc1", "reflection cycle test")
            .await;
        collector
            .record_turn(
                "cyc1",
                make_turn(1, false, vec![make_tool_call("x", false)]),
            )
            .await;
        collector
            .record_turn(
                "cyc1",
                make_turn(2, false, vec![make_tool_call("x", false)]),
            )
            .await;
        collector
            .finish_trajectory(
                "cyc1",
                TaskOutcome::Failure {
                    error_category: "tool_error".to_string(),
                    root_cause: "unknown".to_string(),
                },
            )
            .await;

        // Add a success trajectory
        collector.start_trajectory("cyc2", "good task").await;
        collector
            .record_turn("cyc2", make_turn(1, true, vec![make_tool_call("a", true)]))
            .await;
        collector
            .record_turn("cyc2", make_turn(2, true, vec![make_tool_call("b", true)]))
            .await;
        collector
            .finish_trajectory("cyc2", TaskOutcome::Success { quality_score: 0.9 })
            .await;

        let report = reflector
            .run_reflection_cycle(&collector)
            .await
            .expect("reflection cycle should succeed");

        assert_eq!(report.trajectory_count, 2);
        assert!(
            report.insights_extracted > 0,
            "should extract at least one insight"
        );
    }
}
