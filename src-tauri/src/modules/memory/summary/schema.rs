#![allow(dead_code)] // first production consumer lands in 8A.7 RollingSummarizer

//! Session summary record schema.
//!
//! A [`SessionSummaryRecord`] is the single source of truth for one
//! conversation's rolling LLM-compressed summary plus the `snapshot`
//! checkpoint that tracks what the deep-memory extractor has already
//! processed.  The [`SqliteSessionSummaryStore`](super::store::SqliteSessionSummaryStore)
//! persists one row per session and a JSON sidecar at
//! `<scope_root>/summaries/<session_id>.json` for cold backup and human
//! inspection.
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-B1 for the full data-model rationale and §0.5 Δ-6 for
//! the on-disk path convention.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One rolling-summary row per session.
///
/// Field semantics:
/// - `summary` is the latest LLM-compressed text (overwritten on each
///   rolling-summary turn).
/// - `snapshot` is the version most recently consumed by
///   `deep_memory::process_dirty_sessions`; defaults to `""` so a brand
///   new record is always considered dirty.
/// - `is_dirty` is therefore `summary != snapshot` (computed at query
///   time by [`SqliteSessionSummaryStore::list_dirty`]; no SQL VIRTUAL
///   column is used so we stay portable across SQLite library versions).
/// - `message_count` is the cumulative count of conversation messages
///   already covered by `summary` (used by T-B2 for incremental slicing).
/// - `source` distinguishes scheduled rolling-summary updates from
///   context-window compaction events (T-B5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummaryRecord {
    /// Owning session identifier.  Acts as the SQLite primary key and
    /// as the JSON sidecar filename, so it MUST be filesystem-safe
    /// (no `/`, no `..`); see
    /// [`super::store::safe_session_filename`].
    pub session_id: String,
    /// Optional project binding for cross-session scope queries.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Wall-clock timestamp of the first time this record was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock timestamp of the most recent `save()`.
    pub updated_at: DateTime<Utc>,
    /// Latest LLM-compressed summary text (covers all conversation
    /// turns up through `message_count`).
    pub summary: String,
    /// Last summary version processed by the deep-memory extractor;
    /// defaults to `""` so new records read as dirty until
    /// [`super::store::SessionSummaryStore::mark_processed`] runs.
    #[serde(default)]
    pub snapshot: String,
    /// Wall-clock timestamp of the last `mark_processed` call.
    #[serde(default)]
    pub snapshot_at: Option<DateTime<Utc>>,
    /// Total conversation messages already folded into `summary`.
    #[serde(default)]
    pub message_count: usize,
    /// Provenance — separates scheduled rolling updates (`Rolling`)
    /// from context-window compaction (`Compact`, T-B5).
    #[serde(default)]
    pub source: SummarySource,
}

impl SessionSummaryRecord {
    /// True iff the deep-memory extractor has not yet seen the latest
    /// `summary` (`summary != snapshot`).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.summary != self.snapshot
    }
}

/// Origin of a [`SessionSummaryRecord`] revision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SummarySource {
    /// Scheduled rolling-summary update (every N turns).  Default for
    /// any record whose `source` field is missing on disk.
    #[default]
    Rolling,
    /// Context-window compaction triggered by token-budget overflow
    /// (T-B5 in the v2 plan).
    Compact,
}

impl SummarySource {
    /// Stable string used for the SQLite `source` column and JSON
    /// serialisation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            SummarySource::Rolling => "rolling",
            SummarySource::Compact => "compact",
        }
    }

    /// Inverse of [`Self::as_str`].  Unknown values fall back to
    /// `Rolling` so a corrupted column never poisons recall.
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "compact" => SummarySource::Compact,
            _ => SummarySource::Rolling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_dirty_when_summary_differs_from_snapshot() {
        let now = Utc::now();
        let record = SessionSummaryRecord {
            session_id: "s1".into(),
            project_id: None,
            created_at: now,
            updated_at: now,
            summary: "hello".into(),
            snapshot: String::new(),
            snapshot_at: None,
            message_count: 1,
            source: SummarySource::Rolling,
        };
        assert!(record.is_dirty());
    }

    #[test]
    fn is_clean_when_snapshot_matches() {
        let now = Utc::now();
        let record = SessionSummaryRecord {
            session_id: "s1".into(),
            project_id: None,
            created_at: now,
            updated_at: now,
            summary: "hello".into(),
            snapshot: "hello".into(),
            snapshot_at: Some(now),
            message_count: 1,
            source: SummarySource::Rolling,
        };
        assert!(!record.is_dirty());
    }

    #[test]
    fn summary_source_round_trip() {
        for src in [SummarySource::Rolling, SummarySource::Compact] {
            assert_eq!(SummarySource::from_str_lossy(src.as_str()), src);
        }
    }

    #[test]
    fn summary_source_unknown_falls_back_to_rolling() {
        assert_eq!(
            SummarySource::from_str_lossy("totally_made_up"),
            SummarySource::Rolling
        );
    }
}
