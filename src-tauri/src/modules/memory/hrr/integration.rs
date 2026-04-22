//! Hybrid Memory Provider — LanceDB + HRR integration
//!
//! [`HybridMemoryProvider`] combines LanceDB (primary vector store) with
//! Holographic Store (algebraic reasoning supplement) to provide:
//! - ANN vector search via LanceDB
//! - Algebraic reasoning via HRR (bind/unbind/reason)
//! - Configurable HRR enable/disable
//!
//! # `#![allow(dead_code)]` justification
//! This module integrates HRR into the memory pipeline. It will be wired
//! into the agent loop and the active retrieval flow.

#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::RwLock;

use super::store::{HRRError, HolographicStore};
use super::HRRVector;
use crate::modules::memory::embedding::FastEmbedProvider;
use crate::modules::memory::providers::{LanceDBError, LanceDBMemory, ScoredMemory};
use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};

/// Configuration for HybridMemoryProvider
#[derive(Debug, Clone, Default)]
pub struct HybridConfig {
    /// Whether HRR algebraic reasoning is enabled
    pub hrr_enabled: bool,
    /// HRR store capacity override (0 = use default O(√dim) bound)
    pub hrr_capacity: usize,
}

/// Error type for HybridMemoryProvider
#[derive(Debug, thiserror::Error)]
pub enum HybridError {
    #[error("embedding failed: {0}")]
    Embedding(String),

    #[error("vector store failed: {0}")]
    LanceDB(String),

    #[error("HRR failed: {0}")]
    Hrr(String),
}

impl From<LanceDBError> for HybridError {
    fn from(e: LanceDBError) -> Self {
        HybridError::LanceDB(e.to_string())
    }
}

impl From<HRRError> for HybridError {
    fn from(e: HRRError) -> Self {
        HybridError::Hrr(e.to_string())
    }
}

/// Hybrid Memory Provider
///
/// Primary store: LanceDB (persistent, ANN search, unlimited capacity).
/// Supplement: HolographicStore (algebraic reasoning, O(√dim) capacity).
///
/// When HRR is enabled, store operations write to both backends and
/// recall operations can use algebraic reasoning for complex queries.
pub struct HybridMemoryProvider {
    embedder: Arc<FastEmbedProvider>,
    lancedb: Arc<RwLock<LanceDBMemory>>,
    hrr_store: Arc<HolographicStore>,
    config: HybridConfig,
}

impl HybridMemoryProvider {
    /// Create a new HybridMemoryProvider
    ///
    /// Initializes FastEmbed, opens/creates LanceDB, and creates the HRR store.
    pub async fn new(config: HybridConfig) -> Result<Self, HybridError> {
        let embedder =
            FastEmbedProvider::new().map_err(|e| HybridError::Embedding(e.to_string()))?;

        let lancedb_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".if2ai")
            .join("memory")
            .join("hybrid_db");

        // Ensure parent directory exists
        if let Some(parent) = lancedb_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let lancedb = LanceDBMemory::new(lancedb_path)
            .await
            .map_err(|e| HybridError::LanceDB(e.to_string()))?;

        let hrr_store = Arc::new(HolographicStore::from_embedder(&embedder).await);

        Ok(Self {
            embedder: Arc::new(embedder),
            lancedb: Arc::new(RwLock::new(lancedb)),
            hrr_store,
            config,
        })
    }

    /// Check if HRR is enabled
    pub fn is_hrr_enabled(&self) -> bool {
        self.config.hrr_enabled
    }

    /// Search using only LanceDB vector similarity
    pub async fn vector_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, HybridError> {
        let query_embedding = self
            .embedder
            .embed_one(query)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;

        let db = self.lancedb.read().await;
        let results = db
            .search(&query_embedding, limit)
            .await
            .map_err(HybridError::from)?;

        Ok(results)
    }

    /// Search using HRR algebraic reasoning
    ///
    /// Embeds the query, probes the HRR store, and returns ranked results.
    /// Only meaningful when HRR is enabled and the store has entries.
    pub async fn hrr_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>, HybridError> {
        if !self.config.hrr_enabled {
            return Ok(Vec::new());
        }

        let query_embedding = self
            .embedder
            .embed_one(query)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;

        let hrr_query = HRRVector(query_embedding);
        let results = self.hrr_store.probe(&hrr_query, limit).await;

        Ok(results)
    }

    /// Algebraic reasoning: bind a key-value pair into HRR store
    ///
    /// Useful for storing relational facts: "Alice -> likes -> Pizza"
    /// becomes `bind(bind(alice_key, relation_key), value_vector)`.
    pub async fn hrr_bind(
        &self,
        composite_key: &str,
        key: &str,
        value: &str,
    ) -> Result<(), HybridError> {
        if !self.config.hrr_enabled {
            return Ok(());
        }

        let key_vec = self
            .embedder
            .embed_one(key)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;
        let value_vec = self
            .embedder
            .embed_one(value)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;

        let composite = super::bind(&HRRVector(key_vec), &HRRVector(value_vec));
        self.hrr_store
            .store(composite_key, &composite)
            .await
            .map_err(HybridError::from)?;

        Ok(())
    }

    /// Algebraic unbind: retrieve a value from a composite using a key
    pub async fn hrr_unbind(
        &self,
        _composite_key: &str,
        query_key: &str,
    ) -> Result<Option<HRRVector>, HybridError> {
        if !self.config.hrr_enabled {
            return Ok(None);
        }

        // Probe to find the closest matching composite
        let query_vec = self
            .embedder
            .embed_one(query_key)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;
        let hrr_query = HRRVector(query_vec);

        let results = self.hrr_store.probe(&hrr_query, 1).await;
        let Some((best_key, _score)) = results.into_iter().next() else {
            return Ok(None);
        };

        // Retrieve the composite and unbind
        let stored_vec = self
            .embedder
            .embed_one(&best_key)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;
        let hrr_stored = HRRVector(stored_vec);

        let key_vec = self
            .embedder
            .embed_one(query_key)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;
        let hrr_key = HRRVector(key_vec);

        let retrieved = super::unbind(&hrr_stored, &hrr_key);
        Ok(Some(retrieved))
    }

    /// Detect contradictions in HRR store
    ///
    /// Compares two text snippets and returns true if they encode
    /// opposing information (cosine similarity < -0.8).
    pub async fn detect_contradiction(&self, a: &str, b: &str) -> Result<bool, HybridError> {
        if !self.config.hrr_enabled {
            return Ok(false);
        }

        let a_vec = self
            .embedder
            .embed_one(a)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;
        let b_vec = self
            .embedder
            .embed_one(b)
            .map_err(|e| HybridError::Embedding(e.to_string()))?;

        let result = self
            .hrr_store
            .contradict(&HRRVector(a_vec), &HRRVector(b_vec))
            .await;

        Ok(result)
    }

    /// Return the number of entries in the HRR store
    pub async fn hrr_len(&self) -> usize {
        self.hrr_store.len().await
    }

    /// Return the number of entries in LanceDB
    pub async fn lancedb_count(&self) -> Result<usize, HybridError> {
        let db = self.lancedb.read().await;
        let count = db.count().await.map_err(HybridError::from)?;
        Ok(count)
    }
}

/// Convert a ScoredMemory from LanceDB to a MemoryEntry
fn scored_to_entry(scored: &ScoredMemory) -> MemoryEntry {
    let category = match scored.category.as_str() {
        "core" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
        "working" => MemoryCategory::Working,
        "procedural" => MemoryCategory::Procedural,
        "reflection" => MemoryCategory::Reflection,
        other => MemoryCategory::Custom(other.to_string()),
    };

    let now = chrono::Utc::now();
    MemoryEntry {
        key: scored.key.clone(),
        content: scored.content.clone(),
        category,
        created_at: now,
        updated_at: now,
        importance: 0.5,
        access_count: 0,
        trust_score: 0.0,
        session_id: None,
        project_id: None,
    }
}

/// MemoryProvider trait implementation for HybridMemoryProvider
///
/// Stores primarily in LanceDB. When HRR is enabled, also writes to
/// the HolographicStore for algebraic reasoning capability.
#[async_trait::async_trait]
impl MemoryProvider for HybridMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        let embedding = self
            .embedder
            .embed_one(content)
            .map_err(|e| MemoryError::Generic(format!("embedding failed: {e}")))?;

        let now = chrono::Utc::now();
        let entry = MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        };

        // Write to LanceDB (primary store)
        let db = self.lancedb.read().await;
        db.insert(&entry, &embedding)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb insert failed: {e}")))?;

        // Write to HRR store if enabled
        if self.config.hrr_enabled {
            let hrr_vec = HRRVector(embedding.clone());
            self.hrr_store
                .store(key, &hrr_vec)
                .await
                .map_err(|e| MemoryError::Generic(format!("hrr store failed: {e}")))?;
        }

        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        // Primary: LanceDB search
        let query_embedding = self
            .embedder
            .embed_one(query)
            .map_err(|e| MemoryError::Generic(format!("embedding failed: {e}")))?;

        let db = self.lancedb.read().await;
        let results = match category {
            Some(cat) => db
                .search_with_filter(&query_embedding, Some(cat), limit)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb search failed: {e}")))?,
            None => db
                .search(&query_embedding, limit)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb search failed: {e}")))?,
        };

        Ok(results.iter().map(scored_to_entry).collect())
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        // Delete from LanceDB
        let db = self.lancedb.read().await;
        db.delete(key)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb delete failed: {e}")))?;

        // Delete from HRR store if it exists
        if self.config.hrr_enabled {
            self.hrr_store.delete(key).await;
        }

        Ok(())
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        // LanceDB: search + delete all matching
        let all_results = self
            .recall("", Some(category), usize::MAX)
            .await
            .map_err(|e| MemoryError::Generic(format!("search for purge failed: {e}")))?;

        let db = self.lancedb.read().await;
        for entry in &all_results {
            db.delete(&entry.key)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb delete during purge: {e}")))?;
        }

        // HRR store doesn't have category filtering, so we skip it for purge
        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let recall_results = self.recall("", category, usize::MAX).await?;
        Ok(recall_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_config_defaults() {
        let config = HybridConfig::default();
        assert!(!config.hrr_enabled);
        assert_eq!(config.hrr_capacity, 0);
    }

    #[test]
    fn scored_to_entry_conversion() {
        let scored = ScoredMemory {
            key: "test_key".to_string(),
            content: "test content".to_string(),
            category: "core".to_string(),
            score: 0.95,
        };

        let entry = scored_to_entry(&scored);
        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.content, "test content");
        assert!(matches!(entry.category, MemoryCategory::Core));
    }

    #[tokio::test]
    #[ignore] // Requires model download
    async fn hybrid_search_returns_empty_when_no_data() {
        let config = HybridConfig::default();
        let provider = HybridMemoryProvider::new(config).await.unwrap();

        let results = provider.vector_search("test", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires model download
    async fn hrr_search_returns_empty_when_disabled() {
        let config = HybridConfig::default(); // hrr_enabled: false
        let provider = HybridMemoryProvider::new(config).await.unwrap();

        let results = provider.hrr_search("test", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires model download
    async fn detect_contradiction_returns_false_when_disabled() {
        let config = HybridConfig::default();
        let provider = HybridMemoryProvider::new(config).await.unwrap();

        let result = provider
            .detect_contradiction("hello", "world")
            .await
            .unwrap();
        assert!(!result);
    }

    // -----------------------------------------------------------------
    // Zero-dependency HRR coverage (H5)
    //
    // These tests intentionally avoid HybridMemoryProvider so they don't
    // need a FastEmbed model download or LanceDB on disk. They exercise
    // the algebraic surface (bind / unbind / probe / contradict) directly
    // against `HolographicStore` using `MockEmbedder`-derived vectors.
    // -----------------------------------------------------------------

    use super::super::store::HolographicStore;
    use super::super::{bind, unbind};
    use crate::modules::memory::embedding::MockEmbedder;

    fn embed_vec(embedder: &MockEmbedder, text: &str) -> HRRVector {
        HRRVector(embedder.embed_one(text).expect("mock embed"))
    }

    #[tokio::test]
    async fn hrr_store_round_trip_with_mock_embedder() {
        let embedder = MockEmbedder::new();
        let store = HolographicStore::new(MockEmbedder::DIMENSION);

        let key = "fact:capital_of_france";
        let value = embed_vec(&embedder, "Paris");
        store.store(key, &value).await.expect("store");

        // Probing with the same vector should surface the same key at rank 0.
        let hits = store.probe(&value, 5).await;
        assert!(!hits.is_empty(), "probe should return at least one hit");
        assert_eq!(hits[0].0, key, "expected stored key as top hit");
    }

    #[tokio::test]
    async fn hrr_bind_unbind_recovers_value_with_mock_embedder() {
        let embedder = MockEmbedder::new();

        let key_vec = embed_vec(&embedder, "likes");
        let value_vec = embed_vec(&embedder, "pizza");

        let composite = bind(&key_vec, &value_vec);
        let recovered = unbind(&composite, &key_vec);

        // Recovered vector should correlate strongly with the original value.
        // We use a generous threshold because mock vectors are random
        // unit-norm — the algebraic structure is what matters here.
        let dot: f32 = recovered
            .0
            .iter()
            .zip(value_vec.0.iter())
            .map(|(a, b)| a * b)
            .sum();
        assert!(
            dot > 0.1,
            "unbind should recover non-trivial correlation (dot = {dot})"
        );
    }

    #[tokio::test]
    async fn hrr_probe_returns_empty_for_missing_key() {
        let embedder = MockEmbedder::new();
        let store = HolographicStore::new(MockEmbedder::DIMENSION);

        let probe_vec = embed_vec(&embedder, "never_stored");
        let hits = store.probe(&probe_vec, 5).await;
        assert!(hits.is_empty(), "expected no hits in empty store");
    }
}
