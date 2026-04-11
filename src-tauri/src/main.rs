#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::PathBuf;

use commands::AppState;
use commands::{delete_session, list_sessions, run_agent_turn};

#[allow(unused_imports)]
use tauri::Manager;

fn main() {
    // Initialize components
    let sessions_dir = if let Ok(path) = std::env::var("IF2AI_SESSIONS_DIR") {
        PathBuf::from(path)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
        PathBuf::from(home).join(".if2ai").join("sessions")
    };

    let session_manager = modules::session::SessionManager::new(sessions_dir);
    let tool_registry = modules::tools::ToolRegistry::new();

    // Create app state
    let app_state = AppState::new(session_manager, tool_registry);

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            run_agent_turn,
            list_sessions,
            delete_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
