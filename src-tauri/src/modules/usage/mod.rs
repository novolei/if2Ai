//! Usage tracking — durable per-call token usage with daily / weekly /
//! monthly aggregations driven by the system-wide `logical_day`
//! 04:00 cutoff.
//!
//! The store is a single SQLite file under
//! `~/.if2ai/usage/usage.sqlite`.  Every LLM call that lands a usable
//! `TokenUsage` should call [`record_turn_usage`]; aggregations
//! ([`UsageStore::summary`]) are read by the Settings/Usage page IPC.
//!
//! Caller labels we ship with today (open enum, future-extensible):
//!   - `chat`         — the user-facing turn loop in `turn_service`
//!   - `summarizer`   — RollingSummarizer LLM calls
//!   - `compiler`     — memory compilation / reflection LLM calls
//!   - `utility`      — small one-shot prompts (greetings, classifiers)
//!   - `utility_large` — long-form one-shot prompts (e.g. session_summary)
//!
//! The schema also records `provider_id` + `model_id` so a future view
//! can break usage down per model without a migration.

use std::sync::Arc;

use crate::modules::runtime::usage::TokenUsage;

pub mod sqlite_store;

pub use sqlite_store::{global_store, UsageStoreError};

/// Caller-side label for one LLM call. Free-form string; the Settings
/// page renders the standard five labels above and groups any unknown
/// caller under "其他".
pub const CALLER_CHAT: &str = "chat";
pub const CALLER_SUMMARIZER: &str = "summarizer";
pub const CALLER_COMPILER: &str = "compiler";
pub const CALLER_UTILITY: &str = "utility";
pub const CALLER_UTILITY_LARGE: &str = "utility_large";

/// Time bucket selector the IPC accepts. Chosen to match the four cards
/// on the Usage settings page.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageWindow {
    Today,
    ThisWeek,
    ThisMonth,
    AllTime,
}

impl UsageWindow {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::ThisWeek => "this_week",
            Self::ThisMonth => "this_month",
            Self::AllTime => "all_time",
        }
    }
}

/// One row of the Usage settings table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CallerUsage {
    pub caller: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub turns: u64,
}

/// IPC payload returned by `usage_summary`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub window: String,
    pub by_caller: Vec<CallerUsage>,
    pub total: CallerUsage,
}

/// Trait surface so future tests can swap in an in-memory store.
pub trait UsageStore: Send + Sync {
    fn record(&self, record: TurnUsageRecord) -> Result<(), UsageStoreError>;
    fn summary(&self, window: UsageWindow) -> Result<UsageSummary, UsageStoreError>;
}

/// Single-row write payload.
#[derive(Debug, Clone)]
pub struct TurnUsageRecord {
    pub caller: String,
    pub provider_id: String,
    pub model_id: String,
    pub usage: TokenUsage,
    pub cost_usd: f64,
    pub session_id: Option<String>,
}

/// Convenience: fire-and-forget record into the global store.
/// Errors are logged; we never fail a turn because usage logging
/// failed (the data is observability, not control flow).
pub fn record_turn_usage(record: TurnUsageRecord) {
    let store = global_store();
    if let Err(err) = store.record(record) {
        tracing::warn!(target: "if2ai::usage", "record_turn_usage failed: {err}");
    }
}

/// Wrap an `Arc<dyn UsageStore>` for downstream injection sites.
pub type SharedUsageStore = Arc<dyn UsageStore>;
