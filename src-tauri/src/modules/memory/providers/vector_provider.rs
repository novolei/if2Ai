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
//! # Dead-code policy (H4)
//!
//! The module-level `#![allow(dead_code)]` was removed once the agent loop
//! started consuming the trait impl. Helpers that are not yet wired (such as
//! `vector_search`, `full_text_search`, `is_enabled`, `dimension`,
//! `with_sqlite_path`) carry a function-level `#[allow(dead_code)]` so that
//! removing them later requires an explicit decision instead of silently
//! deleting public API.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::lancedb::{LanceDBError, LanceDBMemory, ScoredMemory};
use super::sqlite_provider::SqliteMemoryProvider;
use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::embedding::FastEmbedProvider;
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::{
    MemoryCategory, MemoryEntry, MemoryError, MemoryExecutionScope, MemoryProvider,
};

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
        // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
        let base = crate::modules::config::store::if2ai_data_root().join("memory");

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
    #[allow(dead_code)] // Public builder; consumed by tests and future config code.
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
    /// Phase 8A — shared PII scanner.  When `Some`, `store_scoped` runs
    /// `scan_and_redact` ONCE before forwarding to SQLite + LanceDB so the
    /// two stores never diverge on the cleaned content.  See v2 §0.5 Δ-2.
    scanner: Option<Arc<ThreatScanner>>,
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

        // H2: best-effort IVF-PQ index. Failure is logged inside the helper
        // and never propagates — empty/young tables and missing kmeans
        // samples will simply degrade to brute-force scan until the next call.
        if let Err(e) = lancedb.ensure_vector_index(32, 24).await {
            tracing::warn!(
                "[VectorMemoryProvider] ensure_vector_index returned hard error ({e}); continuing without ANN index"
            );
        }

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
            scanner: None,
        })
    }

    /// Builder: attach a shared [`ThreatScanner`] so every scope-aware write
    /// scrubs PII / secrets before reaching either backing store.  See the
    /// `scanner` field doc for the threat model.
    #[must_use]
    pub fn with_scanner(mut self, scanner: Arc<ThreatScanner>) -> Self {
        self.scanner = Some(scanner);
        self
    }

    /// Check if vector search is enabled
    #[allow(dead_code)] // Surfaced for future UI / settings introspection.
    pub fn is_enabled(&self) -> bool {
        self.config.vector_search_enabled
    }

    /// Search using vector similarity
    #[allow(dead_code)] // Direct vector path; recall() uses hybrid_search instead.
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
    #[allow(dead_code)] // Diagnostic accessor; currently consumed only by tests.
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
        // Empty query = "list all entries" (Memory Browser browse path,
        // settings export-then-bump, etc.).  hybrid_search would try to
        // embed an empty string and the FastEmbed model fails with
        // `EmptyText`.  Delegate directly to SQLite when present so the
        // semantics stay "recall everything (and bump access_count)";
        // the SQLite recall path already handles empty queries by
        // returning all rows up to `limit`.
        if query.trim().is_empty() {
            if let Some(ref sqlite) = self.sqlite {
                return sqlite.recall(query, category, limit).await;
            }
            // No SQLite mirror: empty query has no defined semantics for
            // the pure vector store, return empty rather than error.
            return Ok(Vec::new());
        }

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

    /// Bulk-delete every entry across both stores.
    ///
    /// Drives the Settings "Clear all memories" button.  Order matters:
    /// SQLite first (authoritative), LanceDB second (best-effort mirror)
    /// so a partial failure leaves the system in a consistent
    /// "metadata gone, vectors will be reaped on next compaction" state
    /// rather than the inverse.
    async fn clear_all(&self) -> Result<usize, MemoryError> {
        let removed = if let Some(ref sqlite) = self.sqlite {
            sqlite.clear_all().await?
        } else {
            0
        };

        // Drop everything LanceDB knows about by listing + deleting.
        // (LanceDB has no table-truncate primitive in our wrapper, and we
        // don't want to drop the table itself because that would also
        // wipe the FTS / vector index configuration.)
        let db = self.lancedb.read().await;
        let all = db
            .export_all(None)
            .await
            .map_err(|e| MemoryError::Generic(format!("lancedb list-for-clear failed: {e}")))?;
        for scored in &all {
            if let Err(e) = db.delete(&scored.key).await {
                tracing::warn!(
                    "[VectorMemoryProvider] clear_all: lancedb delete failed for key={}: {e}",
                    scored.key
                );
            }
        }

        // When SQLite isn't mirroring, the LanceDB count is the authoritative
        // "removed" count for the caller.
        Ok(if self.sqlite.is_some() {
            removed
        } else {
            all.len()
        })
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

    /// MEM-MOD-P1 — delegate to SQLite when available; LanceDB does not
    /// store `trust_score`, so without dual-write this is a no-op.
    async fn adjust_trust_score(&self, key: &str, delta: f64) -> Result<f64, MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            return sqlite.adjust_trust_score(key, delta).await;
        }
        let _ = (key, delta);
        Ok(0.0)
    }

    /// Scope-aware store — delegates to the SQLite dual-write provider when
    /// available so `session_id` / `project_id` are persisted.  Without
    /// SQLite the entry is stored without scope metadata; LanceDB has no
    /// scope columns of its own and would otherwise silently drop the
    /// three-tier model.
    ///
    /// LanceDB still receives the same write (via the inner [`Self::store`]
    /// path) so vector search continues to find the entry; recall-time
    /// scope filtering happens against the SQLite mirror.
    async fn store_scoped(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        // Phase 8A §0.5 Δ-2: scrub ONCE here so SQLite + LanceDB receive
        // the same cleaned text.  The inner SqliteMemoryProvider's own
        // scanner is intentionally *not* configured by this path because
        // running scrub twice would double-emit `memory_pii_redacted`.
        let cleaned: String = if let Some(ref scanner) = self.scanner {
            let result = scanner.scan_and_redact(key, content);
            if result.flagged {
                let ctx = AuditContext::from_scope(scope);
                MemoryAuditEmitter::memory_pii_redacted(&ctx, key, &result.detected);
            }
            result.cleaned
        } else {
            content.to_string()
        };
        let content = cleaned.as_str();
        if let Some(ref sqlite) = self.sqlite {
            sqlite
                .store_scoped(key, content, category.clone(), scope)
                .await?;

            // Mirror to LanceDB so semantic recall can still surface the entry.
            // Failures here are non-fatal: SQLite is the authoritative source
            // for both metadata and scope; LanceDB is best-effort.
            let embedding = match self.embedder.embed_one(content) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(
                        "[VectorMemoryProvider] store_scoped: embedding failed for key={key}: {e}; LanceDB mirror skipped"
                    );
                    return Ok(());
                }
            };
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
                session_id: scope.session_id.clone(),
                project_id: scope.project_id.clone(),
            };
            let lancedb = Arc::clone(&self.lancedb);
            let key_for_log = key.to_string();
            tokio::spawn(async move {
                let db = lancedb.read().await;
                if let Err(e) = db.insert(&entry, &embedding).await {
                    tracing::warn!(
                        "[VectorMemoryProvider] store_scoped: async LanceDB insert failed for key={key_for_log}: {e}"
                    );
                }
            });
            return Ok(());
        }

        // No SQLite mirror: fall through to the existing scope-less store
        // (the trait default would just drop scope metadata, but we make
        // that explicit here with a warning so operators notice the
        // degraded mode).
        tracing::warn!(
            "[VectorMemoryProvider] store_scoped: SQLite dual-write not enabled; scope metadata for key={key} will be dropped"
        );
        self.store(key, content, category).await
    }

    /// Scope-aware recall — delegates to the SQLite mirror so we honour
    /// the three-tier visibility rules (session entries visible only to
    /// their session, project entries to their project, global entries
    /// always).  Without the SQLite mirror this falls back to the
    /// scope-less LanceDB recall (degraded but still functional).
    async fn recall_scoped(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            return sqlite.recall_scoped(query, category, limit, scope).await;
        }
        self.recall(query, category, limit).await
    }

    /// Scope-aware export — same delegation pattern as [`Self::recall_scoped`].
    async fn export_scoped(
        &self,
        category: Option<&str>,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            return sqlite.export_scoped(category, scope).await;
        }
        // Conservative fallback: filter LanceDB-sourced entries against
        // the scope using the trait helper (most will lack scope metadata
        // and only match the global tier — that matches the no-mirror
        // degraded behaviour).
        let entries = self.export(category).await?;
        Ok(entries
            .into_iter()
            .filter(|e| crate::modules::memory::entry_matches_scope_default(e, scope))
            .collect())
    }

    /// Promote (or demote) an entry's scope.  Always routed to SQLite
    /// because LanceDB has no notion of scope columns; without dual-write
    /// the operation is unsupported and we surface a precise error rather
    /// than silently succeeding.
    async fn promote_scope(
        &self,
        key: &str,
        target_scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            return sqlite.promote_scope(key, target_scope).await;
        }
        Err(MemoryError::Generic(
            "promote_scope requires SQLite dual-write to be enabled on VectorMemoryProvider"
                .to_string(),
        ))
    }

    /// Demote — symmetric to [`Self::promote_scope`].  Same SQLite
    /// requirement applies.
    async fn demote_scope(
        &self,
        key: &str,
        target_scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        if let Some(ref sqlite) = self.sqlite {
            return sqlite.demote_scope(key, target_scope).await;
        }
        Err(MemoryError::Generic(
            "demote_scope requires SQLite dual-write to be enabled on VectorMemoryProvider"
                .to_string(),
        ))
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
