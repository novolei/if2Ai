//! DW-001 scanner tick helpers — report loading + stage persistence.
//!
//! `load_scanner_tick_input` is an async helper that loads the N most
//! recent `HarnessRunReport`s from disk and derives `failure_rate` /
//! `sample_size` for the promotion state machine.
//!
//! `ScannerStageState` persists `PromotionStage` across scanner ticks
//! via `Arc<Mutex<ScannerStageState>>` inside `spawn_self_edit_scanner_interval`.

use crate::modules::harness::run_report::{HarnessRunReport, TaskOutcome};
use crate::modules::harness::HarnessReportStore;
use crate::modules::learning::self_edit::{PromotionStage, StageTransition};

/// Input snapshot fed into one scanner tick.
#[derive(Debug)]
pub struct ScannerTickInput {
    /// Most-recent reports (newest first).
    pub reports: Vec<HarnessRunReport>,
    /// `failed / total`; 0.0 when no reports.
    pub failure_rate: f32,
    /// `reports.len()`
    pub sample_size: usize,
}

/// Load the `max_reports` most recent reports from `store` and derive
/// rolling-window stats.  Returns all-zero on missing / unreadable store.
pub async fn load_scanner_tick_input(
    store: &HarnessReportStore,
    max_reports: usize,
) -> ScannerTickInput {
    let entries = match store.list().await {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(?err, "[scanner_state] failed to list reports; holding");
            return ScannerTickInput {
                reports: Vec::new(),
                failure_rate: 0.0,
                sample_size: 0,
            };
        }
    };

    let window: Vec<_> = entries.into_iter().take(max_reports).collect();
    let mut reports = Vec::with_capacity(window.len());
    for entry in &window {
        match store.load(&entry.run_id).await {
            Ok(Some(r)) => reports.push(r),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    run_id = %entry.run_id, ?err,
                    "[scanner_state] skipping corrupt report"
                );
            }
        }
    }

    let sample_size = reports.len();
    let failed = reports
        .iter()
        .filter(|r| r.task.outcome == TaskOutcome::Failed)
        .count();
    let failure_rate = if sample_size == 0 {
        0.0
    } else {
        failed as f32 / sample_size as f32
    };

    ScannerTickInput {
        reports,
        failure_rate,
        sample_size,
    }
}

/// Mutable promotion stage state preserved across scanner ticks.
#[derive(Debug, Clone)]
pub struct ScannerStageState {
    pub stage: PromotionStage,
}

impl Default for ScannerStageState {
    fn default() -> Self {
        Self {
            stage: PromotionStage::Shadow,
        }
    }
}

impl ScannerStageState {
    /// Apply the `StageTransition` returned by `next_stage()`.
    pub fn apply_transition(&mut self, t: StageTransition) {
        match t {
            StageTransition::Promote(s) | StageTransition::Demote(s) => self.stage = s,
            StageTransition::Hold => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_empty_store_returns_zero_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 0);
        assert!((input.failure_rate - 0.0).abs() < f32::EPSILON);
        assert!(input.reports.is_empty());
    }

    #[tokio::test]
    async fn load_reports_derives_correct_failure_rate() {
        let tmp = tempfile::tempdir().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        for (id, outcome) in [
            ("r1", TaskOutcome::Success),
            ("r2", TaskOutcome::Success),
            ("r3", TaskOutcome::Failed),
        ] {
            let mut r = HarnessRunReport::new_empty(id, chrono::Utc::now());
            r.task.outcome = outcome;
            store.save(&r).await.unwrap();
        }
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 3);
        assert!((input.failure_rate - 1.0_f32 / 3.0).abs() < 0.01);
    }

    #[test]
    fn stage_state_default_is_shadow() {
        let s = ScannerStageState::default();
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_applies_promote() {
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Promote(PromotionStage::Canary1Pct));
        assert_eq!(s.stage, PromotionStage::Canary1Pct);
    }

    #[test]
    fn stage_state_holds() {
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Hold);
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_demotes() {
        let mut s = ScannerStageState {
            stage: PromotionStage::Canary1Pct,
        };
        s.apply_transition(StageTransition::Demote(PromotionStage::Shadow));
        assert_eq!(s.stage, PromotionStage::Shadow);
    }
}
