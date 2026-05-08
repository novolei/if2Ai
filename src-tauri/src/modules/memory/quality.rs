//! Memory quality scoring engine.
//!
//! Computes a composite quality score for each [`MemoryEntry`] based on four
//! weighted factors: source reliability, temporal freshness, usage frequency,
//! and internal consistency (inverse of contradiction count).
//!
//! The engine is stateless — it reads the entry fields at scoring time and
//! returns the result without persisting anything.  Callers (e.g. the
//! `MemoryTicker` daily job or the `after_turn` pipeline) are responsible
//! for writing the computed `quality_score` back to the provider.

use chrono::{DateTime, Utc};

use crate::modules::memory::MemoryEntry;

/// Weight configuration for the four quality factors.
///
/// All weights should sum to 1.0 for the score to stay in `[0.0, 1.0]`
/// without clamping, but the scorer clamps regardless as a safety net.
#[derive(Debug, Clone)]
pub struct QualityWeights {
    /// Weight for the `source_reliability` factor (default 0.3).
    pub source_weight: f64,
    /// Weight for the temporal freshness factor (default 0.2).
    pub freshness_weight: f64,
    /// Weight for the usage frequency factor (default 0.25).
    pub usage_weight: f64,
    /// Weight for the consistency (inverse contradiction) factor (default 0.25).
    pub consistency_weight: f64,
}

impl Default for QualityWeights {
    fn default() -> Self {
        Self {
            source_weight: 0.3,
            freshness_weight: 0.2,
            usage_weight: 0.25,
            consistency_weight: 0.25,
        }
    }
}

/// Stateless quality scorer for memory entries.
///
/// Instantiate once (typically at application start) and call [`Self::score`]
/// or [`Self::batch_rescore`] as needed.
#[derive(Debug, Clone)]
pub struct QualityScorer {
    weights: QualityWeights,
}

impl Default for QualityScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityScorer {
    /// Create a scorer with the default weight configuration.
    pub fn new() -> Self {
        Self {
            weights: QualityWeights::default(),
        }
    }

    /// Create a scorer with custom weights.
    pub fn with_weights(weights: QualityWeights) -> Self {
        Self { weights }
    }

    /// Compute the composite quality score for a single memory entry.
    ///
    /// The score is a weighted sum of four normalised factors:
    ///
    /// 1. **Source reliability** — `entry.source_reliability` used directly.
    /// 2. **Freshness** — `1.0 / (1.0 + hours_since_update / 168.0)` where
    ///    168 hours ≈ one week.  Newer entries score closer to 1.0.
    /// 3. **Usage** — `min(1.0, access_count / 50.0)`.  Frequently accessed
    ///    entries score higher, saturating at 50 accesses.
    /// 4. **Consistency** — `1.0 / (1.0 + contradiction_count)`.  Entries
    ///    with no contradictions score 1.0; each contradiction reduces the
    ///    score hyperbolically.
    ///
    /// The result is clamped to `[0.0, 1.0]`.
    pub fn score(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f64 {
        let w = &self.weights;

        // Factor 1: source reliability (already in [0, 1]).
        let source = entry.source_reliability.clamp(0.0, 1.0);

        // Factor 2: temporal freshness.
        let hours_since_update = (now - entry.updated_at).num_hours().max(0) as f64;
        let freshness = 1.0 / (1.0 + hours_since_update / 168.0);

        // Factor 3: usage frequency (normalised, saturates at 50).
        let usage = (entry.access_count as f64 / 50.0).min(1.0);

        // Factor 4: consistency (inverse contradiction count).
        let consistency = 1.0 / (1.0 + entry.contradiction_count as f64);

        let quality = w.source_weight * source
            + w.freshness_weight * freshness
            + w.usage_weight * usage
            + w.consistency_weight * consistency;

        quality.clamp(0.0, 1.0)
    }

    /// Recompute `quality_score` for every entry in the slice in place.
    ///
    /// This is a convenience wrapper around [`Self::score`] for batch
    /// operations (e.g. daily ticker rescoring).
    pub fn batch_rescore(&self, entries: &mut [MemoryEntry], now: DateTime<Utc>) {
        for entry in entries.iter_mut() {
            entry.quality_score = self.score(entry, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::MemoryCategory;
    use chrono::Duration;

    fn make_entry() -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: "test-key".to_string(),
            content: "test content".to_string(),
            category: MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
            quality_score: 0.5,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: 4,
            context_tags: Vec::new(),
        }
    }

    #[test]
    fn default_weights_sum_to_one() {
        let w = QualityWeights::default();
        let sum = w.source_weight + w.freshness_weight + w.usage_weight + w.consistency_weight;
        assert!((sum - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fresh_entry_with_defaults_scores_reasonably() {
        let scorer = QualityScorer::new();
        let entry = make_entry();
        let now = Utc::now();
        let score = scorer.score(&entry, now);
        // source=0.5*0.3=0.15, freshness≈1.0*0.2=0.2, usage=0*0.25=0, consistency=1.0*0.25=0.25
        // expected ≈ 0.6
        assert!(score > 0.5 && score < 0.7, "score={score}");
    }

    #[test]
    fn old_entry_scores_lower_freshness() {
        let scorer = QualityScorer::new();
        let mut entry = make_entry();
        let now = Utc::now();
        entry.updated_at = now - Duration::hours(168 * 4); // 4 weeks old
        let score = scorer.score(&entry, now);
        // freshness factor should be much lower
        assert!(score < 0.5, "score={score}");
    }

    #[test]
    fn high_contradiction_count_lowers_score() {
        let scorer = QualityScorer::new();
        let mut entry = make_entry();
        entry.contradiction_count = 10;
        let now = Utc::now();
        let score = scorer.score(&entry, now);
        let clean = scorer.score(&make_entry(), now);
        assert!(score < clean, "contradicted={score}, clean={clean}");
    }

    #[test]
    fn batch_rescore_updates_all() {
        let scorer = QualityScorer::new();
        let now = Utc::now();
        let mut entries = vec![make_entry(), make_entry()];
        entries[1].source_reliability = 1.0;
        scorer.batch_rescore(&mut entries, now);
        assert!(entries[1].quality_score > entries[0].quality_score);
    }

    #[test]
    fn score_clamped_to_unit_interval() {
        let scorer = QualityScorer::with_weights(QualityWeights {
            source_weight: 10.0,
            freshness_weight: 10.0,
            usage_weight: 10.0,
            consistency_weight: 10.0,
        });
        let mut entry = make_entry();
        entry.source_reliability = 1.0;
        entry.access_count = 100;
        let score = scorer.score(&entry, Utc::now());
        assert!(score <= 1.0, "score={score}");
    }
}
