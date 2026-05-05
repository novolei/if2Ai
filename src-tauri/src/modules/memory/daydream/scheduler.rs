//! DayDream scheduler — idle detection and tokio-based background scheduling.
//!
//! The scheduler monitors user activity and triggers a memory consolidation
//! cycle when the agent has been idle for longer than the configured threshold.
//! It can also be triggered manually or on session-end.

use super::consolidator::MemoryConsolidator;
use super::report::*;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};

/// 历史报告保留上限。
const MAX_HISTORY: usize = 50;

/// DayDream 空闲检测调度器。
///
/// Monitors `last_user_activity` and fires a consolidation cycle via
/// [`MemoryConsolidator`] when the idle threshold is exceeded.  All
/// state is behind `Arc` / `RwLock` so the scheduler can be shared
/// across Tauri command handlers and the background monitor task.
pub struct DayDreamScheduler {
    last_user_activity: Arc<RwLock<Instant>>,
    running: Arc<AtomicBool>,
    config: Arc<RwLock<DayDreamConfig>>,
    history: Arc<RwLock<Vec<DayDreamReport>>>,
    state: Arc<RwLock<DayDreamState>>,
}

impl DayDreamScheduler {
    /// 创建一个新的调度器实例。
    #[must_use]
    pub fn new(config: DayDreamConfig) -> Self {
        let initial_state = if config.enabled {
            DayDreamState::Idle
        } else {
            DayDreamState::Disabled
        };
        Self {
            last_user_activity: Arc::new(RwLock::new(Instant::now())),
            running: Arc::new(AtomicBool::new(false)),
            config: Arc::new(RwLock::new(config)),
            history: Arc::new(RwLock::new(Vec::new())),
            state: Arc::new(RwLock::new(initial_state)),
        }
    }

    /// 启动后台监控循环。
    ///
    /// - 每 60 秒检查一次空闲时长。
    /// - 超过 `idle_trigger_minutes` 且未在运行中时触发巩固。
    /// - 巩固完成后记录报告到 history。
    pub fn start(self: &Arc<Self>, consolidator: Arc<MemoryConsolidator>) -> JoinHandle<()> {
        let scheduler = Arc::clone(self);
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(60);
            loop {
                tokio::time::sleep(check_interval).await;

                let cfg = scheduler.config.read().await.clone();
                if !cfg.enabled {
                    continue;
                }

                // Already running a cycle — skip.
                if scheduler.running.load(Ordering::SeqCst) {
                    continue;
                }

                let idle_threshold = Duration::from_secs(cfg.idle_trigger_minutes * 60);
                let last_activity = *scheduler.last_user_activity.read().await;
                if last_activity.elapsed() < idle_threshold {
                    continue;
                }

                // Trigger a consolidation cycle.
                if let Err(e) = scheduler.run_cycle_inner(&consolidator, &cfg).await {
                    tracing::warn!(
                        error = %e,
                        "[daydream] background consolidation cycle failed"
                    );
                }
            }
        })
    }

    /// 会话结束时立即触发巩固（如果配置启用）。
    pub async fn on_session_end(&self, consolidator: &MemoryConsolidator) {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled || !cfg.session_end_trigger {
            return;
        }
        if self.running.load(Ordering::SeqCst) {
            tracing::debug!("[daydream] session-end trigger skipped: cycle already running");
            return;
        }
        if let Err(e) = self.run_cycle_inner(consolidator, &cfg).await {
            tracing::warn!(
                error = %e,
                "[daydream] session-end consolidation cycle failed"
            );
        }
    }

    /// 记录用户活动，重置空闲计时器。
    pub fn touch(&self) {
        let activity = Arc::clone(&self.last_user_activity);
        // Fire-and-forget write — non-blocking for the caller.
        tokio::spawn(async move {
            *activity.write().await = Instant::now();
        });
    }

    /// 获取当前状态。
    pub async fn state(&self) -> DayDreamState {
        self.state.read().await.clone()
    }

    /// 获取历史报告（最近 50 条）。
    pub async fn history(&self) -> Vec<DayDreamReport> {
        self.history.read().await.clone()
    }

    /// 更新配置。
    pub async fn update_config(&self, config: DayDreamConfig) {
        let new_state = if config.enabled {
            // Preserve Running/Completed if we are in one of those states.
            let current = self.state.read().await.clone();
            match current {
                DayDreamState::Disabled | DayDreamState::Idle => DayDreamState::Idle,
                other => other,
            }
        } else {
            DayDreamState::Disabled
        };
        *self.state.write().await = new_state;
        *self.config.write().await = config;
    }

    /// 获取当前配置。
    pub async fn get_config(&self) -> DayDreamConfig {
        self.config.read().await.clone()
    }

    /// 手动触发一次巩固周期。
    pub async fn trigger_manual(
        &self,
        consolidator: &MemoryConsolidator,
    ) -> Result<DayDreamReport, MemoryError> {
        let cfg = self.config.read().await.clone();
        self.run_cycle_inner(consolidator, &cfg).await
    }

    // ── internal ────────────────────────────────────────────────────

    /// 执行一次巩固周期并将报告写入历史。
    async fn run_cycle_inner(
        &self,
        consolidator: &MemoryConsolidator,
        config: &DayDreamConfig,
    ) -> Result<DayDreamReport, MemoryError> {
        // CAS guard — prevent concurrent runs.
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(MemoryError::Generic(
                "DayDream cycle already in progress".into(),
            ));
        }

        let started_at = chrono::Utc::now();
        *self.state.write().await = DayDreamState::Running { started_at };

        let scope = MemoryExecutionScope::global();
        let result = consolidator.run_cycle(&scope, config).await;

        // Always release the guard.
        self.running.store(false, Ordering::SeqCst);

        match result {
            Ok(report) => {
                let finished_at = report.finished_at;
                *self.state.write().await = DayDreamState::Completed { finished_at };
                // Append to history (bounded).
                let mut hist = self.history.write().await;
                hist.push(report.clone());
                if hist.len() > MAX_HISTORY {
                    let excess = hist.len() - MAX_HISTORY;
                    hist.drain(..excess);
                }
                tracing::info!(
                    cycle_id = %report.cycle_id,
                    duration_ms = report.duration_ms,
                    "[daydream] consolidation cycle completed"
                );
                Ok(report)
            }
            Err(e) => {
                *self.state.write().await = DayDreamState::Idle;
                Err(e)
            }
        }
    }
}
