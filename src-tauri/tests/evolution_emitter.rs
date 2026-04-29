//! WU-001 — Integration tests for the evolution emitter.
//!
//! Skip the AppHandle path: constructing a real Tauri AppHandle from
//! a unit test is heavy and out of scope per the WU-001 contract.
//! We exercise the **failure-isolation** + **kill-switch** paths
//! that wire-up Packs depend on; the actual Tauri emit is exercised
//! in-app via the `App.tsx` listener wiring.

use serde::Serialize;

use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{
    emit_evolution_event, evolution_emit_disabled, EmitError, DISABLE_EMIT_ENV,
};

#[derive(Serialize)]
struct GoodPayload {
    check_name: String,
    state: String,
}

/// Payload type whose `Serialize` impl ALWAYS returns Err — exercises
/// the emitter's serialization-failure branch without depending on
/// any specific serde quirk (e.g. `serde_json` happily converts NaN
/// to `null` by default).
struct BadPayload;

impl serde::Serialize for BadPayload {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "test-only forced serialize error",
        ))
    }
}

#[test]
fn emit_does_not_panic_on_bad_payload() {
    let result = emit_evolution_event(
        None,
        RuntimeEventType::CompressionEvent,
        "metric",
        CorrelationIds::default(),
        &BadPayload,
        None,
    );
    match result {
        Err(EmitError::Serialize(_)) => {}
        Ok(_) => panic!("BadPayload must surface a Serialize error"),
        Err(other) => panic!("expected Serialize, got {other:?}"),
    }
}

#[test]
fn envelope_event_type_matches() {
    // Use the kill-switch so this test never depends on a tauri runtime.
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let env = emit_evolution_event(
        None,
        RuntimeEventType::SkillSedimented,
        "draft",
        CorrelationIds::default(),
        &GoodPayload {
            check_name: "auto-skill-1".into(),
            state: "ready".into(),
        },
        None,
    )
    .expect("emit must succeed when kill-switch is on");
    assert_eq!(env.event_type, RuntimeEventType::SkillSedimented);
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn env_disable_flag_skips_emit() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    assert!(evolution_emit_disabled());
    std::env::set_var(DISABLE_EMIT_ENV, "true");
    assert!(evolution_emit_disabled());
    std::env::set_var(DISABLE_EMIT_ENV, "TRUE");
    assert!(evolution_emit_disabled());
    std::env::set_var(DISABLE_EMIT_ENV, "0");
    assert!(!evolution_emit_disabled());
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}
