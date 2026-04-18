//! Window commands — settings, browser viewer, and main-window utilities.
//!
//! # BrowserViewer architecture
//!
//! The BrowserViewer uses Tauri's `Window::add_child` (requires `unstable`
//! feature) to embed a **native WKWebView** (macOS) directly inside the viewer
//! window.  This is equivalent to Electron's `WebContentsView` and renders real
//! live webpages without the X-Frame-Options / CSP restrictions that block
//! `<iframe>` approaches.
//!
//! Layout inside the viewer window (1200 × 800):
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  toolbar WebviewWindow  (h = TOOLBAR_HEIGHT, full width) │
//! │  [traffic lights] [URL bar] [Back/Fwd/Reload] [Stop]     │
//! ├──────────────────────────────────────────────────────────┤
//! │                                                          │
//! │   content Webview  (native WKWebView, fills remainder)   │
//! │   loads external URLs → real browser, no iframe jail     │
//! │                                                          │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! When the AI navigates via the `browser` tool, `browser_tool.rs` calls
//! `sync_viewer_url()` which navigates the content Webview in real-time.

use std::sync::Arc;

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl,
};
use tauri::{TitleBarStyle, WebviewWindowBuilder};

use crate::modules::browser::BrowserRegistry;
use crate::modules::viewer_registry::VIEWER_CONTENT_WEBVIEWS;

/// Height of the toolbar overlay in logical pixels.
const TOOLBAR_HEIGHT: f64 = 52.0;

/// Open (or focus) an independent BrowserViewer window for `session_id`.
///
/// The window contains:
/// 1. A thin React toolbar rendered by `BrowserViewerPage` (drag region + URL bar + controls).
/// 2. A native content `Webview` (`Window::add_child`) that shows the live webpage — exactly
///    the same URL the AI is visiting.  The content view is navigated by `sync_viewer_url`
///    in `browser_tool.rs` every time the AI performs a `navigate` action.
#[tauri::command]
#[allow(dead_code)]
pub fn open_browser_viewer_window(
    app: AppHandle,
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    // Sanitise session_id for use as a Tauri window label.
    let safe_id: String = session_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let label = format!("browser-viewer-{safe_id}");
    let content_label = format!("browser-viewer-content-{safe_id}");

    // Re-focus if the window already exists.
    if let Some(win) = app.get_webview_window(&label) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // ── 1. Create the toolbar window ──────────────────────────────────────────
    let toolbar_url = format!("index.html?window=browser-viewer&session_id={safe_id}");
    let win = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(toolbar_url.into()))
        .title("AI Browser Viewer")
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 560.0)
        .center()
        .resizable(true)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(|e| e.to_string())?;

    // ── 2. Embed the live content WKWebView below the toolbar ─────────────────
    // Start at about:blank; the URL will be set by sync_viewer_url() once the
    // AI navigates, or by the navigate_viewer_window command from the toolbar.
    let content_webview = WebviewBuilder::new(
        &content_label,
        WebviewUrl::External(
            "about:blank"
                .parse()
                .map_err(|e: url::ParseError| e.to_string())?,
        ),
    )
    // No CSP injection — we are loading real external sites.
    .disable_drag_drop_handler();

    // `win` is a WebviewWindow; we need the underlying Window handle.
    // The Window is accessible via the Manager trait.
    let window_handle = app
        .get_window(&label)
        .ok_or_else(|| "window not found after creation".to_string())?;

    // Get the current inner size to calculate content area.
    let inner_size = win
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(win.scale_factor().unwrap_or(1.0));

    let content_view = window_handle
        .add_child(
            content_webview,
            LogicalPosition::new(0.0, TOOLBAR_HEIGHT),
            LogicalSize::new(
                inner_size.width,
                (inner_size.height - TOOLBAR_HEIGHT).max(0.0),
            ),
        )
        .map_err(|e| e.to_string())?;

    // Register the content Webview so browser_tool can navigate it later.
    VIEWER_CONTENT_WEBVIEWS.insert(session_id.clone(), content_view.clone());

    // If the AI has already navigated somewhere, immediately load that URL so
    // the viewer doesn't sit at about:blank when opened after navigation.
    if let Some(url_str) = registry.current_url(&session_id) {
        if let Ok(url) = url_str.parse::<url::Url>() {
            let _ = content_view.navigate(url);
        }
    }

    // ── 3. Keep content bounds in sync when the toolbar window is resized ─────
    {
        let session_id_clone = session_id.clone();
        let win_clone = win.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Resized(_) = event {
                _update_content_bounds(&win_clone, &session_id_clone);
            }
        });
    }

    // ── 4. Clean up the registry entry when the viewer window is closed ───────
    {
        let session_id_clone = session_id.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event {
                VIEWER_CONTENT_WEBVIEWS.remove(&session_id_clone);
            }
        });
    }

    win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Recalculate and apply content Webview bounds for the given session.
///
/// Called on window resize so the content Webview fills the area below the toolbar.
fn _update_content_bounds(win: &tauri::WebviewWindow, session_id: &str) {
    let Ok(inner_size) = win.inner_size() else {
        return;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let logical = inner_size.to_logical::<f64>(scale);
    let content_height = (logical.height - TOOLBAR_HEIGHT).max(0.0);

    if let Some(webview) = VIEWER_CONTENT_WEBVIEWS.get(session_id) {
        let _ = webview.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
        let _ = webview.set_size(LogicalSize::new(logical.width, content_height));
    }
}

/// Navigate the embedded content Webview to `url`.
///
/// Called by the toolbar's back/forward/reload buttons and URL-bar submissions.
#[tauri::command]
#[allow(dead_code)]
pub fn navigate_viewer_window(session_id: String, url: String) -> Result<(), String> {
    let parsed = url
        .parse::<url::Url>()
        .map_err(|e| format!("invalid URL: {e}"))?;
    if let Some(webview) = VIEWER_CONTENT_WEBVIEWS.get(&session_id) {
        webview.navigate(parsed).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Tell the viewer's content Webview to go back in history.
#[tauri::command]
#[allow(dead_code)]
pub fn browser_viewer_go_back(session_id: String, app: AppHandle) -> Result<(), String> {
    _eval_in_content(&session_id, &app, "history.back()")
}

/// Tell the viewer's content Webview to go forward in history.
#[tauri::command]
#[allow(dead_code)]
pub fn browser_viewer_go_forward(session_id: String, app: AppHandle) -> Result<(), String> {
    _eval_in_content(&session_id, &app, "history.forward()")
}

/// Reload the viewer's content Webview.
#[tauri::command]
#[allow(dead_code)]
pub fn browser_viewer_reload(session_id: String, app: AppHandle) -> Result<(), String> {
    _eval_in_content(&session_id, &app, "location.reload()")
}

/// Evaluate a small JS snippet inside the content Webview.
fn _eval_in_content(session_id: &str, app: &AppHandle, script: &str) -> Result<(), String> {
    if let Some(webview) = VIEWER_CONTENT_WEBVIEWS.get(session_id) {
        webview.eval(script).map_err(|e| e.to_string())?;
    } else {
        // Fall back: find the webview by label if it isn't in the registry yet.
        let content_label = format!("browser-viewer-content-{session_id}");
        if let Some(webview) = app.get_webview(&content_label) {
            webview.eval(script).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Emit a `viewer-url-changed` event to the toolbar window whenever the content
/// Webview navigates (so the URL bar stays in sync).
///
/// This must be wired to the `did-navigate` equivalent in the Webview.
/// For now the toolbar gets URL updates via the `browser-status` Tauri event.
#[allow(dead_code)]
pub fn emit_viewer_url(app: &AppHandle, session_id: &str, url: &str) {
    let label = format!("browser-viewer-{session_id}");
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.emit("viewer-url-changed", serde_json::json!({ "url": url }));
    }
}

// ── Other window commands ─────────────────────────────────────────────────────

/// Open the settings window.
#[tauri::command]
#[allow(dead_code)]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let settings_window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("设置")
    .inner_size(960.0, 740.0)
    .min_inner_size(880.0, 680.0)
    .center()
    .resizable(true)
    .title_bar_style(TitleBarStyle::Overlay)
    .hidden_title(true)
    .build()
    .map_err(|e| e.to_string())?;

    settings_window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Close the settings window.
#[tauri::command]
#[allow(dead_code)]
pub fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Focus the main chat window and prefill the composer prompt.
#[tauri::command]
#[allow(dead_code)]
pub fn focus_main_window_and_prefill_prompt(app: AppHandle, prompt: String) -> Result<(), String> {
    let main_window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    main_window.show().map_err(|e| e.to_string())?;
    main_window.set_focus().map_err(|e| e.to_string())?;
    main_window
        .emit(
            "if2ai-chat-prefill",
            serde_json::json!({ "prompt": prompt }),
        )
        .map_err(|e| e.to_string())?;
    if let Some(settings_window) = app.get_webview_window("settings") {
        settings_window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
