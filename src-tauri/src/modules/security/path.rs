//! Path validation — prevent traversal attacks
//!
//! Ensures all file operations stay within allowed base directories.

use std::path::{Path, PathBuf};

/// Validate that a requested path is under a base directory
///
/// Prevents path traversal attacks using `../` or symlink tricks.
pub fn validate_safe_path(base: &Path, requested: &Path) -> Result<PathBuf, PathError> {
    // Canonicalize base path first
    if !base.exists() {
        std::fs::create_dir_all(base).map_err(|_| PathError::BasePathInvalid)?;
    }
    let base_canonical = base
        .canonicalize()
        .map_err(|_| PathError::BasePathInvalid)?;

    // Combine base with requested path
    let full_path = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base.join(requested)
    };

    // If the path exists, canonicalize and check directly
    if full_path.exists() {
        let canonical = full_path
            .canonicalize()
            .map_err(|_| PathError::PathTraversalAttempt)?;
        if !canonical.starts_with(&base_canonical) {
            return Err(PathError::PathTraversalAttempt);
        }
        return Ok(canonical);
    }

    // Path doesn't exist: find nearest existing ancestor and walk remaining components
    let mut current = full_path.clone();
    while !current.exists() {
        let parent = current.parent().ok_or(PathError::PathTraversalAttempt)?;
        current = parent.to_path_buf();
    }

    let mut canonical = current
        .canonicalize()
        .map_err(|_| PathError::PathTraversalAttempt)?;

    // Walk remaining path components from the ancestor to the full path
    let remaining = full_path
        .strip_prefix(&current)
        .map_err(|_| PathError::PathTraversalAttempt)?;
    for component in remaining.components() {
        match component {
            std::path::Component::Normal(_) => {
                canonical = canonical.join(component);
            }
            std::path::Component::ParentDir => {
                canonical = canonical
                    .parent()
                    .ok_or(PathError::PathTraversalAttempt)?
                    .to_path_buf();
                // Check after each '..' that we're still under base
                if !canonical.starts_with(&base_canonical) {
                    return Err(PathError::PathTraversalAttempt);
                }
            }
            _ => {
                canonical = canonical.join(component);
            }
        }
    }

    if !canonical.starts_with(&base_canonical) {
        return Err(PathError::PathTraversalAttempt);
    }

    Ok(canonical)
}

/// Get safe memory database path
pub fn get_memory_db_path() -> Result<PathBuf, PathError> {
    let base = dirs::data_local_dir().ok_or(PathError::NoDataDirectory)?;
    let memory_dir = base.join(".if2ai").join("memory");

    std::fs::create_dir_all(&memory_dir).map_err(|_| PathError::DirectoryCreationFailed)?;

    let safe_path = validate_safe_path(&base, &memory_dir)?;

    Ok(safe_path.join("memory.db"))
}

/// Error types for path validation
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("base path is invalid")]
    BasePathInvalid,

    #[error("path traversal attempt detected")]
    PathTraversalAttempt,

    #[error("no data directory available")]
    NoDataDirectory,

    #[error("directory creation failed")]
    DirectoryCreationFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn allows_child_path() {
        let temp = std::env::temp_dir().join("if2ai_test_validate");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let child = temp.join("child").join("file.db");
        let result = validate_safe_path(&temp, &child);
        assert!(result.is_ok());
        let temp_canonical = temp.canonicalize().unwrap();
        assert!(result.unwrap().starts_with(&temp_canonical));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn rejects_parent_escape() {
        let temp = std::env::temp_dir().join("if2ai_test_escape");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        // Try to escape with ../
        let escape_path = temp.join("child").join("..").join("..");
        let result = validate_safe_path(&temp, &escape_path);
        // Should reject since resolved path is outside base
        if let Err(e) = result {
            assert!(matches!(e, PathError::PathTraversalAttempt));
        }

        let _ = fs::remove_dir_all(&temp);
    }
}
