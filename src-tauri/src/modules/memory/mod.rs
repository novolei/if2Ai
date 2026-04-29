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
pub mod compiler;
pub mod conversation_recall_vector;
pub mod decision_tree;
pub mod embedding;
pub mod hrr;
pub mod inject;
pub mod intent;
pub mod job_runner;
pub mod learned_traits;
pub mod llm;
pub mod migrations;
pub mod pinned;
pub mod policy;
pub mod promotion;
mod providers;
pub mod reflection_loop;
pub mod retrieval;
pub mod scope;
pub mod security;
pub mod summary;
pub mod ticker;
pub mod verification_gate;
pub mod working_memory;

// JobRunner module — `JobRunner` itself is consumed by scheduler /
// bootstrap; the `JobAttempt` / `JobError` / `JobStatus` enums are
// part of the public job-status surface but no bin caller imports
// them directly (they appear in trait signatures + tests only).
pub use job_runner::JobRunner;
#[allow(unused_imports)]
pub use job_runner::{JobAttempt, JobError, JobStatus};

// UtilityLlm shim — `UtilityLlm` trait + `MockUtilityLlm` are
// consumed widely; `ProviderUtilityLlm` is the production
// implementation held on AppState, which the bin reaches for via
// the deep path (`llm::ProviderUtilityLlm::new`) so this re-export
// is for external consumers only.
#[allow(unused_imports)]
pub use llm::ProviderUtilityLlm;
#[allow(unused_imports)]
pub use llm::{ChatProviderUtilityLlm, MockUtilityLlm, UtilityLlm};

pub use providers::{SqliteMemoryProvider, VectorMemoryProvider, VectorProviderConfig};

// Session summary store — `SessionSummaryStore` trait + the two
// implementations are wired into AppState. `SessionSummaryRecord`
// and `SummarySource` are part of the public schema surface; bin
// consumers reach for them via the deeper path
// (`summary::schema::*`) so the top-level re-export is for external
// users / tests.
pub use summary::{NullSessionSummaryStore, SessionSummaryStore, SqliteSessionSummaryStore};
#[allow(unused_imports)]
pub use summary::{SessionSummaryRecord, SummarySource};

// Pinned-memory subsystem — `PinnedStore` trait + impls are used.
// The supporting DTOs (`PinScope` / `PinSource` / `PinnedItem` /
// the two cap consts) are public for embedders + tests; bin code
// reaches for them via `pinned::types::*` directly.
pub use pinned::{NullPinnedStore, PinnedStore, SqlitePinnedStore};
#[allow(unused_imports)]
pub use pinned::{PinScope, PinSource, PinnedItem, MAX_PINS_PER_SCOPE, MAX_PIN_CONTENT_CHARS};

// System-prompt memory injection — `build_memory_injection` and
// `MemoryInjection` are consumed by application::memory_injection_service.
// `CHARS_PER_TOKEN_ESTIMATE` is part of the public surface for the
// occasional ad-hoc consumer (tests, external embedders), so we keep
// it exported even though the bin doesn't reach for it directly.
#[allow(unused_imports)]
pub use inject::CHARS_PER_TOKEN_ESTIMATE;
pub use inject::{build_memory_injection, MemoryInjection};

// MemoryExecutionScope is part of the trait surface; MemoryScopeResolver is imported
// directly from scope:: by callers (tools), so only re-export the type needed for signatures.
pub use scope::MemoryExecutionScope;

// MemoryCompiler — held on AppState; CompilePaths / CompileResult
// are reached for directly via the deep path by commands/memory/compile.rs,
// but kept re-exported here so external consumers don't need to know
// the submodule layout.
pub use compiler::MemoryCompiler;
#[allow(unused_imports)]
pub use compiler::{CompilePaths, CompileResult, SkipReason};

// MemoryTicker — wired to AppState and triggered from runtime turn
// hooks. `DailyStep` / `TickerState` are part of the public surface
// for diagnostics consumers (TickerStatusCard etc) but no current bin
// caller imports them — keep re-exported, allow unused.
#[allow(unused_imports)]
pub use ticker::{DailyStep, TickerState};
pub use ticker::{MemoryTicker, TickerConfig};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// MEM-MOD-P6 — One historical snapshot of a memory entry as captured
/// just before an UPDATE / DELETE / consolidate operation.  Mirrors the
/// `memory_entry_history` SQLite row.  Newest snapshot first when
/// returned by [`MemoryProvider::list_history`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHistoryEntry {
    pub key: String,
    pub content: String,
    pub category: String,
    pub importance: f64,
    pub trust_score: f64,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: chrono::DateTime<chrono::Utc>,
    pub source: String,
}

/// Memory category for organizing memory entries.
///
/// MEM-MOD-P2 added three new built-in categories so the agent has a
/// shared vocabulary for the four memory facets the post-Letta /
/// post-Mem0 designs converge on:
///
/// | category       | lifetime              | examples                            |
/// |----------------|-----------------------|-------------------------------------|
/// | `core`         | persistent identity   | "我叫 RL", "I prefer concise replies" |
/// | `daily`        | logical-day rollups   | today's compile target              |
/// | `conversation` | per-turn scratch      | last user query, intermediate tool out |
/// | `working`      | active-task scratch   | "currently refactoring foo.rs"       |
/// | `procedural`   | how-to recipes        | "to deploy: bun build && rsync ..."  |
/// | `reflection`   | meta-observations    | "user dislikes follow-up questions"  |
/// | `custom`       | escape hatch          | anything else                        |
///
/// Adding a new variant ONLY requires a matching arm here + in
/// [`crate::modules::memory::providers::sqlite_provider::scope::parse_category`];
/// the storage layer treats the string opaquely so no schema migration
/// is needed (the column is already `TEXT`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    /// MEM-MOD-P2 — short-lived task context (cleared when task ends).
    Working,
    /// MEM-MOD-P2 — durable how-to recipes ("when X happens, do Y").
    Procedural,
    /// MEM-MOD-P2 — agent's meta-observations about the user / itself.
    Reflection,
    Custom(String),
}

impl MemoryCategory {
    /// Convert the category to its string representation.
    pub fn as_str(&self) -> &str {
        match self {
            MemoryCategory::Core => "core",
            MemoryCategory::Daily => "daily",
            MemoryCategory::Conversation => "conversation",
            MemoryCategory::Working => "working",
            MemoryCategory::Procedural => "procedural",
            MemoryCategory::Reflection => "reflection",
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

/// Memory Audit P3 #14 — bridge `MemoryError` into the `Result<_, String>`
/// shape that all `#[tauri::command]` IPCs return.
///
/// Without this, every command that touches the memory provider has to
/// write `.map_err(|e| e.to_string())?` on every line. With it, `?`
/// propagates a `MemoryError` directly into the IPC error payload —
/// the `Display` impl from `thiserror::Error` produces the same
/// human-readable message that the old `.map_err` chain produced, so
/// the wire shape stays identical and existing frontend error handling
/// keeps working.
///
/// New code should prefer the `?`-only style; the old `.map_err` calls
/// can be migrated opportunistically without a flag day.
impl From<MemoryError> for String {
    fn from(e: MemoryError) -> Self {
        e.to_string()
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

    /// MEM-MOD-P3 — Fetch a single memory entry by exact key match. Returns
    /// `Ok(None)` when the key does not exist (preferred over `KeyNotFound`
    /// because callers commonly probe optional keys). Default implementation
    /// scans the result of an unscoped recall — providers with a primary-key
    /// index should override for O(1) lookup.
    async fn get_by_key(&self, key: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        let entries = self.recall(key, None, 50).await?;
        Ok(entries.into_iter().find(|e| e.key == key))
    }

    /// MEM-MOD-P3 — Replace the `content` of an existing entry, leaving
    /// `created_at`, `importance`, `access_count`, `trust_score` untouched.
    /// `updated_at` is set to `now`. Returns `KeyNotFound` if the key
    /// does not exist.
    ///
    /// Default impl is a no-op that returns `KeyNotFound` so providers
    /// that do not yet support partial updates remain compile-clean.
    async fn update_content(&self, key: &str, content: &str) -> Result<(), MemoryError> {
        let _ = (key, content);
        Err(MemoryError::KeyNotFound(
            "update_content not implemented for this provider".to_string(),
        ))
    }

    /// MEM-MOD-P3 — Create a typed link `source → target` (e.g.
    /// `"supersedes"`, `"evidence_for"`, `"contradicts"`).  Idempotent:
    /// the underlying SQLite table has a UNIQUE constraint on
    /// `(source_key, target_key, link_type)`.  Default impl is a no-op.
    async fn create_link(
        &self,
        source_key: &str,
        target_key: &str,
        link_type: &str,
    ) -> Result<(), MemoryError> {
        let _ = (source_key, target_key, link_type);
        Ok(())
    }

    /// MEM-MOD-P3 — Merge several existing entries into a new
    /// `consolidated_key` containing `consolidated_content`, then create
    /// `"consolidated_into"` links from each source key to the new entry.
    /// The source entries are NOT deleted (preserves provenance for P6
    /// temporal versioning); use `memory_forget` if true deletion is
    /// desired.  Returns the number of source links created.
    async fn consolidate(
        &self,
        source_keys: &[String],
        consolidated_key: &str,
        consolidated_content: &str,
        category: MemoryCategory,
    ) -> Result<usize, MemoryError> {
        let _ = (
            source_keys,
            consolidated_key,
            consolidated_content,
            category,
        );
        Ok(0)
    }

    /// MEM-MOD-P6 — Return the temporal history of a memory key in
    /// reverse-chronological order (newest snapshot first). Each entry
    /// is a `(content, category, valid_from, valid_to, source)` tuple.
    /// Default impl returns an empty list — override in providers that
    /// implement the v3 audit table.
    async fn list_history(&self, key: &str) -> Result<Vec<MemoryHistoryEntry>, MemoryError> {
        let _ = key;
        Ok(Vec::new())
    }

    /// MEM-MOD-P1 — Adjust the persisted `trust_score` of a memory entry by
    /// `delta`, clamping the resulting value to `[-1.0, 1.0]`. Returns the
    /// new `trust_score` so callers (LLM tools, audit emitter) can echo it
    /// back without an extra read.
    ///
    /// The default implementation is a no-op (returns `Ok(0.0)`) so
    /// in-memory / vector-only providers remain functional during tests.
    /// Production providers (SQLite) override this to persist the change.
    async fn adjust_trust_score(&self, key: &str, delta: f64) -> Result<f64, MemoryError> {
        let _ = (key, delta);
        Ok(0.0)
    }

    /// P1-7 — Persist one turn into conversation recall (FTS on SQLite).
    ///
    /// Default is a no-op so vector-only / hybrid Lance backends stay quiet.
    async fn conversation_recall_ingest(
        &self,
        _session_id: &str,
        _project_id: Option<&str>,
        _turn_id: &str,
        _user_message: &str,
        _assistant_excerpt: &str,
    ) -> Result<(), MemoryError> {
        Ok(())
    }

    /// P1-7 — Search prior turns in the session via FTS (vector hybrid TBD).
    async fn conversation_recall_search(
        &self,
        _query: &str,
        _session_id: &str,
        _project_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<String>, MemoryError> {
        Err(MemoryError::Generic(
            "conversation_recall_search is not available for this memory backend".to_string(),
        ))
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
    // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
    let db_path = crate::modules::config::store::if2ai_data_root()
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
