//! Viewer registry — tracks open BrowserViewer content WebView handles.
//!
//! Shared by:
//! - `commands::window` (inserts/removes on open/destroy, updates bounds)
//! - `modules::tools::builtin::browser_tool` (mirrors AI navigation into the viewer)
//!
//! Each entry maps `session_id` → the native content `Webview` that was added
//! as a child view in the BrowserViewer window (`Window::add_child`).

use std::sync::LazyLock;

use dashmap::DashMap;
use url::Url;

/// Maps `session_id` → content `Webview` handle for the open BrowserViewer.
pub(crate) static VIEWER_CONTENT_WEBVIEWS: LazyLock<DashMap<String, tauri::Webview<tauri::Wry>>> =
    LazyLock::new(DashMap::new);

/// Mirror an AI navigation into an open BrowserViewer window (no-op if none).
///
/// Called by `browser_tool.rs` after every successful `navigate` action so the
/// native content WebView follows the AI without any user interaction.
pub(crate) fn sync_viewer_url(session_id: &str, url: &str) {
    if let Some(webview) = VIEWER_CONTENT_WEBVIEWS.get(session_id) {
        if let Ok(parsed) = url.parse::<Url>() {
            let _ = webview.navigate(parsed);
        }
    }
}
