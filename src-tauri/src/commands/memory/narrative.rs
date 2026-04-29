//! Memory narrative IPC — session summaries listing for the
//! MemoryNarrativeViewer (Phase 8B.11 / T-UI-3).
//!
//! Read-only; reuses the existing `SessionSummaryStore` plumbing
//! installed on `AppState` from 8A.5.
//!
//! Extracted from the legacy single-file `commands/memory.rs` during
//! the Memory System Audit P3 god-file split — content unchanged.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::scope::MemoryExecutionScope;

/// Frontend-facing DTO for one
/// [`crate::modules::memory::summary::schema::SessionSummaryRecord`]
/// row. Mirrors the backend record one-to-one but flattens
/// [`crate::modules::memory::summary::schema::SummarySource`] to a
/// stable `"rolling"` / `"compact"` string so the React layer can
/// pattern-match without importing a Rust enum shape.
///
/// `created_at` / `updated_at` are serialised as RFC-3339 / ISO-8601
/// strings via `chrono`'s default `Serialize` impl so they round-trip
/// with `new Date(...)` in the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummaryDto {
    /// Owning session identifier (SQLite primary key + sidecar filename).
    pub session_id: String,
    /// Optional project binding for project-scope filtering.
    pub project_id: Option<String>,
    /// Latest LLM-compressed summary text.
    pub summary: String,
    /// Wall-clock timestamp of the first save.
    pub created_at: DateTime<Utc>,
    /// Wall-clock timestamp of the most recent save.
    pub updated_at: DateTime<Utc>,
    /// Total conversation messages folded into `summary`.
    pub message_count: usize,
    /// Provenance — `"rolling"` (scheduled rolling-summary update) or
    /// `"compact"` (context-window compaction event, T-B5).
    pub source: String,
}

impl From<crate::modules::memory::summary::schema::SessionSummaryRecord> for SessionSummaryDto {
    fn from(r: crate::modules::memory::summary::schema::SessionSummaryRecord) -> Self {
        Self {
            session_id: r.session_id,
            project_id: r.project_id,
            summary: r.summary,
            created_at: r.created_at,
            updated_at: r.updated_at,
            message_count: r.message_count,
            source: r.source.as_str().to_string(),
        }
    }
}

/// List session summaries within an optional time window for the
/// MemoryNarrativeViewer (Phase 8B.11 / T-UI-3).
///
/// `scope` accepts `"current"` / `"all"` / `"project"` / `"global"`,
/// matching the same vocabulary used by `memory_compile_now`.  Until
/// project / session sidecars land we collapse every accepted label
/// onto the global execution scope so the wire-shape stays stable.
///
/// `limit` caps the returned count (default 100, hard ceiling 500 so
/// a runaway frontend cannot fault the IPC channel by requesting tens
/// of thousands of rows). `since_days` (optional) restricts to summaries
/// whose `updated_at` is within the last N days; `None` defaults to 90.
///
/// Results are returned newest-first (descending `updated_at`).
#[tauri::command]
pub async fn memory_summaries_list(
    state: State<'_, AppState>,
    scope: String,
    limit: Option<usize>,
    since_days: Option<u32>,
) -> Result<Vec<SessionSummaryDto>, String> {
    let exec_scope = match scope.as_str() {
        "current" | "all" | "project" | "global" => MemoryExecutionScope::global(),
        other => {
            return Err(format!(
                "invalid scope {other:?}; expected 'current'|'all'|'project'|'global'"
            ));
        }
    };
    let days = i64::from(since_days.unwrap_or(90).max(1));
    let now = Utc::now();
    let since = now - chrono::Duration::days(days);
    let cap = limit.unwrap_or(100).min(500);
    let mut rows = state
        .summary_store
        .list_in_range(&exec_scope, since, now)
        .await
        .map_err(|e| e.to_string())?;
    rows.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
    rows.truncate(cap);
    Ok(rows.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod summary_command_tests {
    use super::*;
    use crate::modules::memory::summary::schema::{SessionSummaryRecord, SummarySource};

    fn make_record(session_id: &str, summary: &str, source: SummarySource) -> SessionSummaryRecord {
        let now = Utc::now();
        SessionSummaryRecord {
            session_id: session_id.into(),
            project_id: None,
            created_at: now,
            updated_at: now,
            summary: summary.into(),
            snapshot: String::new(),
            snapshot_at: None,
            message_count: 3,
            source,
        }
    }

    #[test]
    fn dto_from_record_flattens_source_to_lowercase_string() {
        let dto: SessionSummaryDto = make_record("s1", "hello", SummarySource::Rolling).into();
        assert_eq!(dto.session_id, "s1");
        assert_eq!(dto.summary, "hello");
        assert_eq!(dto.message_count, 3);
        assert_eq!(dto.source, "rolling");

        let compact: SessionSummaryDto =
            make_record("s2", "compact", SummarySource::Compact).into();
        assert_eq!(compact.source, "compact");
    }

    #[test]
    fn dto_serialises_snake_case_fields() {
        let dto: SessionSummaryDto = make_record("s1", "hi", SummarySource::Rolling).into();
        let json = serde_json::to_value(&dto).expect("serialize");
        for key in [
            "session_id",
            "project_id",
            "summary",
            "created_at",
            "updated_at",
            "message_count",
            "source",
        ] {
            assert!(json.get(key).is_some(), "missing field {key}: {json}");
        }
        assert_eq!(json["source"], "rolling");
    }

    #[test]
    fn memory_summaries_list_validates_scope() {
        // Pure sync validator: replays the scope-vocabulary check that
        // memory_summaries_list runs before touching `summary_store`,
        // so we can assert the rejection without standing up an
        // AppState. Mirrors `resolve_paths`'s fixture style.
        fn validate(scope: &str) -> Result<MemoryExecutionScope, String> {
            match scope {
                "current" | "all" | "project" | "global" => Ok(MemoryExecutionScope::global()),
                other => Err(format!(
                    "invalid scope {other:?}; expected 'current'|'all'|'project'|'global'"
                )),
            }
        }
        for ok in ["current", "all", "project", "global"] {
            assert!(validate(ok).is_ok(), "{ok} must be accepted");
        }
        let err = validate("session-only").expect_err("unknown scope must error");
        assert!(err.contains("invalid scope"), "unexpected msg: {err}");
        assert!(err.contains("session-only"));
    }
}
