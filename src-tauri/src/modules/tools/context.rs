//! Tool Context Module
//!
//! Provides ToolContext for passing workdir and permission information to tools.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{collections::hash_map::DefaultHasher, hash::Hasher};

/// Tool execution context containing workdir and permission information.
/// This context is passed to tool handlers when they are executed,
/// allowing tools to respect filesystem boundaries and permission modes.
#[allow(dead_code)]
#[derive(Debug)]
pub struct ToolContext {
    /// Optional session id for session-scoped tool state.
    pub session_id: Option<String>,
    /// The allowed working directory for file operations.
    pub workdir: PathBuf,
    /// The permission mode controlling what operations are allowed.
    pub permission_mode: crate::modules::runtime::permissions::PermissionMode,
}

/// Shared tool context using Arc<Mutex> for thread-safe access.
///
/// This type is used to share the tool context across async tasks
/// while allowing mutable access for context updates.
pub type SharedToolContext = Arc<Mutex<ToolContext>>;

/// Build a deterministic context fingerprint for cross-session interference tracing.
#[must_use]
pub fn context_fingerprint(session_id: &str, workdir: &std::path::Path) -> String {
    let mut hasher = DefaultHasher::new();
    hasher.write(session_id.as_bytes());
    hasher.write(b"|");
    hasher.write(workdir.to_string_lossy().as_bytes());
    format!("{:016x}", hasher.finish())
}

#[allow(dead_code)]
impl ToolContext {
    /// Creates a new ToolContext with the given workdir and permission mode.
    #[must_use]
    pub fn new(
        workdir: PathBuf,
        permission_mode: crate::modules::runtime::permissions::PermissionMode,
    ) -> Self {
        Self {
            session_id: None,
            workdir,
            permission_mode,
        }
    }

    /// Creates a new ToolContext with explicit session id.
    #[must_use]
    pub fn new_with_session(
        session_id: Option<String>,
        workdir: PathBuf,
        permission_mode: crate::modules::runtime::permissions::PermissionMode,
    ) -> Self {
        Self {
            session_id,
            workdir,
            permission_mode,
        }
    }

    /// Returns a default ToolContext with the current directory and DangerFullAccess mode.
    #[must_use]
    pub fn default_for_workdir(workdir: PathBuf) -> Self {
        Self {
            session_id: None,
            workdir,
            permission_mode: crate::modules::runtime::permissions::PermissionMode::DangerFullAccess,
        }
    }
}
