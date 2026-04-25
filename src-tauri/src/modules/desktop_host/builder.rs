use std::sync::Arc;

use tauri::Runtime;

use crate::modules::browser::BrowserRegistry;
use crate::modules::memory::MemoryTicker;

const DEFAULT_UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFGNjM2OTVFODk2QTA5MUUKUldRZUNXcUpYbWxqSCtuMGpCZDRRRkpuM1RUSmZuc2VPdXFKQnRYRTFkdGt3UTJUUEgwMW5TM1YK";

/// Attach native host plugins and managed state to the Tauri
/// builder.
///
/// This keeps `main.rs` at the level of a composition root:
/// build domain state once, then hand the fully-assembled host
/// surface to the desktop shell module.
pub fn attach_native_host<
    R: Runtime,
    A: Send + Sync + 'static,
    T: Send + Sync + 'static,
    D: Send + Sync + 'static,
>(
    builder: tauri::Builder<R>,
    app_state: A,
    browser_registry: Arc<BrowserRegistry>,
    memory_ticker: Arc<MemoryTicker>,
    tts_state: T,
    tts_download_state: D,
) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(configured_updater_pubkey())
                .build(),
        )
        .manage(app_state)
        // Browser registry stays on the native host surface so
        // window/tray/browser commands can access shared viewer state
        // without coupling through AppState.
        .manage(browser_registry)
        .manage(memory_ticker)
        .manage(tts_state)
        .manage(tts_download_state)
}

fn configured_updater_pubkey() -> String {
    std::env::var("IF2AI_UPDATER_PUBKEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            option_env!("IF2AI_UPDATER_PUBKEY")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| DEFAULT_UPDATER_PUBKEY.to_string())
}
