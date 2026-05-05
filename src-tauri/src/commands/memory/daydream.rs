//! DayDream Tauri commands — expose DayDream engine operations to the frontend.
//!
//! The `DayDreamEngine` is managed as a separate Tauri state via
//! `app.manage(Arc<DayDreamEngine>)` to avoid invasive changes to
//! `AppState`.

use std::sync::Arc;
use tauri::State;

use crate::modules::memory::daydream::{
    DayDreamConfig, DayDreamEngine, DayDreamReport, DayDreamState,
};

/// 获取 DayDream 当前运行状态。
#[tauri::command]
pub async fn daydream_status(
    engine: State<'_, Arc<DayDreamEngine>>,
) -> Result<DayDreamState, String> {
    Ok(engine.scheduler.state().await)
}

/// 手动触发一次 DayDream 巩固周期。
#[tauri::command]
pub async fn daydream_trigger(
    engine: State<'_, Arc<DayDreamEngine>>,
) -> Result<DayDreamReport, String> {
    engine
        .scheduler
        .trigger_manual(&engine.consolidator)
        .await
        .map_err(|e| e.to_string())
}

/// 获取 DayDream 配置。
#[tauri::command]
pub async fn daydream_config_get(
    engine: State<'_, Arc<DayDreamEngine>>,
) -> Result<DayDreamConfig, String> {
    Ok(engine.scheduler.get_config().await)
}

/// 更新 DayDream 配置。
#[tauri::command]
pub async fn daydream_config_set(
    engine: State<'_, Arc<DayDreamEngine>>,
    config: DayDreamConfig,
) -> Result<(), String> {
    engine.scheduler.update_config(config).await;
    Ok(())
}

/// 获取 DayDream 历史报告。
#[tauri::command]
pub async fn daydream_history(
    engine: State<'_, Arc<DayDreamEngine>>,
) -> Result<Vec<DayDreamReport>, String> {
    Ok(engine.scheduler.history().await)
}
