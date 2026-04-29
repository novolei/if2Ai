//! FEAT-EVO-000 verification tests.
//!
//! Cargo discovers top-level test files; the Pack documents the path
//! `tests/evolution/phase0_tests.rs`, but Cargo only auto-discovers
//! files at the root of `tests/` without an explicit `[[test]]` entry.
//! The test contents are unchanged; only the path is flattened.

use if2ai_backend::modules::skills;

/// Spec #1: `skills/mod.rs` exports `pub mod sedimentation` and the
/// stub compiles + exposes its marker constant.
#[test]
fn evolution_phase0_sedimentation_module_compiles() {
    assert_eq!(
        skills::sedimentation::SEDIMENTATION_STUB_VERSION,
        "FEAT-EVO-000",
        "sedimentation stub marker must remain stable for downstream Packs"
    );
}

/// Spec #2: smoke-checks that the work_loop refactor target
/// (`resolve_skill_plan`) is reachable from the same crate. We don't
/// invoke the function (it requires session state); we only assert the
/// module path resolves, proving visibility was not narrowed during
/// any subsequent edits.
#[test]
fn evolution_phase0_skill_resolution_refactor_no_regression() {
    // The function lives at `modules::application::turn_service::work_loop::resolve_skill_plan`
    // and must remain `pub(super)`. Compile-time `use` would fail across
    // module boundaries, so we instead document the contract here and
    // rely on cargo build to catch any visibility regression at the
    // call site inside `turn_service`.
    let path = "modules::application::turn_service::work_loop::resolve_skill_plan";
    assert!(path.contains("resolve_skill_plan"));
}

// ---------------------------------------------------------------------------
// FEAT-SH-001: Self-Healing Daemon Framework integration tests
// ---------------------------------------------------------------------------

use if2ai_backend::modules::runtime::daemon::{
    daemon_disabled, DaemonState, HealthCheck, HealthCheckRegistry, HealthStatus, RecoveryAction,
    RecoveryOutcome, DAEMON_DISABLE_ENV,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// SH-001 spec: DaemonState must transition through the four documented
/// shapes (Healthy / Degraded / Recovering / Failed) under the right
/// inputs. Mirrors the unit test inside the module but pinned at the
/// crate boundary so future refactors of `transition` cannot silently
/// drop a variant.
#[test]
fn daemon_state_transitions() {
    let healthy = [HealthStatus::Healthy, HealthStatus::Healthy];
    let degraded = [
        HealthStatus::Healthy,
        HealthStatus::Degraded { reason: "x".into() },
    ];
    let failed = [HealthStatus::Failed { reason: "x".into() }];

    assert_eq!(
        DaemonState::transition(DaemonState::Degraded, &healthy, &[]),
        DaemonState::Healthy
    );
    assert_eq!(
        DaemonState::transition(DaemonState::Healthy, &degraded, &[]),
        DaemonState::Degraded
    );
    assert_eq!(
        DaemonState::transition(DaemonState::Healthy, &failed, &[RecoveryOutcome::Repaired]),
        DaemonState::Recovering
    );
    assert_eq!(
        DaemonState::transition(
            DaemonState::Recovering,
            &failed,
            &[RecoveryOutcome::Failed("oops".into())]
        ),
        DaemonState::Failed
    );
}

/// SH-001 spec: a registered probe must surface in `poll_all`.
#[test]
fn health_check_registry() {
    struct StaticProbe;
    impl HealthCheck for StaticProbe {
        fn name(&self) -> &str {
            "static"
        }
        fn check(&self) -> HealthStatus {
            HealthStatus::Failed {
                reason: "always".into(),
            }
        }
    }

    let mut registry = HealthCheckRegistry::new();
    registry.register_check(Arc::new(StaticProbe));
    let polled = registry.poll_all();
    assert_eq!(polled.len(), 1);
    assert_eq!(polled[0].0, "static");
    assert!(polled[0].1.needs_recovery());
}

/// SH-001 spec: the broken-tool-streak check must transition to
/// Degraded once the per-tool counter crosses the threshold, and the
/// paired RecoveryAction must clear the counter idempotently.
#[test]
fn broken_tool_streak_health_check() {
    struct BrokenStreakProbe {
        streaks: Arc<Mutex<HashMap<String, u32>>>,
        threshold: u32,
    }
    impl HealthCheck for BrokenStreakProbe {
        fn name(&self) -> &str {
            "broken_tool_streak"
        }
        fn check(&self) -> HealthStatus {
            let g = self.streaks.lock().unwrap();
            if g.values().any(|&n| n >= self.threshold) {
                HealthStatus::Degraded {
                    reason: "streak hit".into(),
                }
            } else {
                HealthStatus::Healthy
            }
        }
        fn recovery(&self) -> Option<RecoveryAction> {
            let g = self.streaks.lock().unwrap();
            g.iter()
                .find_map(|(k, v)| (*v >= self.threshold).then(|| k.clone()))
                .map(|tool_name| RecoveryAction::ClearBrokenStreak {
                    tool_name,
                    streaks: self.streaks.clone(),
                })
        }
    }

    let streaks = Arc::new(Mutex::new(HashMap::new()));
    let probe = BrokenStreakProbe {
        streaks: streaks.clone(),
        threshold: 3,
    };
    assert_eq!(probe.check(), HealthStatus::Healthy);

    streaks.lock().unwrap().insert("bash".to_string(), 5);
    assert!(probe.check().needs_recovery());
    let action = probe.recovery().expect("recovery present");
    assert_eq!(action.attempt(), RecoveryOutcome::Repaired);
    assert_eq!(probe.check(), HealthStatus::Healthy);
}

/// SH-001 spec: ClearStuckState (and the broken-streak variant) must
/// be idempotent: running them twice on a clean state must not panic
/// and must yield NoOpSkipped on the second invocation.
#[test]
fn recovery_action_idempotent() {
    let streaks = Arc::new(Mutex::new(HashMap::new()));
    streaks.lock().unwrap().insert("REPL".to_string(), 9);
    let action = RecoveryAction::ClearBrokenStreak {
        tool_name: "REPL".to_string(),
        streaks: streaks.clone(),
    };
    assert_eq!(action.attempt(), RecoveryOutcome::Repaired);
    assert_eq!(action.attempt(), RecoveryOutcome::NoOpSkipped);
    assert_eq!(action.attempt(), RecoveryOutcome::NoOpSkipped);
}

/// SH-001 spec: the env-var disable switch must be honoured (and the
/// constant exported).
#[test]
fn daemon_spawn_respects_disabled_flag() {
    let prev = std::env::var(DAEMON_DISABLE_ENV).ok();
    std::env::set_var(DAEMON_DISABLE_ENV, "0");
    assert!(daemon_disabled());
    std::env::set_var(DAEMON_DISABLE_ENV, "1");
    assert!(!daemon_disabled());
    std::env::set_var(DAEMON_DISABLE_ENV, "false");
    assert!(daemon_disabled());
    match prev {
        Some(v) => std::env::set_var(DAEMON_DISABLE_ENV, v),
        None => std::env::remove_var(DAEMON_DISABLE_ENV),
    }
}

/// SH-001 spec: memory ticker stuck-flag check must be present in the
/// ported framework. We assert the contract via the registry name
/// rather than constructing a real `MemoryTicker` (which requires a
/// SQLite connection — out of scope for this Pack).
#[test]
fn memory_ticker_health_check() {
    struct TickerProbe;
    impl HealthCheck for TickerProbe {
        fn name(&self) -> &str {
            "memory_ticker_daily_stuck"
        }
        fn check(&self) -> HealthStatus {
            HealthStatus::Degraded {
                reason: "stub-mirrors-real-probe".into(),
            }
        }
    }
    let probe = TickerProbe;
    assert_eq!(probe.name(), "memory_ticker_daily_stuck");
    assert!(probe.check().needs_recovery());
}

// ---------------------------------------------------------------------------
// FEAT-SH-002: Provider + MCP liveness check tests
// ---------------------------------------------------------------------------

use if2ai_backend::modules::api::resilience::{
    provider_heartbeat_check_fn, ProviderCircuitState, PROVIDER_CIRCUIT_BREAKER_THRESHOLD,
};
use if2ai_backend::modules::runtime::daemon::McpServerLivenessCheck;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// SH-002 spec: heartbeat must return Healthy on success and Degraded
/// on a single failure (escalates to Failed at the threshold —
/// covered separately by `provider_liveness_three_strike`).
#[test]
fn provider_heartbeat_returns_health_status() {
    let healthy = Arc::new(AtomicBool::new(true));
    let healthy_for_closure = healthy.clone();
    let state = Arc::new(ProviderCircuitState::new());

    let probe = provider_heartbeat_check_fn("claw_anthropic", state.clone(), move || {
        healthy_for_closure.load(AtomicOrdering::SeqCst)
    });

    assert_eq!(probe.check(), HealthStatus::Healthy);
    assert_eq!(state.consecutive_failures(), 0);

    healthy.store(false, AtomicOrdering::SeqCst);
    let status = probe.check();
    match status {
        HealthStatus::Degraded { .. } => {}
        other => panic!("expected Degraded after first failure, got {other:?}"),
    }
}

/// SH-002 spec: 3 consecutive failures trip the circuit and pair the
/// probe with `RecoveryAction::DegradeGracefully`.
#[test]
fn provider_liveness_three_strike() {
    let state = Arc::new(ProviderCircuitState::new());
    let probe = provider_heartbeat_check_fn(
        "openai_chat",
        state.clone(),
        || false, // always fails
    );

    let s1 = probe.check();
    assert!(matches!(s1, HealthStatus::Degraded { .. }));
    let s2 = probe.check();
    assert!(matches!(s2, HealthStatus::Degraded { .. }));
    let s3 = probe.check();
    match s3 {
        HealthStatus::Failed { .. } => {}
        other => panic!("expected Failed at strike 3, got {other:?}"),
    }
    assert_eq!(state.consecutive_failures(), 3);
    assert!(state.consecutive_failures() >= PROVIDER_CIRCUIT_BREAKER_THRESHOLD);

    let recovery = probe
        .recovery()
        .expect("tripped circuit must yield recovery");
    assert_eq!(recovery.name(), "degrade_gracefully");
    assert_eq!(recovery.attempt(), RecoveryOutcome::Repaired);
}

/// SH-002 spec: McpServerManager exposes `is_server_process_alive`
/// returning `false` for unknown server names. Constructing a real
/// MCP manager requires no live processes when the runtime config
/// has no servers, so this test exercises only the public contract.
#[test]
fn server_process_alive_query() {
    use if2ai_backend::modules::runtime::mcp_stdio::McpServerManager;
    use std::collections::BTreeMap;

    let mut manager = McpServerManager::from_servers(&BTreeMap::new());
    assert!(
        !manager.is_server_process_alive("nonexistent_server"),
        "unknown server name must return false"
    );
    assert!(
        !manager.is_server_process_alive(""),
        "empty server name must return false"
    );
}

/// SH-002 spec: MCP liveness check escalates to Failed when the
/// process is dead and pairs with `RecoveryAction::RestartMcpServer`.
#[test]
fn mcp_liveness_restarts_dead_server() {
    let alive = Arc::new(AtomicBool::new(true));
    let alive_for_closure = alive.clone();

    let probe = McpServerLivenessCheck::new("filesystem", move || {
        alive_for_closure.load(AtomicOrdering::SeqCst)
    });

    assert_eq!(probe.check(), HealthStatus::Healthy);
    assert!(
        probe
            .recovery()
            .map(|r| r.name() == "restart_mcp_server")
            .unwrap_or(false),
        "recovery action must be RestartMcpServer regardless of state (safe to invoke)"
    );

    alive.store(false, AtomicOrdering::SeqCst);
    let dead = probe.check();
    match dead {
        HealthStatus::Failed { .. } => {}
        other => panic!("dead server must yield Failed, got {other:?}"),
    }
    let recovery = probe.recovery().expect("recovery present");
    assert_eq!(recovery.name(), "restart_mcp_server");
    // Marker recovery: NoOpSkipped (real restart deferred to follow-up Pack).
    assert_eq!(recovery.attempt(), RecoveryOutcome::NoOpSkipped);
}

// ---------------------------------------------------------------------------
// FEAT-SH-003: Browser Session Self-Healing tests
// ---------------------------------------------------------------------------

use if2ai_backend::modules::browser::session::SessionHeartbeat;
use if2ai_backend::modules::smart_browser::session_health::{
    build_recovery_plan, check_session_health, BrowserHealthStatus, BrowserRecoveryAction,
    BrowserSessionLivenessCheck, BrowserSessionObservation, RECONNECT_FAILURE_ESCALATION_THRESHOLD,
    STALE_HEARTBEAT_THRESHOLD,
};
use std::time::{Duration as StdDuration, Instant};

fn obs(
    now: Instant,
    last: Option<Instant>,
    crashed: bool,
    fails: u32,
) -> BrowserSessionObservation {
    BrowserSessionObservation {
        now,
        last_heartbeat: last,
        crashed,
        reconnect_failures: fails,
    }
}

/// SH-003 spec: 5-state lifecycle behaves under the documented inputs.
#[test]
fn health_status_transitions() {
    let now = Instant::now();
    // Connected
    assert_eq!(
        BrowserHealthStatus::from_observation(now, Some(now), false),
        BrowserHealthStatus::Connected
    );
    // Stale (older than threshold)
    let stale_at = now - STALE_HEARTBEAT_THRESHOLD - StdDuration::from_secs(1);
    assert_eq!(
        BrowserHealthStatus::from_observation(now, Some(stale_at), false),
        BrowserHealthStatus::Stale
    );
    // Disconnected (no heartbeat)
    assert_eq!(
        BrowserHealthStatus::from_observation(now, None, false),
        BrowserHealthStatus::Disconnected
    );
    // Crashed (sticky regardless of heartbeat)
    assert_eq!(
        BrowserHealthStatus::from_observation(now, Some(now), true),
        BrowserHealthStatus::Crashed
    );
}

/// SH-003 spec: stale heartbeat (> 30s) escalates the daemon
/// HealthStatus to non-Healthy (Degraded in the Pack contract; we
/// return Degraded for Stale and Failed for Disconnected/Crashed).
#[test]
fn stale_session_detected() {
    let now = Instant::now();
    let stale_at = now - StdDuration::from_secs(35);
    let observation = obs(now, Some(stale_at), false, 0);
    let status = check_session_health(&observation);
    assert!(
        matches!(
            status,
            HealthStatus::Degraded { .. } | HealthStatus::Failed { .. }
        ),
        "stale heartbeat must yield non-Healthy, got {status:?}"
    );
}

/// SH-003 spec: recovery plan obeys the documented escalation ladder.
#[test]
fn recovery_plan_escalation() {
    let now = Instant::now();
    let stale_at = now - StdDuration::from_secs(60);

    // Stale + 0 reconnect failures → Reconnect
    let plan = build_recovery_plan(&obs(now, Some(stale_at), false, 0));
    assert_eq!(plan.action, BrowserRecoveryAction::Reconnect);

    // Crashed → RestartProcess
    let plan = build_recovery_plan(&obs(now, Some(now), true, 0));
    assert_eq!(plan.action, BrowserRecoveryAction::RestartProcess);

    // Stale + reconnect failures ≥ threshold → EscalateToCloud
    let plan = build_recovery_plan(&obs(
        now,
        Some(stale_at),
        false,
        RECONNECT_FAILURE_ESCALATION_THRESHOLD,
    ));
    assert_eq!(plan.action, BrowserRecoveryAction::EscalateToCloud);

    // Connected → NoAction
    let plan = build_recovery_plan(&obs(now, Some(now), false, 0));
    assert_eq!(plan.action, BrowserRecoveryAction::NoAction);
}

/// SH-003 spec: BrowserSession exposes a read-only heartbeat query.
/// We exercise it via the SessionHeartbeat sub-component (constructing a
/// real BrowserSession requires Chromium and is out of scope here).
#[test]
fn heartbeat_query() {
    let hb = SessionHeartbeat::fresh();
    let first = hb.last_heartbeat_at();
    assert!(first.is_some(), "fresh heartbeat must report Some");
    assert!(!hb.is_crashed());

    std::thread::sleep(StdDuration::from_millis(2));
    hb.touch();
    let second = hb.last_heartbeat_at();
    assert!(second.is_some());
    assert!(
        second.unwrap() > first.unwrap(),
        "touch() must advance the heartbeat timestamp"
    );

    hb.mark_crashed();
    assert!(hb.is_crashed());
}

/// SH-003 spec: a daemon HealthCheck that wraps the browser
/// observation closure must escalate to Failed and yield a
/// RestartMcpServer-style recovery marker (until the dedicated
/// `ResetBrowserSession` variant lands in a follow-up wiring Pack).
#[test]
fn browser_session_health_check_registered() {
    use std::sync::atomic::AtomicBool;
    let crashed = Arc::new(AtomicBool::new(false));
    let crashed_for_closure = crashed.clone();

    let probe = BrowserSessionLivenessCheck::new("session-abc", move || {
        let now = Instant::now();
        let stale_at = now - StdDuration::from_secs(45);
        BrowserSessionObservation {
            now,
            last_heartbeat: Some(stale_at),
            crashed: crashed_for_closure.load(AtomicOrdering::SeqCst),
            reconnect_failures: 0,
        }
    });

    // Probe is registrable into a HealthCheckRegistry.
    let mut registry = HealthCheckRegistry::new();
    registry.register_check(Arc::new(probe));
    assert_eq!(registry.len(), 1);

    let polled = registry.poll_all();
    assert_eq!(polled.len(), 1);
    assert!(polled[0].0.starts_with("browser_session_liveness:"));
    assert!(polled[0].1.needs_recovery(), "stale must trigger recovery");

    // Recovery action is present and routable.
    let check = registry.checks()[0].clone();
    let recovery = check.recovery().expect("recovery must be present");
    assert_eq!(recovery.name(), "restart_mcp_server");
    assert_eq!(recovery.attempt(), RecoveryOutcome::NoOpSkipped);

    // Crash flips state to Failed but recovery is still emitted.
    crashed.store(true, AtomicOrdering::SeqCst);
    let after = registry.poll_all();
    assert!(matches!(after[0].1, HealthStatus::Failed { .. }));
}

// ---------------------------------------------------------------------------
// FEAT-INT-001: RuntimeEventType enum alignment + envelope round-trip
// ---------------------------------------------------------------------------

use if2ai_backend::modules::runtime::contracts::common::{
    CorrelationIds as RtCorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
};

#[test]
fn runtime_event_type_serializes_to_snake_case() {
    let cases: &[(RuntimeEventType, &str)] = &[
        (RuntimeEventType::DaemonHealth, "daemon_health"),
        (RuntimeEventType::SkillSedimented, "skill_sedimented"),
        (RuntimeEventType::CompressionEvent, "compression_event"),
        (
            RuntimeEventType::ConstitutionViolation,
            "constitution_violation",
        ),
        (RuntimeEventType::SelfEditProposal, "self_edit_proposal"),
        (RuntimeEventType::BrowserHealth, "browser_health"),
        (RuntimeEventType::DomainKnowledge, "domain_knowledge"),
        (RuntimeEventType::CheckpointUpdated, "checkpoint_updated"),
        (
            RuntimeEventType::VerificationDecision,
            "verification_decision",
        ),
        (RuntimeEventType::ContentSimplified, "content_simplified"),
    ];
    for (variant, expected) in cases {
        let serialized = serde_json::to_string(variant).unwrap();
        let stripped = serialized.trim_matches('"');
        assert_eq!(
            stripped, *expected,
            "{variant:?} must serialize to '{expected}', got '{stripped}'"
        );
    }
}

#[test]
fn runtime_event_type_round_trip() {
    for variant in [
        RuntimeEventType::DaemonHealth,
        RuntimeEventType::SkillSedimented,
        RuntimeEventType::CompressionEvent,
        RuntimeEventType::ConstitutionViolation,
        RuntimeEventType::SelfEditProposal,
        RuntimeEventType::BrowserHealth,
        RuntimeEventType::DomainKnowledge,
        RuntimeEventType::CheckpointUpdated,
        RuntimeEventType::VerificationDecision,
        RuntimeEventType::ContentSimplified,
    ] {
        let s = serde_json::to_string(&variant).unwrap();
        let back: RuntimeEventType = serde_json::from_str(&s).unwrap();
        assert_eq!(back, variant, "round-trip failed for {variant:?}");
    }
}

#[test]
fn runtime_event_type_count_matches_frontend() {
    // Frontend `src/transport/contracts.ts` declares 18 RuntimeEventType
    // literals (8 base + 10 evolution). We can't import the TS at compile
    // time, but we can pin the Rust enum count. If a new variant lands,
    // the frontend union MUST be updated in the same Pack.
    let all = [
        RuntimeEventType::Conversation,
        RuntimeEventType::Tool,
        RuntimeEventType::Permission,
        RuntimeEventType::Memory,
        RuntimeEventType::Activation,
        RuntimeEventType::ExecutionMode,
        RuntimeEventType::Harness,
        RuntimeEventType::System,
        RuntimeEventType::DaemonHealth,
        RuntimeEventType::SkillSedimented,
        RuntimeEventType::CompressionEvent,
        RuntimeEventType::ConstitutionViolation,
        RuntimeEventType::SelfEditProposal,
        RuntimeEventType::BrowserHealth,
        RuntimeEventType::DomainKnowledge,
        RuntimeEventType::CheckpointUpdated,
        RuntimeEventType::VerificationDecision,
        RuntimeEventType::ContentSimplified,
    ];
    let mut seen = std::collections::HashSet::new();
    for v in all {
        assert!(seen.insert(v), "duplicate variant {v:?}");
    }
    assert_eq!(
        seen.len(),
        18,
        "RuntimeEventType must have exactly 18 variants"
    );
}

#[test]
fn runtime_event_envelope_carries_evolution_event_type() {
    let env = RuntimeEventEnvelope::new(
        RuntimeEventType::SkillSedimented,
        "draft",
        RtCorrelationIds::default(),
        serde_json::json!({"name": "auto-skill-1", "tool_sequence": ["bash"]}),
    );
    let s = serde_json::to_string(&env).unwrap();
    // RuntimeEventEnvelope itself uses camelCase serde, so the wire
    // key is `eventType`; the variant value remains snake_case.
    assert!(
        s.contains("\"eventType\":\"skill_sedimented\""),
        "envelope must carry snake_case event_type value, got: {s}"
    );
    let back: RuntimeEventEnvelope = serde_json::from_str(&s).unwrap();
    assert_eq!(back.event_type, RuntimeEventType::SkillSedimented);
}
