//! Lightweight self-repair / watchdog hooks (Steward-inspired P1-6).
//!
//! - Clears a stuck [`MemoryTicker`](crate::modules::memory::MemoryTicker) `daily_running` flag
//!   after a crash or aborted daily pipeline.
//! - Tracks consecutive per-tool failures from the streaming agent loop; the
//!   watchdog logs a warning and resets the streak (bounded “retry window”).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::modules::memory::MemoryTicker;

static TOOL_FAIL_STREAKS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

fn fail_map() -> &'static Mutex<HashMap<String, u32>> {
    TOOL_FAIL_STREAKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record a single tool outcome from the agent loop (streaming or future paths).
pub fn record_tool_outcome(tool_name: &str, ok: bool) {
    let Ok(mut g) = fail_map().lock() else {
        return;
    };
    if ok {
        g.remove(tool_name);
        return;
    }
    *g.entry(tool_name.to_string()).or_insert(0) += 1;
}

fn watchdog_disabled() -> bool {
    std::env::var("IF2AI_SELF_REPAIR_WATCHDOG")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
}

fn interval_secs() -> u64 {
    std::env::var("IF2AI_SELF_REPAIR_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
}

fn broken_tool_threshold() -> u32 {
    std::env::var("IF2AI_SELF_REPAIR_BROKEN_TOOL_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4)
}

fn scan_broken_tools() {
    let threshold = broken_tool_threshold();
    if threshold == 0 {
        return;
    }
    let Ok(mut g) = fail_map().lock() else {
        return;
    };
    let snapshot: Vec<(String, u32)> = g.iter().map(|(k, v)| (k.clone(), *v)).collect();
    for (tool, n) in snapshot {
        if n >= threshold {
            tracing::warn!(
                tool = %tool,
                streak = n,
                "[self_repair] broken-tool streak detected (resetting counter for retry window)"
            );
            g.remove(&tool);
        }
    }
}

/// Spawn a background Tokio task: periodic memory-ticker + tool-health repair.
pub fn spawn_self_repair_watchdog(memory_ticker: Arc<MemoryTicker>) {
    if watchdog_disabled() {
        tracing::info!("[self_repair] watchdog disabled (IF2AI_SELF_REPAIR_WATCHDOG=0)");
        return;
    }
    let secs = interval_secs().max(10);
    let task = move |memory_ticker: Arc<MemoryTicker>| async move {
        let mut interval = tokio::time::interval(Duration::from_secs(secs));
        loop {
            interval.tick().await;
            memory_ticker.repair_clear_stuck_daily();
            scan_broken_tools();
        }
    };
    // P0-Hotfix: Tauri's `setup` callback runs on the tao main thread
    // without a current Tokio runtime; `tokio::spawn` would panic and
    // — because the callback boundary is `extern "C-unwind"` — escalate
    // to `panic_cannot_unwind` → process abort. Mirror the scheduler
    // watchdog fix: prefer the current handle, otherwise fall back to a
    // dedicated thread with its own current-thread runtime.
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(task(memory_ticker));
        }
        Err(_) => {
            tracing::warn!(
                "[self_repair] no current Tokio runtime at watchdog spawn site; \
                 falling back to dedicated thread"
            );
            std::thread::Builder::new()
                .name("if2ai-self-repair-watchdog".into())
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
                    rt.block_on(task(memory_ticker));
                })
                .ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_tool_outcome_resets_on_success() {
        record_tool_outcome("bash", false);
        record_tool_outcome("bash", false);
        record_tool_outcome("bash", true);
        let g = fail_map().lock().unwrap();
        assert!(!g.contains_key("bash"));
    }
}
