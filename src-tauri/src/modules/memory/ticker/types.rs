//! Public configuration + state types used by `MemoryTicker`.
//!
//! Extracted from `ticker/mod.rs` in GFR-T1-F-1 (pure structural move;
//! struct/enum fields and Default impl byte-identical).

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickerConfig {
    /// Roll a summary every N user turns (default `6`, mirrors
    /// openhanako).
    pub turns_per_summary: u32,
    /// Backup timer interval for the daily compile cycle in seconds
    /// (default `3600` = 1h).  The daily job is also kicked off
    /// opportunistically by `notify_turn` when the logical day rolls
    /// over, so the timer only catches truly idle sessions.
    pub daily_check_interval_secs: u64,
    /// When `true` (default), `notify_session_end` also extracts
    /// session experiences via the future
    /// `experience::extractor` (Phase 8D).
    pub experience_enabled: bool,
    /// MEM-MOD-P5 — fire a self-reflection LLM call every N user turns
    /// (default `10`, set `0` to disable).  Each pulse persists a
    /// `Reflection`-category memory.  No-op unless the ticker was
    /// constructed with [`crate::modules::memory::MemoryTicker::with_reflection_runtime`].
    pub reflection_threshold: u32,
}

impl Default for TickerConfig {
    fn default() -> Self {
        Self {
            turns_per_summary: 6,
            daily_check_interval_secs: 3600,
            experience_enabled: true,
            reflection_threshold: 10,
        }
    }
}

/// Sub-tasks of the daily compile cycle.  Tracked individually inside
/// [`TickerState::daily_steps_completed`] so a partial failure
/// (e.g. LLM-throttled longterm) doesn't force the entire cycle to
/// re-run.
///
/// The 6 variants form a topological order:
/// `Today → Week → Longterm → Facts → Assemble → DeepMemory`.  The
/// future `do_daily` (8B.8) walks this list, checks completion, and
/// re-tries only what's missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DailyStep {
    /// Compile today's daily summary from rolling summaries.
    Today,
    /// Compile this week's summary from daily summaries.
    Week,
    /// Compile long-term summary from weekly summaries.
    Longterm,
    /// Extract structured facts from rolling + daily summaries.
    Facts,
    /// Assemble `compiled.md` from all higher-tier summaries + facts.
    Assemble,
    /// Synthesize deep-memory diary entries (Phase 8D).
    DeepMemory,
}

/// Mutable scheduler state, guarded by [`MemoryTicker::state`] Mutex.
///
/// Kept out of [`MemoryTicker`] itself so the surrounding methods can
/// take `&self` and the ticker can live behind `Arc` without an
/// outer wrapper.
#[derive(Debug, Default)]
pub struct TickerState {
    /// `session_id → number of user turns observed since last roll`.
    pub turn_counts: HashMap<String, u32>,
    /// Set of `session_id`s currently mid-roll, prevents
    /// double-spawn.
    pub summary_in_progress: HashSet<String>,
    /// Last logical day on which `do_daily` ran to completion.
    pub last_daily_job_date: Option<NaiveDate>,
    /// Sub-steps of the *current* daily cycle that have completed.
    pub daily_steps_completed: HashSet<DailyStep>,
    /// Logical day the partial daily-step set belongs to.  Resets
    /// the completed set when the day rolls over.
    pub daily_steps_date: Option<NaiveDate>,
    /// `true` while `do_daily` is running — prevents reentrancy.
    pub daily_running: bool,
}
