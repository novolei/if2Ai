//! `PinnedStore` — persistence + scrub coordinator for the
//! pinned-memory subsystem.
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-F1 + §0.5 Δ-2 + Δ-6 + Δ-17, every [`PinnedStore::add`]
//! is a 7-step pipeline:
//!
//! 1. **Length guard** — reject inputs over [`MAX_PIN_CONTENT_CHARS`].
//! 2. **PII scrub** — run [`ThreatScanner::scan_and_redact`] when a
//!    scanner is wired so a `pin_memory` tool call from the LLM cannot
//!    persist a secret in plain text; emit `memory_pii_redacted` when
//!    the scanner fires.
//! 3. **Dedup** — case-insensitive trimmed match on `(content, scope,
//!    project_id)`; on hit, return the existing [`PinnedItem`] without
//!    a second INSERT.
//! 4. **Limit guard** — count current rows in the same `(scope,
//!    project_id)` bucket; reject when the count is already at
//!    [`MAX_PINS_PER_SCOPE`].
//! 5. **INSERT** — UPSERT into the `pinned_items` table with a freshly
//!    minted ULID.
//! 6. **Sidecar rewrite** — re-render `<scope_root>/pinned.md` from the
//!    SQL state via `tmp + rename` so a crash mid-write never leaves a
//!    half-written sidecar (v2 §0.5 Δ-6 — sidecar is best-effort but
//!    must never be torn).
//! 7. **Audit emit** — fire `memory_pinned` carrying the post-insert
//!    pin count so the Telemetry Drawer can render "已固定 N 条".
//!
//! ## Scope contract
//!
//! - `PinScope::Project` rows MUST carry a `project_id`; `list()`
//!   filters by `(scope = 'project' AND project_id = ?)`.
//! - `PinScope::Global` rows have `project_id = NULL`; `list()` for
//!   `scope = Global` filters `project_id IS NULL`.
//! - [`PinnedStore::list_all_for_prompt`] returns
//!   `Global ∪ matching-Project` so the system-prompt injector
//!   (8A.11) gets one merged set in a stable order.
//!
//! ## Hard limits (v2 §0.5 Δ-17)
//!
//! - [`MAX_PINS_PER_SCOPE`] = 50 pins per `(scope, project_id)` bucket
//!   (Global counted separately from each project).
//! - [`MAX_PIN_CONTENT_CHARS`] = 500 characters per pin.
//!
//! ## Single scope_root simplification
//!
//! Today every pin renders into one `<scope_root>/pinned.md` file,
//! regardless of project vs global tier.  v2 §0.5 Δ-6 explicitly
//! defers the multi-root layout (Global at `<data_local>/.if2ai/...`
//! vs Project at `<workdir>/.if2ai/...`) to a later slice — see the
//! TODO inside [`SqlitePinnedStore::rewrite_sidecar`].

#![allow(dead_code)] // first production consumer lands in 8A.10 (pin/unpin tools)

mod provider_impl;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
// Re-exported for `super::*` glob in tests.rs (kept byte-identical).
#[allow(unused_imports)]
use ulid::Ulid;

use crate::modules::memory::pinned::types::{PinScope, PinSource, PinnedItem};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::MemoryError;

/// Per-scope pin cap (v2 §0.5 Δ-17).
pub const MAX_PINS_PER_SCOPE: usize = 50;
/// Per-pin character cap (v2 §0.5 Δ-17).
pub const MAX_PIN_CONTENT_CHARS: usize = 500;

/// Storage seam for [`PinnedItem`].  Lives behind
/// `Arc<dyn PinnedStore>` on `AppState` so other subsystems can be
/// tested with [`NullPinnedStore`] without touching SQLite.
#[async_trait]
pub trait PinnedStore: Send + Sync {
    /// Returns the pins visible to the given `(scope, project_id)`
    /// bucket, ordered by `created_at` ASC (oldest first — matches the
    /// markdown sidecar order).
    ///
    /// `scope` choice:
    /// - [`PinScope::Project`] + `project_id = Some(p)` → only that
    ///   project's pins.
    /// - [`PinScope::Global`] (`project_id` is ignored) → only global
    ///   pins.
    async fn list(
        &self,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<Vec<PinnedItem>, MemoryError>;

    /// Returns the union of `Global` pins + the active project's pins,
    /// global first then project, both ordered by `created_at` ASC so
    /// the system-prompt injector (8A.11) gets a stable rendering
    /// order.
    async fn list_all_for_prompt(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<PinnedItem>, MemoryError>;

    /// Insert a pin (or return the existing item on dedup).  Enforces
    /// [`MAX_PIN_CONTENT_CHARS`], [`MAX_PINS_PER_SCOPE`], and PII
    /// scrub.  Always emits `memory_pinned`; emits `memory_pii_redacted`
    /// when the scanner fires.
    async fn add(
        &self,
        content: &str,
        scope: PinScope,
        source: PinSource,
        project_id: Option<&str>,
    ) -> Result<PinnedItem, MemoryError>;

    /// Delete one pin by id.  Returns `Ok(true)` when a row was removed,
    /// `Ok(false)` when the id was not present (idempotent).
    async fn delete(&self, id: &str) -> Result<bool, MemoryError>;

    /// Re-stamp `created_at` for each id in `ids` so the natural
    /// `ORDER BY created_at ASC` matches the supplied order.  Ids not
    /// in the store are silently skipped.
    async fn reorder(&self, ids: &[String]) -> Result<(), MemoryError>;
}

/// SQLite-backed [`PinnedStore`] with a markdown sidecar at
/// `<scope_root>/pinned.md`.  Construct via [`Self::open`] from
/// `main.rs::run`.
pub struct SqlitePinnedStore {
    db: Arc<Mutex<Connection>>,
    scope_root: PathBuf,
    scanner: Option<Arc<ThreatScanner>>,
}

pub(super) const SELECT_COLUMNS: &str =
    "id, content, scope, project_id, created_at, created_by_kind, created_by_meta";

impl SqlitePinnedStore {
    /// Open (or create) the pinned store backed by `db_path`.  Creates
    /// parent directories + the `pinned_items` table + indexes; the
    /// markdown sidecar lives at `scope_root.join("pinned.md")`.
    pub fn open(
        db_path: &Path,
        scope_root: PathBuf,
        scanner: Option<Arc<ThreatScanner>>,
    ) -> Result<Self, MemoryError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MemoryError::Generic(format!("failed to create pinned db parent {parent:?}: {e}"))
            })?;
        }
        std::fs::create_dir_all(&scope_root).map_err(|e| {
            MemoryError::Generic(format!(
                "failed to create pinned scope_root {scope_root:?}: {e}"
            ))
        })?;

        let conn = Connection::open(db_path).map_err(|e| {
            MemoryError::Generic(format!("failed to open pinned db {db_path:?}: {e}"))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pinned_items (
                id              TEXT PRIMARY KEY,
                content         TEXT NOT NULL,
                scope           TEXT NOT NULL,
                project_id      TEXT,
                created_at      TEXT NOT NULL,
                created_by_kind TEXT NOT NULL,
                created_by_meta TEXT NOT NULL DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_pinned_scope ON pinned_items(scope);
            CREATE INDEX IF NOT EXISTS idx_pinned_project ON pinned_items(project_id);
            CREATE INDEX IF NOT EXISTS idx_pinned_created ON pinned_items(created_at);",
        )
        .map_err(|e| MemoryError::Generic(format!("failed to init pinned_items schema: {e}")))?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            scope_root,
            scanner,
        })
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> Result<PinnedItem, rusqlite::Error> {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        let scope_s: String = row.get(2)?;
        let project_id: Option<String> = row.get(3)?;
        let created_at_s: String = row.get(4)?;
        let kind: String = row.get(5)?;
        let meta: String = row.get(6)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    4,
                    "datetime".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        let created_by = match kind.as_str() {
            "tool" => {
                #[derive(serde::Deserialize)]
                struct ToolMeta {
                    tool_name: String,
                    session_id: String,
                }
                serde_json::from_str::<ToolMeta>(&meta)
                    .map(|m| PinSource::Tool {
                        tool_name: m.tool_name,
                        session_id: m.session_id,
                    })
                    .unwrap_or(PinSource::User)
            }
            _ => PinSource::User,
        };

        Ok(PinnedItem {
            id,
            content,
            scope: PinScope::from_str_lossy(&scope_s),
            project_id,
            created_at,
            created_by,
        })
    }

    /// Re-render the markdown sidecar from the current SQL state.
    ///
    /// Best-effort: a failure here is propagated as [`MemoryError`] so
    /// the caller can log + decide.  Atomicity is guaranteed via a
    /// `tmp + rename` pair so a crash mid-write never leaves a torn
    /// sidecar.
    ///
    /// TODO(8B): split per-scope into distinct files (Global at
    /// `<data_local>/.if2ai/memory/pinned.md`, Project at
    /// `<workdir>/.if2ai/memory/pinned.md`) once multi-scope-root
    /// resolution lands.  Today both scopes share one sidecar to keep
    /// the slice small.
    async fn rewrite_sidecar(
        &self,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<(), MemoryError> {
        let pins = self.list(scope, project_id).await?;
        let header = match scope {
            PinScope::Global => "# Pinned (Global)\n\n",
            PinScope::Project => "# Pinned (Project)\n\n",
        };
        let body = pins
            .iter()
            .map(|p| format!("- {}", p.content.replace('\n', " ")))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = if body.is_empty() {
            header.to_string()
        } else {
            format!("{header}{body}\n")
        };

        let scope_root = self.scope_root.clone();
        tokio::task::spawn_blocking(move || -> Result<(), MemoryError> {
            let path = scope_root.join("pinned.md");
            let tmp = scope_root.join("pinned.md.tmp");
            std::fs::write(&tmp, payload.as_bytes()).map_err(|e| {
                MemoryError::Generic(format!("write pinned.md.tmp at {tmp:?}: {e}"))
            })?;
            std::fs::rename(&tmp, &path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                MemoryError::Generic(format!("rename pinned.md at {path:?}: {e}"))
            })?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("pinned sidecar join error: {e}")))?
    }

    fn count_in_bucket(
        conn: &Connection,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<usize, MemoryError> {
        let n: i64 = match (scope, project_id) {
            (PinScope::Project, Some(p)) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'project' AND project_id = ?1",
                    params![p],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
            (PinScope::Project, None) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'project' AND project_id IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
            (PinScope::Global, _) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'global' AND project_id IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
        };
        Ok(n.max(0) as usize)
    }
}

/// No-op `PinnedStore` for environments without SQLite (test stubs,
/// the degraded mode without panicking).
pub struct NullPinnedStore;

impl NullPinnedStore {
    /// Construct a new no-op store.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullPinnedStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PinnedStore for NullPinnedStore {
    async fn list(
        &self,
        _scope: PinScope,
        _project_id: Option<&str>,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        Ok(Vec::new())
    }
    async fn list_all_for_prompt(
        &self,
        _scope: &MemoryExecutionScope,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        Ok(Vec::new())
    }
    async fn add(
        &self,
        _content: &str,
        _scope: PinScope,
        _source: PinSource,
        _project_id: Option<&str>,
    ) -> Result<PinnedItem, MemoryError> {
        Err(MemoryError::Generic(
            "pinned store is in null fallback mode (SQLite open failed at startup)".to_string(),
        ))
    }
    async fn delete(&self, _id: &str) -> Result<bool, MemoryError> {
        Ok(false)
    }
    async fn reorder(&self, _ids: &[String]) -> Result<(), MemoryError> {
        Ok(())
    }
}
