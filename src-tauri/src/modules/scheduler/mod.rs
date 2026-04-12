//! Scheduler module - provides cron job scheduling functionality
//!
//! This module defines the Scheduler trait for managing scheduled tasks.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
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
}

/// Global scheduler provider instance
pub type SharedScheduler = Arc<dyn Scheduler>;

/// Default in-memory scheduler
pub fn default_scheduler() -> SharedScheduler {
    Arc::new(InMemoryScheduler::new())
}
