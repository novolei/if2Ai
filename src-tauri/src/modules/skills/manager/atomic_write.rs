#![allow(unused)]

//! Atomic file write operations using temp file + rename pattern.
//!
//! Ported from Hermes `skill_manager_tool.py` lines 257-271.
//!
//! This ensures that partial writes cannot corrupt existing files.
//! On Unix, rename(2) is atomic if the old and new files are on the same filesystem.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for atomic write operations.
#[derive(Debug, Error)]
pub enum AtomicWriteError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("content encoding error: {0}")]
    Encoding(String),
}

/// Result type for atomic write operations.
pub type AtomicWriteResult<T> = Result<T, AtomicWriteError>;

/// Options for atomic write operations.
#[derive(Debug, Clone)]
pub struct AtomicWriteOptions {
    /// Directory to create temp file in. Defaults to the parent directory of target.
    pub temp_dir: Option<PathBuf>,
    /// Suffix for the temp file. Defaults to target extension.
    pub suffix: Option<String>,
    /// Prefix for the temp file. Defaults to ".tmp".
    pub prefix: Option<String>,
}

impl Default for AtomicWriteOptions {
    fn default() -> Self {
        Self {
            temp_dir: None,
            suffix: None,
            prefix: Some(".tmp".into()),
        }
    }
}

/// Write content to a file atomically.
///
/// 1. Creates a temp file in the same directory as target (or specified temp_dir)
/// 2. Writes content to the temp file
/// 3. Renames temp file to target (atomic on same filesystem)
///
/// If step 2 fails, the temp file is removed.
/// If step 3 fails, the temp file is removed.
///
/// # Arguments
///
/// * `target` - The final destination path
/// * `content` - The content to write
/// * `options` - Optional configuration for the atomic write
///
/// # Errors
///
/// Returns `AtomicWriteError` if:
/// Atomic write using temp file + rename pattern.
///
/// Uses NamedTempFile which keeps the temp file alive until it's renamed.
fn atomic_write_impl(
    target: &Path,
    content: impl AsRef<[u8]>,
    options: AtomicWriteOptions,
) -> AtomicWriteResult<()> {
    use std::io::Write;

    let target = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());

    // Determine temp directory
    let temp_base = options.temp_dir.unwrap_or_else(|| {
        target
            .parent()
            .unwrap_or_else(|| Path::new("/tmp"))
            .to_path_buf()
    });

    // Use NamedTempFile which keeps the temp file alive when file handle is dropped
    let mut temp_file = tempfile::NamedTempFile::new_in(&temp_base)
        .map_err(|e| AtomicWriteError::Io(io::Error::other(e)))?;

    // Write content
    temp_file.write_all(content.as_ref())?;
    temp_file.flush()?;

    // Get path before we rename (NamedTempFile keeps file alive after drop)
    let temp_path = temp_file.path().to_path_buf();

    // Atomic rename to target
    fs::rename(&temp_path, &target).map_err(|e| {
        // On failure, try to clean up but don't fail the error
        let _ = fs::remove_file(&temp_path);
        AtomicWriteError::Io(e)
    })?;

    // Explicitly close the temp file before it gets dropped
    drop(temp_file);

    Ok(())
}

/// Atomic write entry point.
pub fn atomic_write(
    target: &Path,
    content: impl AsRef<[u8]>,
    options: AtomicWriteOptions,
) -> AtomicWriteResult<()> {
    atomic_write_impl(target, content, options)
}

/// Write string content to a file atomically.
pub fn atomic_write_str(
    target: &Path,
    content: &str,
    options: AtomicWriteOptions,
) -> AtomicWriteResult<()> {
    atomic_write(target, content.as_bytes(), options)
}

/// Perform an atomic write with rollback capability.
///
/// If the write fails after creating a backup, the backup is restored.
pub fn atomic_write_with_rollback<F>(
    target: &Path,
    content: &str,
    backup_path: &Path,
    f: F,
) -> AtomicWriteResult<()>
where
    F: FnOnce() -> AtomicWriteResult<()>,
{
    // If target exists, create backup first
    if target.exists() {
        let target_content = fs::read(target)?;
        fs::write(backup_path, target_content)?;
    }

    // Attempt the operation
    let result = atomic_write_str(target, content, AtomicWriteOptions::default());

    match result {
        Ok(()) => {
            // Operation succeeded, run callback (e.g., security scan)
            let callback_result = f();

            if callback_result.is_err() {
                // Callback failed, rollback to backup
                if backup_path.exists() {
                    let backup_content = fs::read(backup_path)?;
                    let _ = atomic_write(target, &backup_content, AtomicWriteOptions::default());
                    let _ = fs::remove_file(backup_path);
                }
                return callback_result;
            }

            // Remove backup on success
            if backup_path.exists() {
                let _ = fs::remove_file(backup_path);
            }

            Ok(())
        }
        Err(e) => {
            // Write failed, attempt to restore from backup
            if backup_path.exists() {
                let backup_content = fs::read(backup_path)?;
                let _ = atomic_write(target, &backup_content, AtomicWriteOptions::default());
                let _ = fs::remove_file(backup_path);
            }
            Err(e)
        }
    }
}

/// Read a file into a string, creating parent directories if needed.
pub fn ensure_read(path: &Path) -> AtomicWriteResult<String> {
    fs::read_to_string(path).map_err(AtomicWriteError::Io)
}

/// Remove a file safely, ignoring errors if file doesn't exist.
pub fn safe_remove(path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    }
}

/// Create a directory and all parent directories if they don't exist.
pub fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write_basic() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("test.txt");

        atomic_write_str(&target, "hello world", AtomicWriteOptions::default()).unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_atomic_write_preserves_existing_on_failure() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("test.txt");
        let backup = tmp.path().join("test.txt.bak");

        // Write initial content
        atomic_write_str(&target, "initial", AtomicWriteOptions::default()).unwrap();

        // Atomic write with rollback should preserve original on failure
        let result = atomic_write_with_rollback(&target, "new content", &backup, || {
            Err(AtomicWriteError::Encoding("simulated failure".into()))
        });

        assert!(result.is_err());

        // Original content should be preserved
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "initial");
    }

    #[test]
    fn test_safe_remove() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("to_remove.txt");

        // Should succeed even if file doesn't exist
        assert!(safe_remove(&file).is_ok());

        // Create and remove
        fs::write(&file, "hello").unwrap();
        assert!(safe_remove(&file).is_ok());
        assert!(!file.exists());
    }

    #[test]
    fn test_ensure_dir() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a/b/c/d");

        ensure_dir(&nested).unwrap();
        assert!(nested.exists());

        // Should be idempotent
        ensure_dir(&nested).unwrap();
    }
}
