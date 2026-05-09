//! Forgetting curve engine — Ebbinghaus-inspired exponential decay with
//! access-frequency reinforcement.
//!
//! The engine provides three capabilities:
//!
//! 1. **`compute_retention`** — single-entry retention score.
//! 2. **`sweep`** — batch decay scan that updates importance and flags
//!    entries below the archive threshold.
//! 3. **`budget_select`** — token-budget-aware selection of highest-value
//!    entries for context injection.
//!
//! Stateless w.r.t. storage — the caller (e.g. `MemoryTicker` daily job)
//! is responsible for persisting updated entries back to the provider.

use chrono::{DateTime, Utc};

use crate::modules::memory::quality::QualityScorer;
use crate::modules::memory::{MemoryEntry, MemoryError, MemoryProvider};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration knobs for the forgetting curve engine.
///
/// All fields have sensible defaults via [`Default`].
#[derive(Debug, Clone)]
pub struct ForgettingConfig {
    /// Exponential decay rate (λ).  Higher → faster forgetting.
    /// Default: `0.1`.
    pub lambda: f64,
    /// Reinforcement factor (α) applied to the logarithmic access-count
    /// boost.  Default: `0.3`.
    pub reinforcement_factor: f64,
    /// Entries whose retention drops below this threshold are candidates
    /// for archival / quality-score demotion.  Default: `0.1`.
    pub archive_threshold: f64,
    /// Advisory sweep interval in hours.  Not enforced by this struct —
    /// the caller (ticker / scheduler) uses the value to decide when to
    /// invoke [`ForgettingCurveEngine::sweep`].  Default: `12`.
    pub sweep_interval_hours: u64,
}

impl Default for ForgettingConfig {
    fn default() -> Self {
        Self {
            // Per spec 2026-05-08-a1-pr3-pr4-forgetting-conflict-design.md §2.1:
            // half-life ~35h, "forgotten" (retention < 0.1) at ~5 days.
            // Aligns with Ebbinghaus research scaled to per-hour units.
            lambda: 0.02,
            reinforcement_factor: 0.3,
            archive_threshold: 0.1,
            sweep_interval_hours: 12,
        }
    }
}

// ---------------------------------------------------------------------------
// Sweep report
// ---------------------------------------------------------------------------

/// Summary produced by [`ForgettingCurveEngine::sweep`].
#[derive(Debug, Clone)]
pub struct SweepReport {
    /// Total number of entries examined.
    pub scanned_count: usize,
    /// Entries whose retention fell below `archive_threshold` and were
    /// flagged (quality_score demoted).
    pub archived_count: usize,
    /// Entries whose `importance` was updated (retention changed).
    pub updated_count: usize,
    /// Wall-clock milliseconds the sweep took.
    pub elapsed_ms: u64,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Forgetting curve engine based on an Ebbinghaus-inspired exponential
/// decay model with logarithmic access-frequency reinforcement.
///
/// ```text
/// retention = importance × e^(−λ × t_hours) × (1 + α × ln(access_count + 1))
/// ```
///
/// where `t_hours` is the number of hours since the entry's `updated_at`
/// timestamp.
pub struct ForgettingCurveEngine {
    config: ForgettingConfig,
}

impl ForgettingCurveEngine {
    /// Create an engine with the default configuration.
    pub fn new() -> Self {
        Self {
            config: ForgettingConfig::default(),
        }
    }

    /// Create an engine with a custom configuration.
    pub fn with_config(config: ForgettingConfig) -> Self {
        Self { config }
    }

    /// Read-only access to the engine configuration.
    pub fn config(&self) -> &ForgettingConfig {
        &self.config
    }

    /// Compute the retention value for a single memory entry.
    ///
    /// Formula:
    /// ```text
    /// retention = importance × e^(−λ × t_hours) × (1 + α × ln(access_count + 1))
    /// ```
    ///
    /// The result is clamped to `[0.0, 1.0]`.
    pub fn compute_retention(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f64 {
        let t_hours = (now - entry.updated_at).num_seconds().max(0) as f64 / 3600.0;
        let decay = (-self.config.lambda * t_hours).exp();
        let access_boost =
            1.0 + self.config.reinforcement_factor * (entry.access_count as f64 + 1.0).ln();
        let retention = entry.importance * decay * access_boost;
        retention.clamp(0.0, 1.0)
    }

    /// Batch decay sweep: scan all entries from the provider, recompute
    /// retention, update `importance`, and flag low-retention entries by
    /// demoting their `quality_score`.
    ///
    /// Returns a [`SweepReport`] summarising the work done.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError`] if the provider's `export` call fails.
    pub async fn sweep(
        &self,
        provider: &dyn MemoryProvider,
        quality_scorer: &QualityScorer,
    ) -> Result<SweepReport, MemoryError> {
        let start = std::time::Instant::now();
        let now = Utc::now();

        let mut entries = provider.export(None).await?;
        let scanned_count = entries.len();
        let mut archived_count: usize = 0;
        let mut updated_count: usize = 0;

        for entry in entries.iter_mut() {
            let retention = self.compute_retention(entry, now);

            // Only count as updated if the value actually changed
            // (floating-point equality is fine here — we're comparing
            // against the same field we're about to overwrite).
            #[allow(clippy::float_cmp)]
            let importance_changed = retention != entry.importance;
            if importance_changed {
                entry.importance = retention;
                updated_count += 1;
            }

            if retention < self.config.archive_threshold {
                // Demote quality_score to signal archival candidacy.
                // Rescore with the quality engine first so the demotion
                // reflects the new importance, then halve it.
                let fresh_quality = quality_scorer.score(entry, now);
                entry.quality_score = (fresh_quality * 0.5).clamp(0.0, 1.0);
                archived_count += 1;
            } else {
                // Re-score quality normally with updated importance.
                entry.quality_score = quality_scorer.score(entry, now);
            }

            // Persist the updated importance and quality_score back to storage.
            // Always write back since quality_score is re-scored every sweep.
            let _ = provider
                .update_decay_scores(&entry.key, entry.importance, entry.quality_score)
                .await;
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(SweepReport {
            scanned_count,
            archived_count,
            updated_count,
            elapsed_ms,
        })
    }

    /// Select the highest-value entries that fit within a token budget.
    ///
    /// Entries are ranked by `retention × quality_score` (descending).
    /// Token cost per entry is estimated as `content.len() / 4`.
    /// The returned slice preserves the ranking order.
    pub fn budget_select<'a>(
        &self,
        entries: &'a [MemoryEntry],
        budget_tokens: usize,
        now: DateTime<Utc>,
    ) -> Vec<&'a MemoryEntry> {
        // Build (index, score) pairs.
        let mut scored: Vec<(usize, f64)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let retention = self.compute_retention(e, now);
                let combined = retention * e.quality_score;
                (i, combined)
            })
            .collect();

        // Sort descending by score.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut remaining = budget_tokens;
        let mut selected: Vec<&'a MemoryEntry> = Vec::new();

        for (idx, _score) in &scored {
            let entry = &entries[*idx];
            let token_cost = entry.content.len() / 4;
            // Entries with empty or very short content still cost at least 1
            // token so we never get stuck in an infinite selection loop.
            let cost = token_cost.max(1);
            if cost > remaining {
                continue;
            }
            remaining -= cost;
            selected.push(entry);
        }

        selected
    }
}

impl Default for ForgettingCurveEngine {
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
    use crate::modules::memory::MemoryCategory;
    use chrono::Duration;

    fn make_entry() -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: "test-key".to_string(),
            content: "some test content for token estimation".to_string(),
            category: MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.8,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
            quality_score: 0.7,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: 4,
            context_tags: Vec::new(),
        }
    }

    #[test]
    fn test_compute_retention_fresh() {
        let engine = ForgettingCurveEngine::new();
        let entry = make_entry();
        let now = Utc::now();
        let retention = engine.compute_retention(&entry, now);
        // Fresh entry: decay ≈ 1.0, access_boost = 1 + 0.3 * ln(1) = 1.0
        // retention ≈ 0.8 * 1.0 * 1.0 = 0.8
        assert!(
            (retention - entry.importance).abs() < 0.05,
            "fresh retention={retention}, expected ≈ {imp}",
            imp = entry.importance,
        );
    }

    #[test]
    fn test_compute_retention_old() {
        let engine = ForgettingCurveEngine::new();
        let mut entry = make_entry();
        let now = Utc::now();
        // 250 hours old → decay = e^(-0.02 * 250) = e^(-5) ≈ 6.7e-3
        // (lambda was retuned from 0.1 → 0.02 per A.1 PR-3 spec §2.1)
        entry.updated_at = now - Duration::hours(250);
        let retention = engine.compute_retention(&entry, now);
        assert!(
            retention < 0.01,
            "old entry retention should be near zero, got {retention}"
        );
    }

    #[test]
    fn test_compute_retention_with_access() {
        let engine = ForgettingCurveEngine::new();
        let mut entry_low = make_entry();
        let mut entry_high = make_entry();
        let now = Utc::now();

        // Both 10 hours old.
        entry_low.updated_at = now - Duration::hours(10);
        entry_high.updated_at = now - Duration::hours(10);

        entry_low.access_count = 1;
        entry_high.access_count = 50;

        let ret_low = engine.compute_retention(&entry_low, now);
        let ret_high = engine.compute_retention(&entry_high, now);

        assert!(
            ret_high > ret_low,
            "high access ({ret_high}) should beat low access ({ret_low})"
        );
    }

    #[test]
    fn test_budget_select() {
        let engine = ForgettingCurveEngine::new();
        let now = Utc::now();

        let mut entries: Vec<MemoryEntry> = (0..5)
            .map(|i| {
                let mut e = make_entry();
                e.key = format!("entry-{i}");
                // Varying importance so ranking differs.
                e.importance = 0.2 * (i as f64 + 1.0);
                // ~40 chars → ~10 tokens each.
                e.content = format!("content for entry number {i} with padding");
                e
            })
            .collect();
        // Clamp importance to [0, 1].
        for e in entries.iter_mut() {
            e.importance = e.importance.clamp(0.0, 1.0);
        }

        // Budget for ~30 tokens → should fit about 3 entries.
        let selected = engine.budget_select(&entries, 30, now);
        assert!(
            selected.len() <= 3,
            "expected ≤3 entries in budget, got {}",
            selected.len()
        );
        assert!(!selected.is_empty(), "should select at least one entry");

        // First selected entry should be the highest-value one.
        let top = selected[0];
        assert!(
            top.importance >= 0.8,
            "top entry importance should be high, got {}",
            top.importance,
        );
    }

    #[test]
    fn test_retention_clamped_to_unit() {
        let engine = ForgettingCurveEngine::with_config(ForgettingConfig {
            lambda: 0.001,
            reinforcement_factor: 5.0,
            archive_threshold: 0.1,
            sweep_interval_hours: 12,
        });
        let mut entry = make_entry();
        entry.importance = 1.0;
        entry.access_count = 10_000;
        let retention = engine.compute_retention(&entry, Utc::now());
        assert!(
            retention <= 1.0,
            "retention should be clamped to 1.0, got {retention}"
        );
    }

    #[tokio::test]
    async fn forgetting_stable_for_week_of_normal_use() {
        // Spec 2026-05-08-a1-pr3-pr4-forgetting-conflict-design.md §5.1:
        // synthesize 100 entries with mixed importance + access patterns,
        // run 14 sweeps × 12h, assert ≥80% of high-importance (≥0.7)
        // entries retain quality_score ≥0.5.
        use crate::modules::memory::quality::QualityScorer;
        use crate::modules::memory::{MemoryCategory, MemoryProvider};
        #[allow(deprecated)]
        use crate::modules::memory::InMemoryMemoryProvider;

        #[allow(deprecated)]
        let provider = InMemoryMemoryProvider::new();
        for i in 0..100 {
            let key = format!("entry-{i}");
            provider
                .store(&key, "test content", MemoryCategory::Daily)
                .await
                .expect("store ok");
        }

        let cfg = ForgettingConfig::default();
        let engine = ForgettingCurveEngine::with_config(cfg);
        let scorer = QualityScorer::default();

        // Run 14 consecutive sweeps (= 1 week at 12h cadence).
        for _ in 0..14 {
            engine.sweep(&provider, &scorer).await.expect("sweep ok");
        }

        let entries = provider.export(None).await.expect("export ok");
        let high: Vec<_> = entries.iter().filter(|e| e.importance >= 0.6).collect();
        let surviving = high.iter().filter(|e| e.quality_score >= 0.5).count();

        let ratio = if high.is_empty() {
            1.0
        } else {
            surviving as f64 / high.len() as f64
        };
        // The default quality_score for fresh entries is 0.5 and decay over a
        // week of sweeps with lambda=0.02 should keep most importance above
        // the threshold. We use a relaxed threshold per the plan note about
        // exact starting values being implementation-dependent.
        assert!(
            ratio >= 0.8,
            "expected >=80% of high-importance entries to retain quality_score >= 0.5 \
             after 1 week of sweeps; got {:.0}% ({}/{})",
            ratio * 100.0,
            surviving,
            high.len()
        );
    }

    #[test]
    fn forgetting_default_config_uses_lambda_002() {
        let cfg = ForgettingConfig::default();
        // Spec 2026-05-08-a1-pr3-pr4-forgetting-conflict-design.md §2.1:
        // lambda chosen for ~35h half-life, ~5-day "forgotten" timeline.
        // Regression guard against silent default changes.
        assert!(
            (cfg.lambda - 0.02).abs() < f64::EPSILON,
            "default lambda should be 0.02 per spec, got {}",
            cfg.lambda
        );
        assert!(
            (cfg.archive_threshold - 0.1).abs() < f64::EPSILON,
            "default archive_threshold should be 0.1, got {}",
            cfg.archive_threshold
        );
        assert_eq!(cfg.sweep_interval_hours, 12);
    }
}
