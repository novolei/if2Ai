//! Failure clustering pipeline (Phase M5 m5.2, closeout).
//!
//! Pure aggregation over one or more [`HarnessRunReport`]s into
//! typed [`FailureCluster`] groups keyed by [`FailureCategory`].
//! Consumers (reflection generation, candidate evaluator,
//! diagnostics surface) operate on the cluster set rather than
//! re-walking raw `BlockingFailure` lists.
//!
//! Honest scope:
//!
//! - **Pure compute** — no IO, no model calls.
//! - **Closed taxonomy** via [`FailureCategory`] (M5 closeout).
//! - **Stable signature** — [`cluster_failures`] takes a slice
//!   of reports so suite-level callers can pass a mixed batch
//!   without re-bundling.

#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::failure_taxonomy::{classify_blocking_failure, FailureCategory};
use crate::modules::harness::{HarnessRunReport, Severity};

/// Stable contract version for [`ClusteredFailureSet`].
pub const FAILURE_CLUSTERING_VERSION: &str = "failure-clustering@m5.2";

/// One representative failure inside a [`FailureCluster`].
/// Keeps just enough context for reflection consumers + a
/// future diagnostics surface — full failure detail lives on
/// the report itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureSignature {
    /// Stable failure code from the originating
    /// `BlockingFailure.code`.
    pub code: String,
    /// Highest severity observed for this code in this
    /// cluster.
    pub max_severity: Severity,
    /// `run_id` of one report carrying the failure (for traceability).
    pub run_id: String,
    /// Number of times this exact code appeared in the cluster
    /// across all input reports.
    pub occurrences: usize,
}

/// Cluster of failures sharing the same [`FailureCategory`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureCluster {
    pub category: FailureCategory,
    /// Representative failure signatures (one entry per unique
    /// `code`).  Sorted by `occurrences` desc.
    pub signatures: Vec<FailureSignature>,
    /// Total failures rolled into this cluster.
    pub total_failures: usize,
    /// Maximum severity across the cluster.
    pub max_severity: Severity,
}

/// Top-level cluster set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusteredFailureSet {
    pub clustering_version: String,
    pub clusters: Vec<FailureCluster>,
    pub input_run_ids: Vec<String>,
    pub total_failures: usize,
}

impl ClusteredFailureSet {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            clustering_version: FAILURE_CLUSTERING_VERSION.to_string(),
            clusters: Vec::new(),
            input_run_ids: Vec::new(),
            total_failures: 0,
        }
    }
}

/// Cluster every `BlockingFailure` across the supplied reports.
/// Failures sharing the same [`FailureCategory`] roll into one
/// [`FailureCluster`]; within a cluster, identical
/// `BlockingFailure.code` values collapse to a single
/// [`FailureSignature`] with `occurrences > 1`.
#[must_use]
pub fn cluster_failures(reports: &[&HarnessRunReport]) -> ClusteredFailureSet {
    let mut bucket: BTreeMap<FailureCategory, BTreeMap<String, FailureSignature>> = BTreeMap::new();
    let mut total = 0usize;
    let mut input_ids: Vec<String> = Vec::with_capacity(reports.len());
    for report in reports {
        input_ids.push(report.run_id.clone());
        for f in &report.blocking_failures {
            total += 1;
            let category = classify_blocking_failure(f);
            let by_code = bucket.entry(category).or_default();
            let entry = by_code
                .entry(f.code.clone())
                .or_insert_with(|| FailureSignature {
                    code: f.code.clone(),
                    max_severity: f.severity,
                    run_id: report.run_id.clone(),
                    occurrences: 0,
                });
            entry.occurrences += 1;
            if severity_rank(f.severity) > severity_rank(entry.max_severity) {
                entry.max_severity = f.severity;
            }
        }
    }
    let mut clusters: Vec<FailureCluster> = bucket
        .into_iter()
        .map(|(category, sigs_map)| {
            let mut signatures: Vec<FailureSignature> = sigs_map.into_values().collect();
            signatures.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
            let total_failures = signatures.iter().map(|s| s.occurrences).sum();
            let max_severity = signatures
                .iter()
                .map(|s| s.max_severity)
                .max_by_key(|s| severity_rank(*s))
                .unwrap_or(Severity::Info);
            FailureCluster {
                category,
                signatures,
                total_failures,
                max_severity,
            }
        })
        .collect();
    // Stable order: highest severity first, then category label.
    clusters.sort_by(|a, b| {
        severity_rank(b.max_severity)
            .cmp(&severity_rank(a.max_severity))
            .then_with(|| a.category.label().cmp(b.category.label()))
    });
    ClusteredFailureSet {
        clustering_version: FAILURE_CLUSTERING_VERSION.to_string(),
        clusters,
        input_run_ids: input_ids,
        total_failures: total,
    }
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Blocking => 4,
        Severity::Warning => 3,
        Severity::Info => 2,
        Severity::Pass => 1,
        Severity::Skeleton => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::{
        AggregateMetrics, BlockingFailure, EvidenceBundle, HarnessRunReport, TaskOutcome,
        TaskRunResult, HARNESS_RUN_REPORT_VERSION,
    };
    use chrono::Utc;

    fn report(run_id: &str, failures: Vec<BlockingFailure>) -> HarnessRunReport {
        let now = Utc::now();
        HarnessRunReport {
            report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
            run_id: run_id.into(),
            label: None,
            session_id: None,
            project_id: None,
            started_at: now,
            ended_at: now,
            task: TaskRunResult {
                task_id: "t".into(),
                outcome: TaskOutcome::Failed,
                turn_count: 1,
                last_turn_succeeded: false,
                error_summary: None,
            },
            aggregate: AggregateMetrics::default(),
            blocking_failures: failures,
            evidence: EvidenceBundle::default(),
        }
    }

    fn fail(code: &str, sev: Severity) -> BlockingFailure {
        BlockingFailure {
            code: code.into(),
            severity: sev,
            message: String::new(),
            evidence_ref: None,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn clusters_by_category_with_occurrence_counts() {
        let r1 = report(
            "r1",
            vec![
                fail("memory_rejected", Severity::Warning),
                fail("memory_rejected", Severity::Blocking),
                fail("tool_failure", Severity::Warning),
            ],
        );
        let r2 = report("r2", vec![fail("memory_scope_violation", Severity::Blocking)]);
        let set = cluster_failures(&[&r1, &r2]);
        assert_eq!(set.total_failures, 4);
        let mem = set
            .clusters
            .iter()
            .find(|c| c.category == FailureCategory::MemoryIssue)
            .unwrap();
        assert_eq!(mem.total_failures, 3);
        // Severity rolls up to Blocking (the worst occurrence).
        assert_eq!(mem.max_severity, Severity::Blocking);
        // Highest-occurring signature first.
        assert_eq!(mem.signatures[0].code, "memory_rejected");
        assert_eq!(mem.signatures[0].occurrences, 2);
    }

    #[test]
    fn empty_input_returns_empty_set() {
        let set = cluster_failures(&[]);
        assert_eq!(set.total_failures, 0);
        assert!(set.clusters.is_empty());
    }
}
