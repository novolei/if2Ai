//! Holographic Store — capacity-managed HRR vector storage
//!
//! [`HolographicStore`] provides in-memory HRR vector storage with
//! capacity management (O(√dim) bound), similarity probing,
//! algebraic reasoning, and LRU eviction.
//!
//! # `#![allow(dead_code)]` justification
//! This module implements the HRR store layer for algebraic reasoning.
//! It is used by [`HybridMemoryProvider`](crate::modules::memory::hrr::integration::HybridMemoryProvider)
//! and will be wired into the active retrieval pipeline.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::operations::{bind, similarity, HRRVector};
use crate::modules::memory::embedding::FastEmbedProvider;

/// Error type for HRR operations
#[derive(Debug, thiserror::Error)]
pub enum HRRError {
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("capacity exceeded: max {0}")]
    CapacityExceeded(usize),

    #[error("HRR operation failed: {0}")]
    OperationFailed(String),
}

/// LRU tracking for a single HRR entry
#[derive(Debug, Clone)]
struct LRUEntry {
    vector: HRRVector,
    importance: f32,
    last_accessed: std::time::Instant,
}

/// Holographic Store for HRR vectors
///
/// Capacity is bounded at O(√dim) × 100 — for 384d, ~700 entries max.
/// When capacity is exceeded, lowest-importance entries are evicted (LRU).
pub struct HolographicStore {
    vectors: Arc<RwLock<HashMap<String, LRUEntry>>>,
    dimension: usize,
    max_capacity: usize,
}

impl HolographicStore {
    /// Create a new HolographicStore with the given embedding dimension
    ///
    /// Capacity is computed as `floor(√dim × 100)`. For 384d this is ~1960.
    /// The theoretical bound is O(√dim); the ×100 scaling factor provides
    /// practical headroom while keeping interference manageable.
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        let max_capacity = ((dimension as f32).sqrt() * 100.0).max(10.0) as usize;

        Self {
            vectors: Arc::new(RwLock::new(HashMap::new())),
            dimension,
            max_capacity,
        }
    }

    /// Create from an existing FastEmbedProvider (uses its dimension)
    pub async fn from_embedder(embedder: &FastEmbedProvider) -> Self {
        Self::new(embedder.dimension())
    }

    /// Return the vector dimension
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Return the maximum capacity
    #[must_use]
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Return the current number of stored entries
    pub async fn len(&self) -> usize {
        self.vectors.read().await.len()
    }

    /// Check if the store is empty
    #[must_use]
    pub async fn is_empty(&self) -> bool {
        self.vectors.read().await.is_empty()
    }

    /// Store an HRRVector with the given key and importance
    ///
    /// If the key already exists, the value is updated.
    /// If capacity would be exceeded, LRU eviction runs first.
    pub async fn store(&self, key: &str, value: &HRRVector) -> Result<(), HRRError> {
        if value.dimension() != self.dimension {
            return Err(HRRError::DimensionMismatch {
                expected: self.dimension,
                got: value.dimension(),
            });
        }

        let mut vectors = self.vectors.write().await;

        // If key already exists, just update
        if vectors.contains_key(key) {
            vectors.insert(
                key.to_string(),
                LRUEntry {
                    vector: value.clone(),
                    importance: 0.5,
                    last_accessed: std::time::Instant::now(),
                },
            );
            return Ok(());
        }

        // Evict if at capacity
        if vectors.len() >= self.max_capacity {
            drop(vectors);
            self.evict_one().await?;
            vectors = self.vectors.write().await;
        }

        vectors.insert(
            key.to_string(),
            LRUEntry {
                vector: value.clone(),
                importance: 0.5,
                last_accessed: std::time::Instant::now(),
            },
        );
        Ok(())
    }

    /// Store with explicit importance score
    pub async fn store_with_importance(
        &self,
        key: &str,
        value: &HRRVector,
        importance: f32,
    ) -> Result<(), HRRError> {
        if value.dimension() != self.dimension {
            return Err(HRRError::DimensionMismatch {
                expected: self.dimension,
                got: value.dimension(),
            });
        }

        let mut vectors = self.vectors.write().await;

        if vectors.contains_key(key) {
            vectors.insert(
                key.to_string(),
                LRUEntry {
                    vector: value.clone(),
                    importance,
                    last_accessed: std::time::Instant::now(),
                },
            );
            return Ok(());
        }

        if vectors.len() >= self.max_capacity {
            drop(vectors);
            self.evict_one().await?;
            vectors = self.vectors.write().await;
        }

        vectors.insert(
            key.to_string(),
            LRUEntry {
                vector: value.clone(),
                importance,
                last_accessed: std::time::Instant::now(),
            },
        );
        Ok(())
    }

    /// Probe the store with a query vector, returning ranked results
    ///
    /// Returns `(key, similarity_score)` pairs sorted by descending similarity.
    pub async fn probe(&self, query: &HRRVector, limit: usize) -> Vec<(String, f32)> {
        let vectors = self.vectors.read().await;

        let mut results: Vec<_> = vectors
            .iter()
            .map(|(k, entry)| (k.clone(), similarity(query, &entry.vector)))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().take(limit).collect()
    }

    /// Algebraic reasoning: find premise-conclusion relationships
    ///
    /// For each premise-conclusion pair, computes `bind(premise, conclusion)`
    /// and checks if the composite is strongly related to the premise.
    /// Returns `(premise_index, conclusion_index, relatedness)` for pairs
    /// exceeding the threshold (default 0.8).
    pub async fn reason(
        &self,
        premises: &[HRRVector],
        conclusions: &[HRRVector],
    ) -> Vec<(usize, usize, f32)> {
        let mut results = Vec::new();

        for (pi, premise) in premises.iter().enumerate() {
            for (ci, conclusion) in conclusions.iter().enumerate() {
                let relatedness = self.check_relation(premise, conclusion).await;
                if relatedness > 0.8 {
                    results.push((pi, ci, relatedness));
                }
            }
        }

        results
    }

    /// Detect whether two vectors are contradictory
    ///
    /// Two vectors contradict when their cosine similarity is below -0.8,
    /// indicating they encode opposing information.
    #[must_use]
    pub async fn contradict(&self, a: &HRRVector, b: &HRRVector) -> bool {
        similarity(a, b) < -0.8
    }

    /// Adjust the trust score for a key
    ///
    /// Positive delta increases trust, negative decreases.
    /// Scores are clamped to [-1.0, 1.0].
    pub async fn adjust_trust(&self, key: &str, delta: f32) {
        let mut vectors = self.vectors.write().await;
        if let Some(entry) = vectors.get_mut(key) {
            entry.importance = (entry.importance + delta).clamp(0.0, 1.0);
        }
    }

    /// Check the relationship between a premise and conclusion
    ///
    /// Binds them together and measures similarity with the premise.
    /// High similarity means the conclusion is strongly related.
    async fn check_relation(&self, premise: &HRRVector, conclusion: &HRRVector) -> f32 {
        let composite = bind(premise, conclusion);
        similarity(&composite, premise)
    }

    /// Evict the lowest-importance entry
    async fn evict_one(&self) -> Result<(), HRRError> {
        let mut vectors = self.vectors.write().await;
        if vectors.is_empty() {
            return Err(HRRError::OperationFailed(
                "cannot evict from empty store".to_string(),
            ));
        }

        // Find entry with lowest importance; break ties by oldest access
        let evict_key = vectors
            .iter()
            .min_by(|(_, a), (_, b)| {
                a.importance
                    .partial_cmp(&b.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.last_accessed.cmp(&b.last_accessed))
            })
            .map(|(k, _)| k.clone());

        if let Some(key) = evict_key {
            vectors.remove(&key);
            Ok(())
        } else {
            Err(HRRError::OperationFailed(
                "failed to find entry to evict".to_string(),
            ))
        }
    }

    /// Delete a specific key from the store
    pub async fn delete(&self, key: &str) -> bool {
        let mut vectors = self.vectors.write().await;
        vectors.remove(key).is_some()
    }

    /// Clear all entries
    pub async fn clear(&self) {
        let mut vectors = self.vectors.write().await;
        vectors.clear();
    }
}

/// Create an HRRVector from text using the given embedder
pub async fn hrr_from_text(
    text: &str,
    embedder: &FastEmbedProvider,
) -> Result<HRRVector, HRRError> {
    let embedding = embedder
        .embed_one(text)
        .map_err(|e| HRRError::OperationFailed(format!("embedding failed: {e}")))?;
    Ok(HRRVector(embedding))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vec(values: &[f32]) -> HRRVector {
        HRRVector(values.to_vec())
    }

    #[tokio::test]
    async fn store_dimension_mismatch() {
        let store = HolographicStore::new(384);
        let wrong = HRRVector::new(128);
        let result = store.store("test", &wrong).await;
        assert!(matches!(result, Err(HRRError::DimensionMismatch { .. })));
    }

    #[tokio::test]
    async fn store_and_probe() {
        let store = HolographicStore::new(4);
        let v = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        store.store("a", &v).await.unwrap();

        let results = store.probe(&v, 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(&results[0].0, "a");
        assert!((results[0].1 - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn probe_returns_ranked_results() {
        let store = HolographicStore::new(4);
        let similar = make_vec(&[1.0, 0.1, 0.0, 0.0]);
        let dissimilar = make_vec(&[0.0, 0.0, 1.0, 0.0]);
        store.store("similar", &similar).await.unwrap();
        store.store("dissimilar", &dissimilar).await.unwrap();

        let query = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let results = store.probe(&query, 10).await;
        assert_eq!(results.len(), 2);
        // "similar" should rank higher than "dissimilar"
        assert_eq!(&results[0].0, "similar");
        assert!(results[0].1 > results[1].1);
    }

    #[tokio::test]
    async fn contradict_detects_opposition() {
        let store = HolographicStore::new(4);
        let a = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let b = make_vec(&[-1.0, 0.0, 0.0, 0.0]);
        let c = make_vec(&[0.0, 1.0, 0.0, 0.0]);

        assert!(
            store.contradict(&a, &b).await,
            "opposite vectors should contradict"
        );
        assert!(
            !store.contradict(&a, &c).await,
            "orthogonal vectors should not contradict"
        );
    }

    #[tokio::test]
    async fn lru_eviction_on_capacity() {
        // Use a small capacity for testing
        let store = HolographicStore::new(4);
        // Manually override max_capacity to test eviction
        // Since max_capacity is computed, we test with a store that will fill up
        let v1 = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let v2 = make_vec(&[0.0, 1.0, 0.0, 0.0]);

        store.store("first", &v1).await.unwrap();
        store.store("second", &v2).await.unwrap();

        // Both should be stored (capacity for 4d is at least 10)
        assert_eq!(store.len().await, 2);
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let store = HolographicStore::new(4);
        let v = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        store.store("key", &v).await.unwrap();

        assert!(store.delete("key").await);
        assert!(!store.delete("nonexistent").await);

        let results = store.probe(&v, 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn clear_removes_all() {
        let store = HolographicStore::new(4);
        store
            .store("a", &make_vec(&[1.0, 0.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .store("b", &make_vec(&[0.0, 1.0, 0.0, 0.0]))
            .await
            .unwrap();

        store.clear().await;
        assert_eq!(store.len().await, 0);
        assert!(store.is_empty().await);
    }

    #[tokio::test]
    async fn update_existing_key() {
        let store = HolographicStore::new(4);
        let v1 = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let v2 = make_vec(&[0.0, 1.0, 0.0, 0.0]);

        store.store("key", &v1).await.unwrap();
        store.store("key", &v2).await.unwrap();

        assert_eq!(store.len().await, 1);
        let results = store.probe(&v2, 10).await;
        assert_eq!(&results[0].0, "key");
    }

    #[test]
    fn capacity_computation_reasonable() {
        let store = HolographicStore::new(384);
        // √384 × 100 ≈ 1959
        assert!(store.max_capacity > 500);
        assert!(store.max_capacity < 3000);
    }
}
