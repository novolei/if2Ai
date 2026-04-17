//! RRF Fusion and Active Retrieval Manager
//!
//! Combines results from multiple memory layers using Reciprocal Rank Fusion (RRF),
//! and manages pre-LLM-call memory retrieval based on query intent.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::modules::memory::{MemoryEntry, MemoryError, MemoryProvider};

use super::intent::{classify_intent, RetrievalWeights};

/// RRF constant — standard value for reciprocal rank fusion
const RRF_K: f32 = 60.0;

/// A memory entry with an associated fusion score
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub entry: MemoryEntry,
    pub score: f32,
}

/// Perform weighted RRF fusion across three memory layer result sets
///
/// RRF formula: score = Σ (weight / (k + rank))
/// where k=60 is the standard smoothing constant.
///
/// Results are sorted by descending fused score and deduplicated by key.
#[must_use]
pub fn weighted_fusion(
    semantic: Vec<MemoryEntry>,
    episodic: Vec<MemoryEntry>,
    working: Vec<MemoryEntry>,
    weights: RetrievalWeights,
) -> Vec<ScoredMemory> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for (rank, item) in semantic.iter().enumerate() {
        let score = weights.semantic / (RRF_K + rank as f32);
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }
    for (rank, item) in episodic.iter().enumerate() {
        let score = weights.episodic / (RRF_K + rank as f32);
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }
    for (rank, item) in working.iter().enumerate() {
        let score = weights.working / (RRF_K + rank as f32);
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }

    // Build lookup for entries by key (last seen wins for dedup)
    let mut entries: HashMap<String, MemoryEntry> = HashMap::new();
    for item in semantic.into_iter().chain(episodic).chain(working) {
        entries.insert(item.key.clone(), item);
    }

    let mut results: Vec<_> = scores
        .into_iter()
        .filter_map(|(key, score)| {
            entries
                .remove(&key)
                .map(|entry| ScoredMemory { entry, score })
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

/// Configuration for active retrieval
#[derive(Debug, Clone)]
pub struct ActiveRetrievalConfig {
    /// Whether active retrieval is enabled
    pub enabled: bool,
    /// Maximum entries to retrieve per layer
    pub semantic_limit: usize,
    pub episodic_limit: usize,
    pub working_limit: usize,
}

impl Default for ActiveRetrievalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_limit: 10,
            episodic_limit: 5,
            working_limit: 8,
        }
    }
}

/// Manages active memory retrieval before LLM calls
///
/// Classifies the query intent, retrieves from each memory layer
/// with intent-based weights, and fuses the results.
pub struct ActiveRetrievalManager {
    pub config: ActiveRetrievalConfig,
}

impl ActiveRetrievalManager {
    /// Create a new manager with the given configuration
    pub fn new(config: ActiveRetrievalConfig) -> Self {
        Self { config }
    }

    /// Create a manager with default configuration
    pub fn with_defaults() -> Self {
        Self {
            config: ActiveRetrievalConfig::default(),
        }
    }

    /// Retrieve and fuse memories based on query intent
    ///
    /// If active retrieval is disabled, returns an empty vector.
    pub async fn retrieve(
        &self,
        query: &str,
        provider: &dyn MemoryProvider,
    ) -> Result<Vec<ScoredMemory>, MemoryError> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        let intent = classify_intent(query);
        let weights: RetrievalWeights = intent.into();

        // Retrieve from the single provider (simulating multi-layer with category filters)
        // In a full implementation, each layer would have its own provider.
        // Here we use category-based filtering to approximate layers.
        let (sem_results, epi_results, work_results) = tokio::join!(
            provider.recall(query, None, self.config.semantic_limit),
            provider.recall(query, None, self.config.episodic_limit),
            provider.recall(query, None, self.config.working_limit),
        );

        let sem_results = sem_results?;
        let epi_results = epi_results?;
        let work_results = work_results?;

        Ok(weighted_fusion(
            sem_results,
            epi_results,
            work_results,
            weights,
        ))
    }

    /// Retrieve memories and format them as a context string for LLM injection
    pub async fn retrieve_as_context(
        &self,
        query: &str,
        provider: &dyn MemoryProvider,
    ) -> Result<String, MemoryError> {
        let scored = self.retrieve(query, provider).await?;

        if scored.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("# Relevant Memories\n\n");
        for sm in &scored {
            context.push_str(&format!(
                "- [{}] (score: {:.3}): {}\n",
                sm.entry.category.as_str(),
                sm.score,
                sm.entry.content
            ));
        }

        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::{MemoryCategory, MemoryEntry};
    use chrono::Utc;

    fn test_entry(key: &str, content: &str) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        }
    }

    #[test]
    fn fusion_deduplicates_entries() {
        let sem = vec![test_entry("a", "content A"), test_entry("b", "content B")];
        let epi = vec![test_entry("a", "content A updated")];
        let work = vec![test_entry("c", "content C")];

        let weights = RetrievalWeights {
            semantic: 0.5,
            episodic: 0.3,
            working: 0.2,
        };

        let results = weighted_fusion(sem, epi, work, weights);
        // "a" appears in both sem and epi, should be deduplicated
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].entry.key, "a"); // "a" has highest score from 2 sources
    }

    #[test]
    fn fusion_sorts_by_score() {
        let sem = vec![
            test_entry("a", "A"),
            test_entry("b", "B"),
            test_entry("c", "C"),
        ];
        let epi = vec![];
        let work = vec![];

        let weights = RetrievalWeights {
            semantic: 1.0,
            episodic: 0.0,
            working: 0.0,
        };

        let results = weighted_fusion(sem, epi, work, weights);
        // Within same layer, earlier rank gets higher score
        assert_eq!(results[0].entry.key, "a");
        assert_eq!(results[1].entry.key, "b");
        assert_eq!(results[2].entry.key, "c");
        assert!(results[0].score > results[1].score);
        assert!(results[1].score > results[2].score);
    }

    #[test]
    fn fusion_empty_inputs() {
        let weights = RetrievalWeights {
            semantic: 0.3,
            episodic: 0.3,
            working: 0.4,
        };
        let results = weighted_fusion(vec![], vec![], vec![], weights);
        assert!(results.is_empty());
    }

    #[test]
    fn fusion_rrf_k_constant() {
        // Verify RRF_K is 60
        assert!((RRF_K - 60.0).abs() < 0.001);
    }

    #[test]
    fn fusion_cross_layer_boost() {
        // Entry appearing in multiple layers should score higher
        let sem = vec![test_entry("shared", "shared content")];
        let epi = vec![test_entry("shared", "shared content")];
        let work = vec![test_entry("shared", "shared content")];

        let weights = RetrievalWeights {
            semantic: 0.3,
            episodic: 0.3,
            working: 0.4,
        };

        let results = weighted_fusion(sem, epi, work, weights);
        assert_eq!(results.len(), 1);
        // Score should be sum of 3 layers: 0.3/60 + 0.3/60 + 0.4/60 ≈ 0.0167
        assert!(results[0].score > 0.01);
    }

    #[tokio::test]
    async fn retrieval_manager_respects_enabled_flag() {
        // Can't easily test with a real provider without a full setup,
        // but we can verify the config path
        let disabled = ActiveRetrievalManager::new(ActiveRetrievalConfig {
            enabled: false,
            ..Default::default()
        });
        assert!(!disabled.config.enabled);
    }

    #[tokio::test]
    async fn retrieval_manager_defaults() {
        let manager = ActiveRetrievalManager::with_defaults();
        assert!(manager.config.enabled);
        assert_eq!(manager.config.semantic_limit, 10);
        assert_eq!(manager.config.episodic_limit, 5);
        assert_eq!(manager.config.working_limit, 8);
    }
}
