//! Regression corpus contracts (Phase M4.7).
//!
//! Typed shape for the **input side** of the M4 harness regression
//! suite: a versioned collection of task definitions partitioned
//! into the 5 canonical corpus tiers.  Companion to
//! [`super::suite_report`] which aggregates the **output side**.
//!
//! Honest scope of this module:
//!
//! - Defines the **task / corpus types**.  Loading a corpus from
//!   YAML is supported; **executing** a task (driving the agent
//!   through it) is intentionally out of scope — that requires
//!   coupling to the chat completion engine + tool registry,
//!   which is M5 territory.
//! - Each task carries an optional `expected_outcome` /
//!   `expected_blockers` so M4.7 / M4.8 can compare actual run
//!   reports against the corpus authors' intent (NOT a grading
//!   rubric — graders own that).
//! - Tier alphabet is **closed**.  Adding a tier is a breaking
//!   contract bump.
//!
//! Out of scope:
//!
//! - Task execution / agent driver (M5).
//! - Per-task strategy selection (M5).
//! - Cross-corpus diff (M4.8 gate territory).
//! - Live YAML reloading.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use super::run_report::TaskOutcome;

/// Stable corpus contract version.  Bumping is breaking; suite
/// reports record this version so historical comparisons can
/// refuse mismatched corpora.
pub const HARNESS_CORPUS_VERSION: &str = "harness-corpus@m4.7";

/// Closed-set corpus tier alphabet.  Mirrors the M4 runbook §5.8
/// "必须建立分层" list verbatim.
///
/// Intent of each tier:
///
/// - `Smoke` —— minimum signal that the agent loop boots and
///   responds.  Run on every commit / PR.
/// - `CriticalPath` —— common production user journeys.  Failure
///   here is high signal even at low frequency.
/// - `MemorySensitive` —— exercises memory recall / write /
///   compaction edge cases.  Pairs with the M3 memory subsystem.
/// - `ToolRisk` —— uses high-risk tools (bash / file_edit /
///   browser) where permission compliance / boundary decisions
///   matter most.
/// - `ResumeRecovery` —— stream-error → resume cycles, mid-run
///   tool retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusTier {
    Smoke,
    CriticalPath,
    MemorySensitive,
    ToolRisk,
    ResumeRecovery,
}

impl CorpusTier {
    /// All tiers in declaration order.  Used by suite-summary
    /// rendering to ensure stable iteration regardless of which
    /// tasks the corpus actually contains.
    pub const ALL: [CorpusTier; 5] = [
        Self::Smoke,
        Self::CriticalPath,
        Self::MemorySensitive,
        Self::ToolRisk,
        Self::ResumeRecovery,
    ];

    /// Stable wire label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::CriticalPath => "critical_path",
            Self::MemorySensitive => "memory_sensitive",
            Self::ToolRisk => "tool_risk",
            Self::ResumeRecovery => "resume_recovery",
        }
    }
}

/// Per-task definition.  `prompt` is the raw user message the
/// task expects to be fed to `start_agent_stream`; the task
/// runner (M5) is responsible for actual execution.
//
// `Eq` intentionally omitted: `weight: f64` only implements
// `PartialEq`.  Compare with `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusTask {
    pub id: String,
    pub tier: CorpusTier,
    pub prompt: String,
    /// Optional human-readable description (e.g. why this task
    /// exists, what regression it guards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Expected coarse outcome when the corpus author hand-ran
    /// this task on the baseline.  `None` means "no fixed
    /// expectation" (exploratory / fuzzing task).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_outcome: Option<TaskOutcome>,
    /// Stable blocker codes the corpus author considers
    /// **acceptable** for this task (e.g. `tool_failure` is OK
    /// because the task intentionally exercises a flaky tool).
    /// Anything outside this list will count as a regression in
    /// suite-level grading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_blockers: Vec<String>,
    /// Per-task weight used by suite-level scoring.  Default 1.0.
    /// `Smoke` tasks usually weight 1.0; `CriticalPath` may weight
    /// higher.  M4.8 gate consumes this.
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// Free-form tags for grouping / filtering (e.g. `["browser",
    /// "macOS-only"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn default_weight() -> f64 {
    1.0
}

/// One regression corpus file.  Held by value so it can be
/// loaded once and shared via `Arc`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionCorpus {
    /// Stable corpus contract version (`HARNESS_CORPUS_VERSION`).
    pub corpus_version: String,
    /// Caller-supplied corpus name (e.g. `"if2ai-baseline-2026-04"`).
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Wall-clock time the corpus file was loaded.  Useful for
    /// suite reports that want to record corpus provenance.
    /// Reviewer W-1 fix: `serde(default)` so YAML files don't
    /// have to carry a stub `loadedAt` (which the loader
    /// overwrites anyway).
    #[serde(default = "Utc::now")]
    pub loaded_at: DateTime<Utc>,
    /// Optional source-of-truth path the corpus was loaded from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    /// All tasks in this corpus, grouped by tier under `tasks`.
    pub tasks: Vec<CorpusTask>,
}

impl RegressionCorpus {
    /// Construct an empty corpus.  Used by tests and by callers
    /// that programmatically build corpora.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            corpus_version: HARNESS_CORPUS_VERSION.to_string(),
            name: name.into(),
            description: None,
            loaded_at: Utc::now(),
            source_path: None,
            tasks: Vec::new(),
        }
    }

    /// Filter to tasks of a single tier.  Returns a borrow so
    /// callers don't pay the clone cost.
    pub fn tasks_in_tier(&self, tier: CorpusTier) -> impl Iterator<Item = &CorpusTask> {
        self.tasks.iter().filter(move |t| t.tier == tier)
    }

    /// Per-tier task counts.  Always returns counts for **every**
    /// tier (zero for empty tiers) so consumers iterate stably.
    #[must_use]
    pub fn tier_counts(&self) -> Vec<(CorpusTier, usize)> {
        CorpusTier::ALL
            .iter()
            .map(|tier| {
                let n = self.tasks.iter().filter(|t| t.tier == *tier).count();
                (*tier, n)
            })
            .collect()
    }

    /// Validate corpus invariants.  Currently:
    ///
    /// 1. Task ids are unique.
    /// 2. Every task carries a non-empty `prompt`.
    /// 3. Weights are finite and non-negative.
    pub fn validate(&self) -> Result<(), CorpusError> {
        use std::collections::HashSet;
        let mut seen: HashSet<&str> = HashSet::with_capacity(self.tasks.len());
        for t in &self.tasks {
            if !seen.insert(t.id.as_str()) {
                return Err(CorpusError::DuplicateTaskId(t.id.clone()));
            }
            if t.prompt.trim().is_empty() {
                return Err(CorpusError::EmptyPrompt(t.id.clone()));
            }
            if !t.weight.is_finite() || t.weight < 0.0 {
                return Err(CorpusError::InvalidWeight {
                    task_id: t.id.clone(),
                    weight: t.weight,
                });
            }
        }
        Ok(())
    }
}

/// Corpus error family.  YAML parse / IO errors propagate via
/// `From` impls so callers can `?` directly.
#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("duplicate task id: {0}")]
    DuplicateTaskId(String),
    #[error("task '{0}' has empty prompt")]
    EmptyPrompt(String),
    #[error("task '{task_id}' has invalid weight {weight}")]
    InvalidWeight { task_id: String, weight: f64 },
    #[error("corpus version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: String, got: String },
}

/// Load a corpus from YAML on disk.  Validates invariants before
/// returning.  Stamps `source_path` and `loaded_at` so the
/// corpus carries its provenance.
///
/// YAML schema mirrors `RegressionCorpus` (camelCase via serde).
/// Minimal example:
///
/// ```yaml
/// corpusVersion: harness-corpus@m4.7
/// name: if2ai-baseline-2026-04
/// description: smoke + critical-path baseline
/// loadedAt: "2026-04-20T00:00:00Z"
/// tasks:
///   - id: smoke-001
///     tier: smoke
///     prompt: "What's 2 + 2?"
///     expectedOutcome: success
///     weight: 1.0
/// ```
pub async fn load_corpus_from_yaml(
    path: impl AsRef<Path>,
) -> Result<RegressionCorpus, CorpusError> {
    let path = path.as_ref();
    let bytes = fs::read(path).await?;
    let mut corpus: RegressionCorpus = serde_yaml::from_slice(&bytes)?;
    // Reviewer Blocking-1 fix: enforce corpus_version at load
    // time.  Without this check, YAML files written against
    // older / future versions are silently accepted and produce
    // suite reports the M4.8 gate would mis-classify.
    if corpus.corpus_version != HARNESS_CORPUS_VERSION {
        return Err(CorpusError::VersionMismatch {
            expected: HARNESS_CORPUS_VERSION.to_string(),
            got: corpus.corpus_version.clone(),
        });
    }
    corpus.source_path = Some(path.to_path_buf());
    // Always overwrite loaded_at — file may carry an older
    // timestamp that doesn't reflect when this process loaded it.
    corpus.loaded_at = Utc::now();
    corpus.validate()?;
    Ok(corpus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn task(id: &str, tier: CorpusTier) -> CorpusTask {
        CorpusTask {
            id: id.to_string(),
            tier,
            prompt: format!("prompt for {id}"),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        }
    }

    #[test]
    fn empty_corpus_is_valid() {
        let c = RegressionCorpus::new("empty");
        assert!(c.validate().is_ok());
        for (_tier, n) in c.tier_counts() {
            assert_eq!(n, 0);
        }
    }

    #[test]
    fn tier_counts_cover_all_tiers_even_when_empty() {
        let mut c = RegressionCorpus::new("c");
        c.tasks.push(task("s1", CorpusTier::Smoke));
        c.tasks.push(task("s2", CorpusTier::Smoke));
        c.tasks.push(task("c1", CorpusTier::CriticalPath));
        let counts = c.tier_counts();
        assert_eq!(counts.len(), 5);
        let smoke = counts
            .iter()
            .find(|(t, _)| *t == CorpusTier::Smoke)
            .unwrap();
        assert_eq!(smoke.1, 2);
        let resume = counts
            .iter()
            .find(|(t, _)| *t == CorpusTier::ResumeRecovery)
            .unwrap();
        assert_eq!(resume.1, 0);
    }

    #[test]
    fn duplicate_task_id_fails_validate() {
        let mut c = RegressionCorpus::new("c");
        c.tasks.push(task("dup", CorpusTier::Smoke));
        c.tasks.push(task("dup", CorpusTier::ToolRisk));
        match c.validate() {
            Err(CorpusError::DuplicateTaskId(id)) => assert_eq!(id, "dup"),
            other => panic!("expected DuplicateTaskId, got {other:?}"),
        }
    }

    #[test]
    fn empty_prompt_fails_validate() {
        let mut c = RegressionCorpus::new("c");
        let mut t = task("x", CorpusTier::Smoke);
        t.prompt = "   ".to_string();
        c.tasks.push(t);
        assert!(matches!(c.validate(), Err(CorpusError::EmptyPrompt(_))));
    }

    #[test]
    fn negative_weight_fails_validate() {
        let mut c = RegressionCorpus::new("c");
        let mut t = task("x", CorpusTier::Smoke);
        t.weight = -1.0;
        c.tasks.push(t);
        assert!(matches!(
            c.validate(),
            Err(CorpusError::InvalidWeight { .. })
        ));
    }

    #[test]
    fn nan_weight_fails_validate() {
        let mut c = RegressionCorpus::new("c");
        let mut t = task("x", CorpusTier::Smoke);
        t.weight = f64::NAN;
        c.tasks.push(t);
        assert!(matches!(
            c.validate(),
            Err(CorpusError::InvalidWeight { .. })
        ));
    }

    #[test]
    fn tasks_in_tier_filters_correctly() {
        let mut c = RegressionCorpus::new("c");
        c.tasks.push(task("a", CorpusTier::Smoke));
        c.tasks.push(task("b", CorpusTier::ToolRisk));
        c.tasks.push(task("c", CorpusTier::Smoke));
        let smoke_ids: Vec<&str> = c
            .tasks_in_tier(CorpusTier::Smoke)
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(smoke_ids, vec!["a", "c"]);
    }

    #[tokio::test]
    async fn load_corpus_from_yaml_round_trips() {
        let yaml = r#"
corpusVersion: harness-corpus@m4.7
name: test-corpus
loadedAt: "2026-04-20T00:00:00Z"
tasks:
  - id: smoke-001
    tier: smoke
    prompt: "What's 2 + 2?"
    expectedOutcome: success
    weight: 1.0
  - id: tool-001
    tier: tool_risk
    prompt: "List files in the workdir"
    expectedBlockers: ["tool_failure"]
    weight: 2.5
    tags: ["bash", "filesystem"]
"#;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("corpus.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        let c = load_corpus_from_yaml(&path).await.unwrap();
        assert_eq!(c.name, "test-corpus");
        assert_eq!(c.tasks.len(), 2);
        assert_eq!(c.tasks[0].tier, CorpusTier::Smoke);
        assert_eq!(c.tasks[1].weight, 2.5);
        assert_eq!(
            c.tasks[1].expected_blockers,
            vec!["tool_failure".to_string()]
        );
        assert_eq!(c.source_path.as_deref(), Some(path.as_path()));
    }

    #[tokio::test]
    async fn load_corpus_rejects_version_mismatch() {
        // Reviewer Blocking-1 regression test.
        let yaml = r#"
corpusVersion: harness-corpus@m4.3-old
name: stale
loadedAt: "2026-04-20T00:00:00Z"
tasks:
  - id: x
    tier: smoke
    prompt: "p"
"#;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("stale.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        let err = load_corpus_from_yaml(&path).await.unwrap_err();
        assert!(matches!(err, CorpusError::VersionMismatch { .. }));
    }

    #[tokio::test]
    async fn load_corpus_omits_loaded_at_via_serde_default() {
        // Reviewer W-1 regression test.
        let yaml = r#"
corpusVersion: harness-corpus@m4.7
name: no-timestamp
tasks:
  - id: x
    tier: smoke
    prompt: "p"
"#;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("no_ts.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        let c = load_corpus_from_yaml(&path).await.unwrap();
        assert_eq!(c.name, "no-timestamp");
    }

    #[tokio::test]
    async fn load_corpus_rejects_duplicate_ids() {
        let yaml = r#"
corpusVersion: harness-corpus@m4.7
name: bad
loadedAt: "2026-04-20T00:00:00Z"
tasks:
  - id: dup
    tier: smoke
    prompt: "a"
  - id: dup
    tier: smoke
    prompt: "b"
"#;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        assert!(load_corpus_from_yaml(&path).await.is_err());
    }
}
