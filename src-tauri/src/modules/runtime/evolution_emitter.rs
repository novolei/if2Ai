//! WU-001 — Agent Evolution emitter infrastructure.
//!
//! Single helper that the WU-002~008 wire-up Packs call to broadcast
//! one of the 10 evolution event families (FEAT-INT-001) onto two
//! transports:
//!
//! 1. **Frontend** — `app_handle.emit("runtime_event", envelope)`
//!    (the channel the `evolutionEventStore.applyEnvelope` listener
//!    subscribes to in `App.tsx`).
//! 2. **Durable log** — best-effort, opt-in via the caller passing a
//!    `RunEventLogger` handle. WU-001 keeps the logger optional so
//!    early-boot emits (with no run yet) don't fail.
//!
//! Failure isolation by Pack contract:
//! - Serialization failures return `Err` but never panic.
//! - `IF2AI_DISABLE_EVOLUTION_EMIT=1` short-circuits to `Ok(())`
//!   without touching any transport (production kill-switch).
//! - Transport failures (Tauri emit error / log write failure) are
//!   downgraded to `tracing::warn!` — the caller's main flow keeps
//!   going.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::modules::runtime::contracts::common::{
    CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
};
use crate::modules::runtime::event_log::RunEventLogger;

/// Tauri channel name the frontend `App.tsx` listener subscribes to.
/// MUST match `RUNTIME_EVENT_CHANNEL` on the TS side.
pub const RUNTIME_EVENT_CHANNEL: &str = "runtime_event";

/// Env var that hard-disables every evolution emit (for diagnosing
/// "is the new pipeline causing this?" without redeploying).
pub const DISABLE_EMIT_ENV: &str = "IF2AI_DISABLE_EVOLUTION_EMIT";

/// `true` when the kill-switch is set to a truthy value.
#[must_use]
pub fn evolution_emit_disabled() -> bool {
    std::env::var(DISABLE_EMIT_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Build + emit one evolution envelope.
///
/// Returns `Ok(())` on success **and** when the kill-switch is set;
/// returns `Err(EmitError::Serialize)` when the payload cannot be
/// turned into JSON; transport failures are best-effort logged and
/// returned as `Err(EmitError::Transport)` so callers can decide
/// whether to retry.
pub fn emit_evolution_event<T: Serialize>(
    handle: Option<&AppHandle>,
    event_type: RuntimeEventType,
    family: impl Into<String>,
    correlation: CorrelationIds,
    payload: &T,
    logger: Option<&RunEventLogger>,
) -> Result<RuntimeEventEnvelope, EmitError> {
    if evolution_emit_disabled() {
        // Build the envelope so the caller still has something to
        // pipe into harness traces if it wants — but skip every
        // transport. Use a dummy serialization to surface bad payloads
        // even when emit is disabled (so dev catches the issue).
        let _ = serde_json::to_value(payload).map_err(|e| EmitError::Serialize(e.to_string()))?;
        let envelope =
            RuntimeEventEnvelope::new(event_type, family, correlation, serde_json::Value::Null);
        return Ok(envelope);
    }

    let payload_json =
        serde_json::to_value(payload).map_err(|e| EmitError::Serialize(e.to_string()))?;
    let envelope = RuntimeEventEnvelope::new(event_type, family, correlation, payload_json);

    if let Some(handle) = handle {
        if let Err(err) = handle.emit(RUNTIME_EVENT_CHANNEL, &envelope) {
            tracing::warn!(
                error = %err,
                "[evolution_emitter] tauri emit failed; downgrading to warn"
            );
            return Err(EmitError::Transport(err.to_string()));
        }
    }

    if let Some(logger) = logger {
        let kind_label = serde_json::to_value(envelope.event_type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "evolution_unknown".to_string());
        logger.append_sync(
            format!("{kind_label}:{}", envelope.payload_family.0),
            &envelope.payload,
        );
    }

    Ok(envelope)
}

/// Outcome variants for [`emit_evolution_event`].
///
/// Both variants are recoverable — callers SHOULD log them but keep
/// running. The infrastructure must never bring down the agent loop
/// just because an event broadcast missed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    Serialize(String),
    Transport(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::Serialize(msg) => write!(f, "evolution emit: serialize failed: {msg}"),
            EmitError::Transport(msg) => write!(f, "evolution emit: transport failed: {msg}"),
        }
    }
}

impl std::error::Error for EmitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Healthy {
        check_name: String,
        state: String,
    }

    #[test]
    fn envelope_event_type_matches() {
        let _ = std::env::var(DISABLE_EMIT_ENV).is_ok();
        std::env::set_var(DISABLE_EMIT_ENV, "1");
        let env = emit_evolution_event(
            None,
            RuntimeEventType::DaemonHealth,
            "transition",
            CorrelationIds::default(),
            &Healthy {
                check_name: "memory_ticker_daily_stuck".into(),
                state: "degraded".into(),
            },
            None,
        )
        .expect("emit must succeed even with kill-switch ON");
        assert_eq!(env.event_type, RuntimeEventType::DaemonHealth);
        std::env::remove_var(DISABLE_EMIT_ENV);
    }

    #[test]
    fn env_disable_flag_skips_emit() {
        std::env::set_var(DISABLE_EMIT_ENV, "1");
        assert!(evolution_emit_disabled());
        std::env::set_var(DISABLE_EMIT_ENV, "true");
        assert!(evolution_emit_disabled());
        std::env::set_var(DISABLE_EMIT_ENV, "0");
        assert!(!evolution_emit_disabled());
        std::env::remove_var(DISABLE_EMIT_ENV);
        assert!(!evolution_emit_disabled());
    }
}
