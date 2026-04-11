//! Project Manager - Project CRUD with JSON file persistence
//!
//! Manages projects stored in ~/.if2ai/projects/<id>/project.json

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use super::{Project, ProjectError, ProjectMeta};

/// Format SystemTime as RFC3339 string.
fn format_time(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs_in_day = secs % 86400;
    let hours = secs_in_day / 3600;
    let mins = (secs_in_day % 3600) / 60;
    let secs_in_min = secs_in_day % 60;
    format!("2026-04-12T{:02}:{:02}:{:02}Z", hours, mins, secs_in_min)
}

/// ProjectManager handles project persistence to JSON files.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProjectManager {
    /// Base directory for all projects: ~/.if2ai/projects/
    projects_dir: PathBuf,
}

#[allow(dead_code)]
impl ProjectManager {
    /// Creates a new ProjectManager with the specified projects directory.
    ///
    /// # Panics
    ///
    /// Panics if the projects directory cannot be created.
    #[must_use]
    pub fn new(projects_dir: PathBuf) -> Self {
        Self { projects_dir }
    }

    /// Initialize the projects directory.
    async fn init(&self) -> Result<(), ProjectError> {
        fs::create_dir_all(&self.projects_dir)
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to create projects dir: {e}")))?;
        Ok(())
    }

    /// Get the path to a project directory.
    fn project_dir(&self, id: &str) -> PathBuf {
        self.projects_dir.join(id)
    }

    /// Get the path to a project file.
    fn project_path(&self, id: &str) -> PathBuf {
        self.project_dir(id).join("project.json")
    }

    /// Get the sessions directory for a project.
    pub fn sessions_dir(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("sessions")
    }

    /// Create a new project.
    pub async fn create_project(
        &self,
        name: String,
        workdir: PathBuf,
    ) -> Result<Project, ProjectError> {
        self.init().await?;

        // Validate workdir exists
        if !workdir.exists() {
            return Err(ProjectError::WorkdirNotFound(
                workdir.to_string_lossy().to_string(),
            ));
        }

        let now = SystemTime::now();
        let now_str = format_time(now);

        let project = Project {
            id: Uuid::new_v4().to_string(),
            name,
            workdir,
            created_at: now_str.clone(),
            updated_at: now_str,
        };

        // Create project directory
        let proj_dir = self.project_dir(&project.id);
        fs::create_dir_all(&proj_dir)
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to create project dir: {e}")))?;

        // Create sessions subdirectory
        let sessions_dir = self.sessions_dir(&project.id);
        fs::create_dir_all(&sessions_dir)
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to create sessions dir: {e}")))?;

        // Save project file
        let path = self.project_path(&project.id);
        let contents = serde_json::to_string_pretty(&project)
            .map_err(|e| ProjectError::WriteError(format!("failed to serialize project: {e}")))?;

        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to create project file: {e}")))?;

        file.write_all(contents.as_bytes())
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to write project file: {e}")))?;

        Ok(project)
    }

    /// Get a project by ID.
    pub async fn get_project(&self, id: &str) -> Result<Project, ProjectError> {
        let path = self.project_path(id);

        if !path.exists() {
            return Err(ProjectError::NotFound(id.to_string()));
        }

        let mut file = fs::File::open(&path).await.map_err(|e| {
            ProjectError::ReadError(format!("failed to open {}: {e}", path.display()))
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).await.map_err(|e| {
            ProjectError::ReadError(format!("failed to read {}: {e}", path.display()))
        })?;

        let project: Project = serde_json::from_str(&contents).map_err(|e| {
            ProjectError::InvalidData(format!("failed to parse project {}: {e}", path.display()))
        })?;

        Ok(project)
    }

    /// List all projects (sorted by updated_at descending).
    pub async fn list_projects(&self) -> Result<Vec<ProjectMeta>, ProjectError> {
        self.init().await?;

        let mut entries = fs::read_dir(&self.projects_dir)
            .await
            .map_err(|e| ProjectError::ReadError(format!("failed to read projects dir: {e}")))?;

        let mut projects = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ProjectError::ReadError(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.is_dir() {
                // Try to read project.json in this directory
                let project_file = path.join("project.json");
                if project_file.exists() {
                    match self.read_project_meta(&path).await {
                        Ok(meta) => projects.push(meta),
                        Err(_) => continue, // Skip invalid project directories
                    }
                }
            }
        }

        // Sort by updated_at, newest first
        projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(projects)
    }

    /// Read project meta from a directory path.
    async fn read_project_meta(&self, dir_path: &Path) -> Result<ProjectMeta, ProjectError> {
        let project_file = dir_path.join("project.json");
        let mut file = fs::File::open(&project_file).await.map_err(|e| {
            ProjectError::ReadError(format!("failed to open {}: {e}", project_file.display()))
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).await.map_err(|e| {
            ProjectError::ReadError(format!("failed to read {}: {e}", project_file.display()))
        })?;

        let project: Project = serde_json::from_str(&contents)
            .map_err(|e| ProjectError::InvalidData(format!("failed to parse project: {e}")))?;

        // Count sessions in the sessions subdirectory
        let sessions_dir = dir_path.join("sessions");
        let session_count = if sessions_dir.exists() {
            let mut count = 0;
            let mut entries = fs::read_dir(&sessions_dir).await.map_err(|e| {
                ProjectError::ReadError(format!("failed to read sessions dir: {e}"))
            })?;
            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                ProjectError::ReadError(format!("failed to read session entry: {e}"))
            })? {
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    count += 1;
                }
            }
            count
        } else {
            0
        };

        Ok(ProjectMeta::from_project(&project, session_count))
    }

    /// Update project name.
    pub async fn rename_project(
        &self,
        id: &str,
        new_name: String,
    ) -> Result<Project, ProjectError> {
        let mut project = self.get_project(id).await?;

        project.name = new_name;
        project.updated_at = format_time(SystemTime::now());

        let path = self.project_path(id);
        let contents = serde_json::to_string_pretty(&project)
            .map_err(|e| ProjectError::WriteError(format!("failed to serialize project: {e}")))?;

        let mut file = fs::File::create(&path).await.map_err(|e| {
            ProjectError::WriteError(format!("failed to create {}: {e}", path.display()))
        })?;

        file.write_all(contents.as_bytes()).await.map_err(|e| {
            ProjectError::WriteError(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(project)
    }

    /// Delete a project and all its sessions.
    pub async fn delete_project(&self, id: &str) -> Result<(), ProjectError> {
        let proj_dir = self.project_dir(id);

        if !proj_dir.exists() {
            return Err(ProjectError::NotFound(id.to_string()));
        }

        fs::remove_dir_all(&proj_dir)
            .await
            .map_err(|e| ProjectError::WriteError(format!("failed to delete project dir: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[tokio::test]
    async fn create_and_get_project() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let manager = ProjectManager::new(temp_dir.clone());

        // Create a temp workdir
        let workdir = temp_dir.join("workdir");
        fs::create_dir_all(&workdir).await.unwrap();

        let project = manager
            .create_project("Test Project".to_string(), workdir.clone())
            .await
            .unwrap();

        assert_eq!(project.name, "Test Project");
        assert!(!project.id.is_empty());
        assert_eq!(project.workdir, workdir);

        let restored = manager.get_project(&project.id).await.unwrap();
        assert_eq!(restored.id, project.id);
        assert_eq!(restored.name, "Test Project");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn list_projects() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let manager = ProjectManager::new(temp_dir.clone());

        // Create temp workdirs
        let workdir1 = temp_dir.join("workdir1");
        let workdir2 = temp_dir.join("workdir2");
        fs::create_dir_all(&workdir1).await.unwrap();
        fs::create_dir_all(&workdir2).await.unwrap();

        manager
            .create_project("Project 1".to_string(), workdir1)
            .await
            .unwrap();
        manager
            .create_project("Project 2".to_string(), workdir2)
            .await
            .unwrap();

        let projects = manager.list_projects().await.unwrap();
        assert_eq!(projects.len(), 2);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn rename_project() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let manager = ProjectManager::new(temp_dir.clone());

        let workdir = temp_dir.join("workdir");
        fs::create_dir_all(&workdir).await.unwrap();

        let project = manager
            .create_project("Original".to_string(), workdir)
            .await
            .unwrap();

        let renamed = manager
            .rename_project(&project.id, "New Name".to_string())
            .await
            .unwrap();
        assert_eq!(renamed.name, "New Name");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn delete_project() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let manager = ProjectManager::new(temp_dir.clone());

        let workdir = temp_dir.join("workdir");
        fs::create_dir_all(&workdir).await.unwrap();

        let project = manager
            .create_project("To Delete".to_string(), workdir)
            .await
            .unwrap();

        manager.delete_project(&project.id).await.unwrap();

        let result = manager.get_project(&project.id).await;
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }
}
