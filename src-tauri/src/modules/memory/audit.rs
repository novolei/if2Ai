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

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::modules::memory::policy::{PolicyDecision, ReasonCode};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::security::DetectedPii;

/// Global Tauri app handle registered at startup so `MemoryAuditEmitter` can
/// forward audit events to the frontend `MemoryChip` / `MemoryWriteCard` UI.
///
/// `OnceLock` is used so the emitter remains a fire-and-forget API while
/// still being able to publish structured events over IPC when wired.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Register the Tauri `AppHandle` used by `MemoryAuditEmitter` to forward
/// events to the frontend.  Must be called once at startup; subsequent
/// invocations are ignored.
pub fn register_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Tauri event payload mirrored to the frontend `memory_event` channel.
///
/// Mirrors the TypeScript `MemoryEventPayload` interface in `src/lib/tauri.ts`.
#[derive(Debug, Clone, Serialize)]
struct MemoryEventPayload<'a> {
    /// One of `memory_captured` / `memory_write_decision` / `memory_persisted`
    /// / `memory_recall_served` / `memory_rejected` / `memory_promoted`.
    event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_workdir: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_category: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_message: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recall_query: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recall_category: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_category: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_category: Option<&'a str>,
    /// v2 §0.5 Δ-4 — variable structured metadata for events whose payload
    /// shape differs from the original 14 fixed fields (e.g. PII detection
    /// arrays, summary section counts, recovered job lists).  Default `None`
    /// + `skip_serializing_if` keeps the JSON wire shape backward compatible.
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
    /// ISO 8601 timestamp captured at emit time so the frontend can sort
    /// events by recency without trusting clock skew across IPC.
    timestamp: String,
}

fn emit_to_frontend(payload: MemoryEventPayload<'_>) {
    let Some(handle) = APP_HANDLE.get() else {
        return;
    };
    if let Err(e) = handle.emit("memory_event", &payload) {
        tracing::warn!(
            "[MemoryAuditEmitter] failed to emit memory_event {event}: {err}",
            event = payload.event,
            err = e
        );
    }
}

fn policy_decision_label(decision: &PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Deny => "deny",
        PolicyDecision::Prompt => "prompt",
    }
}

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

/// One entry in a `memory_ticker_recovery` audit event payload.
///
/// Surfaced when [`crate::modules::memory::ticker::MemoryTicker::start`]
/// detects that a session sidecar file under
/// `<data_local>/.if2ai/memory/summaries/*.json` was modified more
/// recently than its last persisted [`crate::modules::memory::summary::SessionSummaryRecord`]
/// (within the 24h cutoff window).
///
/// `recover_unsummarized` does **not** synthetically re-roll the
/// summary from cold disk — the message transcript is unavailable at
/// boot.  It only records the dirty session and emits the audit event
/// so an operator can see "已补摘要 N 个 session" in the
/// TelemetryDrawer; the actual re-roll happens on the next user turn
/// once messages are in scope.  This mirrors the openhanako reference
/// implementation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // serialized via memory_ticker_recovery audit event.
pub struct RecoveredSummary {
    /// Session identifier (sidecar filename stem).
    pub session_id: String,
    /// File modification time (when the session sidecar was last
    /// touched on disk).
    pub mtime: chrono::DateTime<chrono::Utc>,
    /// Timestamp of the last persisted summary, or `None` when the
    /// session never had one (first-time roll required).
    pub summary_at: Option<chrono::DateTime<chrono::Utc>>,
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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_captured",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: Some(category),
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_write_decision",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: Some(policy_decision_label(decision)),
            reason_code: Some(reason_code.label()),
            reason_message: Some(message),
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_persisted",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: Some(category),
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_recall_served",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: Some(query),
            recall_category: category,
            result_count: Some(result_count),
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_rejected",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: Some("deny"),
            reason_code: Some(reason_code.label()),
            reason_message: Some(message),
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_promoted` event.
    ///
    /// Emit a non-destructive `memory_promotion_candidate` event.
    ///
    /// Surfaced by [`crate::modules::memory::promotion::MemoryPromotionEngine`]
    /// during periodic background scans (post-turn hook) so the frontend can
    /// proactively show "you have N candidate promotions" without the user
    /// opening the Memory Browser.
    ///
    /// Reuses the `from_category` / `to_category` payload slots to carry the
    /// scope tier names (`"session"` / `"project"` / `"global"`) — kept this
    /// way to avoid widening `MemoryEventPayload` for one more tag pair.
    /// `reason_message` carries the human-readable rationale.
    #[allow(dead_code)] // consumer lands in commands::agent (prior-session change)
    pub fn memory_promotion_candidate(
        ctx: &AuditContext<'_>,
        key: &str,
        from_tier: &'static str,
        to_tier: &'static str,
        reason: &str,
    ) {
        tracing::info!(
            event = "memory_promotion_candidate",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            from_tier = from_tier,
            to_tier = to_tier,
            reason = reason,
        );
        emit_to_frontend(MemoryEventPayload {
            event: "memory_promotion_candidate",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: Some(reason),
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: Some(from_tier),
            to_category: Some(to_tier),
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

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
        emit_to_frontend(MemoryEventPayload {
            event: "memory_promoted",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: Some(from_category),
            to_category: Some(to_category),
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Reverse of [`Self::memory_promoted`].  Emit when a previously promoted
    /// entry is narrowed back down (e.g. user clicks "撤销" on a recent
    /// promotion).  Carries the same payload shape so the Telemetry Drawer
    /// can render promote / demote with a single timeline component.
    pub fn memory_demoted(ctx: &AuditContext<'_>, key: &str, from_tier: &str, to_tier: &str) {
        tracing::info!(
            event = "memory_demoted",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            from_tier = from_tier,
            to_tier = to_tier,
        );
        emit_to_frontend(MemoryEventPayload {
            event: "memory_demoted",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: None,
            from_category: Some(from_tier),
            to_category: Some(to_tier),
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_cleared` event after the user triggers a global
    /// "wipe all memory" from Settings.  Carries the number of removed
    /// entries in `result_count` so the Telemetry Drawer can show "N
    /// entries cleared" without a separate field.
    ///
    /// `ctx` is intentionally a thin / global context (no session /
    /// project) since this operation deliberately crosses every scope.
    /// Emit a `memory_pii_redacted` event after [`crate::modules::memory::security::ThreatScanner::scan_and_redact`]
    /// found PII in the candidate write and the writer substituted
    /// `[REDACTED:<kind>]` markers in place of every hit.
    ///
    /// `detected` is forwarded verbatim under the `extra.detected` key so
    /// the Telemetry Drawer can render per-kind counts and excerpts without
    /// widening the fixed `MemoryEventPayload` schema (v2 §0.5 Δ-3 + Δ-4).
    pub fn memory_pii_redacted(ctx: &AuditContext<'_>, key: &str, detected: &[DetectedPii]) {
        tracing::warn!(
            event = "memory_pii_redacted",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            memory_key = key,
            detected_count = detected.len(),
        );
        let extra = serde_json::json!({
            "detected_count": detected.len(),
            "detected": detected,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_pii_redacted",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: Some(key),
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(detected.len()),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_job_failed` event after a [`crate::modules::memory::job_runner::JobRunner`]
    /// invocation finished with `Err(_)` but is still under the retry budget.
    ///
    /// The variable metadata (`job` / `attempt` / `max_retries` / `error`)
    /// rides under `extra` per v2 §0.5 Δ-3 + Δ-4 so the fixed
    /// [`MemoryEventPayload`] schema does not need to grow per-event fields.
    ///
    /// `allow(dead_code)`: only consumed inside `JobRunner::run` (and via
    /// `pub(crate)` callers in 8A.7+); the bin target sees no direct
    /// caller until those slices land.
    #[allow(dead_code)]
    pub fn memory_job_failed(
        ctx: &AuditContext<'_>,
        job_kind: &str,
        attempt: u32,
        max_retries: u32,
        error: &str,
    ) {
        tracing::warn!(
            event = "memory_job_failed",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            job = job_kind,
            attempt = attempt,
            max_retries = max_retries,
            error = error,
        );
        let extra = serde_json::json!({
            "job": job_kind,
            "attempt": attempt,
            "max_retries": max_retries,
            "error": error,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_job_failed",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: Some(error),
            recall_query: None,
            recall_category: None,
            result_count: Some(attempt as usize),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_job_skipped` event after a [`crate::modules::memory::job_runner::JobRunner`]
    /// hit `attempt >= max_retries` and flipped the entry's status to
    /// `Skipped`, so subsequent calls for the same `(kind, target)` will
    /// short-circuit until [`crate::modules::memory::job_runner::JobRunner::reset`]
    /// is called.
    ///
    /// `total_failures` is the cumulative attempt count that triggered the
    /// skip (typically `max_retries`); `last_error` is the most recent
    /// error string captured before the skip.  Both ride under `extra`
    /// per v2 §0.5 Δ-3 + Δ-4.
    ///
    /// `allow(dead_code)`: see [`Self::memory_job_failed`].
    #[allow(dead_code)]
    pub fn memory_job_skipped(
        ctx: &AuditContext<'_>,
        job_kind: &str,
        total_failures: u32,
        last_error: &str,
    ) {
        tracing::error!(
            event = "memory_job_skipped",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            job = job_kind,
            total_failures = total_failures,
            last_error = last_error,
        );
        let extra = serde_json::json!({
            "job": job_kind,
            "total_failures": total_failures,
            "last_error": last_error,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_job_skipped",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: Some(last_error),
            recall_query: None,
            recall_category: None,
            result_count: Some(total_failures as usize),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_summary_rolled` event after
    /// [`crate::modules::memory::summary::rolling::RollingSummarizer::rolling_summary`]
    /// successfully writes a new [`crate::modules::memory::summary::SessionSummaryRecord`].
    ///
    /// Variable metadata (`session_id` / `turn_count` / `chars_before` /
    /// `chars_after` / `latency_ms`) rides under `extra` per
    /// v2 §0.5 Δ-3 + Δ-4 so the fixed [`MemoryEventPayload`] schema
    /// stays stable.  The TelemetryDrawer renders the event as a "第 N
    /// 轮 · 已更新摘要" timeline item with the char-delta and LLM
    /// latency next to it.
    ///
    /// `allow(dead_code)`: the first producer is the RollingSummarizer
    /// that lands together with this function in slice 8A.7; the bin
    /// target sees no consumer until `AppState` wires the hook in 8B.
    #[allow(dead_code)]
    pub fn memory_summary_rolled(
        ctx: &AuditContext<'_>,
        session_id: &str,
        turn_count: u32,
        chars_before: usize,
        chars_after: usize,
        latency_ms: u64,
    ) {
        tracing::info!(
            event = "memory_summary_rolled",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = session_id,
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            turn_count = turn_count,
            chars_before = chars_before,
            chars_after = chars_after,
            latency_ms = latency_ms,
        );
        let extra = serde_json::json!({
            "session_id": session_id,
            "turn_count": turn_count,
            "chars_before": chars_before,
            "chars_after": chars_after,
            "latency_ms": latency_ms,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_summary_rolled",
            trace_id: ctx.trace_id,
            session_id: Some(session_id),
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(turn_count as usize),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_pinned` event after [`crate::modules::memory::pinned::PinnedStore::add`]
    /// successfully writes a [`crate::modules::memory::pinned::PinnedItem`]
    /// (or returns an existing one via the dedup short-circuit).
    ///
    /// Variable metadata (`scope`, `content_excerpt`, `total_pins`) rides
    /// under `extra` per v2 §0.5 Δ-3 + Δ-4 so the fixed
    /// [`MemoryEventPayload`] schema stays stable.  The Telemetry Drawer
    /// renders the event as a "已固定 · {scope}" timeline item.
    ///
    /// `allow(dead_code)`: the first producer is the SqlitePinnedStore in
    /// 8A.9 itself; the bin target sees no caller until the pin_memory
    /// tool wires it in 8A.10.
    #[allow(dead_code)]
    pub fn memory_pinned(
        ctx: &AuditContext<'_>,
        scope: &'static str,
        content_excerpt: &str,
        total_pins: usize,
    ) {
        tracing::info!(
            event = "memory_pinned",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            pin_scope = scope,
            total_pins = total_pins,
        );
        let extra = serde_json::json!({
            "scope": scope,
            "content_excerpt": content_excerpt,
            "total_pins": total_pins,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_pinned",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: Some(scope),
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(total_pins),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_unpinned` event after [`crate::modules::memory::pinned::PinnedStore::delete`]
    /// removes one or more pinned items.
    ///
    /// `removed_count` is the number of rows actually deleted (idempotent
    /// callers may pass `0` when the id was already gone).  `keyword`
    /// carries the originating intent (the deleted id, or the search
    /// keyword that drove the bulk unpin from the future
    /// PinnedMemoryEditor UI in 8A.12).
    ///
    /// `allow(dead_code)`: see [`Self::memory_pinned`].
    #[allow(dead_code)]
    pub fn memory_unpinned(
        ctx: &AuditContext<'_>,
        scope: &'static str,
        removed_count: usize,
        keyword: &str,
    ) {
        tracing::info!(
            event = "memory_unpinned",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            pin_scope = scope,
            removed_count = removed_count,
            keyword = keyword,
        );
        let extra = serde_json::json!({
            "scope": scope,
            "removed_count": removed_count,
            "keyword": keyword,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_unpinned",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: Some(scope),
            policy_decision: None,
            reason_code: None,
            reason_message: Some(keyword),
            recall_query: None,
            recall_category: None,
            result_count: Some(removed_count),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_compiled` event after a `compile_today` /
    /// `compile_week` / `compile_longterm` / `compile_facts` cycle
    /// rewrote its `*.md` artifact (Phase 8B.3 / T-C3).
    ///
    /// `kind` is the section discriminator (`"today"` / `"week"` /
    /// `"longterm"` / `"facts"`); `result` is `"compiled"` for a real
    /// rewrite or `"skipped"` for a cache hit / quota exhaustion that
    /// the caller still wishes to surface.  `chars_in` / `chars_out` /
    /// `latency_ms` ride under `extra` per v2 §0.5 Δ-3 + Δ-4 so the
    /// fixed [`MemoryEventPayload`] schema does not need to grow per
    /// audit family.
    ///
    /// `allow(dead_code)`: first producers are the three compile_*
    /// functions in slice 8B.3; the bin target sees no direct caller
    /// until `MemoryCompiler` wires them in this slice's mod.rs.
    #[allow(dead_code)]
    pub fn memory_compiled(
        ctx: &AuditContext<'_>,
        kind: &'static str,
        result: &'static str,
        chars_in: usize,
        chars_out: usize,
        latency_ms: u64,
    ) {
        tracing::info!(
            event = "memory_compiled",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            kind = kind,
            result = result,
            chars_in = chars_in,
            chars_out = chars_out,
            latency_ms = latency_ms,
        );
        let extra = serde_json::json!({
            "kind": kind,
            "result": result,
            "chars_in": chars_in,
            "chars_out": chars_out,
            "latency_ms": latency_ms,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_compiled",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: Some(kind),
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(chars_out),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_assembled` event after
    /// [`crate::modules::memory::compiler::assemble::assemble`]
    /// concatenates the four `*.md` artefacts into `memory.md`
    /// (Phase 8B.4 / T-C4).
    ///
    /// `chars` is the final on-disk char count of `memory.md`;
    /// `sections` lists the four bilingual section titles in priority
    /// order (`facts → today → week → longterm`).  Both ride under
    /// `extra` per v2 §0.5 Δ-3 + Δ-4 so the fixed
    /// [`MemoryEventPayload`] schema does not need to grow per audit
    /// family.  The Telemetry Drawer renders this as a "memory.md ·
    /// {chars} 字" timeline item.
    ///
    /// `allow(dead_code)`: first producer is the `assemble` function
    /// landing in this slice; the bin target sees no direct caller
    /// until `MemoryCompiler::assemble` is wired by 8B.5+.
    #[allow(dead_code)]
    pub fn memory_assembled(ctx: &AuditContext<'_>, chars: usize, sections: &[&str]) {
        tracing::info!(
            event = "memory_assembled",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            chars = chars,
        );
        let extra = serde_json::json!({
            "chars": chars,
            "sections": sections,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_assembled",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(chars),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_ticker_recovery` event after
    /// [`crate::modules::memory::ticker::MemoryTicker::start`] completes
    /// its `recover_unsummarized` scan.  `extra.recovered` carries the
    /// per-session list so the TelemetryDrawer can render a single
    /// "已补摘要 N 个 session" timeline item with details on hover.
    ///
    /// Per v2 §0.5 Δ-3 (no `Arc<Self>` field) + Δ-4 (variable metadata
    /// via `extra`).
    ///
    /// `allow(dead_code)`: the only producer is the ticker startup hook
    /// (Phase 8B.9); the bin target sees no direct caller until the app
    /// boot path spawns the ticker.
    #[allow(dead_code)]
    pub fn memory_ticker_recovery(ctx: &AuditContext<'_>, recovered: &[RecoveredSummary]) {
        let count = recovered.len();
        tracing::info!(
            event = "memory_ticker_recovery",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            recovered_count = count,
        );
        let extra = serde_json::json!({
            "recovered_count": count,
            "recovered": recovered,
        });
        emit_to_frontend(MemoryEventPayload {
            event: "memory_ticker_recovery",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: None,
            recall_query: None,
            recall_category: None,
            result_count: Some(count),
            from_category: None,
            to_category: None,
            extra: Some(extra),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Emit a `memory_cleared` event after the user triggers a global
    /// "wipe all memory" from Settings.  Carries the number of removed
    /// entries in `result_count` so the Telemetry Drawer can show "N
    /// entries cleared" without a separate field.
    ///
    /// `ctx` is intentionally a thin / global context (no session /
    /// project) since this operation deliberately crosses every scope.
    pub fn memory_cleared(ctx: &AuditContext<'_>, removed: usize) {
        tracing::warn!(
            event = "memory_cleared",
            trace_id = ctx.trace_id.unwrap_or("-"),
            session_id = ctx.session_id.unwrap_or("-"),
            project_id = ctx.project_id.unwrap_or("-"),
            workdir = ctx.effective_workdir.unwrap_or("-"),
            removed = removed,
        );
        emit_to_frontend(MemoryEventPayload {
            event: "memory_cleared",
            trace_id: ctx.trace_id,
            session_id: ctx.session_id,
            project_id: ctx.project_id,
            effective_workdir: ctx.effective_workdir,
            memory_key: None,
            memory_category: None,
            policy_decision: None,
            reason_code: None,
            reason_message: Some("memory store wiped via Settings"),
            recall_query: None,
            recall_category: None,
            result_count: Some(removed),
            from_category: None,
            to_category: None,
            extra: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
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
