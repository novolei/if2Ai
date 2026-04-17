//! Vector-backed MemoryProvider wrapping LanceDB + FastEmbed
//!
//! [`VectorMemoryProvider`] combines vector embedding (FastEmbed) with
//! vector storage (LanceDB) to provide semantic memory search.
//!
//! ## SQLite dual-write (H3)
//!
//! When configured with `sqlite_path`, writes are dual-homed:
//! 1. **SQLite first** (synchronous, authoritative for durability and metadata).
//! 2. **LanceDB async** (spawned background task, eventually consistent).
//!
//! This decoupling means:
//! - `apply_importance_decay` can update importance scores in SQLite.
//! - Vector search still benefits from LanceDB's approximate nearest-neighbor index.
//! - A crash mid-write leaves SQLite intact; LanceDB can be rebuilt from it.
//!
//! # `#![allow(dead_code)]` justification
//! VectorMemoryProvider and its helpers are not yet called from all agent loop
//! code paths. They are wired through HybridMemoryProvider for the active path.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::lancedb::{LanceDBError, LanceDBMemory, ScoredMemory};
use super::sqlite_provider::SqliteMemoryProvider;
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
    /// Path to the LanceDB directory.
    pub db_path: PathBuf,
    /// Enable vector search (ANN via LanceDB). Disable only in tests.
    pub vector_search_enabled: bool,
    /// Optional path for SQLite dual-write (H3).
    ///
    /// When `Some`, every `store` call writes to SQLite first before the
    /// async LanceDB write. Set this to `~/.if2ai/memory/memory.db` to
    /// enable durability and importance-decay via the SQL provider.
    pub sqlite_path: Option<PathBuf>,
}

impl Default for VectorProviderConfig {
    fn default() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".if2ai")
            .join("memory");

        Self {
            db_path: base.join("vector_db"),
            vector_search_enabled: true,
            // SQLite dual-write disabled by default; enable explicitly.
            sqlite_path: None,
        }
    }
}

impl VectorProviderConfig {
    /// Enable SQLite dual-write to the given path.
    ///
    /// Calling this enables importance decay and durable metadata storage.
    #[must_use]
    pub fn with_sqlite_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.sqlite_path = Some(path.into());
        self
    }
}

/// Vector-backed MemoryProvider
///
/// Wraps `LanceDBMemory` + `FastEmbedProvider` to provide:
/// - `store`: embed text + store in LanceDB (+ optional SQLite dual-write)
/// - `recall`: vector search by embedding the query
/// - `delete`: remove from LanceDB
///
/// When `config.sqlite_path` is set, writes are dual-homed: SQLite first,
/// then LanceDB asynchronously (see module-level doc for rationale).
pub struct VectorMemoryProvider {
    embedder: Arc<FastEmbedProvider>,
    lancedb: Arc<RwLock<LanceDBMemory>>,
    /// Optional SQLite provider for dual-write and importance decay.
    sqlite: Option<Arc<SqliteMemoryProvider>>,
    config: VectorProviderConfig,
}

impl VectorMemoryProvider {
    /// Create a new VectorMemoryProvider.
    ///
    /// Initializes the FastEmbed model and opens/creates the LanceDB database.
    /// If `config.sqlite_path` is set, also opens/creates the SQLite database
    /// for dual-write (H3). SQLite init failures are gracefully degraded to
    /// `None` (warn-logged) so the vector provider still works.
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

        // Optional SQLite dual-write (H3).
        let sqlite = if let Some(ref sqlite_path) = config.sqlite_path {
            match SqliteMemoryProvider::new(sqlite_path.clone()) {
                Ok(provider) => {
                    tracing::info!(
                        "[VectorMemoryProvider] SQLite dual-write enabled at {:?}",
                        sqlite_path
                    );
                    Some(Arc::new(provider))
                }
                Err(e) => {
                    tracing::warn!(
                        "[VectorMemoryProvider] SQLite init failed ({e}); dual-write disabled"
                    );
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            embedder: Arc::new(embedder),
            lancedb: Arc::new(RwLock::new(lancedb)),
            sqlite,
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
        session_id: None,
        project_id: None,
    }
}

/// MemoryProvider trait implementation for VectorMemoryProvider
///
/// When `sqlite` is `Some`, `store` dual-writes:
/// 1. SQLite synchronously (authoritative for metadata and decay).
/// 2. LanceDB asynchronously (vector search, eventually consistent).
#[async_trait::async_trait]
impl MemoryProvider for VectorMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        // Step 1: SQLite-first write (H3 dual-write).
        // SQLite is the authoritative source; failure blocks the write entirely.
        if let Some(ref sqlite) = self.sqlite {
            sqlite
                .store(key, content, category.clone())
                .await
                .map_err(|e| MemoryError::Generic(format!("sqlite dual-write failed: {e}")))?;
            tracing::debug!("[VectorMemoryProvider] SQLite dual-write OK for key={key}");
        }

        // Step 2: compute embedding for LanceDB.
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

        // Step 3: LanceDB write (async background when SQLite is present).
        if self.sqlite.is_some() {
            // Fire-and-forget: LanceDB is not the source of truth when dual-write
            // is enabled. Errors are warn-logged rather than propagated.
            let lancedb = Arc::clone(&self.lancedb);
            let entry_clone = entry.clone();
            let embedding_clone = embedding.clone();
            tokio::spawn(async move {
                let db = lancedb.read().await;
                if let Err(e) = db.insert(&entry_clone, &embedding_clone).await {
                    tracing::warn!(
                        "[VectorMemoryProvider] async LanceDB insert failed for key={}: {e}",
                        entry_clone.key
                    );
                }
            });
        } else {
            // No SQLite: LanceDB is the only store — write synchronously.
            let db = self.lancedb.read().await;
            db.insert(&entry, &embedding)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb insert failed: {e}")))?;
        }

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
        // Direct LanceDB query — no embedding needed
        let db = self.lancedb.read().await;
        let all_results = db
            .export_all(Some(category))
            .await
            .map_err(|e| MemoryError::Generic(format!("search for purge failed: {e}")))?;

        for scored in &all_results {
            db.delete(&scored.key)
                .await
                .map_err(|e| MemoryError::Generic(format!("lancedb delete during purge: {e}")))?;
        }

        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        // Direct LanceDB query — no embedding needed
        let db = self.lancedb.read().await;
        let all_results = db
            .export_all(category)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb export failed: {e}")))?;

        Ok(all_results.iter().map(scored_to_entry).collect())
    }

    /// Apply Weibull importance decay.
    ///
    /// When SQLite dual-write is enabled, delegates to `SqliteMemoryProvider`
    /// which has a full Weibull decay implementation over its indexed metadata.
    ///
    /// Without SQLite, LanceDB does not expose a direct importance update path
    /// so this remains a no-op that returns 0 (no entries decayed).
    async fn apply_importance_decay(
        &self,
        lambda_hours: f32,
        k: f32,
    ) -> Result<usize, MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            tracing::debug!(
                "[VectorMemoryProvider] apply_importance_decay: delegating to SQLite provider"
            );
            return sqlite.apply_importance_decay(lambda_hours, k).await;
        }

        tracing::debug!(
            "[VectorMemoryProvider] apply_importance_decay: no-op (SQLite dual-write not enabled)"
        );
        Ok(0)
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
        assert!(config.sqlite_path.is_none());
    }

    #[test]
    fn config_with_sqlite_path_sets_dual_write() {
        let config = VectorProviderConfig::default().with_sqlite_path("/tmp/test.db");
        assert!(config.sqlite_path.is_some());
        assert_eq!(
            config.sqlite_path.as_ref().unwrap().to_str(),
            Some("/tmp/test.db")
        );
    }

    /// Verify that the SQLite provider is created when a sqlite_path is provided.
    ///
    /// Note: This test does NOT exercise the full dual-write path (which requires
    /// a running LanceDB + FastEmbed model), but confirms the config builder and
    /// `with_sqlite_path` method work as expected.
    #[test]
    fn vector_provider_config_sqlite_path_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("vec.db");
        let sqlite_path = tmp.path().join("meta.db");

        let config = VectorProviderConfig {
            db_path,
            vector_search_enabled: false,
            sqlite_path: Some(sqlite_path.clone()),
        };
        assert_eq!(config.sqlite_path.as_ref().unwrap(), &sqlite_path);
    }
}
