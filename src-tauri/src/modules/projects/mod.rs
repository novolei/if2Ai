//! Projects module - Project management for multi-project support
//!
//! Provides Project, ProjectMeta, and ProjectError types for the project system.

mod manager;

use crate::modules::runtime::permissions::PermissionMode;
pub use manager::ProjectManager;

/// Project metadata — represents a workspace/project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    /// Unique project ID (UUID v4).
    pub id: String,
    /// Human-readable project name (editable).
    pub name: String,
    /// Absolute path to the project working directory.
    pub workdir: std::path::PathBuf,
    /// Permission mode for this project.
    #[serde(default)]
    pub permission_mode: PermissionMode,
    /// Creation timestamp (RFC3339).
    pub created_at: String,
    /// Last accessed timestamp (RFC3339).
    pub updated_at: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            workdir: std::path::PathBuf::new(),
            permission_mode: PermissionMode::WorkspaceWrite,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

/// Project metadata for listing (without full workdir PathBuf).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeta {
    /// Unique project ID (UUID v4).
    pub id: String,
    /// Human-readable project name.
    pub name: String,
    /// Display-only workdir path (PathBuf → String).
    pub workdir: String,
    /// Creation timestamp (RFC3339).
    pub created_at: String,
    /// Last updated timestamp (RFC3339).
    pub updated_at: String,
    /// Number of sessions in this project.
    pub session_count: usize,
}

impl ProjectMeta {
    /// Create a ProjectMeta from a Project and session count.
    #[must_use]
    pub fn from_project(project: &Project, session_count: usize) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            workdir: project.workdir.to_string_lossy().to_string(),
            created_at: project.created_at.clone(),
            updated_at: project.updated_at.clone(),
            session_count,
        }
    }
}

/// Errors that can occur during project operations.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProjectError {
    /// Project not found.
    NotFound(String),
    /// Project already exists.
    AlreadyExists(String),
    /// Work directory not found or not accessible.
    WorkdirNotFound(String),
    /// Failed to read project data.
    ReadError(String),
    /// Failed to write project data.
    WriteError(String),
    /// Invalid project data.
    InvalidData(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "project not found: {id}"),
            Self::AlreadyExists(name) => write!(f, "project already exists: {name}"),
            Self::WorkdirNotFound(path) => write!(f, "work directory not found: {path}"),
            Self::ReadError(msg) => write!(f, "failed to read project: {msg}"),
            Self::WriteError(msg) => write!(f, "failed to write project: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid project data: {msg}"),
        }
    }
}

impl std::error::Error for ProjectError {}
