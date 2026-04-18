//! Memory module — provides long-term memory storage and retrieval
//!
//! This module defines the MemoryProvider trait and related types for
//! storing and retrieving persistent memory across sessions.
//!
//! # Providers
//!
//! - [`SqliteMemoryProvider`] — SQLite-backed (default, persistent)
//! - [`InMemoryMemoryProvider`] — In-memory (deprecated, for tests only)
//! - [`VectorMemoryProvider`] — Vector-backed (FastEmbed + LanceDB, P1)

pub mod audit;
pub mod compat;
pub mod embedding;
pub mod hrr;
pub mod inject;
pub mod intent;
pub mod job_runner;
pub mod llm;
pub mod pinned;
pub mod policy;
pub mod promotion;
mod providers;
pub mod retrieval;
pub mod scope;
pub mod security;
pub mod summary;
pub mod working_memory;

// Re-exports for the JobRunner module.  `JobAttempt` / `JobStatus` /
// `JobError` are part of the surface consumed by 8A.7+ slices
// (rolling summary, compile, fact extract) and the future
// MemoryJobsStatusCard UI; tagged `allow(unused_imports)` while those
// callers land in subsequent slices so the bin build stays warning-free.
#[allow(unused_imports)]
pub use job_runner::{JobAttempt, JobError, JobRunner, JobStatus};
// Phase 8A.5 — UtilityLlm shim is the only LLM seam visible to memory
// subsystems (v2 §0.5 Δ-1).  Producers (rolling summary, compile,
// extractor, diary) land in 8A.6+; tagged `allow(unused_imports)` until
// then so the bin build stays warning-free.
#[allow(unused_imports)]
pub use llm::{MockUtilityLlm, ProviderUtilityLlm, UtilityLlm};
pub use providers::{SqliteMemoryProvider, VectorMemoryProvider, VectorProviderConfig};
// Phase 8A.5 — session summary store (T-B1).  Consumers land in 8A.6
// (RollingSummaryPrompt) and 8A.7 (RollingSummarizer); held on
// `AppState` from this slice so subsequent slices only need to read
// `state.summary_store`.
#[allow(unused_imports)]
pub use summary::{
    NullSessionSummaryStore, SessionSummaryRecord, SessionSummaryStore, SqliteSessionSummaryStore,
    SummarySource,
};
// Phase 8A.9 — pinned-memory subsystem (T-F1).  First production
// consumer (pin_memory tool) lands in 8A.10; held on `AppState` from
// this slice so subsequent slices only need to read
// `state.pinned_store`.
#[allow(unused_imports)]
pub use pinned::{
    NullPinnedStore, PinScope, PinSource, PinnedItem, PinnedStore, SqlitePinnedStore,
    MAX_PINS_PER_SCOPE, MAX_PIN_CONTENT_CHARS,
};
// Phase 8A.11 — system-prompt memory injection (T-F4).  First
// production wiring (commands/agent.rs un-stash) lands in 8A.12 or a
// subsequent slice; tagged `allow(unused_imports)` until then so the
// bin build stays warning-free.
#[allow(unused_imports)]
pub use inject::{build_memory_injection, MemoryInjection, CHARS_PER_TOKEN_ESTIMATE};
// MemoryExecutionScope is part of the trait surface; MemoryScopeResolver is imported
// directly from scope:: by callers (tools), so only re-export the type needed for signatures.
pub use scope::MemoryExecutionScope;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory entry stored in the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Importance score (0.0 - 1.0), used for eviction prioritization
    pub importance: f64,
    /// Number of times this entry has been accessed
    pub access_count: u64,
    /// Trust score (-1.0 to 1.0), adjusted by feedback
    pub trust_score: f64,
    /// Optional session identifier for scope isolation.
    ///
    /// When `Some`, this entry is only visible to callers with matching session scope.
    /// `None` means the entry is globally visible (pre-scope legacy entries).
    pub session_id: Option<String>,
    /// Optional project identifier for scope isolation.
    ///
    /// When `Some`, this entry is only visible to callers with matching project scope.
    pub project_id: Option<String>,
}

/// Memory category for organizing memory entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    Custom(String),
}

impl MemoryCategory {
    /// Convert the category to its string representation.
    pub fn as_str(&self) -> &str {
        match self {
            MemoryCategory::Core => "core",
            MemoryCategory::Daily => "daily",
            MemoryCategory::Conversation => "conversation",
            MemoryCategory::Custom(s) => s,
        }
    }
}

/// Error type for memory operations
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum MemoryError {
    #[error("memory error: {0}")]
    Generic(String),
    #[error("key not found: {0}")]
    KeyNotFound(String),
    #[error("category not found: {0}")]
    CategoryNotFound(String),
    /// Phase 8A.9 — emitted by [`pinned::store::PinnedStore::add`] when the
    /// caller would exceed the per-scope pin cap (`MAX_PINS_PER_SCOPE`,
    /// v2 §0.5 Δ-17).  The `usize` is the cap that was hit.
    #[error("pinned items reached the per-scope limit ({0})")]
    PinnedLimitExceeded(usize),
    /// Phase 8A.9 — emitted by [`pinned::store::PinnedStore::add`] when the
    /// caller's content exceeds `MAX_PIN_CONTENT_CHARS` (v2 §0.5 Δ-17).
    /// `got` is the actual char count, `limit` is the cap.
    #[error("pinned content exceeds per-pin limit ({limit} chars; got {got})")]
    PinnedContentTooLong { got: usize, limit: usize },
}

impl From<rusqlite::Error> for MemoryError {
    fn from(e: rusqlite::Error) -> Self {
        MemoryError::Generic(e.to_string())
    }
}

/// Three-tier scope visibility check used by the default `export_scoped`
/// implementation and the `commands::memory` filter path.
///
/// Mirrors the SQL `WHERE` rules in `SqliteMemoryProvider::recall_scoped`:
/// - `session+project` → own session OR project-level OR global.
/// - `session-only`    → own session OR global.
/// - `project-only`    → project-level OR global.
/// - `global`          → only truly unscoped entries.
#[must_use]
pub(crate) fn entry_matches_scope_default(
    entry: &MemoryEntry,
    scope: &MemoryExecutionScope,
) -> bool {
    match (&scope.session_id, &scope.project_id) {
        (Some(s), Some(p)) => {
            entry.session_id.as_deref() == Some(s.as_str())
                || (entry.session_id.is_none() && entry.project_id.as_deref() == Some(p.as_str()))
                || (entry.session_id.is_none() && entry.project_id.is_none())
        }
        (Some(s), None) => {
            entry.session_id.as_deref() == Some(s.as_str())
                || (entry.session_id.is_none() && entry.project_id.is_none())
        }
        (None, Some(p)) => {
            (entry.session_id.is_none() && entry.project_id.as_deref() == Some(p.as_str()))
                || (entry.session_id.is_none() && entry.project_id.is_none())
        }
        (None, None) => entry.session_id.is_none() && entry.project_id.is_none(),
    }
}

/// Trait for memory storage providers
/// Implement this trait to provide different storage backends (in-memory, file-based, etc.)
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// Store a memory entry
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError>;

    /// Recall memory entries matching a query
    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Delete a specific memory entry by key
    async fn delete(&self, key: &str) -> Result<(), MemoryError>;

    /// Purge all entries in a category
    async fn purge_category(&self, category: &str) -> Result<(), MemoryError>;

    /// Wipe **every** memory entry across all categories and scopes.
    ///
    /// Backs the "Clear all memories" affordance in the Memory Settings
    /// page.  This is intentionally distinct from
    /// [`Self::purge_category`]: it ignores category boundaries and is
    /// expected to be a single fast bulk delete on backends that support
    /// it (SQLite TRUNCATE-equivalent, LanceDB drop-rows).
    ///
    /// The default implementation provides a slow but safe fallback by
    /// fanning out to [`Self::export`] and deleting each entry one at a
    /// time, so providers that don't override it still behave correctly.
    /// Returns the number of rows that were removed.
    async fn clear_all(&self) -> Result<usize, MemoryError> {
        let entries = self.export(None).await?;
        let mut removed = 0usize;
        for entry in &entries {
            // Best-effort: skip rather than abort the whole operation if
            // a single key has already been removed concurrently.
            match self.delete(&entry.key).await {
                Ok(()) => removed += 1,
                Err(MemoryError::KeyNotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(removed)
    }

    /// Export entries, optionally filtered by category
    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Store a memory entry bound to a specific execution scope.
    ///
    /// When `scope.session_id` or `scope.project_id` is `Some`, the entry is tagged
    /// with those identifiers so that future `recall_scoped` calls can filter by scope.
    ///
    /// The default implementation ignores scope and delegates to `store()` for
    /// backward-compatible providers. Override in concrete implementations to persist
    /// scope metadata.
    async fn store_scoped(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        let _ = scope; // scope is ignored by the default no-op delegation
        self.store(key, content, category).await
    }

    /// Recall memory entries within a specific execution scope.
    ///
    /// When the scope has a non-`None` `session_id` or `project_id`, results are
    /// filtered to entries that either match the scope or were stored without scope
    /// (global entries). Global entries are always visible to all scopes.
    ///
    /// The default implementation ignores scope and delegates to `recall()` for
    /// backward-compatible providers. Override in concrete implementations to apply
    /// actual scope filtering.
    async fn recall_scoped(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let _ = scope; // scope is ignored by the default no-op delegation
        self.recall(query, category, limit).await
    }

    /// Export memory entries within a specific execution scope.
    ///
    /// Same three-tier visibility rules as `recall_scoped` but without a query
    /// filter — used by the Memory Browser export / scoped-listing surface.
    /// Override this in concrete providers to push the scope filter down to
    /// the storage engine; the default fans out to `export()` plus an
    /// in-memory filter for backward-compat.
    async fn export_scoped(
        &self,
        category: Option<&str>,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.export(category).await?;
        Ok(entries
            .into_iter()
            .filter(|e| entry_matches_scope_default(e, scope))
            .collect())
    }

    /// Promote (or demote) an existing entry to a different scope.
    ///
    /// Used by the memory-promotion pipeline (`MemoryPromotionEngine`) to move
    /// an entry from `session` → `project` or `project` → `global`.  The
    /// default implementation is a no-op for backward compatibility — override
    /// in concrete providers that support in-place scope updates.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError::KeyNotFound`] when no entry with `key` exists.
    async fn promote_scope(
        &self,
        key: &str,
        target_scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        let _ = (key, target_scope);
        Err(MemoryError::Generic(
            "promote_scope not supported by this provider".to_string(),
        ))
    }

    /// Reverse of [`Self::promote_scope`] — narrow an entry's visibility back
    /// down (e.g. `global → project`, `project → session`) so a mistaken
    /// promotion can be rolled back.  At the storage layer this writes the
    /// same `session_id` / `project_id` columns; the distinct method exists
    /// so call sites and audit listeners can attribute "demote" intent
    /// separately from "promote".
    ///
    /// The default implementation delegates to `promote_scope`, since both
    /// operations resolve to the same UPDATE.  Providers that want different
    /// semantics (e.g. soft-archive on demote) can override this hook.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError::KeyNotFound`] when no entry with `key` exists.
    async fn demote_scope(
        &self,
        key: &str,
        target_scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        self.promote_scope(key, target_scope).await
    }

    /// Apply Weibull importance decay to all stored entries.
    ///
    /// Each entry's `importance` is multiplied by the Weibull survival factor
    /// `exp(-(age_hours / lambda)^k)`, where age is measured from `created_at`.
    /// Entries are updated in-place in the backing store.
    ///
    /// The default implementation is a no-op so that providers that do not yet
    /// support importance write-back remain unaffected. Override this method in
    /// concrete providers to enable real decay.
    ///
    /// # Parameters
    /// - `lambda_hours`: Weibull scale (hours). Default in `WeibullDecay` is 168 (7 days).
    /// - `k`: Weibull shape. Default is 1.2.
    async fn apply_importance_decay(
        &self,
        lambda_hours: f32,
        k: f32,
    ) -> Result<usize, MemoryError> {
        // Silence "unused variable" warnings for the default no-op.
        let _ = (lambda_hours, k);
        Ok(0)
    }
}

/// In-memory implementation of MemoryProvider
///
/// **Deprecated**: This provider loses all data on process restart.
/// Use [`SqliteMemoryProvider`] instead for production use.
/// This is only intended for testing.
#[deprecated(
    since = "0.1.0",
    note = "Use SqliteMemoryProvider instead — InMemoryMemoryProvider loses all data on restart"
)]
#[derive(Debug, Default)]
pub struct InMemoryMemoryProvider {
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

#[allow(deprecated)]
impl InMemoryMemoryProvider {
    /// Create a new in-memory memory provider.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

#[allow(deprecated)]
#[async_trait]
impl MemoryProvider for InMemoryMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        let now = chrono::Utc::now();
        let entry = MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: category.clone(),
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        };
        let mut entries = self.entries.write().await;
        entries.insert(key.to_string(), entry);
        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.entries.read().await;
        let query_lower = query.to_lowercase();

        let filtered: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| {
                // Check category filter
                let category_match = category.map(|c| e.category.as_str() == c).unwrap_or(true);
                // Check query match (in key or content)
                let query_match = query.is_empty()
                    || e.key.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower);
                category_match && query_match
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let mut entries = self.entries.write().await;
        if entries.remove(key).is_none() {
            return Err(MemoryError::KeyNotFound(key.to_string()));
        }
        Ok(())
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        let mut entries = self.entries.write().await;
        entries.retain(|_, e| e.category.as_str() != category);
        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.entries.read().await;
        let filtered: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| category.map(|c| e.category.as_str() == c).unwrap_or(true))
            .cloned()
            .collect();
        Ok(filtered)
    }
}

/// Global memory provider instance
pub type SharedMemoryProvider = Arc<dyn MemoryProvider>;

/// Default SQLite memory provider
///
/// Creates a persistent provider backed by `~/.if2ai/memory/memory.db`.
/// Data survives process restarts.
#[allow(dead_code)]
pub async fn default_memory_provider() -> SharedMemoryProvider {
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("memory")
        .join("memory.db");

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match SqliteMemoryProvider::new(db_path) {
        Ok(provider) => Arc::new(provider),
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create SqliteMemoryProvider: {e}, falling back to in-memory"
            );
            #[allow(deprecated)]
            let fallback = InMemoryMemoryProvider::new();
            Arc::new(fallback)
        }
    }
}
