//! DayDream report data structures —巩固周期的配置、状态与报告。
//!
//! All types here are serializable so they can be sent to the frontend
//! via Tauri IPC and persisted to disk when needed.

use super::reflector::DayDreamReflectionReport;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Day Dream 运行状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DayDreamState {
    /// 空闲等待中。
    Idle,
    /// 正在执行巩固周期。
    Running {
        /// Cycle start timestamp.
        started_at: DateTime<Utc>,
    },
    /// 最近一次已完成。
    Completed {
        /// Cycle finish timestamp.
        finished_at: DateTime<Utc>,
    },
    /// 已禁用。
    Disabled,
}

/// 巩固策略 — 控制每个 Day Dream 周期的激进程度。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsolidationStrategy {
    /// 仅修剪明显过时项。
    Conservative,
    /// 修剪 + 合并相似项。
    Balanced,
    /// 修剪 + 合并 + 主动刷新。
    Aggressive,
}

impl Default for ConsolidationStrategy {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Day Dream 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayDreamConfig {
    /// 是否启用 Day Dream 后台巩固。
    pub enabled: bool,
    /// 空闲触发阈值（分钟）。
    pub idle_trigger_minutes: u64,
    /// 会话结束时是否触发巩固。
    pub session_end_trigger: bool,
    /// 每周期处理的记忆条目上限。
    pub max_entries_per_cycle: usize,
    /// LLM token 预算（每个巩固周期）。
    pub llm_budget_tokens: usize,
    /// 巩固策略。
    pub strategy: ConsolidationStrategy,
}

impl Default for DayDreamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_trigger_minutes: 15,
            session_end_trigger: true,
            max_entries_per_cycle: 100,
            llm_budget_tokens: 4000,
            strategy: ConsolidationStrategy::default(),
        }
    }
}

/// 修剪报告 — 记录被移除的过时记忆。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PruneReport {
    /// 扫描的记忆条目数。
    pub scanned: usize,
    /// 被修剪的条目数。
    pub pruned: usize,
    /// 每条被修剪记忆的原因。
    pub reasons: Vec<String>,
}

/// 合并报告 — 记录相似记忆的聚类与合并。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MergeReport {
    /// 发现的相似聚类数。
    pub clusters_found: usize,
    /// 成功合并的聚类数。
    pub merged: usize,
    /// 被合并吞掉的条目数。
    pub entries_consumed: usize,
}

/// 刷新报告 — 记录过时记忆的主动更新。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RefreshReport {
    /// 候选刷新条目数。
    pub candidates: usize,
    /// 成功刷新的条目数。
    pub refreshed: usize,
    /// 判定无需刷新的条目数。
    pub unchanged: usize,
}

/// 完整的 Day Dream 巩固报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayDreamReport {
    /// 本次周期的唯一 ID（ULID）。
    pub cycle_id: String,
    /// 周期开始时间。
    pub started_at: DateTime<Utc>,
    /// 周期结束时间。
    pub finished_at: DateTime<Utc>,
    /// 耗时（毫秒）。
    pub duration_ms: u64,
    /// 修剪阶段报告。
    pub prune: PruneReport,
    /// 合并阶段报告。
    pub merge: MergeReport,
    /// 刷新阶段报告。
    pub refresh: RefreshReport,
    /// 反省阶段报告（Step 4，可选）。
    pub reflection: Option<DayDreamReflectionReport>,
    /// 使用的巩固策略。
    pub strategy: ConsolidationStrategy,
    /// 执行过程中的错误列表。
    pub errors: Vec<String>,
    /// Step 6 自动链接发现的新链接数。
    pub links_discovered: usize,
}
