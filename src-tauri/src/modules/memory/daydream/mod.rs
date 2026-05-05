//! DayDream engine — background memory consolidation inspired by
//! Claude Code AutoDream and human sleep-consolidation mechanisms.
//!
//! # Architecture
//!
//! ```text
//! DayDreamEngine
//!   ├── DayDreamScheduler   — idle detection + tokio scheduling
//!   ├── MemoryConsolidator  — Prune / Merge / Refresh pipeline
//!   └── report types        — DayDreamReport, DayDreamConfig, …
//! ```

pub mod consolidator;
pub mod reflector;
pub mod report;
pub mod scheduler;

pub use consolidator::MemoryConsolidator;
#[allow(unused_imports)]
pub use reflector::{DayDreamReflectionReport, DayDreamReflector};
pub use report::*;
pub use scheduler::DayDreamScheduler;

use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::SharedMemoryProvider;
use std::sync::Arc;

/// Day Dream 引擎 — 统一入口。
///
/// Owns the scheduler and consolidator, providing a single handle for
/// the rest of the application to start / stop / query the background
/// memory consolidation system.
pub struct DayDreamEngine {
    /// 调度器：管理空闲检测与定时触发。
    pub scheduler: Arc<DayDreamScheduler>,
    /// 巩固器：执行 Prune / Merge / Refresh。
    pub consolidator: Arc<MemoryConsolidator>,
}

impl DayDreamEngine {
    /// 创建一个新的 DayDream 引擎实例。
    ///
    /// When `llm` is `Some`, the consolidator will use LLM-assisted merge
    /// and refresh logic.  When `None`, it falls back to rule-based logic.
    #[must_use]
    pub fn new(
        provider: SharedMemoryProvider,
        config: DayDreamConfig,
        llm: Option<Arc<dyn UtilityLlm>>,
    ) -> Self {
        let scheduler = Arc::new(DayDreamScheduler::new(config));
        let mut consolidator = MemoryConsolidator::new(provider);
        if let Some(llm) = llm {
            consolidator = consolidator.with_llm(llm);
        }
        let consolidator = Arc::new(consolidator);
        Self {
            scheduler,
            consolidator,
        }
    }

    /// 启动 DayDream 后台服务。
    ///
    /// Returns a `JoinHandle` for the background monitor loop.  The
    /// loop is non-blocking and runs entirely within the tokio runtime.
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        self.scheduler.start(Arc::clone(&self.consolidator))
    }

    /// 停止 DayDream（将 config.enabled 设为 false）。
    pub async fn stop(&self) {
        let mut cfg = self.scheduler.get_config().await;
        cfg.enabled = false;
        self.scheduler.update_config(cfg).await;
    }
}
