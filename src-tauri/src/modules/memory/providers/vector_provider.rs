//! Vector-backed MemoryProvider wrapping LanceDB + FastEmbed
//!
//! [`VectorMemoryProvider`] combines vector embedding (FastEmbed) with
//! vector storage (LanceDB) to provide semantic memory search.
//!
//! # `#![allow(dead_code)]` justification
//! VectorMemoryProvider and its helpers are not yet called from the agent loop.
//! They provide the full MemoryProvider trait impl (store/recall/delete/purge/export)
//! and will be wired when HybridMemoryProvider is activated (Phase 6bw.7+).

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::lancedb::{LanceDBError, LanceDBMemory, ScoredMemory};
use crate::modules::memory::embedding::FastEmbedProvider;
use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};

/// Error type for VectorMemoryProvider operations
#[derive(Debug, thiserror::Error)]
pub enum VectorProviderError {
    #[error("embedding failed: {0}")]
    EmbeddingError(String),

    #[error("vector store failed: {0}")]
    VectorStoreError(String),
}

impl From<LanceDBError> for VectorProviderError {
    fn from(e: LanceDBError) -> Self {
        VectorProviderError::VectorStoreError(e.to_string())
    }
}

/// Configuration for vector memory provider
#[derive(Debug, Clone)]
pub struct VectorProviderConfig {
    pub db_path: PathBuf,
    pub vector_search_enabled: bool,
}

impl Default for VectorProviderConfig {
    fn default() -> Self {
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".if2ai")
            .join("memory")
            .join("vector_db");

        Self {
            db_path,
            vector_search_enabled: true,
        }
    }
}

/// Vector-backed MemoryProvider
///
/// Wraps `LanceDBMemory` + `FastEmbedProvider` to provide:
/// - `store`: embed text + store in LanceDB
/// - `recall`: vector search by embedding the query
/// - `delete`: remove from LanceDB
pub struct VectorMemoryProvider {
    embedder: Arc<FastEmbedProvider>,
    lancedb: Arc<RwLock<LanceDBMemory>>,
    config: VectorProviderConfig,
}

impl VectorMemoryProvider {
    /// Create a new VectorMemoryProvider
    ///
    /// Initializes the FastEmbed model and opens/creates the LanceDB database.
    pub async fn new(config: VectorProviderConfig) -> Result<Self, VectorProviderError> {
        let embedder = FastEmbedProvider::new()
            .map_err(|e| VectorProviderError::EmbeddingError(e.to_string()))?;

        let lancedb_path = config.db_path.clone();

        // Ensure parent directory exists
        if let Some(parent) = lancedb_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let lancedb = LanceDBMemory::new(lancedb_path)
            .await
            .map_err(|e| VectorProviderError::VectorStoreError(e.to_string()))?;

        Ok(Self {
            embedder: Arc::new(embedder),
            lancedb: Arc::new(RwLock::new(lancedb)),
            config,
        })
    }

    /// Check if vector search is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.vector_search_enabled
    }

    /// Search using vector similarity
    pub async fn vector_search(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, VectorProviderError> {
        if !self.config.vector_search_enabled {
            return Ok(Vec::new());
        }

        let query_embedding = self
            .embedder
            .embed_one(query)
            .map_err(|e| VectorProviderError::EmbeddingError(e.to_string()))?;

        let db = self.lancedb.read().await;

        let results = match category {
            Some(cat) => {
                db.search_with_filter(&query_embedding, Some(cat), limit)
                    .await?
            }
            None => db.search(&query_embedding, limit).await?,
        };

        Ok(results)
    }

    /// Full-text search via LanceDB
    pub async fn full_text_search(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, VectorProviderError> {
        if !self.config.vector_search_enabled {
            return Ok(Vec::new());
        }

        let db = self.lancedb.read().await;
        let results = db.fts_search(query, category, limit).await?;
        Ok(results)
    }

    /// Hybrid search combining vector and FTS with RRF fusion
    pub async fn hybrid_search(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, VectorProviderError> {
        if !self.config.vector_search_enabled {
            return self.full_text_search(query, category, limit).await;
        }

        let query_embedding = self
            .embedder
            .embed_one(query)
            .map_err(|e| VectorProviderError::EmbeddingError(e.to_string()))?;

        let db = self.lancedb.read().await;

        // Parallel search
        let (vec_results, fts_results) = tokio::join!(
            db.search_with_filter(&query_embedding, category, limit),
            db.fts_search(query, category, limit),
        );

        let vec_results = vec_results?;
        let fts_results = fts_results?;

        // Combine and RRF fusion with k=60
        let mut combined = Vec::with_capacity(vec_results.len() + fts_results.len());
        combined.extend(vec_results);
        combined.extend(fts_results);
        let fused = rrf_fusion(combined, 60);

        Ok(fused.into_iter().take(limit).collect())
    }

    /// Return the embedding dimension
    pub fn dimension(&self) -> usize {
        self.embedder.dimension()
    }
}

/// Convert a ScoredMemory from LanceDB to a MemoryEntry
fn scored_to_entry(scored: &ScoredMemory) -> MemoryEntry {
    let category = match scored.category.as_str() {
        "core" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
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
    }
}

/// MemoryProvider trait implementation for VectorMemoryProvider
///
/// Note: This implementation stores data only in LanceDB (vector store).
/// A production setup would dual-write to both LanceDB and SQLite
/// for hybrid search + structured metadata.
#[async_trait::async_trait]
impl MemoryProvider for VectorMemoryProvider {
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
        };

        let db = self.lancedb.read().await;
        db.insert(&entry, &embedding)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb insert failed: {e}")))?;

        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let scored = self
            .hybrid_search(query, category, limit)
            .await
            .map_err(|e| MemoryError::Generic(format!("hybrid search failed: {e}")))?;

        Ok(scored.iter().map(scored_to_entry).collect())
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let db = self.lancedb.read().await;
        db.delete(key)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb delete failed: {e}")))?;
        Ok(())
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        // LanceDB doesn't have a bulk delete by filter in the simple API.
        // We retrieve all entries and delete matching ones.
        let all_results = self
            .hybrid_search("", Some(category), usize::MAX)
            .await
            .map_err(|e| MemoryError::Generic(format!("search for purge failed: {e}")))?;

        let db = self.lancedb.read().await;
        for scored in &all_results {
            db.delete(&scored.key)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb delete during purge: {e}")))?;
        }

        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        // Export all entries, optionally filtered by category
        let all_results = self
            .hybrid_search("", category, usize::MAX)
            .await
            .map_err(|e| MemoryError::Generic(format!("search for export failed: {e}")))?;

        Ok(all_results.iter().map(scored_to_entry).collect())
    }
}

/// RRF fusion across multiple result lists
fn rrf_fusion(results: Vec<ScoredMemory>, k: u32) -> Vec<ScoredMemory> {
    use std::collections::HashMap;

    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut content_map: HashMap<String, (String, String)> = HashMap::new();

    for (rank, item) in results.iter().enumerate() {
        let score = 1.0 / (k as f32 + rank as f32);
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
        content_map
            .entry(item.key.clone())
            .or_insert_with(|| (item.content.clone(), item.category.clone()));
    }

    let mut fused: Vec<ScoredMemory> = scores
        .into_iter()
        .map(|(key, score)| {
            let (content, category) = content_map
                .remove(&key)
                .unwrap_or((String::new(), String::new()));
            ScoredMemory {
                key,
                content,
                category,
                score,
            }
        })
        .collect();

    fused.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rrf_fusion_deduplicates() {
        let list = vec![
            ScoredMemory {
                key: "a".to_string(),
                content: "A".to_string(),
                category: "core".to_string(),
                score: 0.1,
            },
            ScoredMemory {
                key: "b".to_string(),
                content: "B".to_string(),
                category: "core".to_string(),
                score: 0.05,
            },
            ScoredMemory {
                key: "a".to_string(),
                content: "A2".to_string(),
                category: "core".to_string(),
                score: 0.2,
            },
            ScoredMemory {
                key: "c".to_string(),
                content: "C".to_string(),
                category: "core".to_string(),
                score: 0.15,
            },
        ];

        let fused = rrf_fusion(list, 60);
        // "a" appears twice in the list, should be deduplicated
        assert_eq!(fused.len(), 3);
        // "a" should have the highest score (from 2 positions)
        assert_eq!(fused[0].key, "a");
    }

    #[test]
    fn config_default_path() {
        let config = VectorProviderConfig::default();
        assert!(config.vector_search_enabled);
        assert!(config.db_path.ends_with("vector_db"));
    }
}
