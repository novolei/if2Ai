//! IPC surface for the durable per-role usage store.
//!
//! Frontend (`src/modules/settings/pages/UsageSettingsPage.tsx`)
//! polls `usage_summary(window)` to render the four cards + by-caller
//! table. The store itself lives at
//! `~/.if2ai/usage/usage.sqlite`; see
//! [`crate::modules::usage::sqlite_store`].

#[allow(unused_imports)]
use crate::modules::usage::UsageStore as _;
use crate::modules::usage::{global_store, UsageSummary, UsageWindow};

// `UsageStore` is brought into scope so the `summary(...)` method
// resolves on the `Arc<dyn UsageStore>` returned by `global_store()`.

/// Aggregate the durable usage store for one of four windows. The
/// caller passes the canonical `today | this_week | this_month |
/// all_time` string (matches [`UsageWindow::as_str`]).
#[tauri::command]
pub async fn usage_summary(window: String) -> Result<UsageSummary, String> {
    let parsed = match window.as_str() {
        "today" => UsageWindow::Today,
        "this_week" => UsageWindow::ThisWeek,
        "this_month" => UsageWindow::ThisMonth,
        "all_time" => UsageWindow::AllTime,
        other => {
            return Err(format!(
                "unknown usage window '{other}'; expected today | this_week | this_month | all_time"
            ))
        }
    };
    global_store()
        .summary(parsed)
        .map_err(|e| format!("usage_summary failed: {e}"))
}
