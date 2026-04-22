#![allow(dead_code)] // first production consumer lands in 8A.7 RollingSummarizer

//! Session-summary subsystem (Phase 8A.5 / Sprint 1 T-B1).
//!
//! Provides the [`SessionSummaryStore`] trait for persisting per-session
//! rolling summaries plus the [`SqliteSessionSummaryStore`]
//! implementation used in production.  See `store.rs` for the dual-write
//! contract (SQLite is authoritative; JSON sidecar is best-effort).

pub mod prompt;
pub mod rolling;
pub mod schema;
pub mod store;

// Prompt builder + budget helpers — consumed by RollingSummarizer
// (rolling.rs) and unit tests.
pub use prompt::{
    build_conversation_text, build_rolling_summary_prompt, compute_budget, RollingSummaryPrompt,
    SummaryBudget, ASSISTANT_CAP, MAX_MAX_TOKENS, MAX_TOTAL_BUDGET, MIN_MAX_TOKENS,
    MIN_TOTAL_BUDGET, PER_TURN_BUDGET_CHARS,
};
pub use schema::{SessionSummaryRecord, SummarySource};
pub use store::{NullSessionSummaryStore, SessionSummaryStore, SqliteSessionSummaryStore};

// RollingSummarizer — held on AppState as Arc<RollingSummarizer>;
// triggered from turn finalize and runtime compaction paths.
pub use rolling::RollingSummarizer;
