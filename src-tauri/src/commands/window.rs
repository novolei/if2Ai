//! Window commands - open settings window
//!
//! Provides commands for managing application windows.

use tauri::{AppHandle, Manager, WebviewWindowBuilder, WebviewUrl};

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

    // Create new settings window
    let settings_window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("设置")
    .inner_size(800.0, 600.0)
    .min_inner_size(600.0, 400.0)
    .center()
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;

    // Set focus to the new window
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
