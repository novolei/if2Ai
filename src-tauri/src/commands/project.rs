//! Project commands - Tauri IPC for project management
//!
//! Provides project CRUD commands for the frontend.

use std::path::PathBuf;
use std::process::Command;

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

/// Open a project workdir in the system file manager.
#[tauri::command]
#[allow(dead_code)]
pub async fn open_project_in_finder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let project = state
        .project_manager
        .get_project(&id)
        .await
        .map_err(project_error_to_string)?;

    let workdir = project.workdir;

    tokio::task::spawn_blocking(move || {
        open_path_in_file_manager(&workdir).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Create a permanent git worktree for a project.
#[tauri::command]
#[allow(dead_code)]
pub async fn create_permanent_worktree(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let project = state
        .project_manager
        .get_project(&id)
        .await
        .map_err(project_error_to_string)?;

    let source_dir = project.workdir;
    let parent = source_dir
        .parent()
        .ok_or_else(|| String::from("项目工作目录没有父目录，无法创建工作树"))?
        .to_path_buf();
    let slug = slugify(&project.name);
    let target_dir = parent.join(format!("{slug}-worktree"));
    let branch_name = format!("if2ai/{slug}");

    if target_dir.exists() {
        return Err(format!("工作树目标已存在: {}", target_dir.display()));
    }

    let created_target = target_dir.clone();
    tokio::task::spawn_blocking(move || {
        let output = Command::new("git")
            .arg("-C")
            .arg(&source_dir)
            .arg("worktree")
            .arg("add")
            .arg(&created_target)
            .arg("-b")
            .arg(&branch_name)
            .output()
            .map_err(|err| err.to_string())?;

        if output.status.success() {
            Ok(created_target.to_string_lossy().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_was_dash = false;

    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_was_dash = false;
            ch.to_ascii_lowercase()
        } else if ch.is_whitespace() || matches!(ch, '_' | '-' | '.') {
            if last_was_dash {
                continue;
            }
            last_was_dash = true;
            '-'
        } else {
            continue;
        };
        slug.push(next);
    }

    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        String::from("if2ai")
    } else {
        trimmed
    }
}

fn open_path_in_file_manager(path: &PathBuf) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).status()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).status()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).status()?;
    }

    Ok(())
}
