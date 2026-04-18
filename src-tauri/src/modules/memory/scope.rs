//! Memory Execution Scope — enforces project/session isolation for memory operations.
//!
//! All memory reads and writes should be bound to a `MemoryExecutionScope` to prevent
//! cross-session and cross-project namespace collisions.
//!
//! # Usage
//!
//! ```ignore
//! use crate::modules::memory::scope::{MemoryScopeResolver, MemoryExecutionScope};
//!
//! let scope = MemoryScopeResolver::resolve(
//!     Some("session-abc"),
//!     Some("project-xyz"),
//!     Some("/workdir/path"),
//! );
//! ```

use serde::{Deserialize, Serialize};

/// Immutable execution scope that binds memory operations to a specific runtime context.
///
/// When a scope is provided to `store_scoped`/`recall_scoped`, all memory operations
/// are filtered to the matching `session_id` and/or `project_id`, preventing
/// cross-session namespace collisions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryExecutionScope {
    /// The active session identifier, if any.
    pub session_id: Option<String>,
    /// The active project identifier, if any.
    pub project_id: Option<String>,
    /// The effective working directory for filesystem-bound memory, if any.
    pub workdir: Option<String>,
}

impl MemoryExecutionScope {
    /// Return a "global" scope with no session, project, or workdir binding.
    ///
    /// Global scope is used for backward-compatible operations that are not
    /// tied to any session or project context.
    #[must_use]
    pub fn global() -> Self {
        Self {
            session_id: None,
            project_id: None,
            workdir: None,
        }
    }

    /// Returns `true` if this scope has no isolation constraints.
    ///
    /// A global scope has `None` for all fields and behaves like an unscoped operation.
    /// Used by policy engine and audit emitter (Memory Control Plane V1) to short-circuit
    /// scope-specific logic for globally-visible entries.
    #[must_use]
    #[allow(dead_code)] // public API consumed by MemoryPolicyEngine (fix-mcp-policy)
    pub fn is_global(&self) -> bool {
        self.session_id.is_none() && self.project_id.is_none() && self.workdir.is_none()
    }
}

/// Resolves a `MemoryExecutionScope` from runtime context inputs.
///
/// Call `MemoryScopeResolver::resolve()` at the command/tool entry boundary
/// using the current session and project identifiers. The resulting scope
/// is then threaded through all memory operations for that request.
pub struct MemoryScopeResolver;

impl MemoryScopeResolver {
    /// Resolve a scope from optional runtime context values.
    ///
    /// Any `None` fields indicate that the corresponding dimension is unbounded
    /// (i.e., accessible across all sessions or projects).
    ///
    /// Used by CLI entry points and the harness control plane (Phase 6E) where
    /// session/project IDs are available as separate values rather than a ToolContext.
    #[must_use]
    #[allow(dead_code)] // public API consumed by harness CLI and policy engine
    pub fn resolve(
        session_id: Option<&str>,
        project_id: Option<&str>,
        workdir: Option<&str>,
    ) -> MemoryExecutionScope {
        MemoryExecutionScope {
            session_id: session_id.map(String::from),
            project_id: project_id.map(String::from),
            workdir: workdir.map(String::from),
        }
    }

    /// Resolve scope from a `ToolContext`, using `session_id`, `project_id`
    /// and `workdir` fields.
    ///
    /// This is the preferred entry point when resolving scope inside a tool
    /// handler. With a fully populated `ToolContext` (typical for sessions
    /// dispatched through `ToolExecutionBroker`) the returned scope carries
    /// all three tiers (`session_id` + `project_id` + `workdir`), enabling the
    /// provider layer to apply session/project/global visibility rules.
    #[must_use]
    pub fn from_tool_context(
        ctx: &crate::modules::tools::context::ToolContext,
    ) -> MemoryExecutionScope {
        MemoryExecutionScope {
            session_id: ctx.session_id.clone(),
            project_id: ctx.project_id.clone(),
            workdir: Some(ctx.workdir.to_string_lossy().into_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_scope_is_global() {
        let scope = MemoryExecutionScope::global();
        assert!(scope.is_global());
    }

    #[test]
    fn resolved_scope_with_session_is_not_global() {
        let scope = MemoryScopeResolver::resolve(Some("session-abc"), None, None);
        assert!(!scope.is_global());
        assert_eq!(scope.session_id.as_deref(), Some("session-abc"));
        assert!(scope.project_id.is_none());
    }

    #[test]
    fn resolved_scope_with_all_fields() {
        let scope = MemoryScopeResolver::resolve(Some("sess"), Some("proj"), Some("/workdir"));
        assert_eq!(scope.session_id.as_deref(), Some("sess"));
        assert_eq!(scope.project_id.as_deref(), Some("proj"));
        assert_eq!(scope.workdir.as_deref(), Some("/workdir"));
        assert!(!scope.is_global());
    }

    #[test]
    fn global_scope_all_none() {
        let scope = MemoryScopeResolver::resolve(None, None, None);
        assert!(scope.is_global());
    }
}
