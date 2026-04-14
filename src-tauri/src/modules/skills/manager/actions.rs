#![allow(unused)]

//! Skill management CRUD actions.
//!
//! Ported from Hermes `skill_manager_tool.py`.

use std::fs;
use std::path::{Path, PathBuf};

use super::atomic_write::{
    atomic_write_str, ensure_dir, safe_remove, AtomicWriteError, AtomicWriteOptions,
};
use super::validator::{AllowedSubdirs, SkillValidator, ValidationError};
use crate::modules::skills::guard::{ScanResult, SkillsGuard};
use thiserror::Error;

/// Skill management error type.
#[derive(Debug, Error)]
pub enum SkillManagerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("security scan blocked: {0}")]
    SecurityBlocked(String),
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("skill already exists: {0}")]
    AlreadyExists(String),
    #[error("operation failed: {0}")]
    OperationFailed(String),
}

impl From<ValidationError> for SkillManagerError {
    fn from(e: ValidationError) -> Self {
        SkillManagerError::Validation(e.to_string())
    }
}

impl From<AtomicWriteError> for SkillManagerError {
    fn from(e: AtomicWriteError) -> Self {
        SkillManagerError::Io(std::io::Error::other(e.to_string()))
    }
}

/// Result type for skill management operations.
pub type SkillManagerResult<T> = Result<T, SkillManagerError>;

/// Input for a skill management operation.
#[derive(Debug, Clone)]
pub struct SkillManageInput {
    /// The action to perform.
    pub action: SkillManageAction,
    /// Skill name.
    pub name: String,
    /// Content for create/edit actions.
    pub content: Option<String>,
    /// Category for create action.
    pub category: Option<String>,
    /// File path for write_file/remove_file actions.
    pub file_path: Option<String>,
    /// File content for write_file action.
    pub file_content: Option<String>,
    /// Old string for patch action.
    pub old_string: Option<String>,
    /// New string for patch action.
    pub new_string: Option<String>,
    /// Whether to replace all occurrences for patch action.
    pub replace_all: bool,
}

/// The skill management action to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillManageAction {
    /// Create a new skill (SKILL.md + directory structure)
    Create,
    /// Edit/overwrite the entire SKILL.md
    Edit,
    /// Fuzzy find-and-replace in SKILL.md
    Patch,
    /// Delete the skill directory
    Delete,
    /// Add or overwrite a supporting file
    WriteFile,
    /// Remove a supporting file
    RemoveFile,
}

/// Output of a skill management operation.
#[derive(Debug, Clone)]
pub struct SkillManageOutput {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Human-readable message.
    pub message: String,
    /// Path to the skill (if applicable).
    pub skill_path: Option<PathBuf>,
    /// Reason if blocked by security scan.
    pub blocked_reason: Option<String>,
}

impl SkillManageOutput {
    pub fn success(message: impl Into<String>, skill_path: Option<PathBuf>) -> Self {
        Self {
            success: true,
            message: message.into(),
            skill_path,
            blocked_reason: None,
        }
    }

    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            success: false,
            message: String::new(),
            skill_path: None,
            blocked_reason: Some(reason.into()),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            skill_path: None,
            blocked_reason: None,
        }
    }
}

/// Execute a skill management action.
///
/// This is a helper function that dispatches to the appropriate action handler.
pub fn execute_action(
    action: SkillManageAction,
    name: &str,
    skills_dir: &Path,
    input: &SkillManageInput,
    guard: &SkillsGuard,
) -> SkillManagerResult<SkillManageOutput> {
    // Validate name first
    SkillValidator::validate_name(name)?;

    let skill_dir = skills_dir.join(name);

    match action {
        SkillManageAction::Create => create_skill(
            name,
            input.content.as_deref().unwrap_or(""),
            input.category.as_deref(),
            skills_dir,
            guard,
        ),
        SkillManageAction::Edit => {
            edit_skill(name, input.content.as_deref().unwrap_or(""), skills_dir)
        }
        SkillManageAction::Patch => patch_skill(
            name,
            input.old_string.as_deref().unwrap_or(""),
            input.new_string.as_deref().unwrap_or(""),
            input.replace_all,
            skills_dir,
        ),
        SkillManageAction::Delete => delete_skill(name, skills_dir),
        SkillManageAction::WriteFile => write_skill_file(
            name,
            input.file_path.as_deref().unwrap_or(""),
            input.file_content.as_deref().unwrap_or(""),
            skills_dir,
        ),
        SkillManageAction::RemoveFile => {
            remove_skill_file(name, input.file_path.as_deref().unwrap_or(""), skills_dir)
        }
    }
}

/// Create a new skill.
fn create_skill(
    name: &str,
    content: &str,
    category: Option<&str>,
    skills_dir: &Path,
    guard: &SkillsGuard,
) -> SkillManagerResult<SkillManageOutput> {
    // Validate inputs
    SkillValidator::validate_category(category)?;
    SkillValidator::validate_content_size(content)?;
    // Note: validate_frontmatter is not called here because we auto-generate
    // frontmatter with the name field. The user-provided content is body only.

    let skill_dir = skills_dir.join(name);

    // Check if skill already exists
    if skill_dir.exists() {
        return Err(SkillManagerError::AlreadyExists(name.into()));
    }

    // Create skill directory
    ensure_dir(&skill_dir)?;

    // Build SKILL.md with frontmatter
    let frontmatter = build_frontmatter(name, category, content);
    let skill_md_path = skill_dir.join("SKILL.md");

    // Security scan before writing
    // We scan the content to be written
    let temp_dir =
        tempfile::TempDir::new().map_err(|e| SkillManagerError::Io(std::io::Error::other(e)))?;
    let temp_skill_dir = temp_dir.path().join(name);
    ensure_dir(&temp_skill_dir)?;
    let temp_skill_md = temp_skill_dir.join("SKILL.md");
    atomic_write_str(&temp_skill_md, &frontmatter, AtomicWriteOptions::default())?;

    let scan_result = guard.scan(&temp_skill_dir, "agent-created");

    if !scan_result.findings.is_empty() {
        // Security scan found issues
        let blocked_reason = format!(
            "Security scan found {} findings: {:?}",
            scan_result.findings.len(),
            scan_result.verdict
        );
        return Ok(SkillManageOutput::blocked(blocked_reason));
    }

    // Write SKILL.md
    atomic_write_str(&skill_md_path, &frontmatter, AtomicWriteOptions::default())?;

    Ok(SkillManageOutput::success(
        format!("Created skill '{}'", name),
        Some(skill_dir),
    ))
}

/// Edit an existing skill's SKILL.md (full overwrite).
fn edit_skill(
    name: &str,
    content: &str,
    skills_dir: &Path,
) -> SkillManagerResult<SkillManageOutput> {
    SkillValidator::validate_content_size(content)?;

    let skill_dir = skills_dir.join(name);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_dir.exists() {
        return Err(SkillManagerError::NotFound(name.into()));
    }

    // Build SKILL.md with frontmatter (auto-generated name from skill name)
    let frontmatter = build_frontmatter(name, None, content);
    atomic_write_str(&skill_md_path, &frontmatter, AtomicWriteOptions::default())?;

    Ok(SkillManageOutput::success(
        format!("Updated skill '{}'", name),
        Some(skill_dir),
    ))
}

/// Patch an existing skill's SKILL.md (find and replace).
fn patch_skill(
    name: &str,
    old: &str,
    new: &str,
    replace_all: bool,
    skills_dir: &Path,
) -> SkillManagerResult<SkillManageOutput> {
    let skill_dir = skills_dir.join(name);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_dir.exists() {
        return Err(SkillManagerError::NotFound(name.into()));
    }

    let content = fs::read_to_string(&skill_md_path)?;

    if !content.contains(old) {
        return Err(SkillManagerError::OperationFailed(format!(
            "String '{}' not found in SKILL.md",
            old
        )));
    }

    let new_content = if replace_all {
        content.replace(old, new)
    } else {
        content.replacen(old, new, 1)
    };

    atomic_write_str(&skill_md_path, &new_content, AtomicWriteOptions::default())?;

    Ok(SkillManageOutput::success(
        format!("Patched skill '{}'", name),
        Some(skill_dir),
    ))
}

/// Delete a skill directory.
fn delete_skill(name: &str, skills_dir: &Path) -> SkillManagerResult<SkillManageOutput> {
    let skill_dir = skills_dir.join(name);

    if !skill_dir.exists() {
        return Err(SkillManagerError::NotFound(name.into()));
    }

    fs::remove_dir_all(&skill_dir)?;

    Ok(SkillManageOutput::success(
        format!("Deleted skill '{}'", name),
        None,
    ))
}

/// Write a supporting file to a skill.
fn write_skill_file(
    name: &str,
    file_path: &str,
    file_content: &str,
    skills_dir: &Path,
) -> SkillManagerResult<SkillManageOutput> {
    // Validate file path is in allowed subdirectory
    if !AllowedSubdirs::is_allowed(file_path) {
        return Err(SkillManagerError::Validation(format!(
            "File path '{}' is not in an allowed subdirectory. Allowed: {:?}",
            file_path,
            AllowedSubdirs::SET
        )));
    }

    let skill_dir = skills_dir.join(name);

    if !skill_dir.exists() {
        return Err(SkillManagerError::NotFound(name.into()));
    }

    let target_path = skill_dir.join(file_path);

    // Validate file size
    let size = file_content.len() as u64;
    SkillValidator::validate_file_size(size)?;

    // Ensure parent directory exists
    if let Some(parent) = target_path.parent() {
        ensure_dir(parent)?;
    }

    atomic_write_str(&target_path, file_content, AtomicWriteOptions::default())?;

    Ok(SkillManageOutput::success(
        format!("Wrote file '{}' to skill '{}'", file_path, name),
        Some(skill_dir),
    ))
}

/// Remove a supporting file from a skill.
fn remove_skill_file(
    name: &str,
    file_path: &str,
    skills_dir: &Path,
) -> SkillManagerResult<SkillManageOutput> {
    // Validate file path is in allowed subdirectory
    if !AllowedSubdirs::is_allowed(file_path) {
        return Err(SkillManagerError::Validation(format!(
            "File path '{}' is not in an allowed subdirectory. Allowed: {:?}",
            file_path,
            AllowedSubdirs::SET
        )));
    }

    let skill_dir = skills_dir.join(name);

    if !skill_dir.exists() {
        return Err(SkillManagerError::NotFound(name.into()));
    }

    let target_path = skill_dir.join(file_path);

    if !target_path.exists() {
        return Err(SkillManagerError::NotFound(format!(
            "File '{}' not found in skill '{}'",
            file_path, name
        )));
    }

    // Don't allow removing SKILL.md
    if target_path
        .file_name()
        .map(|n| n == "SKILL.md")
        .unwrap_or(false)
    {
        return Err(SkillManagerError::OperationFailed(
            "Cannot remove SKILL.md".into(),
        ));
    }

    safe_remove(&target_path)?;

    Ok(SkillManageOutput::success(
        format!("Removed file '{}' from skill '{}'", file_path, name),
        Some(skill_dir),
    ))
}

/// Build frontmatter for a new skill's SKILL.md.
fn build_frontmatter(name: &str, category: Option<&str>, content: &str) -> String {
    let mut frontmatter = format!("---\nname: {}\n", name);

    if let Some(cat) = category {
        frontmatter.push_str(&format!("category: {}\n", cat));
    }

    frontmatter.push_str("---\n\n");
    frontmatter.push_str(content);

    frontmatter
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_guard() -> SkillsGuard {
        SkillsGuard::new()
    }

    #[test]
    fn test_build_frontmatter() {
        let fm = build_frontmatter("test-skill", Some("coding"), "Hello world");
        assert!(fm.contains("name: test-skill"));
        assert!(fm.contains("category: coding"));
        assert!(fm.contains("Hello world"));
    }

    #[test]
    fn test_patch_replace_all() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "foo foo foo").unwrap();

        let result = patch_skill("test-skill", "foo", "bar", true, tmp.path()).unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&skill_md).unwrap();
        assert_eq!(content, "bar bar bar");
    }

    #[test]
    fn test_patch_replace_one() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "foo foo foo").unwrap();

        let result = patch_skill("test-skill", "foo", "bar", false, tmp.path()).unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&skill_md).unwrap();
        assert_eq!(content, "bar foo foo");
    }

    #[test]
    fn test_delete_skill() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "content").unwrap();

        let result = delete_skill("test-skill", tmp.path()).unwrap();
        assert!(result.success);
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_write_skill_file_allowed_subdir() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: test\n---\ncontent").unwrap();

        let result = write_skill_file("test-skill", "scripts/helper.sh", "echo hello", tmp.path());
        assert!(result.is_ok());
        assert!(skill_dir.join("scripts/helper.sh").exists());
    }

    #[test]
    fn test_write_skill_file_disallowed_subdir() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: test\n---\ncontent").unwrap();

        let result = write_skill_file("test-skill", "subdir/file.txt", "content", tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_skill_file() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: test\n---\ncontent").unwrap();
        let helper = skill_dir.join("scripts/helper.sh");
        std::fs::create_dir_all(helper.parent().unwrap()).unwrap();
        std::fs::write(&helper, "echo hello").unwrap();

        let result = remove_skill_file("test-skill", "scripts/helper.sh", tmp.path()).unwrap();
        assert!(result.success);
        assert!(!helper.exists());
    }

    #[test]
    fn test_remove_skill_file_not_found() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: test\n---\ncontent").unwrap();

        let result = remove_skill_file("test-skill", "scripts/nonexistent.sh", tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_remove_skill_md() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: test\n---\ncontent").unwrap();

        let result = remove_skill_file("test-skill", "SKILL.md", tmp.path());
        assert!(result.is_err());
    }
}
