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
    let provider_probe = crate::modules::runtime::daemon::make_provider_api_key_probe();
    spawn_self_healing_daemon_with_extras(ticker, vec![browser_probe, provider_probe]);
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

/// DW-001 — Periodic self-edit scanner driver.
///
/// Calls `run_scanner_once` every 60s with a placeholder embedder +
/// `MockUtilityLlm::empty()` until a deeper-wiring Pack plumbs the
/// real provider/embedder handles. Even with placeholders the
/// emit pipeline is exercised on every tick (proving the full chain
/// is healthy in production).
fn spawn_self_edit_scanner_interval(app_handle: tauri::AppHandle) {
    use crate::modules::learning::self_edit::scanner::{
        run_scanner_once, self_edit_scanner_disabled, DEFAULT_SCANNER_INTERVAL,
    };
    use crate::modules::learning::self_edit::PromotionStage;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    use crate::modules::runtime::evolution_emitter::emit_evolution_event;
    use crate::modules::skills::sedimentation::Embedder;

    if self_edit_scanner_disabled() {
        tracing::info!("[setup] DW-001 self-edit scanner disabled (IF2AI_DISABLE_SELF_EDIT=1)");
        return;
    }

    struct ConstEmbedder;
    impl Embedder for ConstEmbedder {
        fn embed(&self, _text: &str) -> Vec<f32> {
            vec![1.0, 0.0, 0.0]
        }
    }

    let task = async move {
        let mut ticker = tokio::time::interval(DEFAULT_SCANNER_INTERVAL);
        loop {
            ticker.tick().await;
            let llm: std::sync::Arc<dyn crate::modules::memory::UtilityLlm> =
                std::sync::Arc::new(crate::modules::memory::MockUtilityLlm::empty());
            let outcome_fut = std::panic::AssertUnwindSafe(run_scanner_once(
                &[],
                &[],
                llm,
                &ConstEmbedder,
                PromotionStage::Shadow,
                0.0,
                0,
            ));
            let outcome = match futures::FutureExt::catch_unwind(outcome_fut).await {
                Ok(o) => o,
                Err(_) => {
                    tracing::warn!("[setup] DW-001 self-edit scanner tick panicked");
                    continue;
                }
            };
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
                    RuntimeEventType::VerificationDecision,
                    "scanner_pass",
                    CorrelationIds::default(),
                    &serde_json::json!({
                        "proposalId": proposal.id,
                        "verdict": match verdict.verdict {
                            crate::modules::learning::self_edit::Verdict::Pass => "pass",
                            crate::modules::learning::self_edit::Verdict::Fail => "fail",
                        },
                        "failedGates": verdict.failed_gates,
                    }),
                    None,
                );
            }
        }
    };

    match tokio::runtime::Handle::try_current() {
        Ok(rt) => {
            rt.spawn(task);
        }
        Err(_) => {
            tracing::debug!(
                "[setup] DW-001 no current tokio runtime; falling back to dedicated thread"
            );
            std::thread::Builder::new()
                .name("if2ai-self-edit-scanner".into())
                .spawn(move || {
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("[setup] DW-001 fallback runtime build failed: {e}");
                            return;
                        }
                    };
                    rt.block_on(task);
                })
                .ok();
        }
    }
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
}
