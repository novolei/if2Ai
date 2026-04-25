//! SQLite-backed implementation of [`super::UsageStore`].
//!
//! Stores one row per LLM call under `~/.if2ai/usage/usage.sqlite` and
//! buckets aggregations by `day_local` / `week_local` / `month_local`,
//! computed from the project-wide `logical_day` 04:00 LOCAL cutoff so
//! a 02:00 message rolls into "yesterday" — same convention used by the
//! memory rollers.
//!
//! Concurrency: one global `Mutex<Connection>` (single-writer; reads
//! piggyback on the same connection).  Writes are tiny; the lock holds
//! for the duration of one INSERT.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Datelike, Utc};
use rusqlite::{params, Connection};
use thiserror::Error;

use crate::modules::config::store::if2ai_data_root;

use super::{
    CallerUsage, SharedUsageStore, TurnUsageRecord, UsageStore, UsageSummary, UsageWindow,
};

#[derive(Debug, Error)]
pub enum UsageStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub fn usage_dir() -> PathBuf {
    if2ai_data_root().join("usage")
}

pub fn usage_db_path() -> PathBuf {
    usage_dir().join("usage.sqlite")
}

pub struct SqliteUsageStore {
    conn: Mutex<Connection>,
}

impl SqliteUsageStore {
    /// Open (and migrate, if needed) the usage database at `path`.
    pub fn open(path: PathBuf) -> Result<Self, UsageStoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_default() -> Result<Self, UsageStoreError> {
        Self::open(usage_db_path())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS turn_usage (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts_utc TEXT NOT NULL,
  day_local TEXT NOT NULL,
  week_local TEXT NOT NULL,
  month_local TEXT NOT NULL,
  caller TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  input_tokens INTEGER NOT NULL,
  output_tokens INTEGER NOT NULL,
  cost_usd REAL NOT NULL,
  session_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_day_caller ON turn_usage(day_local, caller);
CREATE INDEX IF NOT EXISTS idx_week_caller ON turn_usage(week_local, caller);
CREATE INDEX IF NOT EXISTS idx_month_caller ON turn_usage(month_local, caller);
"#;

impl UsageStore for SqliteUsageStore {
    fn record(&self, r: TurnUsageRecord) -> Result<(), UsageStoreError> {
        // Token counts: zero is fine, but skip a write for completely
        // empty records (e.g. providers that never emit usage events).
        if r.usage.input_tokens == 0
            && r.usage.output_tokens == 0
            && r.usage.cache_creation_input_tokens == 0
            && r.usage.cache_read_input_tokens == 0
        {
            return Ok(());
        }

        let now_utc = Utc::now();
        let logical_day = crate::modules::runtime::logical_day::get_today();
        let date = logical_day.date;
        let day_local = logical_day.display.clone();
        let iso_week = date.iso_week();
        // Render `YYYY-Www` (zero-padded week number) to match what humans
        // expect when scanning a list of weekly buckets.
        let week_local = format!("{:04}-W{:02}", iso_week.year(), iso_week.week());
        let month_local = format!("{:04}-{:02}", date.year(), date.month());

        let conn = self.conn.lock().expect("usage store mutex poisoned");
        conn.execute(
            "INSERT INTO turn_usage (
                ts_utc, day_local, week_local, month_local,
                caller, provider_id, model_id,
                input_tokens, output_tokens, cost_usd, session_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                now_utc.to_rfc3339(),
                day_local,
                week_local,
                month_local,
                r.caller,
                r.provider_id,
                r.model_id,
                r.usage.input_tokens as i64,
                r.usage.output_tokens as i64,
                r.cost_usd,
                r.session_id,
            ],
        )?;
        Ok(())
    }

    fn summary(&self, window: UsageWindow) -> Result<UsageSummary, UsageStoreError> {
        let conn = self.conn.lock().expect("usage store mutex poisoned");

        let logical_day = crate::modules::runtime::logical_day::get_today();
        let date = logical_day.date;
        let iso_week = date.iso_week();
        let week_local = format!("{:04}-W{:02}", iso_week.year(), iso_week.week());
        let month_local = format!("{:04}-{:02}", date.year(), date.month());

        let (where_clause, bind): (&str, Option<String>) = match window {
            UsageWindow::Today => ("WHERE day_local = ?1", Some(logical_day.display.clone())),
            UsageWindow::ThisWeek => ("WHERE week_local = ?1", Some(week_local)),
            UsageWindow::ThisMonth => ("WHERE month_local = ?1", Some(month_local)),
            UsageWindow::AllTime => ("", None),
        };

        let sql = format!(
            "SELECT caller, \
                    SUM(input_tokens) AS input_tokens, \
                    SUM(output_tokens) AS output_tokens, \
                    SUM(cost_usd) AS cost_usd, \
                    COUNT(*) AS turns \
             FROM turn_usage {where_clause} \
             GROUP BY caller \
             ORDER BY caller"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(b) = bind.as_ref() {
            stmt.query_map([b.as_str()], map_row)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], map_row)?
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut total = CallerUsage {
            caller: "total".to_string(),
            ..Default::default()
        };
        for row in &rows {
            total.input_tokens += row.input_tokens;
            total.output_tokens += row.output_tokens;
            total.cost_usd += row.cost_usd;
            total.turns += row.turns;
        }

        Ok(UsageSummary {
            window: window.as_str().to_string(),
            by_caller: rows,
            total,
        })
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CallerUsage> {
    Ok(CallerUsage {
        caller: row.get(0)?,
        input_tokens: row.get::<_, i64>(1)? as u64,
        output_tokens: row.get::<_, i64>(2)? as u64,
        cost_usd: row.get(3)?,
        turns: row.get::<_, i64>(4)? as u64,
    })
}

static GLOBAL_STORE: OnceLock<SharedUsageStore> = OnceLock::new();

/// Lazily-initialized process-global usage store. The first call opens
/// (and migrates) `~/.if2ai/usage/usage.sqlite`. Open failures fall
/// back to a no-op store so a corrupt user data dir never crashes the
/// app or starts losing chat turns.
pub fn global_store() -> SharedUsageStore {
    GLOBAL_STORE
        .get_or_init(|| match SqliteUsageStore::open_default() {
            Ok(store) => Arc::new(store) as SharedUsageStore,
            Err(err) => {
                tracing::warn!(
                    target: "if2ai::usage",
                    "failed to open usage sqlite store ({err}); per-role usage will not be persisted"
                );
                Arc::new(NoopUsageStore) as SharedUsageStore
            }
        })
        .clone()
}

/// No-op store used as a fallback when SQLite open fails.
struct NoopUsageStore;

impl UsageStore for NoopUsageStore {
    fn record(&self, _record: TurnUsageRecord) -> Result<(), UsageStoreError> {
        Ok(())
    }
    fn summary(&self, window: UsageWindow) -> Result<UsageSummary, UsageStoreError> {
        Ok(UsageSummary {
            window: window.as_str().to_string(),
            by_caller: Vec::new(),
            total: CallerUsage::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::usage::TokenUsage;
    use tempfile::TempDir;

    fn fresh_store() -> (SqliteUsageStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("usage.sqlite");
        let store = SqliteUsageStore::open(path).unwrap();
        (store, tmp)
    }

    #[test]
    fn empty_record_is_ignored() {
        let (store, _tmp) = fresh_store();
        store
            .record(TurnUsageRecord {
                caller: "chat".into(),
                provider_id: "openai".into(),
                model_id: "gpt-4o".into(),
                usage: TokenUsage::default(),
                cost_usd: 0.0,
                session_id: None,
            })
            .unwrap();
        let s = store.summary(UsageWindow::AllTime).unwrap();
        assert_eq!(s.total.turns, 0);
    }

    #[test]
    fn records_aggregate_by_caller_and_total() {
        let (store, _tmp) = fresh_store();
        store
            .record(TurnUsageRecord {
                caller: "chat".into(),
                provider_id: "openai".into(),
                model_id: "gpt-4o".into(),
                usage: TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                cost_usd: 0.001,
                session_id: Some("s1".into()),
            })
            .unwrap();
        store
            .record(TurnUsageRecord {
                caller: "chat".into(),
                provider_id: "openai".into(),
                model_id: "gpt-4o".into(),
                usage: TokenUsage {
                    input_tokens: 200,
                    output_tokens: 80,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                cost_usd: 0.002,
                session_id: Some("s1".into()),
            })
            .unwrap();
        store
            .record(TurnUsageRecord {
                caller: "summarizer".into(),
                provider_id: "openai".into(),
                model_id: "gpt-4o-mini".into(),
                usage: TokenUsage {
                    input_tokens: 60,
                    output_tokens: 10,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                cost_usd: 0.0001,
                session_id: None,
            })
            .unwrap();

        let s = store.summary(UsageWindow::Today).unwrap();
        assert_eq!(s.window, "today");
        assert_eq!(s.by_caller.len(), 2);
        let chat = s.by_caller.iter().find(|c| c.caller == "chat").unwrap();
        assert_eq!(chat.input_tokens, 300);
        assert_eq!(chat.output_tokens, 130);
        assert_eq!(chat.turns, 2);
        let sumr = s
            .by_caller
            .iter()
            .find(|c| c.caller == "summarizer")
            .unwrap();
        assert_eq!(sumr.input_tokens, 60);
        assert_eq!(s.total.input_tokens, 360);
        assert_eq!(s.total.output_tokens, 140);
        assert_eq!(s.total.turns, 3);
        assert!((s.total.cost_usd - 0.0031).abs() < 1e-9);
    }

    #[test]
    fn all_time_summary_returns_every_row() {
        let (store, _tmp) = fresh_store();
        for caller in ["chat", "compiler", "utility"] {
            store
                .record(TurnUsageRecord {
                    caller: caller.into(),
                    provider_id: "p".into(),
                    model_id: "m".into(),
                    usage: TokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                    cost_usd: 0.0,
                    session_id: None,
                })
                .unwrap();
        }
        let s = store.summary(UsageWindow::AllTime).unwrap();
        assert_eq!(s.by_caller.len(), 3);
        assert_eq!(s.total.turns, 3);
    }
}
