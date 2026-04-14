#![allow(unused)]

//! SkillManager — CRUD operations for skill directories.
//!
//! Ported from Hermes `skill_manager_tool.py`.
//!
//! Provides create, edit, patch, delete, write_file, and remove_file operations
//! for skill directories, with security scanning before mutations.

pub mod actions;
pub mod atomic_write;
pub mod validator;

pub use actions::{
    execute_action, SkillManageAction, SkillManageInput, SkillManageOutput, SkillManagerError,
    SkillManagerResult,
};
pub use atomic_write::{atomic_write, atomic_write_str, AtomicWriteOptions, AtomicWriteResult};
pub use validator::{AllowedSubdirs, SkillValidator, ValidationError};

use std::path::{Path, PathBuf};

use crate::modules::skills::guard::SkillsGuard;

/// Skill management error type (alias for compatibility).
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("security blocked: {0}")]
    SecurityBlocked(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("operation failed: {0}")]
    OperationFailed(String),
}

impl From<SkillManagerError> for SkillError {
    fn from(e: SkillManagerError) -> Self {
        match e {
            SkillManagerError::Io(e) => SkillError::Io(e),
            SkillManagerError::Validation(s) => SkillError::Validation(s),
            SkillManagerError::SecurityBlocked(s) => SkillError::SecurityBlocked(s),
            SkillManagerError::NotFound(s) => SkillError::NotFound(s),
            SkillManagerError::AlreadyExists(s) => SkillError::AlreadyExists(s),
            SkillManagerError::OperationFailed(s) => SkillError::OperationFailed(s),
        }
    }
}

impl From<ValidationError> for SkillError {
    fn from(e: ValidationError) -> Self {
        SkillError::Validation(e.to_string())
    }
}

/// Skill context passed to manager operations.
///
/// This provides the necessary environment for skill operations,
/// including the skills root directory and security guard.
#[derive(Debug, Clone)]
pub struct SkillContext {
    /// Root directory containing all skills.
    pub skills_dir: PathBuf,
    /// Security guard for scanning skills.
    pub guard: SkillsGuard,
}

impl SkillContext {
    /// Create a new skill context.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            guard: SkillsGuard::new(),
        }
    }

    /// Create a new skill context with a custom guard.
    pub fn with_guard(skills_dir: PathBuf, guard: SkillsGuard) -> Self {
        Self { skills_dir, guard }
    }
}

/// SkillManager trait for skill CRUD operations.
///
/// Implement this trait to provide skill management capabilities
/// with security scanning integration.
pub trait SkillManager: Send + Sync {
    /// Perform a skill management operation.
    fn manage(
        &self,
        input: SkillManageInput,
        ctx: &SkillContext,
    ) -> Result<SkillManageOutput, SkillError>;

    /// Create a new skill.
    fn create(
        &self,
        name: &str,
        content: &str,
        category: Option<&str>,
    ) -> Result<SkillManageOutput, SkillError>;

    /// Edit an existing skill (overwrite SKILL.md).
    fn edit(&self, name: &str, content: &str) -> Result<SkillManageOutput, SkillError>;

    /// Patch an existing skill (find and replace in SKILL.md).
    fn patch(
        &self,
        name: &str,
        old: &str,
        new: &str,
        replace_all: bool,
    ) -> Result<SkillManageOutput, SkillError>;

    /// Delete a skill.
    fn delete(&self, name: &str) -> Result<SkillManageOutput, SkillError>;

    /// Write a supporting file to a skill.
    fn write_file(
        &self,
        name: &str,
        path: &str,
        content: &str,
    ) -> Result<SkillManageOutput, SkillError>;

    /// Remove a supporting file from a skill.
    fn remove_file(&self, name: &str, path: &str) -> Result<SkillManageOutput, SkillError>;
}

/// Default SkillManager implementation.
#[derive(Debug, Clone)]
pub struct DefaultSkillManager {
    ctx: SkillContext,
}

impl DefaultSkillManager {
    /// Create a new DefaultSkillManager with the given skills directory.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            ctx: SkillContext::new(skills_dir),
        }
    }

    /// Create a new DefaultSkillManager with a custom context.
    pub fn with_context(ctx: SkillContext) -> Self {
        Self { ctx }
    }

    fn execute(
        &self,
        action: SkillManageAction,
        name: &str,
        input: &SkillManageInput,
    ) -> Result<SkillManageOutput, SkillError> {
        execute_action(action, name, &self.ctx.skills_dir, input, &self.ctx.guard)
            .map_err(SkillError::from)
    }
}

impl SkillManager for DefaultSkillManager {
    fn manage(
        &self,
        input: SkillManageInput,
        _ctx: &SkillContext,
    ) -> Result<SkillManageOutput, SkillError> {
        execute_action(
            input.action,
            &input.name,
            &self.ctx.skills_dir,
            &input,
            &self.ctx.guard,
        )
        .map_err(SkillError::from)
    }

    fn create(
        &self,
        name: &str,
        content: &str,
        category: Option<&str>,
    ) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::Create,
            name: name.into(),
            content: Some(content.into()),
            category: category.map(String::from),
            file_path: None,
            file_content: None,
            old_string: None,
            new_string: None,
            replace_all: false,
        };
        self.execute(SkillManageAction::Create, name, &input)
    }

    fn edit(&self, name: &str, content: &str) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::Edit,
            name: name.into(),
            content: Some(content.into()),
            category: None,
            file_path: None,
            file_content: None,
            old_string: None,
            new_string: None,
            replace_all: false,
        };
        self.execute(SkillManageAction::Edit, name, &input)
    }

    fn patch(
        &self,
        name: &str,
        old: &str,
        new: &str,
        replace_all: bool,
    ) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::Patch,
            name: name.into(),
            content: None,
            category: None,
            file_path: None,
            file_content: None,
            old_string: Some(old.into()),
            new_string: Some(new.into()),
            replace_all,
        };
        self.execute(SkillManageAction::Patch, name, &input)
    }

    fn delete(&self, name: &str) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::Delete,
            name: name.into(),
            content: None,
            category: None,
            file_path: None,
            file_content: None,
            old_string: None,
            new_string: None,
            replace_all: false,
        };
        self.execute(SkillManageAction::Delete, name, &input)
    }

    fn write_file(
        &self,
        name: &str,
        path: &str,
        content: &str,
    ) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::WriteFile,
            name: name.into(),
            content: None,
            category: None,
            file_path: Some(path.into()),
            file_content: Some(content.into()),
            old_string: None,
            new_string: None,
            replace_all: false,
        };
        self.execute(SkillManageAction::WriteFile, name, &input)
    }

    fn remove_file(&self, name: &str, path: &str) -> Result<SkillManageOutput, SkillError> {
        let input = SkillManageInput {
            action: SkillManageAction::RemoveFile,
            name: name.into(),
            content: None,
            category: None,
            file_path: Some(path.into()),
            file_content: None,
            old_string: None,
            new_string: None,
            replace_all: false,
        };
        self.execute(SkillManageAction::RemoveFile, name, &input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_skill_manager_create_and_edit() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill
        let result = manager.create(
            "test-skill",
            "# Test Skill\n\nThis is a test skill.",
            Some("testing"),
        );
        if let Err(ref e) = result {
            eprintln!("DEBUG create error: {:?}", e);
        }
        assert!(result.is_ok());
        assert!(result.unwrap().success);

        // Edit the skill
        let result = manager.edit("test-skill", "# Updated\n\nNew content.");
        assert!(result.is_ok());
        assert!(result.unwrap().success);

        // Verify the content
        let skill_md = tmp.path().join("test-skill/SKILL.md");
        let content = std::fs::read_to_string(&skill_md).unwrap();
        assert!(content.contains("Updated"));
    }

    #[test]
    fn test_skill_manager_delete() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill
        manager
            .create("to-delete", "content", None)
            .expect("create should succeed");
        assert!(tmp.path().join("to-delete").exists());

        // Delete the skill
        let result = manager.delete("to-delete");
        assert!(result.is_ok());
        assert!(!tmp.path().join("to-delete").exists());
    }

    #[test]
    fn test_skill_manager_patch() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill
        manager
            .create("patch-test", "Hello World", None)
            .expect("create should succeed");

        // Patch the skill
        let result = manager.patch("patch-test", "World", "Rust", false);
        assert!(result.is_ok());

        let content = std::fs::read_to_string(tmp.path().join("patch-test/SKILL.md")).unwrap();
        assert!(content.contains("Hello Rust"));
    }

    #[test]
    fn test_skill_manager_write_file() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill
        manager
            .create("file-test", "content", None)
            .expect("create should succeed");

        // Write a supporting file
        let result = manager.write_file("file-test", "scripts/helper.sh", "echo hello");
        assert!(result.is_ok());

        assert!(tmp.path().join("file-test/scripts/helper.sh").exists());
    }

    #[test]
    fn test_skill_manager_remove_file() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill with a file
        manager
            .create("remove-test", "content", None)
            .expect("create should succeed");
        let file_path = tmp.path().join("remove-test/scripts/to_remove.txt");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "to be removed").unwrap();

        // Remove the file
        let result = manager.remove_file("remove-test", "scripts/to_remove.txt");
        assert!(result.is_ok());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_skill_manager_not_found() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Try to edit non-existent skill
        let result = manager.edit("nonexistent", "content");
        assert!(result.is_err());
    }

    #[test]
    fn test_skill_manager_already_exists() {
        let tmp = TempDir::new().unwrap();
        let manager = DefaultSkillManager::new(tmp.path().to_path_buf());

        // Create a skill
        manager
            .create("existing", "content", None)
            .expect("create should succeed");

        // Try to create again
        let result = manager.create("existing", "new content", None);
        assert!(result.is_err());
    }
}
