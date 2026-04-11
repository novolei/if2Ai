#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::PathBuf;
use std::process::Command;

use commands::AppState;
use commands::{
    create_project, create_session, delete_project, delete_session, get_project,
    list_project_sessions, list_projects, list_sessions, rename_project, run_agent_turn,
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

/// Clean up all related processes when the app exits.
fn cleanup_processes() {
    let _ = Command::new("pkill")
        .args(["-f", "if2ai-backend"])
        .spawn();
}

fn main() {
    // Set up cleanup hooks
    std::panic::set_hook(Box::new(|_| {
        cleanup_processes();
    }));

    // Initialize directories
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let if2ai_dir = PathBuf::from(&home).join(".if2ai");

    let sessions_dir = if let Ok(path) = std::env::var("IF2AI_SESSIONS_DIR") {
        PathBuf::from(path)
    } else {
        if2ai_dir.join("sessions")
    };

    let projects_dir = if let Ok(path) = std::env::var("IF2AI_PROJECTS_DIR") {
        PathBuf::from(path)
    } else {
        if2ai_dir.join("projects")
    };

    let session_manager = modules::session::SessionManager::new(sessions_dir, projects_dir.clone());
    let tool_registry = modules::tools::ToolRegistry::new();
    let project_manager = modules::projects::ProjectManager::new(projects_dir);

    // Create app state
    let app_state = AppState::new(session_manager, tool_registry, project_manager);

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            run_agent_turn,
            list_sessions,
            delete_session,
            create_session,
            list_project_sessions,
            create_project,
            list_projects,
            get_project,
            rename_project,
            delete_project,
        ])
        .setup(|app| {
            // Create system tray menu
            let show_item = MenuItem::with_id(app, "show", "Show If2Ai", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // Build system tray
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("If2Ai - AI Agent Desktop")
                .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            cleanup_processes();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Hide window on close button instead of exiting
            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
