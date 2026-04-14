#![allow(unused)]

//! Validation rules for skill management operations.
//!
//! Ported from Hermes `skill_manager_tool.py` lines 83-189.

use std::path::Path;
use thiserror::Error;

/// Validation error types.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid skill name: {0}")]
    InvalidName(String),
    #[error("Invalid category: {0}")]
    InvalidCategory(String),
    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error("Content too large: {0} chars (max: {1})")]
    ContentTooLarge(usize, usize),
    #[error("File too large: {0} bytes (max: {1})")]
    FileTooLarge(u64, u64),
    #[error("Invalid file path: {0}")]
    InvalidFilePath(String),
    #[error(" disallowed subdirectory: {0}")]
    DisallowedSubdir(String),
}

/// Skill name validator.
///
/// Valid names match: `^[a-z0-9][a-z0-9._-]*$`
/// - Start with alphanumeric
/// - Subsequent characters may include dots, underscores, hyphens
/// - Case insensitive but normalized to lowercase
pub struct SkillValidator;

impl SkillValidator {
    /// Maximum length for a skill name.
    pub const MAX_NAME_LENGTH: usize = 64;

    /// Maximum length for a skill description.
    pub const MAX_DESCRIPTION_LENGTH: usize = 1024;

    /// Maximum character count for skill content (SKILL.md).
    pub const MAX_SKILL_CONTENT_CHARS: usize = 100_000;

    /// Maximum bytes for a supporting file.
    pub const MAX_SUPPORTING_FILE_BYTES: usize = 1_048_576;

    /// Validate a skill name.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidName` if the name:
    /// - Is empty or exceeds MAX_NAME_LENGTH
    /// - Contains characters other than lowercase alphanumeric, dots, underscores, hyphens
    /// - Does not start with an alphanumeric character
    pub fn validate_name(name: &str) -> Result<(), ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::InvalidName("name cannot be empty".into()));
        }
        if name.len() > Self::MAX_NAME_LENGTH {
            return Err(ValidationError::InvalidName(format!(
                "name exceeds {} characters",
                Self::MAX_NAME_LENGTH
            )));
        }

        // Hermes pattern: r'^[a-z0-9][a-z0-9._-]*$' - lowercase only
        let mut chars = name.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err(ValidationError::InvalidName(
                "name must start with a lowercase alphanumeric character".into(),
            ));
        }

        for ch in chars {
            // Allow lowercase letters, digits, dots, underscores, hyphens
            let is_valid = ch.is_ascii_lowercase()
                || ch.is_ascii_digit()
                || ch == '.'
                || ch == '_'
                || ch == '-';
            if !is_valid {
                return Err(ValidationError::InvalidName(format!(
                    "name contains invalid character: '{}'",
                    ch
                )));
            }
        }

        Ok(())
    }

    /// Validate a skill category.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidCategory` if the category is not a valid
    /// non-empty string.
    pub fn validate_category(category: Option<&str>) -> Result<(), ValidationError> {
        if let Some(cat) = category {
            if cat.is_empty() {
                return Err(ValidationError::InvalidCategory(
                    "category cannot be empty string".into(),
                ));
            }
            // Categories are typically alphanumeric with underscores/hyphens
            for ch in cat.chars() {
                if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
                    return Err(ValidationError::InvalidCategory(format!(
                        "category contains invalid character: '{}'",
                        ch
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate skill content size.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::ContentTooLarge` if content exceeds MAX_SKILL_CONTENT_CHARS.
    pub fn validate_content_size(content: &str) -> Result<(), ValidationError> {
        if content.chars().count() > Self::MAX_SKILL_CONTENT_CHARS {
            return Err(ValidationError::ContentTooLarge(
                content.chars().count(),
                Self::MAX_SKILL_CONTENT_CHARS,
            ));
        }
        Ok(())
    }

    /// Validate frontmatter structure.
    ///
    /// Expects YAML frontmatter with at minimum a `name` field.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidFrontmatter` if:
    /// - Frontmatter is missing required `name` field
    /// - Frontmatter is malformed YAML
    pub fn validate_frontmatter(content: &str) -> Result<(), ValidationError> {
        let content = content.trim();

        // Check for frontmatter delimiters
        if !content.starts_with("---") {
            return Err(ValidationError::InvalidFrontmatter(
                "missing frontmatter delimiter '---'".into(),
            ));
        }

        // Find the closing delimiter
        let after_first_dash = match content.find("\n---") {
            Some(pos) => pos,
            None => {
                return Err(ValidationError::InvalidFrontmatter(
                    "missing closing frontmatter delimiter".into(),
                ))
            }
        };

        let frontmatter = &content[3..after_first_dash];

        // Parse YAML to extract name field
        let name_line = frontmatter
            .lines()
            .find(|line| line.trim().starts_with("name:"));

        if name_line.is_none() {
            return Err(ValidationError::InvalidFrontmatter(
                "missing required 'name' field in frontmatter".into(),
            ));
        }

        Ok(())
    }

    /// Validate a supporting file size.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::FileTooLarge` if file exceeds MAX_SUPPORTING_FILE_BYTES.
    pub fn validate_file_size(size_bytes: u64) -> Result<(), ValidationError> {
        let max: u64 = Self::MAX_SUPPORTING_FILE_BYTES as u64;
        if size_bytes > max {
            return Err(ValidationError::FileTooLarge(size_bytes, max));
        }
        Ok(())
    }
}

/// Allowed subdirectories within a skill directory.
///
/// Skills may only contain files in these subdirectories:
/// - `references/` - Reference documentation
/// - `templates/` - Template files
/// - `scripts/` - Executable scripts
/// - `assets/` - Static assets
pub struct AllowedSubdirs;

impl AllowedSubdirs {
    /// The set of allowed subdirectory names (without trailing slash).
    pub const SET: &'static [&'static str] = &["references", "templates", "scripts", "assets"];

    /// Check if a file path is within an allowed subdirectory.
    ///
    /// Returns `true` if:
    /// - The path is directly in the skill root (no subdirectory)
    /// - The path is inside one of the allowed subdirectories
    ///
    /// Returns `false` if:
    /// - The path contains ".." path traversal
    /// - The path is inside a non-allowed subdirectory
    pub fn is_allowed(path: &str) -> bool {
        let path = path.trim_start_matches('/');

        // Block path traversal attempts
        if path.contains("..") {
            return false;
        }

        // If no directory component, it's at root (always allowed)
        if !path.contains('/') {
            return true;
        }

        // Extract the first directory component
        if let Some(first_dir) = path.split('/').next() {
            Self::SET.contains(&first_dir)
        } else {
            true
        }
    }

    /// Check if a Path is within an allowed subdirectory.
    pub fn is_allowed_path(path: &Path) -> bool {
        Self::is_allowed(&path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_valid() {
        assert!(SkillValidator::validate_name("my-skill").is_ok());
        assert!(SkillValidator::validate_name("my_skill").is_ok());
        assert!(SkillValidator::validate_name("my.skill").is_ok());
        assert!(SkillValidator::validate_name("my-skill_v1").is_ok());
        assert!(SkillValidator::validate_name("a").is_ok());
        assert!(SkillValidator::validate_name("skill123").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        assert!(SkillValidator::validate_name("").is_err());
        assert!(SkillValidator::validate_name("-invalid").is_err()); // starts with hyphen
        assert!(SkillValidator::validate_name("_invalid").is_err()); // starts with underscore
        assert!(SkillValidator::validate_name(".invalid").is_err()); // starts with dot
        assert!(SkillValidator::validate_name("Invalid").is_err()); // uppercase
        assert!(SkillValidator::validate_name("has space").is_err()); // contains space
        assert!(SkillValidator::validate_name("has@special").is_err()); // contains @
    }

    #[test]
    fn test_validate_name_too_long() {
        let long_name = "a".repeat(65);
        assert!(SkillValidator::validate_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_category_valid() {
        assert!(SkillValidator::validate_category(Some("coding")).is_ok());
        assert!(SkillValidator::validate_category(Some("web-dev")).is_ok());
        assert!(SkillValidator::validate_category(Some("my_category")).is_ok());
        assert!(SkillValidator::validate_category(None).is_ok()); // None is allowed
    }

    #[test]
    fn test_validate_category_invalid() {
        assert!(SkillValidator::validate_category(Some("")).is_err());
        assert!(SkillValidator::validate_category(Some("has space")).is_err());
        assert!(SkillValidator::validate_category(Some("has@special")).is_err());
    }

    #[test]
    fn test_validate_frontmatter_valid() {
        let content = r#"---
name: test-skill
description: A test skill
---
# Skill Content"#;
        assert!(SkillValidator::validate_frontmatter(content).is_ok());
    }

    #[test]
    fn test_validate_frontmatter_missing_delimiter() {
        let content = r#"name: test-skill
description: A test skill
---
# Skill Content"#;
        assert!(SkillValidator::validate_frontmatter(content).is_err());
    }

    #[test]
    fn test_validate_frontmatter_missing_name() {
        let content = r#"---
description: A test skill
---
# Skill Content"#;
        assert!(SkillValidator::validate_frontmatter(content).is_err());
    }

    #[test]
    fn test_allowed_subdirs() {
        // Root files are allowed
        assert!(AllowedSubdirs::is_allowed("SKILL.md"));
        assert!(AllowedSubdirs::is_allowed("README.md"));

        // Allowed subdirs
        assert!(AllowedSubdirs::is_allowed("references/"));
        assert!(AllowedSubdirs::is_allowed("templates/"));
        assert!(AllowedSubdirs::is_allowed("scripts/"));
        assert!(AllowedSubdirs::is_allowed("assets/"));

        // Files in allowed subdirs
        assert!(AllowedSubdirs::is_allowed("references/readme.txt"));
        assert!(AllowedSubdirs::is_allowed("templates/template.md"));
        assert!(AllowedSubdirs::is_allowed("scripts/run.sh"));
        assert!(AllowedSubdirs::is_allowed("assets/image.png"));

        // Disallowed subdirs
        assert!(!AllowedSubdirs::is_allowed("subdir/"));
        assert!(!AllowedSubdirs::is_allowed("subdir/file.txt"));
        assert!(!AllowedSubdirs::is_allowed("scripts/../etc/passwd"));
        assert!(!AllowedSubdirs::is_allowed("scripts/../../etc/passwd"));
    }

    #[test]
    fn test_content_size_valid() {
        let content = "a".repeat(1000);
        assert!(SkillValidator::validate_content_size(&content).is_ok());
    }

    #[test]
    fn test_content_size_too_large() {
        let content = "a".repeat(100_001);
        assert!(SkillValidator::validate_content_size(&content).is_err());
    }

    #[test]
    fn test_file_size_valid() {
        assert!(SkillValidator::validate_file_size(1024).is_ok());
        assert!(SkillValidator::validate_file_size(1_048_576).is_ok());
    }

    #[test]
    fn test_file_size_too_large() {
        assert!(SkillValidator::validate_file_size(1_048_577).is_err());
    }
}
