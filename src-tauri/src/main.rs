#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::PathBuf;

use commands::AppState;
use commands::{
    create_project, create_session, delete_project, delete_session, get_project,
    list_project_sessions, list_projects, list_sessions, rename_project, run_agent_turn,
};

#[allow(unused_imports)]
use tauri::Manager;

fn main() {
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
