//! Window commands - open settings window
//!
//! Provides commands for managing application windows.

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

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
