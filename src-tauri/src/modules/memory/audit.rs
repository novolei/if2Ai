//! Memory Audit Emitter — structured audit events for all memory operations.
//!
//! `MemoryAuditEmitter` emits structured events at key points in the memory
//! lifecycle. Each event carries a standard set of trace fields so that
//! memory operations can be correlated across sessions, projects, and tool calls.
//!
//! # Events
//!
//! | Event | Emitted when |
//! |-------|-------------|
//! | `memory_captured` | Agent or tool identifies new information to remember |
//! | `memory_write_decision` | Policy engine produces an allow/deny/prompt decision |
//! | `memory_persisted` | Entry is successfully written to the backing store |
//! | `memory_recall_served` | A recall query returns results to the caller |
//! | `memory_rejected` | A write or recall is blocked by policy (Deny) |
//! | `memory_promoted` | An entry is promoted from session-scoped to global/long-term |
//!
//! # Usage
//!
//! Emit is fire-and-forget via `tracing::info!` events with structured fields.
//! The sink for these events is the application's tracing subscriber (file logs,
//! telemetry backend, etc.).  No async I/O or locking is required at the call site.

use crate::modules::memory::policy::{PolicyDecision, ReasonCode};
use crate::modules::memory::scope::MemoryExecutionScope;

/// Standard fields present on every memory audit event.
///
/// All fields are `Option<&str>` so callers can omit fields that are not
/// available in the current context without changing function signatures.
#[derive(Debug, Clone)]
pub struct AuditContext<'a> {
    /// Unique trace identifier for correlating events across a single operation.
    pub trace_id: Option<&'a str>,
    /// Session identifier from the memory execution scope.
    pub session_id: Option<&'a str>,
    /// Project identifier from the memory execution scope.
    pub project_id: Option<&'a str>,
    /// Effective working directory from the memory execution scope.
    pub effective_workdir: Option<&'a str>,
}

impl<'a> AuditContext<'a> {
    /// Build an `AuditContext` from a `MemoryExecutionScope`.
    #[must_use]
    pub fn from_scope(scope: &'a MemoryExecutionScope) -> Self {
        Self {
            trace_id: None,
            session_id: scope.session_id.as_deref(),
            project_id: scope.project_id.as_deref(),
            effective_workdir: scope.workdir.as_deref(),
        }
    }

    /// Build an `AuditContext` from a scope with an explicit trace ID.
    ///
    /// Used by Phase 6E harness tracing where trace_id is propagated from
    /// the EventBus turn context.
    #[must_use]
    #[allow(dead_code)] // API consumed by Phase 6E harness (fix-phase-6e)
    pub fn from_scope_with_trace(scope: &'a MemoryExecutionScope, trace_id: &'a str) -> Self {
        Self {
            trace_id: Some(trace_id),
            session_id: scope.session_id.as_deref(),
            project_id: scope.project_id.as_deref(),
            effective_workdir: scope.workdir.as_deref(),
        }
    }
}

/// Emits structured audit events for memory operations.
///
/// All methods are synchronous and non-blocking — they log via `tracing::info!`
/// using structured fields that can be captured by any tracing subscriber.
pub struct MemoryAuditEmitter;

impl MemoryAuditEmitter {
    /// Emit a `memory_captured` event.
    ///
    /// Call this when an agent or tool has identified new information worth remembering,
    /// before the write decision has been evaluated.
    pub fn memory_captured(ctx: &AuditContext<'_>, key: &str, category: &str) {
        tracing::info!(
            event = "memory_captured",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            memory_category = category,
        );
    }

    /// Emit a `memory_write_decision` event.
    ///
    /// Call this immediately after the `MemoryPolicyEngine` produces its decision,
    /// before the write is executed (or rejected).
    pub fn memory_write_decision(
        ctx: &AuditContext<'_>,
        key: &str,
        decision: &PolicyDecision,
        reason_code: &ReasonCode,
        message: &str,
    ) {
        tracing::info!(
            event = "memory_write_decision",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            policy_decision = format!("{decision:?}"),
            reason_code = reason_code.label(),
            reason_message = message,
        );
    }

    /// Emit a `memory_persisted` event.
    ///
    /// Call this after a successful write to the backing store.
    pub fn memory_persisted(ctx: &AuditContext<'_>, key: &str, category: &str) {
        tracing::info!(
            event = "memory_persisted",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            memory_category = category,
        );
    }

    /// Emit a `memory_recall_served` event.
    ///
    /// Call this after a recall query completes, recording the result count.
    pub fn memory_recall_served(
        ctx: &AuditContext<'_>,
        query: &str,
        category: Option<&str>,
        result_count: usize,
    ) {
        tracing::info!(
            event = "memory_recall_served",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            recall_query = query,
            recall_category = category.unwrap_or("-"),
            result_count = result_count,
        );
    }

    /// Emit a `memory_rejected` event.
    ///
    /// Call this when a write or recall is blocked by the policy engine (`Deny` decision
    /// in enforce mode).
    pub fn memory_rejected(
        ctx: &AuditContext<'_>,
        key: &str,
        reason_code: &ReasonCode,
        message: &str,
    ) {
        tracing::info!(
            event = "memory_rejected",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            reason_code = reason_code.label(),
            reason_message = message,
        );
    }

    /// Emit a `memory_promoted` event.
    ///
    /// Call this when an entry is elevated from session-scoped to globally-visible
    /// (e.g., a user pins or promotes a session memory for long-term retention).
    /// Will be called from the memory promotion UI flow (FE-A memory-chip component).
    #[allow(dead_code)] // API consumed by future memory promotion UI (FE-A memory-chip)
    pub fn memory_promoted(
        ctx: &AuditContext<'_>,
        key: &str,
        from_category: &str,
        to_category: &str,
    ) {
        tracing::info!(
            event = "memory_promoted",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            from_category = from_category,
            to_category = to_category,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::policy::{PolicyDecision, ReasonCode};
    use crate::modules::memory::scope::MemoryScopeResolver;

    fn test_scope() -> MemoryExecutionScope {
        MemoryScopeResolver::resolve(Some("test-session"), Some("test-project"), None)
    }

    #[test]
    fn audit_context_from_scope_populates_fields() {
        let scope = test_scope();
        let ctx = AuditContext::from_scope(&scope);
        assert_eq!(ctx.session_id, Some("test-session"));
        assert_eq!(ctx.project_id, Some("test-project"));
        assert!(ctx.trace_id.is_none());
    }

    #[test]
    fn audit_context_from_scope_with_trace_sets_trace_id() {
        let scope = test_scope();
        let ctx = AuditContext::from_scope_with_trace(&scope, "trace-xyz");
        assert_eq!(ctx.trace_id, Some("trace-xyz"));
        assert_eq!(ctx.session_id, Some("test-session"));
    }

    #[test]
    fn all_emitter_methods_compile_and_run_without_panic() {
        let scope = test_scope();
        let ctx = AuditContext::from_scope(&scope);

        // These calls emit tracing events — verify they don't panic.
        MemoryAuditEmitter::memory_captured(&ctx, "key", "core");
        MemoryAuditEmitter::memory_write_decision(
            &ctx,
            "key",
            &PolicyDecision::Allow,
            &ReasonCode::NoRuleMatched,
            "allowed",
        );
        MemoryAuditEmitter::memory_persisted(&ctx, "key", "core");
        MemoryAuditEmitter::memory_recall_served(&ctx, "query", Some("core"), 3);
        MemoryAuditEmitter::memory_rejected(&ctx, "key", &ReasonCode::ContentTooLong, "too long");
        MemoryAuditEmitter::memory_promoted(&ctx, "key", "conversation", "core");
    }
}
