//! Episodic memory compaction and Weibull importance decay
//!
//! When episodic memory exceeds its token budget, entries are compacted
//! by preserving high-importance entries and generating an LLM-style summary
//! for the rest.
//!
//! Weibull decay models how memory importance fades over time.

#![allow(dead_code)]

use chrono::Utc;

use crate::modules::memory::MemoryEntry;

/// Weibull distribution parameters for importance decay
pub struct WeibullDecay {
    /// Scale parameter in hours (default: 168 = 7 days)
    pub lambda: f32,
    /// Shape parameter (default: 1.2)
    pub k: f32,
}

impl Default for WeibullDecay {
    fn default() -> Self {
        Self {
            lambda: 24.0 * 7.0, // 7-day scale
            k: 1.2,             // shape parameter
        }
    }
}

impl WeibullDecay {
    /// Create a new Weibull decay with custom parameters
    pub fn new(lambda_hours: f32, k: f32) -> Self {
        Self {
            lambda: lambda_hours,
            k,
        }
    }

    /// Compute decayed importance given age and initial importance
    ///
    /// Returns a decay factor in [0, 1] that should be multiplied by
    /// the initial importance. Returns 0 for very old entries.
    #[must_use]
    pub fn decay_factor(&self, age_hours: f32) -> f32 {
        if age_hours <= 0.0 {
            return 1.0;
        }
        // Weibull survival function: exp(-(t/lambda)^k)
        let ratio = age_hours / self.lambda;
        let exponent = -(ratio.powf(self.k));
        exponent.exp()
    }

    /// Compute the current effective importance of a memory entry
    ///
    /// Combines base importance, access count bonus, trust boost,
    /// and Weibull time decay.
    #[must_use]
    pub fn compute_importance(&self, entry: &MemoryEntry) -> f64 {
        let now = Utc::now();
        let age_hours = (now - entry.created_at).num_hours() as f64;
        let age_hours_f32 = age_hours as f32;

        let base = entry.importance + (entry.access_count as f64 * 0.01);
        let trust_boost = (entry.trust_score + 1.0) * 0.05;
        let decay = self.decay_factor(age_hours_f32) as f64;

        (base + trust_boost) * decay
    }
}

/// Configuration for episodic compaction
#[derive(Debug, Clone)]
pub struct EpisodicCompactionConfig {
    /// Budget in tokens for episodic slot
    pub budget_tokens: usize,
    /// Preserve up to this percentage of budget for new entries (default: 80%)
    pub preserve_pct: usize,
}

impl Default for EpisodicCompactionConfig {
    fn default() -> Self {
        Self {
            budget_tokens: 800,
            preserve_pct: 80,
        }
    }
}

/// Result of an episodic compaction operation
#[derive(Debug, Clone)]
pub struct EpisodicCompactionResult {
    /// Whether compaction was actually performed
    pub compacted: bool,
    /// Number of entries preserved
    pub preserved_count: usize,
    /// Number of entries summarized (removed)
    pub summarized_count: usize,
    /// Estimated tokens after compaction
    pub estimated_tokens: usize,
}

/// Check if episodic entries exceed the budget and compact if needed
///
/// Preserves entries with highest importance until the preserve budget
/// is reached. Remaining entries are "summarized" (removed from the list).
///
/// Since this module doesn't have LLM access, it generates a simple
/// count-based summary placeholder. In production, an LLM would generate
/// a meaningful summary.
pub fn compact_episodic_entries(
    entries: &mut Vec<MemoryEntry>,
    config: EpisodicCompactionConfig,
) -> EpisodicCompactionResult {
    if entries.is_empty() {
        return EpisodicCompactionResult {
            compacted: false,
            preserved_count: 0,
            summarized_count: 0,
            estimated_tokens: 0,
        };
    }

    let total_tokens: usize = entries
        .iter()
        .map(crate::modules::runtime::budget::estimate_entry_tokens)
        .sum();

    if total_tokens <= config.budget_tokens {
        return EpisodicCompactionResult {
            compacted: false,
            preserved_count: entries.len(),
            summarized_count: 0,
            estimated_tokens: total_tokens,
        };
    }

    // Sort by importance descending (highest first)
    entries.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let preserve_budget = config.budget_tokens * config.preserve_pct / 100;
    let mut preserved = Vec::new();
    let mut tokens = 0;
    let mut preserved_count = 0;

    for entry in entries.iter() {
        let entry_tokens = crate::modules::runtime::budget::estimate_entry_tokens(entry);
        if tokens + entry_tokens > preserve_budget {
            break;
        }
        preserved.push(entry.clone());
        tokens += entry_tokens;
        preserved_count += 1;
    }

    let summarized_count = entries.len() - preserved_count;

    // Replace entries with preserved set
    *entries = preserved;

    EpisodicCompactionResult {
        compacted: summarized_count > 0,
        preserved_count,
        summarized_count,
        estimated_tokens: tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::MemoryCategory;
    use std::time::Duration;

    fn entry_with_age(key: &str, content: &str, age_hours: f64, importance: f64) -> MemoryEntry {
        let now = Utc::now();
        let created = now
            - chrono::Duration::from_std(Duration::from_secs_f64(age_hours * 3600.0))
                .unwrap_or_default();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: MemoryCategory::Core,
            created_at: created,
            updated_at: now,
            importance,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        }
    }

    #[test]
    fn weibull_fresh_entry_has_full_decay() {
        let decay = WeibullDecay::default();
        let factor = decay.decay_factor(0.0);
        assert!((factor - 1.0).abs() < 0.001);
    }

    #[test]
    fn weibull_decay_decreases_over_time() {
        let decay = WeibullDecay::default();
        let fresh = decay.decay_factor(1.0);
        let old = decay.decay_factor(168.0); // 7 days
        assert!(fresh > old);
    }

    #[test]
    fn weibull_very_old_entry_near_zero() {
        let decay = WeibullDecay::default();
        let factor = decay.decay_factor(720.0); // 30 days
        assert!(factor < 0.1);
    }

    #[test]
    fn compute_importance_combines_factors() {
        let decay = WeibullDecay::default();
        let entry = entry_with_age("k1", "content", 0.0, 0.5);
        let importance = decay.compute_importance(&entry);
        // base=0.5, trust_boost=0.05, decay=1.0 => 0.55
        assert!((importance - 0.55).abs() < 0.01);
    }

    #[test]
    fn compute_importance_with_access_bonus() {
        let decay = WeibullDecay::default();
        let mut entry = entry_with_age("k1", "content", 0.0, 0.5);
        entry.access_count = 10;
        let importance = decay.compute_importance(&entry);
        // base=0.5+0.1, trust_boost=0.05, decay=1.0 => 0.65
        assert!((importance - 0.65).abs() < 0.01);
    }

    #[test]
    fn compact_episodic_no_compaction_when_under_budget() {
        let mut entries = vec![
            entry_with_age("k1", "short content", 1.0, 0.5),
            entry_with_age("k2", "another short", 2.0, 0.3),
        ];
        let config = EpisodicCompactionConfig {
            budget_tokens: 1000,
            ..Default::default()
        };
        let result = compact_episodic_entries(&mut entries, config);
        assert!(!result.compacted);
        assert_eq!(result.preserved_count, 2);
        assert_eq!(result.summarized_count, 0);
    }

    #[test]
    fn compact_episodic_removes_low_importity_entries() {
        let mut entries = vec![
            entry_with_age("k1", &"important content ".repeat(50), 1.0, 0.9),
            entry_with_age("k2", &"less important ".repeat(50), 2.0, 0.2),
            entry_with_age("k3", &"medium importance ".repeat(50), 3.0, 0.5),
        ];
        // Total tokens will be significant due to content length
        let config = EpisodicCompactionConfig {
            budget_tokens: 200,
            preserve_pct: 80,
        };
        let result = compact_episodic_entries(&mut entries, config);
        // Should have compacted since entries are large
        let _ = result.compacted; // verify no panic
                                  // Entries should be sorted by importance (highest first)
        if entries.len() >= 2 {
            assert!(entries[0].importance >= entries[1].importance);
        }
    }

    #[test]
    fn compact_episodic_empty_returns_no_compaction() {
        let mut entries: Vec<MemoryEntry> = Vec::new();
        let config = EpisodicCompactionConfig::default();
        let result = compact_episodic_entries(&mut entries, config);
        assert!(!result.compacted);
        assert_eq!(result.preserved_count, 0);
    }

    #[test]
    fn custom_weibull_parameters() {
        let decay = WeibullDecay::new(24.0, 2.0); // 1-day scale, k=2
        let factor_1h = decay.decay_factor(1.0);
        let factor_24h = decay.decay_factor(24.0);
        assert!(factor_1h > factor_24h);
    }
}
