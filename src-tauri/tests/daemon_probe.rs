//! WU-002 — Daemon probe registration tests.
//!
//! Exercises the four spec invariants:
//!   1. The 3-probe extras bundle has correct length.
//!   2. A probe returning `Failed` does NOT panic the registry poll
//!      (failure isolation).
//!   3. Emitting a `DaemonHealth` envelope from a non-Healthy poll
//!      result succeeds via the WU-001 emitter (kill-switch path).
//!   4. The `IF2AI_DISABLE_DAEMON_PROBES` env flag returns an empty
//!      probe set so `setup.rs` registers nothing extra.

use std::sync::Arc;

use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::daemon::{
    evolution_probe_set, register_extra_probes, HealthCheck, HealthCheckRegistry, HealthStatus,
    DISABLE_EVOLUTION_PROBES_ENV,
};
use if2ai_backend::modules::runtime::evolution_emitter::{emit_evolution_event, DISABLE_EMIT_ENV};

struct StubProbe {
    label: &'static str,
    fixed: HealthStatus,
}

impl HealthCheck for StubProbe {
    fn name(&self) -> &str {
        self.label
    }
    fn check(&self) -> HealthStatus {
        self.fixed.clone()
    }
}

fn three_probes() -> (
    Vec<Arc<dyn HealthCheck>>,
    Vec<Arc<dyn HealthCheck>>,
    Vec<Arc<dyn HealthCheck>>,
) {
    (
        vec![Arc::new(StubProbe {
            label: "provider_heartbeat:claw",
            fixed: HealthStatus::Healthy,
        })],
        vec![Arc::new(StubProbe {
            label: "mcp_liveness:filesystem",
            fixed: HealthStatus::Healthy,
        })],
        vec![Arc::new(StubProbe {
            label: "browser_session_liveness:s1",
            fixed: HealthStatus::Healthy,
        })],
    )
}

#[test]
fn default_registry_has_three_probes() {
    let prev = std::env::var(DISABLE_EVOLUTION_PROBES_ENV).ok();
    std::env::remove_var(DISABLE_EVOLUTION_PROBES_ENV);
    let (provider, mcp, browser) = three_probes();
    let probes = evolution_probe_set(provider, mcp, browser);
    assert_eq!(probes.len(), 3);
    let mut registry = HealthCheckRegistry::new();
    register_extra_probes(&mut registry, probes);
    assert_eq!(registry.len(), 3);
    if let Some(v) = prev {
        std::env::set_var(DISABLE_EVOLUTION_PROBES_ENV, v);
    }
}

#[test]
fn probe_error_degrades_gracefully() {
    // A probe returning `Failed` must NOT panic the poll loop.
    let prev = std::env::var(DISABLE_EVOLUTION_PROBES_ENV).ok();
    std::env::remove_var(DISABLE_EVOLUTION_PROBES_ENV);
    let probes: Vec<Arc<dyn HealthCheck>> = vec![Arc::new(StubProbe {
        label: "broken_provider",
        fixed: HealthStatus::Failed {
            reason: "simulated outage".into(),
        },
    })];
    let mut registry = HealthCheckRegistry::new();
    register_extra_probes(&mut registry, probes);
    let polled = registry.poll_all();
    assert_eq!(polled.len(), 1);
    assert!(matches!(polled[0].1, HealthStatus::Failed { .. }));
    if let Some(v) = prev {
        std::env::set_var(DISABLE_EVOLUTION_PROBES_ENV, v);
    }
}

#[test]
fn unhealthy_probe_emits_daemon_health_event() {
    // Use the WU-001 kill-switch so the test doesn't need an AppHandle.
    let prev_emit = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let payload = serde_json::json!({
        "checkName": "broken_provider",
        "state": "failed",
    });
    let envelope = emit_evolution_event(
        None,
        RuntimeEventType::DaemonHealth,
        "transition",
        CorrelationIds::default(),
        &payload,
        None,
    )
    .expect("WU-001 emit must succeed under kill-switch");
    assert_eq!(envelope.event_type, RuntimeEventType::DaemonHealth);
    match prev_emit {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn env_flag_disables_probe_registration() {
    let prev = std::env::var(DISABLE_EVOLUTION_PROBES_ENV).ok();
    std::env::set_var(DISABLE_EVOLUTION_PROBES_ENV, "1");
    let (provider, mcp, browser) = three_probes();
    let probes = evolution_probe_set(provider, mcp, browser);
    assert!(probes.is_empty(), "kill-switch must yield zero probes");
    std::env::set_var(DISABLE_EVOLUTION_PROBES_ENV, "true");
    let (p2, m2, b2) = three_probes();
    assert!(evolution_probe_set(p2, m2, b2).is_empty());
    match prev {
        Some(v) => std::env::set_var(DISABLE_EVOLUTION_PROBES_ENV, v),
        None => std::env::remove_var(DISABLE_EVOLUTION_PROBES_ENV),
    }
}
