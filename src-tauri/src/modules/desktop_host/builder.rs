use std::sync::Arc;

use tauri::Runtime;

use crate::modules::browser::BrowserRegistry;
use crate::modules::memory::MemoryTicker;

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
        .manage(app_state)
        // Browser registry stays on the native host surface so
        // window/tray/browser commands can access shared viewer state
        // without coupling through AppState.
        .manage(browser_registry)
        .manage(memory_ticker)
        .manage(tts_state)
        .manage(tts_download_state)
}
