//! JobRunner — durable retry-and-skip coordinator for background memory jobs.
//!
//! Phase 8A T-A2 (v2 §Sprint 1 / T-A2 + §0.5 Δ-3 + Δ-4 + Δ-10).  Every
//! background job that mutates memory state (rolling summary, compile,
//! fact extract, experience extract, diary write) runs through
//! [`JobRunner::run`] so we get three guarantees uniformly:
//!
//! 1. **Bounded retries** — a `(kind, target)` pair that fails
//!    `max_retries` consecutive times flips to `Skipped` and short-circuits
//!    on subsequent calls, preventing poisoned data from re-burning LLM
//!    budget every turn.
//! 2. **Concurrency cap** — a [`tokio::sync::Semaphore`] limits how many
//!    jobs may be in-flight at once across the whole app, protecting the
//!    LLM provider from a thundering herd after a long sleep / boot.
//! 3. **Audit + persistence** — every failure / skip emits a
//!    `memory_job_failed` / `memory_job_skipped` event and is durably
//!    recorded in `jobs.db` so a crash doesn't reset the failure counter.
//!
//! The store is intentionally a separate SQLite file (`jobs.db`) sitting
//! next to `memory.db` (v2 §0.5 Δ-6 + 8A.2 review checklist):
//! co-locating job bookkeeping with user memory would force the same
//! mutex across two unrelated workloads and risk lock-contention in
//! the hot recall path.
//!
//! The closure passed to [`JobRunner::run`] returns
//! [`anyhow::Result<T>`] so individual jobs can `?`-bubble whatever
//! native error type they have without all sharing one enum.
//!
//! ## `allow(dead_code)` rationale
//!
//! Phase 8A.2 lands the runner + tests + AppState wiring; first real
//! producer is 8A.7 (RollingSummarizer).  Every public item is exercised
//! by the in-file `#[cfg(test)] mod tests` suite, but the bin target
//! does not yet *call* `run` / `reset` / `current_attempt` / the
//! private `persist_*` helpers, so we annotate at the module level
//! rather than spray nine separate attributes.  Remove the module
//! attribute once 8A.7 is merged.

#![allow(dead_code)]

use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Semaphore;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};

/// Lifecycle state of a single `(job_kind, job_target)` pair persisted
/// in the `job_attempts` table.
///
/// `Active` and `Done` both allow re-execution; only `Skipped` short-circuits
/// [`JobRunner::run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Pending or recoverable — the next [`JobRunner::run`] call will
    /// invoke the closure.  This is the initial state and the state a
    /// failed-but-still-under-budget job is left in.
    Active,
    /// Attempt counter reached `max_retries`; the closure will not run
    /// again until [`JobRunner::reset`] flips it back to `Active`.
    Skipped,
    /// Most recent execution succeeded.  The next call re-runs the
    /// closure (for periodic / idempotent work) but resets the attempt
    /// counter, so a single transient failure after a long success
    /// streak does not push the job into `Skipped`.
    Done,
}

impl JobStatus {
    fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Active => "active",
            JobStatus::Skipped => "skipped",
            JobStatus::Done => "done",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "skipped" => JobStatus::Skipped,
            "done" => JobStatus::Done,
            _ => JobStatus::Active,
        }
    }
}

/// One row in the `job_attempts` table.
///
/// Returned by [`JobRunner::current_attempt`] for tests and a future
/// MemoryJobsStatusCard UI (T-H2 / T-UI-9) so the user can see why a
/// job is silent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAttempt {
    /// Logical job family, e.g. `"compile_today"` / `"rolling_summary"`.
    pub job_kind: String,
    /// Discriminator inside the family — typically a `session_id` or
    /// scope key so two parallel sessions don't share a single counter.
    pub job_target: String,
    /// Number of times the closure has been invoked.  Reset to `0` by
    /// [`JobRunner::reset`].
    pub attempt: u32,
    /// Stringified error from the most recent failure, or `None` after
    /// a clean success.
    pub last_error: Option<String>,
    /// Wall-clock timestamp of the most recent invocation.
    pub last_attempt_at: DateTime<Utc>,
    /// Lifecycle status — see [`JobStatus`].
    pub status: JobStatus,
}

/// Errors raised by [`JobRunner`] itself (closure errors are surfaced
/// separately as [`JobError::Generic`] only when the runner needs to
/// signal "this attempt failed" to the caller).
#[derive(Debug, Error)]
pub enum JobError {
    /// Underlying `jobs.db` SQLite error — connection open, schema
    /// migration, or row read/write failed.
    #[error("jobs.db sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Job semaphore was closed (only happens in shutdown / tests).
    #[error("job semaphore closed: {0}")]
    SemaphoreClosed(String),
    /// Internal lock poisoning or other JobRunner-owned failure.
    #[error("job runner internal error: {0}")]
    Internal(String),
    /// The wrapped closure returned `Err(_)` and the runner re-surfaces
    /// the message so the caller can decide whether to back off.
    /// Only used while the job is still under `max_retries`; once it
    /// flips to [`JobStatus::Skipped`] the runner returns `Ok(None)`
    /// instead.
    #[error("job attempt failed: {0}")]
    Generic(String),
}

/// Background job coordinator — see module docs.
pub struct JobRunner {
    db: Arc<Mutex<Connection>>,
    max_retries: u32,
    #[allow(dead_code)] // surfaced via `current_attempt` and the future T-UI-9 card
    max_concurrent: usize,
    semaphore: Arc<Semaphore>,
}

impl std::fmt::Debug for JobRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRunner")
            .field("max_retries", &self.max_retries)
            .field("max_concurrent", &self.max_concurrent)
            .field("available_permits", &self.semaphore.available_permits())
            .finish()
    }
}

impl JobRunner {
    /// Open (or create) the jobs database at `jobs_db_path` and build
    /// the runner.  Idempotent: re-opening an existing file simply
    /// runs `CREATE TABLE IF NOT EXISTS` and reuses prior state.
    ///
    /// `max_retries` is the inclusive cap — when `attempt` post-increment
    /// reaches this value the job is marked `Skipped`.
    /// `max_concurrent` sizes the in-process [`Semaphore`]; pick the
    /// LLM provider's recommended parallelism (default 3 per v2
    /// §0.5 Δ-9 `compiler.max_concurrent_llm`).
    pub fn open(
        jobs_db_path: &Path,
        max_retries: u32,
        max_concurrent: usize,
    ) -> Result<Self, JobError> {
        if let Some(parent) = jobs_db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                JobError::Internal(format!(
                    "failed to create jobs.db parent dir {parent:?}: {e}"
                ))
            })?;
        }

        let conn = Connection::open(jobs_db_path)?;
        conn.execute_batch(
            r"CREATE TABLE IF NOT EXISTS job_attempts (
                job_kind        TEXT NOT NULL,
                job_target      TEXT NOT NULL,
                attempt         INTEGER NOT NULL DEFAULT 0,
                last_error      TEXT,
                last_attempt_at TEXT NOT NULL,
                status          TEXT NOT NULL,
                PRIMARY KEY (job_kind, job_target)
            );",
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            max_retries,
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        })
    }

    /// Build a runner backed by an in-memory SQLite database — used by
    /// unit tests in this and sibling memory modules (8A.7
    /// RollingSummarizer) so the suite does not need a tempdir.
    #[cfg(test)]
    pub(crate) fn open_in_memory_for_tests(
        max_retries: u32,
        max_concurrent: usize,
    ) -> Result<Self, JobError> {
        Self::open_in_memory(max_retries, max_concurrent)
    }

    /// Build a runner backed by an in-memory SQLite database — used by
    /// unit tests so the suite does not need a tempdir.
    #[cfg(test)]
    fn open_in_memory(max_retries: u32, max_concurrent: usize) -> Result<Self, JobError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            r"CREATE TABLE IF NOT EXISTS job_attempts (
                job_kind        TEXT NOT NULL,
                job_target      TEXT NOT NULL,
                attempt         INTEGER NOT NULL DEFAULT 0,
                last_error      TEXT,
                last_attempt_at TEXT NOT NULL,
                status          TEXT NOT NULL,
                PRIMARY KEY (job_kind, job_target)
            );",
        )?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            max_retries,
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        })
    }

    /// Read the current row for `(kind, target)`, or return a synthesized
    /// `Active` row with `attempt = 0` if none exists yet.  Synchronous
    /// helper used by tests and a future status UI.
    pub fn current_attempt(&self, kind: &str, target: &str) -> Result<JobAttempt, JobError> {
        let guard = self
            .db
            .lock()
            .map_err(|e| JobError::Internal(format!("jobs.db mutex poisoned: {e}")))?;
        read_attempt(&guard, kind, target)
    }

    /// Reset the row back to `Active` with `attempt = 0` and clear
    /// `last_error`.  Use when the operator manually un-quarantines a
    /// previously-skipped job (T-H2 / future MemoryJobsStatusCard).
    pub async fn reset(&self, kind: &str, target: &str) -> Result<(), JobError> {
        let db = self.db.clone();
        let kind = kind.to_string();
        let target = target.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), JobError> {
            let guard = db
                .lock()
                .map_err(|e| JobError::Internal(format!("jobs.db mutex poisoned: {e}")))?;
            let now = Utc::now().to_rfc3339();
            guard.execute(
                "INSERT INTO job_attempts (job_kind, job_target, attempt, last_error, last_attempt_at, status)
                 VALUES (?1, ?2, 0, NULL, ?3, 'active')
                 ON CONFLICT(job_kind, job_target) DO UPDATE SET
                    attempt = 0,
                    last_error = NULL,
                    last_attempt_at = excluded.last_attempt_at,
                    status = 'active'",
                params![kind, target, now],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| JobError::Internal(format!("reset spawn_blocking joined with error: {e}")))?
    }

    /// Run `f()` under the runner's retry / concurrency / audit envelope.
    ///
    /// Return values:
    /// - `Ok(Some(t))` — the closure ran and succeeded; `status` flipped to
    ///   `Done` and `attempt` reset to `0` so future calls aren't penalised
    ///   by long-ago failures.
    /// - `Ok(None)` — the `(kind, target)` is currently `Skipped` (already
    ///   exhausted its retry budget) **or** this call exhausted the budget
    ///   on its own.  Either way the closure will not run again until
    ///   [`Self::reset`] is invoked.
    /// - `Err(JobError::Generic(msg))` — the closure failed but is still
    ///   under the retry budget; the caller may retry on the next tick.
    ///   `attempt` was incremented and an audit `memory_job_failed`
    ///   event was emitted.
    /// - other `Err(JobError::*)` — infrastructure-level failure (sqlite,
    ///   semaphore); the closure may or may not have run.
    pub async fn run<F, Fut, T>(
        &self,
        kind: &str,
        target: &str,
        audit_ctx: &AuditContext<'_>,
        f: F,
    ) -> Result<Option<T>, JobError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = anyhow::Result<T>> + Send,
    {
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| JobError::SemaphoreClosed(e.to_string()))?;

        let current = self.current_attempt(kind, target)?;
        if matches!(current.status, JobStatus::Skipped) {
            tracing::debug!(
                job = kind,
                target = target,
                "JobRunner::run short-circuited: status already Skipped"
            );
            return Ok(None);
        }

        match f().await {
            Ok(value) => {
                self.persist_done(kind, target).await?;
                Ok(Some(value))
            }
            Err(err) => {
                let err_str = format!("{err:#}");
                let next_attempt = current.attempt.saturating_add(1);
                if next_attempt >= self.max_retries {
                    self.persist_skipped(kind, target, &err_str, next_attempt)
                        .await?;
                    MemoryAuditEmitter::memory_job_skipped(audit_ctx, kind, next_attempt, &err_str);
                    Ok(None)
                } else {
                    self.persist_failed(kind, target, &err_str, next_attempt)
                        .await?;
                    MemoryAuditEmitter::memory_job_failed(
                        audit_ctx,
                        kind,
                        next_attempt,
                        self.max_retries,
                        &err_str,
                    );
                    Err(JobError::Generic(err_str))
                }
            }
        }
    }

    async fn persist_done(&self, kind: &str, target: &str) -> Result<(), JobError> {
        self.upsert_status(kind, target, JobStatus::Done, None, 0)
            .await
    }

    async fn persist_failed(
        &self,
        kind: &str,
        target: &str,
        err: &str,
        attempt: u32,
    ) -> Result<(), JobError> {
        self.upsert_status(kind, target, JobStatus::Active, Some(err), attempt)
            .await
    }

    async fn persist_skipped(
        &self,
        kind: &str,
        target: &str,
        err: &str,
        attempt: u32,
    ) -> Result<(), JobError> {
        self.upsert_status(kind, target, JobStatus::Skipped, Some(err), attempt)
            .await
    }

    async fn upsert_status(
        &self,
        kind: &str,
        target: &str,
        status: JobStatus,
        last_error: Option<&str>,
        attempt: u32,
    ) -> Result<(), JobError> {
        let db = self.db.clone();
        let kind = kind.to_string();
        let target = target.to_string();
        let last_error = last_error.map(|s| s.to_string());
        let status_str = status.as_str().to_string();
        tokio::task::spawn_blocking(move || -> Result<(), JobError> {
            let guard = db
                .lock()
                .map_err(|e| JobError::Internal(format!("jobs.db mutex poisoned: {e}")))?;
            let now = Utc::now().to_rfc3339();
            guard.execute(
                "INSERT INTO job_attempts (job_kind, job_target, attempt, last_error, last_attempt_at, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(job_kind, job_target) DO UPDATE SET
                    attempt = excluded.attempt,
                    last_error = excluded.last_error,
                    last_attempt_at = excluded.last_attempt_at,
                    status = excluded.status",
                params![kind, target, attempt, last_error, now, status_str],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| JobError::Internal(format!("upsert spawn_blocking joined with error: {e}")))?
    }
}

fn read_attempt(conn: &Connection, kind: &str, target: &str) -> Result<JobAttempt, JobError> {
    let row = conn
        .query_row(
            "SELECT attempt, last_error, last_attempt_at, status
             FROM job_attempts
             WHERE job_kind = ?1 AND job_target = ?2",
            params![kind, target],
            |row| {
                let attempt: i64 = row.get(0)?;
                let last_error: Option<String> = row.get(1)?;
                let ts: String = row.get(2)?;
                let status: String = row.get(3)?;
                Ok((attempt, last_error, ts, status))
            },
        )
        .ok();

    Ok(match row {
        Some((attempt, last_error, ts, status)) => {
            let last_attempt_at = DateTime::parse_from_rfc3339(&ts)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            JobAttempt {
                job_kind: kind.to_string(),
                job_target: target.to_string(),
                attempt: attempt.max(0) as u32,
                last_error,
                last_attempt_at,
                status: JobStatus::from_str(&status),
            }
        }
        None => JobAttempt {
            job_kind: kind.to_string(),
            job_target: target.to_string(),
            attempt: 0,
            last_error: None,
            last_attempt_at: Utc::now(),
            status: JobStatus::Active,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::scope::MemoryScopeResolver;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::time::Duration;

    fn ctx_for_test() -> (
        crate::modules::memory::scope::MemoryExecutionScope,
        &'static str,
    ) {
        let scope = MemoryScopeResolver::resolve(Some("sess"), Some("proj"), None);
        (scope, "trace-test")
    }

    #[tokio::test]
    async fn skip_after_max_retries() {
        let runner = JobRunner::open_in_memory(3, 4).unwrap();
        let (scope, _trace) = ctx_for_test();
        let ctx = AuditContext::from_scope(&scope);

        for expected_attempt in 1..=2u32 {
            let res: Result<Option<()>, JobError> = runner
                .run("compile_today", "sess-1", &ctx, || async {
                    Err::<(), _>(anyhow::anyhow!("boom"))
                })
                .await;
            assert!(matches!(res, Err(JobError::Generic(_))));
            let row = runner.current_attempt("compile_today", "sess-1").unwrap();
            assert_eq!(row.attempt, expected_attempt);
            assert_eq!(row.status, JobStatus::Active);
        }

        let third: Result<Option<()>, JobError> = runner
            .run("compile_today", "sess-1", &ctx, || async {
                Err::<(), _>(anyhow::anyhow!("boom"))
            })
            .await;
        assert!(matches!(third, Ok(None)));
        let row = runner.current_attempt("compile_today", "sess-1").unwrap();
        assert_eq!(row.status, JobStatus::Skipped);
        assert_eq!(row.attempt, 3);

        let fourth: Result<Option<()>, JobError> = runner
            .run("compile_today", "sess-1", &ctx, || async {
                panic!("closure must not run while Skipped");
            })
            .await;
        assert!(matches!(fourth, Ok(None)));
    }

    #[tokio::test]
    async fn success_returns_some_t_and_marks_done() {
        let runner = JobRunner::open_in_memory(3, 2).unwrap();
        let (scope, _) = ctx_for_test();
        let ctx = AuditContext::from_scope(&scope);

        let out = runner
            .run("rolling_summary", "sess-A", &ctx, || async {
                Ok::<_, anyhow::Error>(42_i32)
            })
            .await
            .expect("infrastructure call must not fail");
        assert_eq!(out, Some(42));
        let row = runner.current_attempt("rolling_summary", "sess-A").unwrap();
        assert_eq!(row.status, JobStatus::Done);
        assert_eq!(row.attempt, 0, "success path resets attempt counter");
        assert!(row.last_error.is_none());
    }

    #[tokio::test]
    async fn reset_allows_retry() {
        let runner = JobRunner::open_in_memory(2, 2).unwrap();
        let (scope, _) = ctx_for_test();
        let ctx = AuditContext::from_scope(&scope);

        for _ in 0..2 {
            let _ = runner
                .run("fact_extract", "sess-B", &ctx, || async {
                    Err::<(), _>(anyhow::anyhow!("nope"))
                })
                .await;
        }
        let skipped = runner.current_attempt("fact_extract", "sess-B").unwrap();
        assert_eq!(skipped.status, JobStatus::Skipped);

        runner.reset("fact_extract", "sess-B").await.unwrap();
        let after = runner.current_attempt("fact_extract", "sess-B").unwrap();
        assert_eq!(after.status, JobStatus::Active);
        assert_eq!(after.attempt, 0);

        let out = runner
            .run("fact_extract", "sess-B", &ctx, || async {
                Ok::<_, anyhow::Error>(())
            })
            .await
            .unwrap();
        assert_eq!(out, Some(()));
    }

    #[tokio::test]
    async fn concurrent_limit_respected() {
        let runner = Arc::new(JobRunner::open_in_memory(5, 2).unwrap());
        let (scope, _) = ctx_for_test();

        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..4 {
            let runner = runner.clone();
            let in_flight = in_flight.clone();
            let max_seen = max_seen.clone();
            let scope = scope.clone();
            handles.push(tokio::spawn(async move {
                let ctx = AuditContext::from_scope(&scope);
                let target = format!("t-{i}");
                let _ = runner
                    .run("conc", &target, &ctx, || async {
                        let cur = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        let mut snap = max_seen.load(Ordering::SeqCst);
                        while cur > snap {
                            match max_seen.compare_exchange(
                                snap,
                                cur,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            ) {
                                Ok(_) => break,
                                Err(actual) => snap = actual,
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(40)).await;
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                        Ok::<_, anyhow::Error>(())
                    })
                    .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let observed = max_seen.load(Ordering::SeqCst);
        assert!(
            observed <= 2,
            "expected <= 2 concurrent jobs, observed {observed}"
        );
        assert!(observed >= 1, "no job ran concurrently at all?");
    }

    #[tokio::test]
    async fn failure_then_success_resets_attempt_on_done() {
        let runner = JobRunner::open_in_memory(5, 2).unwrap();
        let (scope, _) = ctx_for_test();
        let ctx = AuditContext::from_scope(&scope);
        let calls = Arc::new(AtomicU32::new(0));

        for _ in 0..2 {
            let calls = calls.clone();
            let _ = runner
                .run("exp_extract", "sess-Z", &ctx, || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(anyhow::anyhow!("flaky"))
                })
                .await;
        }
        let mid = runner.current_attempt("exp_extract", "sess-Z").unwrap();
        assert_eq!(mid.attempt, 2);
        assert_eq!(mid.status, JobStatus::Active);

        let calls_clone = calls.clone();
        let out = runner
            .run("exp_extract", "sess-Z", &ctx, || async move {
                calls_clone.fetch_add(1, Ordering::SeqCst);
                Ok::<_, anyhow::Error>("ok")
            })
            .await
            .unwrap();
        assert_eq!(out, Some("ok"));

        let after = runner.current_attempt("exp_extract", "sess-Z").unwrap();
        assert_eq!(
            after.status,
            JobStatus::Done,
            "success after failures must mark Done"
        );
        assert_eq!(
            after.attempt, 0,
            "Done path resets the attempt counter so a future single failure won't trip skip"
        );
        assert!(after.last_error.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn audit_emit_on_failure_and_skip() {
        // Without a registered AppHandle, emit_to_frontend is a no-op and we
        // simply assert tracing-side instrumentation does not panic and that
        // the runner reaches the right terminal state for both branches.
        let runner = JobRunner::open_in_memory(2, 2).unwrap();
        let (scope, _) = ctx_for_test();
        let ctx = AuditContext::from_scope(&scope);

        let first: Result<Option<()>, JobError> = runner
            .run("audit_kind", "tgt", &ctx, || async {
                Err::<(), _>(anyhow::anyhow!("first failure → memory_job_failed"))
            })
            .await;
        assert!(matches!(first, Err(JobError::Generic(_))));
        assert_eq!(
            runner.current_attempt("audit_kind", "tgt").unwrap().status,
            JobStatus::Active
        );

        let second: Result<Option<()>, JobError> = runner
            .run("audit_kind", "tgt", &ctx, || async {
                Err::<(), _>(anyhow::anyhow!("second → memory_job_skipped"))
            })
            .await;
        assert!(matches!(second, Ok(None)));
        assert_eq!(
            runner.current_attempt("audit_kind", "tgt").unwrap().status,
            JobStatus::Skipped
        );
    }
}
