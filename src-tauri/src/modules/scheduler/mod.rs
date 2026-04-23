//! Scheduler module - provides cron job scheduling functionality
//!
//! See [`autonomous_denylist`] for P2-13 autonomous tool denylist constants.
//!
//! This module defines the Scheduler trait for managing scheduled tasks.
//! P1-6 — [`spawn_scheduler_self_repair_watchdog`] clears stuck `CronRun`
//! rows (`finished_at == None`) for future async job execution.

pub mod autonomous_denylist;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// A scheduled cron job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub command: String,
    pub description: String,
    pub status: JobStatus,
}

/// Status of a cron job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Active,
    Paused,
    Disabled,
}

/// A record of a cron job run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronRun {
    pub job_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Error type for scheduler operations
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum SchedulerError {
    #[error("scheduler error: {0}")]
    Generic(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
}

/// Trait for scheduler providers
#[async_trait]
pub trait Scheduler: Send + Sync {
    /// Add a new cron job
    async fn add(
        &self,
        id: &str,
        schedule: &str,
        command: &str,
        description: &str,
    ) -> Result<(), SchedulerError>;

    /// Remove a cron job
    async fn remove(&self, id: &str) -> Result<(), SchedulerError>;

    /// List all cron jobs
    async fn list(&self) -> Result<Vec<CronJob>, SchedulerError>;

    /// Run a job immediately
    async fn run_now(&self, id: &str) -> Result<String, SchedulerError>;

    /// Get run history for a job
    async fn get_runs(&self, id: &str, limit: usize) -> Result<Vec<CronRun>, SchedulerError>;

    /// Update job status
    #[allow(dead_code)]
    async fn set_status(&self, id: &str, status: JobStatus) -> Result<(), SchedulerError>;

    /// Close runs that never completed (`finished_at` unset) after `stuck_after_secs`.
    /// Returns the number of runs repaired.
    async fn repair_stuck_runs(&self, stuck_after_secs: u64) -> usize;
}

/// In-memory implementation of Scheduler
#[derive(Debug, Default)]
pub struct InMemoryScheduler {
    jobs: RwLock<HashMap<String, CronJob>>,
    runs: RwLock<HashMap<String, Vec<CronRun>>>,
}

impl InMemoryScheduler {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
            runs: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl Scheduler for InMemoryScheduler {
    async fn add(
        &self,
        id: &str,
        schedule: &str,
        command: &str,
        description: &str,
    ) -> Result<(), SchedulerError> {
        // Validate cron expression by attempting to parse it
        // We accept standard cron format: min hour day month weekday
        let parts: Vec<&str> = schedule.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(SchedulerError::InvalidCron(format!(
                "expected 5 fields, got {}: {}",
                parts.len(),
                schedule
            )));
        }

        let job = CronJob {
            id: id.to_string(),
            schedule: schedule.to_string(),
            command: command.to_string(),
            description: description.to_string(),
            status: JobStatus::Active,
        };

        let mut jobs = self.jobs.write().await;
        jobs.insert(id.to_string(), job);

        let mut runs = self.runs.write().await;
        runs.insert(id.to_string(), Vec::new());

        Ok(())
    }

    async fn remove(&self, id: &str) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        if jobs.remove(id).is_none() {
            return Err(SchedulerError::JobNotFound(id.to_string()));
        }

        let mut runs = self.runs.write().await;
        runs.remove(id);

        Ok(())
    }

    async fn list(&self) -> Result<Vec<CronJob>, SchedulerError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().cloned().collect())
    }

    async fn run_now(&self, id: &str) -> Result<String, SchedulerError> {
        let jobs = self.jobs.read().await;
        let job = jobs
            .get(id)
            .ok_or_else(|| SchedulerError::JobNotFound(id.to_string()))?;

        // Clone the command before dropping the lock
        let command = job.command.clone();
        drop(jobs);

        if autonomous_denylist::autonomy_denylist_enforced() {
            for tool in autonomous_denylist::AUTONOMOUS_DENIED_TOOLS {
                if command.contains(tool) {
                    return Err(SchedulerError::Generic(format!(
                        "cron command blocked by autonomous denylist (mentions `{tool}`)"
                    )));
                }
            }
        }

        let run = CronRun {
            job_id: id.to_string(),
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            output: Some(format!("Would execute: {}", command)),
            error: None,
        };

        // Record the run
        let mut runs = self.runs.write().await;
        if let Some(job_runs) = runs.get_mut(id) {
            job_runs.push(run);
        }

        Ok(format!("Executed: {}", command))
    }

    async fn get_runs(&self, id: &str, limit: usize) -> Result<Vec<CronRun>, SchedulerError> {
        let runs = self.runs.read().await;
        let job_runs = runs
            .get(id)
            .ok_or_else(|| SchedulerError::JobNotFound(id.to_string()))?;
        Ok(job_runs.iter().rev().take(limit).cloned().collect())
    }

    async fn set_status(&self, id: &str, status: JobStatus) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| SchedulerError::JobNotFound(id.to_string()))?;
        job.status = status;
        Ok(())
    }

    async fn repair_stuck_runs(&self, stuck_after_secs: u64) -> usize {
        let now = chrono::Utc::now();
        let threshold_secs = stuck_after_secs.max(60) as i64;
        let mut runs = self.runs.write().await;
        let mut fixed = 0usize;
        for vec in runs.values_mut() {
            for run in vec.iter_mut() {
                if run.finished_at.is_some() {
                    continue;
                }
                let age_secs = (now - run.started_at).num_seconds();
                if age_secs > threshold_secs {
                    run.finished_at = Some(now);
                    run.error = Some(format!(
                        "self-repair watchdog: run stuck for {age_secs}s (threshold {threshold_secs}s)"
                    ));
                    fixed += 1;
                }
            }
        }
        fixed
    }
}

/// Global scheduler provider instance
pub type SharedScheduler = Arc<dyn Scheduler>;

/// Default in-memory scheduler
pub fn default_scheduler() -> SharedScheduler {
    Arc::new(InMemoryScheduler::new())
}

/// Background repair for stuck in-memory cron runs (see [`Scheduler::repair_stuck_runs`]).
pub fn spawn_scheduler_self_repair_watchdog(scheduler: SharedScheduler) {
    if std::env::var("IF2AI_SCHEDULER_WATCHDOG")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return;
    }
    let interval_secs = std::env::var("IF2AI_SCHEDULER_REPAIR_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120_u64)
        .max(30);
    // P1-6 alignment: prefer the unified `IF2AI_STUCK_AFTER_SECS` env so the
    // scheduler watchdog and runtime stuck-stream detection share one knob;
    // legacy `IF2AI_SCHEDULER_STUCK_AFTER_SECS` is honoured when set
    // explicitly so existing deployments keep working.
    let stuck_secs = std::env::var("IF2AI_SCHEDULER_STUCK_AFTER_SECS")
        .ok()
        .or_else(|| std::env::var("IF2AI_STUCK_AFTER_SECS").ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(900_u64);
    let task = move |scheduler: SharedScheduler| async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let n = scheduler.repair_stuck_runs(stuck_secs).await;
            if n > 0 {
                tracing::warn!("[self_repair] scheduler repaired {n} stuck cron run(s)");
            }
        }
    };
    // P0-Hotfix: bootstrap may call this before Tauri's async runtime is
    // installed (build_app_bootstrap runs synchronously). When no reactor
    // is current, fall back to a dedicated thread that owns its own
    // single-threaded runtime — same pattern used by `init_learning_module`
    // in [bootstrap/app.rs]. This keeps the watchdog optional & safe.
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(task(scheduler));
        }
        Err(_) => {
            tracing::warn!(
                "[self_repair] no current Tokio runtime at watchdog spawn site; \
                 falling back to dedicated thread"
            );
            std::thread::Builder::new()
                .name("if2ai-scheduler-watchdog".into())
                .spawn(move || {
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("[self_repair] failed to build fallback runtime: {e}");
                            return;
                        }
                    };
                    rt.block_on(task(scheduler));
                })
                .ok();
        }
    }
}

#[cfg(test)]
impl InMemoryScheduler {
    pub async fn inject_stuck_run_for_test(
        &self,
        job_id: &str,
        started_at: chrono::DateTime<chrono::Utc>,
    ) {
        let mut runs = self.runs.write().await;
        runs.entry(job_id.to_string()).or_default().push(CronRun {
            job_id: job_id.to_string(),
            started_at,
            finished_at: None,
            output: None,
            error: None,
        });
    }
}

#[cfg(test)]
mod stuck_run_repair_tests {
    use super::*;

    #[tokio::test]
    async fn repair_marks_old_pending_runs() {
        let s = InMemoryScheduler::new();
        let started = chrono::Utc::now() - chrono::Duration::hours(2);
        s.inject_stuck_run_for_test("job-a", started).await;
        let n = s.repair_stuck_runs(300).await;
        assert_eq!(n, 1);
        let runs = s.get_runs("job-a", 5).await.unwrap();
        assert!(runs[0].finished_at.is_some());
        assert!(runs[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("self-repair"));
    }
}
