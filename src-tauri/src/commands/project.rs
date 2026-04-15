//! Project commands - Tauri IPC for project management
//!
//! Provides project CRUD commands for the frontend.

use base64::Engine;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

use tauri::State;

use crate::commands::AppState;
use crate::modules::projects::{Project, ProjectError, ProjectMeta};

/// Convert ProjectError to String for Tauri.
fn project_error_to_string(err: ProjectError) -> String {
    err.to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryEntryPreview {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub modified_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilePreviewPayload {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub mime_type: Option<String>,
    pub content: Option<String>,
    pub data_base64: Option<String>,
    pub editable: bool,
    pub language: Option<String>,
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

/// Open an arbitrary directory path in the system file manager.
#[tauri::command]
#[allow(dead_code)]
pub async fn open_directory_path(path: String) -> Result<(), String> {
    let directory = PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        open_path_in_file_manager(&directory).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// List first-level directory entries for rail preview.
#[tauri::command]
#[allow(dead_code)]
pub async fn list_directory_preview(
    path: String,
    limit: Option<usize>,
) -> Result<Vec<DirectoryEntryPreview>, String> {
    let directory = PathBuf::from(path);
    let max_items = limit.unwrap_or(32).clamp(1, 120);

    tokio::task::spawn_blocking(move || {
        let entries = std::fs::read_dir(&directory).map_err(|err| err.to_string())?;
        let mut items = Vec::new();

        for entry_result in entries {
            let entry = entry_result.map_err(|err| err.to_string())?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let metadata = entry.metadata().map_err(|err| err.to_string())?;
            let kind = if metadata.is_dir() {
                "folder"
            } else if metadata.is_file() {
                "file"
            } else {
                continue;
            };
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis());

            items.push(DirectoryEntryPreview {
                name: file_name,
                path: path.to_string_lossy().to_string(),
                kind: kind.to_string(),
                modified_ms,
            });
        }

        items.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
            ("folder", "file") => std::cmp::Ordering::Less,
            ("file", "folder") => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        items.truncate(max_items);
        Ok(items)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Read a text file for inline preview.
#[tauri::command]
#[allow(dead_code)]
pub async fn read_file_preview(
    path: String,
    max_bytes: Option<usize>,
) -> Result<FilePreviewPayload, String> {
    let file_path = PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        let name = file_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let extension = file_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        let binary_mime_type = preview_mime_type(&extension);
        let max_size = if binary_mime_type.is_some() {
            max_bytes
                .unwrap_or(10 * 1024 * 1024)
                .clamp(32 * 1024, 32 * 1024 * 1024)
        } else {
            max_bytes
                .unwrap_or(128 * 1024)
                .clamp(1024, 512 * 1024)
        };
        let metadata = std::fs::metadata(&file_path).map_err(|err| err.to_string())?;
        if !metadata.is_file() {
            return Err("path is not a file".to_string());
        }
        if metadata.len() as usize > max_size {
            return Err("file too large for inline preview".to_string());
        }

        if let Some(mime_type) = binary_mime_type {
            let bytes = std::fs::read(&file_path).map_err(|err| err.to_string())?;
            let preview_kind = preview_binary_kind(mime_type);
            return Ok(FilePreviewPayload {
                name,
                path: file_path.to_string_lossy().to_string(),
                kind: preview_kind.to_string(),
                mime_type: Some(mime_type.to_string()),
                content: None,
                data_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                editable: false,
                language: None,
            });
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|_| "file is not valid UTF-8 text".to_string())?;

        let text_kind = preview_text_kind(&extension);

        Ok(FilePreviewPayload {
            name,
            path: file_path.to_string_lossy().to_string(),
            kind: text_kind.to_string(),
            mime_type: if text_kind == "html" {
                Some("text/html".to_string())
            } else {
                Some("text/plain".to_string())
            },
            content: Some(content),
            data_base64: None,
            editable: preview_text_editable(text_kind),
            language: preview_language(&extension).map(|value| value.to_string()),
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
#[allow(dead_code)]
pub async fn write_file_contents(path: String, content: String) -> Result<(), String> {
    let file_path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        std::fs::write(&file_path, content).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn preview_mime_type(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "pdf" => Some("application/pdf"),
        "mp4" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        "webm" => Some("video/webm"),
        "m4v" => Some("video/x-m4v"),
        _ => None,
    }
}

fn preview_binary_kind(mime_type: &str) -> &'static str {
    if mime_type == "application/pdf" {
        "pdf"
    } else if mime_type.starts_with("video/") {
        "video"
    } else {
        "image"
    }
}

fn preview_text_kind(extension: &str) -> &'static str {
    match extension {
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        _ => "code",
    }
}

fn preview_text_editable(kind: &str) -> bool {
    matches!(kind, "markdown" | "code" | "html")
}

fn preview_language(extension: &str) -> Option<&'static str> {
    match extension {
        "md" | "markdown" => Some("markdown"),
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" => Some("javascript"),
        "jsx" => Some("jsx"),
        "json" => Some("json"),
        "css" => Some("css"),
        "html" | "htm" => Some("html"),
        "rs" => Some("rust"),
        "py" => Some("python"),
        "sh" => Some("shell"),
        "yml" | "yaml" => Some("yaml"),
        "sql" => Some("sql"),
        _ => Some("text"),
    }
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
