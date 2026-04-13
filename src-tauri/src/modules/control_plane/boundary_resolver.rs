//! Unified filesystem boundary resolver for tool execution.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::modules::tools::registry::ToolError;

/// Shared path resolution and workdir-boundary checks.
pub struct BoundaryResolver;

impl BoundaryResolver {
    /// Resolve user-provided path against workdir.
    #[must_use]
    pub fn resolve_user_path(workdir: &Path, input_path: &Path) -> PathBuf {
        if input_path.is_absolute() {
            input_path.to_path_buf()
        } else {
            workdir.join(input_path)
        }
    }

    /// Canonicalize workdir for trusted boundary comparisons.
    ///
    /// # Errors
    ///
    /// Returns an error when the workdir cannot be canonicalized.
    pub fn canonicalize_workdir(workdir: &Path) -> Result<PathBuf, ToolError> {
        workdir.canonicalize().map_err(|e| {
            ToolError::Handler(format!("invalid workdir '{}': {e}", workdir.display()))
        })
    }

    /// Canonicalize an existing path.
    ///
    /// # Errors
    ///
    /// Returns an error when the path does not exist or canonicalization fails.
    pub fn canonicalize_existing(path: &Path) -> Result<PathBuf, ToolError> {
        path.canonicalize()
            .map_err(|e| ToolError::Handler(format!("invalid path '{}': {e}", path.display())))
    }

    /// Canonicalize a path even when its final leaf does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error when no existing ancestor can be found.
    pub fn canonicalize_with_missing_leaf_support(path: &Path) -> Result<PathBuf, ToolError> {
        let mut cursor = path.to_path_buf();
        let mut suffix: Vec<OsString> = Vec::new();

        while !cursor.exists() {
            if let Some(name) = cursor.file_name() {
                suffix.push(name.to_os_string());
            }
            cursor = cursor
                .parent()
                .ok_or_else(|| {
                    ToolError::Handler(format!(
                        "invalid path '{}': no existing ancestor found",
                        path.display()
                    ))
                })?
                .to_path_buf();
        }

        let mut canonical = cursor.canonicalize().map_err(|e| {
            ToolError::Handler(format!(
                "invalid path '{}': failed to resolve ancestor '{}': {e}",
                path.display(),
                cursor.display()
            ))
        })?;
        for segment in suffix.iter().rev() {
            canonical.push(segment);
        }
        Ok(canonical)
    }

    /// Assert a canonical path is within canonical workdir.
    ///
    /// # Errors
    ///
    /// Returns an error when `canonical_path` escapes `canonical_workdir`.
    pub fn assert_within_workdir(
        canonical_workdir: &Path,
        canonical_path: &Path,
    ) -> Result<(), ToolError> {
        if !canonical_path.starts_with(canonical_workdir) {
            return Err(ToolError::Handler(format!(
                "path '{}' is outside allowed workdir '{}'",
                canonical_path.display(),
                canonical_workdir.display()
            )));
        }
        Ok(())
    }
}
