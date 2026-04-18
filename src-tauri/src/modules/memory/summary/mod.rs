#![allow(dead_code)] // first production consumer lands in 8A.7 RollingSummarizer

//! Session-summary subsystem (Phase 8A.5 / Sprint 1 T-B1).
//!
//! Provides the [`SessionSummaryStore`] trait for persisting per-session
//! rolling summaries plus the [`SqliteSessionSummaryStore`]
//! implementation used in production.  See `store.rs` for the dual-write
//! contract (SQLite is authoritative; JSON sidecar is best-effort).

pub mod schema;
pub mod store;

pub use schema::{SessionSummaryRecord, SummarySource};
pub use store::{NullSessionSummaryStore, SessionSummaryStore, SqliteSessionSummaryStore};
