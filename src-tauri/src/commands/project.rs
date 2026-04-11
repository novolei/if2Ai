//! Project commands - Tauri IPC for project management
//!
//! Provides project CRUD commands for the frontend.

use std::path::PathBuf;

use tauri::State;

use crate::commands::AppState;
use crate::modules::projects::{Project, ProjectError, ProjectMeta};

/// Convert ProjectError to String for Tauri.
fn project_error_to_string(err: ProjectError) -> String {
    err.to_string()
}

/// Create a new project.
#[tauri::command]
#[allow(dead_code)]
pub async fn create_project(
    state: State<'_, AppState>,
    name: String,
    workdir: String,
) -> Result<Project, String> {
    tracing::info!(
        "[Rust] create_project called with name={}, workdir={}",
        name,
        workdir
    );
    let workdir_path = PathBuf::from(&workdir);
    state
        .project_manager
        .create_project(name, workdir_path)
        .await
        .map_err(project_error_to_string)
}

/// List all projects.
#[tauri::command]
#[allow(dead_code)]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectMeta>, String> {
    state
        .project_manager
        .list_projects()
        .await
        .map_err(project_error_to_string)
}

/// Get a project by ID.
#[tauri::command]
#[allow(dead_code)]
pub async fn get_project(state: State<'_, AppState>, id: String) -> Result<Project, String> {
    state
        .project_manager
        .get_project(&id)
        .await
        .map_err(project_error_to_string)
}

/// Rename a project.
#[tauri::command]
#[allow(dead_code)]
pub async fn rename_project(
    state: State<'_, AppState>,
    id: String,
    new_name: String,
) -> Result<Project, String> {
    state
        .project_manager
        .rename_project(&id, new_name)
        .await
        .map_err(project_error_to_string)
}

/// Delete a project and all its sessions.
#[tauri::command]
#[allow(dead_code)]
pub async fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .project_manager
        .delete_project(&id)
        .await
        .map_err(project_error_to_string)
}
