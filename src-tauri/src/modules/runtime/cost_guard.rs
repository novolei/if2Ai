//! Spend and rate limits for LLM calls (inspired by Steward `cost_guard.rs`).
//!
//! Tracks per-calendar-day spend (estimated cents) and per-hour LLM call count.
//! Uses `session_id` as the per-user key when no separate user id exists.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Datelike;
use once_cell::sync::Lazy;

/// Configuration (typically from env via [`CostGuardConfig::from_env`]).
#[derive(Debug, Clone, Default)]
pub struct CostGuardConfig {
    /// Max estimated spend per UTC day in cents. `None` = unlimited.
    pub max_cost_per_day_cents: Option<u64>,
    /// Max LLM stream starts per rolling hour. `None` = unlimited.
    pub max_llm_calls_per_hour: Option<u64>,
    /// Per-session/day cap (same units as global day). `None` = unlimited.
    pub max_cost_per_session_per_day_cents: Option<u64>,
    /// Fixed estimated cost charged per LLM request (stream start), cents.
    pub estimated_cost_per_llm_call_cents: u64,
}

impl CostGuardConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let mut c = Self {
            estimated_cost_per_llm_call_cents: 1,
            ..Default::default()
        };
        if let Ok(v) = std::env::var("IF2AI_COST_MAX_PER_DAY_CENTS") {
            c.max_cost_per_day_cents = v.parse().ok();
        }
        if let Ok(v) = std::env::var("IF2AI_COST_MAX_LLM_CALLS_PER_HOUR") {
            c.max_llm_calls_per_hour = v.parse().ok();
        }
        if let Ok(v) = std::env::var("IF2AI_COST_MAX_PER_SESSION_DAY_CENTS") {
            c.max_cost_per_session_per_day_cents = v.parse().ok();
        }
        if let Ok(v) = std::env::var("IF2AI_COST_ESTIMATE_PER_LLM_CALL_CENTS") {
            if let Ok(n) = v.parse::<u64>() {
                c.estimated_cost_per_llm_call_cents = n.max(1);
            }
        }
        c
    }

    #[must_use]
    pub fn any_limit_enabled(&self) -> bool {
        self.max_cost_per_day_cents.is_some()
            || self.max_llm_calls_per_hour.is_some()
            || self.max_cost_per_session_per_day_cents.is_some()
    }
}

#[derive(Debug, Clone)]
pub enum CostLimitExceeded {
    DailyBudget {
        spent_cents: u64,
        limit_cents: u64,
    },
    HourlyRate {
        calls: u64,
        limit: u64,
    },
    SessionDailyBudget {
        session_id: String,
        spent_cents: u64,
        limit_cents: u64,
    },
}

impl std::fmt::Display for CostLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DailyBudget {
                spent_cents,
                limit_cents,
            } => write!(
                f,
                "Daily cost limit exceeded: spent {}¢ of {}¢",
                spent_cents, limit_cents
            ),
            Self::HourlyRate { calls, limit } => write!(
                f,
                "Hourly LLM call limit exceeded: {} calls of {} per hour",
                calls, limit
            ),
            Self::SessionDailyBudget {
                session_id,
                spent_cents,
                limit_cents,
            } => write!(
                f,
                "Session '{}' daily cost limit exceeded: spent {}¢ of {}¢",
                session_id, spent_cents, limit_cents
            ),
        }
    }
}

struct HourWindow {
    hour_start: Instant,
    calls: u64,
}

struct DaySpend {
    day_id: u32,
    spent_cents: u64,
    per_session: HashMap<String, u64>,
}

struct Inner {
    day: DaySpend,
    hour: HourWindow,
}

impl Inner {
    fn new() -> Self {
        Self {
            day: DaySpend {
                day_id: utc_day_id(),
                spent_cents: 0,
                per_session: HashMap::new(),
            },
            hour: HourWindow {
                hour_start: Instant::now(),
                calls: 0,
            },
        }
    }

    fn rollover_day_if_needed(&mut self) {
        let today = utc_day_id();
        if self.day.day_id != today {
            self.day = DaySpend {
                day_id: today,
                spent_cents: 0,
                per_session: HashMap::new(),
            };
        }
    }

    fn rollover_hour_if_needed(&mut self) {
        if self.hour.hour_start.elapsed() >= Duration::from_secs(3600) {
            self.hour = HourWindow {
                hour_start: Instant::now(),
                calls: 0,
            };
        }
    }
}

fn utc_day_id() -> u32 {
    let now = chrono::Utc::now().date_naive();
    now.year() as u32 * 10_000 + now.month() * 100 + now.day()
}

static COST_STATE: Lazy<Mutex<Inner>> = Lazy::new(|| Mutex::new(Inner::new()));

/// Enforce limits before starting an LLM stream; charge estimate after success.
pub struct CostGuard;

impl CostGuard {
    pub fn check_before_llm_call(
        cfg: &CostGuardConfig,
        session_id: &str,
    ) -> Result<(), CostLimitExceeded> {
        if !cfg.any_limit_enabled() {
            return Ok(());
        }
        let mut g = COST_STATE.lock().expect("cost guard mutex poisoned");
        g.rollover_day_if_needed();
        g.rollover_hour_if_needed();

        if let Some(limit) = cfg.max_llm_calls_per_hour {
            if g.hour.calls >= limit {
                return Err(CostLimitExceeded::HourlyRate {
                    calls: g.hour.calls,
                    limit,
                });
            }
        }

        if let Some(limit) = cfg.max_cost_per_session_per_day_cents {
            let est = cfg.estimated_cost_per_llm_call_cents;
            let spent = g.day.per_session.get(session_id).copied().unwrap_or(0);
            if spent.saturating_add(est) > limit {
                return Err(CostLimitExceeded::SessionDailyBudget {
                    session_id: session_id.to_string(),
                    spent_cents: spent,
                    limit_cents: limit,
                });
            }
        }

        if let Some(limit) = cfg.max_cost_per_day_cents {
            let next = g
                .day
                .spent_cents
                .saturating_add(cfg.estimated_cost_per_llm_call_cents);
            if next > limit {
                return Err(CostLimitExceeded::DailyBudget {
                    spent_cents: g.day.spent_cents,
                    limit_cents: limit,
                });
            }
        }

        Ok(())
    }

    /// Call after a successful LLM stream start (charges the estimate once per outer-loop LLM call).
    pub fn record_llm_call_charged(cfg: &CostGuardConfig, session_id: &str) {
        if !cfg.any_limit_enabled() {
            return;
        }
        let mut g = COST_STATE.lock().expect("cost guard mutex poisoned");
        g.rollover_day_if_needed();
        g.rollover_hour_if_needed();
        g.hour.calls = g.hour.calls.saturating_add(1);
        let est = cfg.estimated_cost_per_llm_call_cents;
        g.day.spent_cents = g.day.spent_cents.saturating_add(est);
        *g.day.per_session.entry(session_id.to_string()).or_insert(0) += est;
    }
}
