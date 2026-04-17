//! Window commands - open settings window
//!
//! Provides commands for managing application windows.

use tauri::{AppHandle, Emitter, Manager, TitleBarStyle, WebviewUrl, WebviewWindowBuilder};

/// Open the settings window.
/// If the settings window already exists, focus it instead of creating a new one.
#[tauri::command]
#[allow(dead_code)]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // Check if settings window already exists
    if let Some(window) = app.get_webview_window("settings") {
        // Window exists, just show and focus it
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Create new settings window.
    // Uses Overlay title bar + hidden title so the custom SettingsSidebar header
    // (with logo and close button) takes full control of dragging and chrome.
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

    // Set focus to the new window
    settings_window.set_focus().map_err(|e| e.to_string())?;

    Ok(())
}

/// Open (or focus) an independent BrowserViewer window for `session_id`.
///
/// The window label is `browser-viewer-<sanitised_session_id>` to keep labels
/// unique and valid. If the window already exists, it is shown and focused
/// rather than creating a duplicate.
///
/// The window loads `index.html?window=browser-viewer&session_id=<id>` so the
/// React router renders `BrowserViewerPage` with the correct session context.
///
/// Note: the viewer displays the current URL in a `<webview>`-style approach
/// (iframe), which is a **separate** navigation and does not share cookies or
/// state with the headless chromiumoxide session.
#[tauri::command]
#[allow(dead_code)]
pub fn open_browser_viewer_window(app: AppHandle, session_id: String) -> Result<(), String> {
    // Sanitise session_id: replace any character that is not alphanumeric or
    // hyphen with a hyphen so the label is always a valid Tauri window label.
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

    // Re-focus if the window is already open.
    if let Some(window) = app.get_webview_window(&label) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Use safe_id in the URL too — avoids query-string injection if session_id
    // ever contains '&', '=', or '?' characters.
    let url = format!("index.html?window=browser-viewer&session_id={safe_id}");

    let viewer = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title("AI Browser Viewer")
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 560.0)
        .center()
        .resizable(true)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(|e| e.to_string())?;

    viewer.set_focus().map_err(|e| e.to_string())?;
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
            serde_json::json!({
                "prompt": prompt,
            }),
        )
        .map_err(|e| e.to_string())?;
    if let Some(settings_window) = app.get_webview_window("settings") {
        settings_window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
