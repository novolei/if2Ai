//! Canonical helper for emitting any runtime event onto the
//! `runtime_event` Tauri channel with run-log mirroring.
//!
//! Thin wrapper over `evolution_emitter::emit_evolution_event` so
//! non-evolution emit sites (chat-runtime: agent-token / permission /
//! memory) can adopt the canonical pipeline without leaking the
//! "evolution" name. See ARCHITECTURE.md §3.4 / §6.1.

use serde::Serialize;
use tauri::AppHandle;

use crate::modules::runtime::contracts::common::{
    CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
};
use crate::modules::runtime::evolution_emitter::{emit_evolution_event, EmitError};
use crate::modules::runtime::event_log::RunEventLogger;

/// Emit one canonical runtime envelope and (best-effort) append it to
/// the run log via `append_sync_from_envelope`.
pub fn dispatch<T: Serialize>(
    handle: Option<&AppHandle>,
    event_type: RuntimeEventType,
    family: impl Into<String>,
    correlation: CorrelationIds,
    payload: &T,
    logger: Option<&RunEventLogger>,
) -> Result<RuntimeEventEnvelope, EmitError> {
    emit_evolution_event(handle, event_type, family, correlation, payload, logger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};

    #[derive(serde::Serialize)]
    struct ChatPayload<'a> {
        event_type: &'a str,
        text: &'a str,
    }

    #[test]
    fn dispatch_returns_envelope_with_event_type_and_family() {
        std::env::set_var(
            crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
            "1",
        );
        let env = dispatch(
            None,
            RuntimeEventType::Conversation,
            "text_delta",
            CorrelationIds {
                run_id: Some("run-1".into()),
                stream_id: Some("s1".into()),
                ..CorrelationIds::default()
            },
            &ChatPayload { event_type: "text_delta", text: "hi" },
            None,
        )
        .expect("dispatch ok with kill-switch");
        assert_eq!(env.event_type, RuntimeEventType::Conversation);
        assert_eq!(env.payload_family.0, "text_delta");
        assert_eq!(env.correlation.run_id.as_deref(), Some("run-1"));
        std::env::remove_var(
            crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
        );
    }
}
