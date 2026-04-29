use std::sync::Arc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    window::Color,
    App, AppHandle, Manager, TitleBarStyle,
};

use crate::modules::browser::BrowserRegistry;
use crate::modules::memory::MemoryTicker;

const TRAY_SHOW_ID: &str = "show";
const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostTrayAction {
    Show,
    Quit,
    Ignore,
}

/// Clean up all related processes when the app exits.
pub fn cleanup_processes() {
    let _ = std::process::Command::new("pkill")
        .args(["-f", "if2ai-backend"])
        .spawn();
}

/// Apply the native desktop-host lifecycle wiring:
/// browser registry bootstrap, memory audit hook, tray, and the
/// main-window close policy.
pub fn setup_desktop_host(app: &App) -> tauri::Result<()> {
    register_browser_app_handle(app);
    register_memory_audit_emitter(app);
    start_memory_ticker(app);
    let ticker = app.state::<Arc<MemoryTicker>>().inner().clone();
    let browser_registry = app.state::<Arc<BrowserRegistry>>().inner().clone();
    spawn_self_healing_daemon_with_browser_probe(ticker, browser_registry);
    register_evolution_probes_for_app(app);
    install_persistent_knowledge_store();
    spawn_mcp_health_refresher();
    spawn_user_mcp_health_refresher();
    resolve_bundled_skills(app);
    install_system_tray(app)?;
    install_main_window_policy(app);
    start_activation_lifecycle(app);
    Ok(())
}

/// Truth-loop iter-4 (WU-002 真化) — replace the legacy
/// `spawn_self_repair_watchdog(ticker)` shim with the new
/// `spawn_self_healing_daemon_with_extras` so the live
/// `BrowserRegistryProbe` runs **inside the same daemon registry**
/// the SH-001 baseline polls. Prior wire-up registered an
/// always-Healthy `StubBrowserProbe` into a parallel registry that
/// nobody spawned — a textbook `landed-stub`.
///
/// Truth-loop iter-6 (WU-002 provider probe) — also adds
/// `ProviderApiKeyProbe` to the extras vec so the daemon reports
/// `Degraded` whenever all known LLM provider credentials are absent.
fn spawn_self_healing_daemon_with_browser_probe(
    ticker: Arc<MemoryTicker>,
    browser_registry: Arc<BrowserRegistry>,
) {
    use crate::modules::runtime::daemon::{
        spawn_self_healing_daemon_with_extras, HealthCheck, HealthStatus,
    };
    use crate::modules::smart_browser::session_health::BrowserHealthStatus;

    /// Aggregate browser probe — surveys every active session in the
    /// registry and returns the worst observed health. No active
    /// session = `Healthy` (no browser, no problem).
    struct BrowserRegistryProbe {
        registry: Arc<BrowserRegistry>,
    }

    impl HealthCheck for BrowserRegistryProbe {
        fn name(&self) -> &str {
            "browser_registry_liveness"
        }
        fn check(&self) -> HealthStatus {
            let snapshots = self.registry.heartbeat_snapshots();
            if snapshots.is_empty() {
                return HealthStatus::Healthy;
            }
            let now = std::time::Instant::now();
            // Severity rank: Crashed > Disconnected > Stale >
            // Recovering > Connected. Map each snapshot through the
            // pure session_health state machine and pick the worst.
            let mut worst = HealthStatus::Healthy;
            for (_session_id, last_heartbeat, crashed) in snapshots {
                let status = BrowserHealthStatus::from_observation(now, last_heartbeat, crashed)
                    .to_health_status();
                worst = pick_worse(worst, status);
            }
            worst
        }
    }

    fn pick_worse(a: HealthStatus, b: HealthStatus) -> HealthStatus {
        fn rank(s: &HealthStatus) -> u8 {
            match s {
                HealthStatus::Healthy => 0,
                HealthStatus::Degraded { .. } => 1,
                HealthStatus::Failed { .. } => 2,
            }
        }
        if rank(&b) > rank(&a) {
            b
        } else {
            a
        }
    }

    let browser_probe: Arc<dyn HealthCheck> = Arc::new(BrowserRegistryProbe {
        registry: browser_registry,
    });
    // WU-002 iter-6: provider API-key probe (env-var-only, cheap, sync).
    let provider_api_key_probe = crate::modules::runtime::daemon::make_provider_api_key_probe();
    // Module C iter-7: provider circuit probe — reads the
    // process-wide `ProviderCircuitState` streak that
    // `provider/resilience.rs::StreamCircuitState` now mirrors on
    // every chat-provider success / failure. No active ping; the
    // daemon promotes `Healthy → Degraded → Failed` as live stream
    // outcomes accumulate, and emits `DegradeGracefully` when the
    // breaker threshold trips.
    let provider_circuit_probe = crate::modules::api::resilience::make_provider_circuit_probe(
        "chat_provider",
        crate::modules::api::resilience::global_provider_circuit(),
    );
    // Module C iter-8: MCP probes — config-presence (sync, env-only)
    // + browser-use stdio liveness (reads the snapshot the refresher
    // task below populates every 30s). Together they give the daemon
    // its first real production view of MCP health without
    // introducing a third McpServerManager singleton.
    let mcp_config_presence_probe =
        crate::modules::runtime::mcp_health::make_mcp_config_presence_probe();
    let mcp_browser_use_probe =
        crate::modules::runtime::mcp_health::make_browser_use_liveness_probe(
            crate::modules::runtime::mcp_health::global_mcp_health(),
        );
    spawn_self_healing_daemon_with_extras(
        ticker,
        vec![
            browser_probe,
            provider_api_key_probe,
            provider_circuit_probe,
            mcp_config_presence_probe,
            mcp_browser_use_probe,
        ],
    );
}

/// DW-004 (truth-loop iter-7) — install a `FileBackedKnowledgeStore`
/// rooted at `<if2ai_data_root>/domain-knowledge.ndjson` as the
/// process-wide [`KnowledgeStore`] singleton. Must run before any
/// `global_knowledge_store()` caller — i.e. before the first
/// streaming turn. Failure to open the file is **non-fatal**: we log
/// at `warn` and let `global_knowledge_store()` fall back to the
/// default in-memory `MockKnowledgeStore`, which keeps the rest of
/// the app running while DK contributions are forfeited for this
/// session only.
fn install_persistent_knowledge_store() {
    use crate::modules::skills::domain_knowledge::{
        install_global_knowledge_store, FileBackedKnowledgeStore, KnowledgeStore,
    };

    let dir = crate::modules::config::store::if2ai_data_root();
    match FileBackedKnowledgeStore::open_or_create(&dir) {
        Ok(store) => {
            let path = store.path().to_path_buf();
            let arc: std::sync::Arc<dyn KnowledgeStore> = std::sync::Arc::new(store);
            match install_global_knowledge_store(arc) {
                Ok(()) => tracing::info!(
                    "[setup] DW-004 installed FileBackedKnowledgeStore at {}",
                    path.display()
                ),
                Err(_) => tracing::warn!(
                    "[setup] DW-004 GLOBAL_KNOWLEDGE_STORE already initialised; \
                     persistent store NOT applied — first writer wins"
                ),
            }
        }
        Err(e) => tracing::warn!(
            "[setup] DW-004 falling back to in-memory KnowledgeStore — \
             could not open {}/domain-knowledge.ndjson: {e}",
            dir.display()
        ),
    }
}

/// Truth-loop iter-8 (Module C MCP probe) — spawn the background
/// task that refreshes [`crate::modules::runtime::mcp_health::global_mcp_health`]
/// from the live `BROWSER_USE_MCP_MANAGER`. The daemon's MCP probe
/// reads this snapshot synchronously, so without this refresher the
/// probe would forever report `Healthy` (its "innocent until proven
/// dead" default).
///
/// - Interval: 30s.  Snapshot stale window is 90s
///   ([`crate::modules::runtime::mcp_health::DEFAULT_STALE_AFTER`]),
///   so a single missed tick does NOT escalate to `Degraded`.
/// - Lock acquisition is bounded by a 100ms tokio timeout: if a
///   browser action is mid-call, we skip this tick rather than block
///   either side.
/// - Kill switch: `IF2AI_DISABLE_MCP_HEALTH=1`.
/// - Failure isolation: any panic / error inside the loop is logged
///   at `warn` and the next tick proceeds (mirrors
///   [`spawn_self_edit_scanner_interval`]).
fn spawn_mcp_health_refresher() {
    use crate::modules::runtime::mcp_health::global_mcp_health;
    use crate::modules::smart_browser::browser_use_mcp::BROWSER_USE_MCP_SERVER_NAME;
    use crate::modules::smart_browser::runtime::browser_use_mcp_manager;
    use std::time::Duration;

    if std::env::var("IF2AI_DISABLE_MCP_HEALTH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        tracing::info!("[setup] iter-8 MCP health refresher disabled (IF2AI_DISABLE_MCP_HEALTH=1)");
        return;
    }

    const INTERVAL: Duration = Duration::from_secs(30);
    const LOCK_TIMEOUT: Duration = Duration::from_millis(100);
    let snapshot = global_mcp_health();

    let task = async move {
        // First tick fires immediately so the daemon sees a real
        // sample within the first probe poll, then settles into
        // 30s cadence.
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let manager = browser_use_mcp_manager();
            let lock_result = tokio::time::timeout(LOCK_TIMEOUT, manager.lock()).await;
            let mut guard = match lock_result {
                Ok(g) => g,
                Err(_) => {
                    tracing::debug!(
                        "[mcp_health] skip tick: browser_use MCP manager held > {}ms",
                        LOCK_TIMEOUT.as_millis()
                    );
                    continue;
                }
            };
            // Today this is exactly one server; written as a loop so
            // adding more persistent stdio servers later requires
            // only widening this slice. `#[allow]` documents the
            // deliberate single-element-loop shape.
            #[allow(clippy::single_element_loop)]
            for name in [BROWSER_USE_MCP_SERVER_NAME] {
                let alive = guard.is_server_process_alive(name);
                snapshot.record(name, alive);
            }
        }
    };

    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(task);
    } else {
        tracing::warn!(
            "[mcp_health] no current Tokio runtime at refresher spawn site; skipping background task"
        );
    }
}

/// Truth-loop iter-9 — analogue of [`spawn_mcp_health_refresher`]
/// for the user-MCP singleton (`runtime::mcp_workbench`). Polls the
/// dynamic server-name set learned from the manager itself, so a
/// settings reload (which swaps the inner `McpServerManager`) is
/// picked up automatically on the next tick.
///
/// Same kill switch (`IF2AI_DISABLE_MCP_HEALTH=1`) and same 100ms
/// lock timeout as the browser-use refresher; if a workbench
/// command is mid-call, we skip this tick.
fn spawn_user_mcp_health_refresher() {
    use crate::modules::runtime::mcp_health::global_mcp_health;
    use crate::modules::runtime::mcp_workbench::global_user_mcp_manager;
    use std::time::Duration;

    if std::env::var("IF2AI_DISABLE_MCP_HEALTH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return;
    }

    const INTERVAL: Duration = Duration::from_secs(30);
    const LOCK_TIMEOUT: Duration = Duration::from_millis(100);
    let snapshot = global_mcp_health();

    let task = async move {
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let arc = global_user_mcp_manager();
            let lock_result = tokio::time::timeout(LOCK_TIMEOUT, arc.lock()).await;
            let mut guard = match lock_result {
                Ok(g) => g,
                Err(_) => {
                    tracing::debug!(
                        "[mcp_health] skip user-mcp tick: workbench manager held > {}ms",
                        LOCK_TIMEOUT.as_millis()
                    );
                    continue;
                }
            };
            let names = guard.server_names();
            for name in names {
                let alive = guard.is_server_process_alive(&name);
                snapshot.record(&name, alive);
            }
        }
    };

    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(task);
    } else {
        tracing::warn!(
            "[mcp_health] no current Tokio runtime at user-mcp refresher spawn site; skipping background task"
        );
    }
}

fn start_activation_lifecycle(app: &App) {
    crate::modules::application::activation::lifecycle_manager::spawn_activation_lifecycle(
        app.handle().clone(),
    );
}

/// Truth-loop iter-4 (2026-04-30) — slimmed-down companion to
/// `spawn_self_healing_daemon_with_browser_probe`.
///
/// The browser probe (formerly `StubBrowserProbe` registered into an
/// orphan `HealthCheckRegistry`) is now the real `BrowserRegistryProbe`
/// living **inside** the SH-001 daemon registry. This function therefore
/// no longer registers any probes — its remaining job is to spawn the
/// DW-001 self-edit scanner background task.
///
/// Provider + MCP heartbeat probes (still `landed-stub` per gap report
/// 2026-04-30 §9.3) will be added back here as `extras` arguments to
/// `spawn_self_healing_daemon_with_extras` in a future iteration.
fn register_evolution_probes_for_app(app: &App) {
    let handle = app.handle().clone();

    // DW-001 — spawn the WU-005 self-edit scanner as a background
    // tokio interval task. Honors `IF2AI_DISABLE_SELF_EDIT=1` and
    // is fully failure-isolated: any panic / error inside the loop
    // is logged at warn and the next tick proceeds.
    spawn_self_edit_scanner_interval(handle);
}

/// DW-001 truth-loop iter-7 — replaces three landed-stubs:
///
/// 1. `MockUtilityLlm::empty()` → `ChatProviderUtilityLlm` (real LLM)
/// 2. `ConstEmbedder`           → `FastEmbedProvider` (real embeddings; fail-safe fallback)
/// 3. `&[], &[]`                → disk-loaded recent `HarnessRunReport`s
///
/// `PromotionStage` is persisted across ticks via `Arc<Mutex<ScannerStageState>>`.
fn spawn_self_edit_scanner_interval(app_handle: tauri::AppHandle) {
    use std::sync::{Arc, Mutex};

    use crate::modules::harness::HarnessReportStore;
    use crate::modules::learning::self_edit::scanner::{
        run_scanner_once, self_edit_scanner_disabled, DEFAULT_SCANNER_INTERVAL,
    };
    use crate::modules::learning::self_edit::scanner_state::{
        load_scanner_tick_input, ScannerStageState,
    };
    use crate::modules::learning::self_edit::PromotionStage;
    use crate::modules::memory::embedding::FastEmbedProvider;
    use crate::modules::memory::llm::ChatProviderUtilityLlm;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    use crate::modules::runtime::evolution_emitter::emit_evolution_event;
    use crate::modules::skills::sedimentation::Embedder;

    if self_edit_scanner_disabled() {
        tracing::info!("[setup] DW-001 self-edit scanner disabled (IF2AI_DISABLE_SELF_EDIT=1)");
        return;
    }

    struct ConstFallbackEmbedder;
    impl Embedder for ConstFallbackEmbedder {
        /// Returns empty vector → cosine_similarity returns 0.0 → no proposals merged.
        /// This is the conservative no-dedup behaviour when FastEmbedProvider is unavailable.
        fn embed(&self, _text: &str) -> Vec<f32> {
            Vec::new()
        }
    }

    let embedder: Arc<dyn Embedder + Send + Sync> = match FastEmbedProvider::new() {
        Ok(p) => {
            tracing::info!("[setup] DW-001 using FastEmbedProvider for dedup");
            Arc::new(p)
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                "[setup] DW-001 FastEmbedProvider init failed; falling back to ConstEmbedder"
            );
            Arc::new(ConstFallbackEmbedder)
        }
    };

    let stage_state = Arc::new(Mutex::new(ScannerStageState::default()));
    let store = HarnessReportStore::with_default_root();
    let llm: std::sync::Arc<dyn crate::modules::memory::UtilityLlm> = std::sync::Arc::new(
        ChatProviderUtilityLlm::new(crate::modules::config::store::if2ai_data_root()),
    );

    let task = async move {
        let mut ticker = tokio::time::interval(DEFAULT_SCANNER_INTERVAL);
        loop {
            ticker.tick().await;

            let tick = load_scanner_tick_input(&store, 100).await;
            let reports_refs: Vec<&crate::modules::harness::run_report::HarnessRunReport> =
                tick.reports.iter().collect();

            let current_stage = stage_state
                .lock()
                .map(|g| g.stage)
                .unwrap_or(PromotionStage::Shadow);

            let outcome_fut = std::panic::AssertUnwindSafe(run_scanner_once(
                &reports_refs,
                &[],
                llm.clone(),
                embedder.as_ref(),
                current_stage,
                tick.failure_rate,
                tick.sample_size,
            ));
            let outcome = match futures::FutureExt::catch_unwind(outcome_fut).await {
                Ok(o) => o,
                Err(_) => {
                    tracing::warn!("[setup] DW-001 self-edit scanner tick panicked");
                    continue;
                }
            };

            if let Ok(mut g) = stage_state.lock() {
                g.apply_transition(outcome.transition);
            }

            for proposal in &outcome.proposals {
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::SelfEditProposal,
                    "draft",
                    CorrelationIds::default(),
                    &serde_json::json!({
                        "id": proposal.id,
                        "kind": proposal.kind.as_str(),
                        "target": proposal.target,
                        "justification": proposal.justification,
                    }),
                    None,
                );
            }
            for (proposal, verdict) in &outcome.verdicts {
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::SelfEditProposal,
                    "verdict",
                    CorrelationIds::default(),
                    &serde_json::json!({
                        "id": proposal.id,
                        "verdict": format!("{:?}", verdict.verdict),
                        "failedGates": verdict.failed_gates,
                    }),
                    None,
                );
            }

            if let Some(reason) = &outcome.skipped_reason {
                tracing::debug!("[DW-001] scanner tick skipped: {reason}");
            } else {
                tracing::info!(
                    proposals = outcome.proposals.len(),
                    verdicts = outcome.verdicts.len(),
                    stage = ?outcome.transition,
                    reports_loaded = tick.sample_size,
                    failure_rate = tick.failure_rate,
                    "[DW-001] scanner tick complete"
                );
            }
        }
    };

    tauri::async_runtime::spawn(task);
}

fn register_browser_app_handle(app: &App) {
    let registry = app.state::<Arc<BrowserRegistry>>().inner().clone();
    registry.set_app_handle(app.handle().clone());
}

fn register_memory_audit_emitter(app: &App) {
    crate::modules::memory::audit::register_app_handle(app.handle().clone());
}

fn start_memory_ticker(app: &App) {
    let ticker_for_start = app.state::<Arc<MemoryTicker>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let scope = crate::modules::memory::scope::MemoryExecutionScope::global();
        ticker_for_start.start(scope).await;
    });
}

fn resolve_bundled_skills(app: &App) {
    let bundled_skills_dir = ["resources/bundled-skills", "bundled-skills"]
        .iter()
        .filter_map(|candidate| {
            app.path()
                .resolve(candidate, tauri::path::BaseDirectory::Resource)
                .ok()
        })
        .find(|path| path.is_dir());
    if let Some(path) = bundled_skills_dir {
        crate::modules::tools::builtin::skill::set_bundled_skills_dir(path.clone());
        tracing::info!(
            "Resolved bundled skills dir from Tauri resources: {}",
            path.display()
        );
    } else {
        tracing::warn!(
            "Failed to resolve bundled skills from Tauri resources; fallback only preserves discovery via workdir-relative paths"
        );
    }
}

fn install_system_tray(app: &App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "Show If2Ai", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("If2Ai - AI Agent Desktop")
        .on_menu_event(|app: &AppHandle, event: tauri::menu::MenuEvent| {
            match resolve_tray_action(event.id.as_ref()) {
                HostTrayAction::Show => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                HostTrayAction::Quit => {
                    // iter-9 — best-effort graceful teardown of any
                    // user-MCP stdio children spawned through the
                    // process-wide singleton. 2s budget so a stuck
                    // child cannot hold up the Quit path; orphan
                    // children would otherwise be reaped by the OS
                    // when `app.exit` ends the parent.
                    let _ = tauri::async_runtime::block_on(async {
                        tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            crate::modules::runtime::mcp_workbench::shutdown_user_mcp_manager(),
                        )
                        .await
                    });
                    cleanup_processes();
                    app.exit(0);
                }
                HostTrayAction::Ignore => {}
            }
        })
        .build(app)?;

    Ok(())
}

fn install_main_window_policy(app: &App) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
        let _ = window.set_background_color(Some(Color(0xf6, 0xf7, 0xf8, 0xff)));
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_clone.hide();
            }
        });
    }
}

fn resolve_tray_action(id: &str) -> HostTrayAction {
    match id {
        TRAY_SHOW_ID => HostTrayAction::Show,
        TRAY_QUIT_ID => HostTrayAction::Quit,
        _ => HostTrayAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_tray_action, HostTrayAction};

    #[test]
    fn tray_action_resolution_only_accepts_native_host_ids() {
        assert_eq!(resolve_tray_action("show"), HostTrayAction::Show);
        assert_eq!(resolve_tray_action("quit"), HostTrayAction::Quit);
        assert_eq!(resolve_tray_action("memory_recall"), HostTrayAction::Ignore);
    }

    #[test]
    fn scanner_stage_default_is_shadow() {
        use crate::modules::learning::self_edit::scanner_state::ScannerStageState;
        use crate::modules::learning::self_edit::PromotionStage;
        assert_eq!(ScannerStageState::default().stage, PromotionStage::Shadow);
    }
}
