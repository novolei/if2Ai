#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::{Path, PathBuf};
use std::process::Command;

use commands::AppState;
use commands::{
    // Onboarding & Configuration Platform (Phase 6G)
    activation_complete,
    activation_get_status,
    activation_start,
    activation_test_message,
    activation_validate,
    browser_viewer_go_back,
    browser_viewer_go_forward,
    browser_viewer_reload,
    channel_configure,
    channel_list,
    channel_list_configured,
    channel_test,
    // Browser control commands (Phase 7B + 7C profile management + 7C.3 takeover)
    clear_browser_profile,
    close_browser_session,
    close_settings_window,
    config_load,
    config_reset_onboarding,
    config_save,
    config_validate,
    create_permanent_worktree,
    create_project,
    create_session,
    delete_project,
    delete_session,
    delete_tts_profile,
    embedded_model_download,
    embedded_model_progress,
    ensure_default_workdir,
    execute_slash_command,
    execute_tool,
    export_trajectories,
    fetch_skills_market_audits,
    focus_main_window_and_prefill_prompt,
    // Harness Control IPC (Phase 6E)
    get_all_session_telemetry,
    get_browser_action_log,
    get_browser_sessions,
    get_browser_settings,
    get_chrome_status,
    get_harness_status,
    get_memory_config,
    get_model_config,
    get_project,
    get_session,
    get_session_telemetry,
    get_tool_definitions,
    get_tts_settings,
    get_web_search_config,
    hub_audit,
    hub_browse,
    hub_check,
    hub_inspect,
    hub_install,
    hub_publish,
    hub_search,
    hub_snapshot_export,
    hub_snapshot_import,
    hub_tap_add,
    hub_tap_list,
    hub_tap_remove,
    hub_uninstall,
    hub_update,
    list_agents,
    list_browser_profiles,
    list_directory_preview,
    list_project_sessions,
    list_projects,
    list_sessions,
    list_skills,
    list_slash_commands,
    list_tools,
    list_toolsets,
    list_tts_profiles,
    memory_clear_all,
    memory_compile_now,
    memory_compiled_clear,
    memory_compiled_read,
    memory_delete,
    memory_demote,
    memory_export,
    memory_promote,
    memory_promotion_candidates,
    memory_purge,
    memory_recall,
    memory_session_set_enabled,
    memory_summaries_list,
    model_get_active,
    model_get_role_config,
    model_list_available,
    model_select,
    model_set_active,
    model_set_role_config,
    model_test,
    navigate_viewer_window,
    onboarding_complete,
    onboarding_get_state,
    onboarding_next_step,
    onboarding_prev_step,
    open_browser_viewer_window,
    open_directory_path,
    open_project_in_finder,
    open_settings_window,
    parse_slash_command,
    pick_folder_dialog,
    pinned_add,
    pinned_delete,
    pinned_get,
    pinned_reorder,
    provider_configure,
    provider_configure_with_models,
    provider_get_all_configured_models,
    provider_get_config,
    provider_get_configured_models,
    provider_list,
    provider_list_configured,
    provider_list_models,
    provider_test,
    read_file_preview,
    release_browser_takeover,
    remove_web_search_provider,
    rename_project,
    rename_session,
    reorder_web_search_providers,
    request_browser_status,
    request_browser_takeover,
    request_intelligence_classify,
    resolve_skill_slash,
    respond_permission,
    run_agent_turn,
    save_tts_profile,
    security_confirm,
    set_browser_settings,
    set_default_tts_profile,
    set_memory_config,
    set_model_config,
    set_session_pinned,
    set_tts_settings,
    start_agent_stream,
    start_harness_recording,
    stop_agent_stream,
    stop_harness_recording,
    // TTS commands (Phase TTS-5)
    stt_download_openflow_model,
    stt_download_whisper_model,
    stt_get_settings,
    stt_model_status,
    stt_save_settings,
    stt_transcribe,
    suggest_slash_commands,
    system_check_run,
    tts_cached_voice_preview,
    tts_delete_user_voice,
    tts_demo_audio,
    tts_health,
    tts_list_voice_assets,
    tts_list_voices,
    tts_model_download_start,
    tts_model_download_status,
    // TTS model download (Phase TTS-UX)
    tts_model_status,
    tts_preview_voice,
    tts_rename_user_voice,
    tts_split_text,
    tts_start_warmup,
    tts_stream_close,
    tts_stream_result,
    tts_stream_start,
    tts_stream_status,
    tts_synthesize,
    tts_upload_user_voice,
    tts_voice_audio,
    tts_warm_voice_preview,
    tts_warmup_status,
    upsert_web_search_provider,
    validate_web_search_key,
    write_file_contents,
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    window::Color,
    Manager, TitleBarStyle,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Clean up all related processes when the app exits.
fn cleanup_processes() {
    let _ = Command::new("pkill").args(["-f", "if2ai-backend"]).spawn();
}

/// Create the memory provider, preferring VectorMemoryProvider but falling
/// back to SQLite with a 30-second timeout guard.
///
/// Priority: Hybrid (HRR + Vector) > Vector (FastEmbed + LanceDB) > SQLite > InMemory
///
/// `scanner` (Phase 8A) is plumbed through to whichever provider wins so
/// every scope-aware write goes through `ThreatScanner::scan_and_redact`
/// before reaching disk (v2 §0.5 Δ-2 defence-in-depth).
fn create_memory_provider(
    scanner: std::sync::Arc<modules::memory::security::ThreatScanner>,
) -> modules::memory::SharedMemoryProvider {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create tokio runtime for memory init: {e}, falling back to SQLite"
            );
            return create_sqlite_provider(scanner);
        }
    };

    // Check if HRR algebraic reasoning is enabled via environment variable.
    // Default: false — HRR is an experimental feature.
    let hrr_enabled = std::env::var("IF2AI_HRR_ENABLED")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if hrr_enabled {
        match runtime.block_on(create_hybrid_provider()) {
            Ok(provider) => {
                tracing::info!("[memory] HybridMemoryProvider (HRR + Vector) initialized");
                return provider;
            }
            Err(e) => {
                tracing::warn!("[memory] HybridMemoryProvider failed: {e}, falling back to Vector");
            }
        }
    }

    // Attempt VectorMemoryProvider with 30s timeout (FastEmbed model load can be slow)
    let vector_result = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let memory_root = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".if2ai")
                .join("memory");
            let db_path = memory_root.join("vector_db");
            // Dual-write to SQLite is what backs every scope-aware operation
            // (`store_scoped`, `recall_scoped`, `promote_scope`,
            // `demote_scope`, `apply_importance_decay`).  Without it the
            // VectorMemoryProvider falls back to no-op trait defaults that
            // strip scope metadata, which silently breaks the three-tier
            // session/project/global model.
            let sqlite_path = Some(memory_root.join("memory.db"));

            let config = modules::memory::VectorProviderConfig {
                db_path,
                vector_search_enabled: true,
                sqlite_path,
            };

            modules::memory::VectorMemoryProvider::new(config).await
        })
        .await
    });

    match vector_result {
        Ok(Ok(provider)) => {
            tracing::info!("[memory] VectorMemoryProvider initialized successfully");
            let provider = provider.with_scanner(scanner);
            std::sync::Arc::new(provider) as modules::memory::SharedMemoryProvider
        }
        Ok(Err(e)) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider initialization failed: {e}, falling back to SQLite"
            );
            create_sqlite_provider(scanner)
        }
        Err(_) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider timed out after 30s, falling back to SQLite"
            );
            create_sqlite_provider(scanner)
        }
    }
}

/// Open the shared [`modules::memory::JobRunner`] backed by `<memory_root>/jobs.db`.
///
/// Tries the persistent path first.  If sqlite open fails (corrupt file,
/// permission error), falls back to an ephemeral pid-scoped tempdir so
/// the app still boots; failure counters simply won't survive a restart
/// in that degraded mode.  Both paths come from the same `JobRunner::open`
/// entry point, so the runner's behaviour itself is identical.
fn open_job_runner_with_fallback(memory_root: &Path) -> modules::memory::JobRunner {
    let primary = memory_root.join("jobs.db");
    match modules::memory::JobRunner::open(&primary, 3, 3) {
        Ok(runner) => runner,
        Err(e) => {
            tracing::error!(
                "[init] JobRunner failed to open {primary:?}: {e}; falling back to ephemeral jobs.db (failure counts will not survive restart)"
            );
            let tmp =
                std::env::temp_dir().join(format!("if2ai-jobs-fallback-{}.db", std::process::id()));
            match modules::memory::JobRunner::open(&tmp, 3, 3) {
                Ok(runner) => runner,
                Err(e2) => {
                    // Ephemeral tempdir open failure is unrecoverable — the
                    // app cannot honour any background-job contract.  Log
                    // and abort early with a clear message rather than
                    // booting into a half-broken state.
                    tracing::error!(
                        "[init] Ephemeral JobRunner open at {tmp:?} also failed: {e2}; aborting startup"
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Create a HybridMemoryProvider (HRR + LanceDB) for algebraic reasoning.
async fn create_hybrid_provider() -> Result<modules::memory::SharedMemoryProvider, String> {
    use crate::modules::memory::hrr::integration::HybridConfig;

    let hrr_config = HybridConfig {
        hrr_enabled: true,
        hrr_capacity: 0,
    };

    let provider = modules::memory::hrr::integration::HybridMemoryProvider::new(hrr_config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(std::sync::Arc::new(provider) as modules::memory::SharedMemoryProvider)
}

/// Create a SQLite-backed memory provider as fallback.
fn create_sqlite_provider(
    scanner: std::sync::Arc<modules::memory::security::ThreatScanner>,
) -> modules::memory::SharedMemoryProvider {
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("memory")
        .join("memory.db");
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match modules::memory::SqliteMemoryProvider::new(db_path) {
        Ok(p) => {
            std::sync::Arc::new(p.with_scanner(scanner)) as modules::memory::SharedMemoryProvider
        }
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create SqliteMemoryProvider: {e}, falling back to in-memory"
            );
            #[allow(deprecated)]
            let fallback = modules::memory::InMemoryMemoryProvider::new();
            std::sync::Arc::new(fallback) as modules::memory::SharedMemoryProvider
        }
    }
}

fn main() {
    // Initialize directories
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let if2ai_dir = PathBuf::from(&home).join(".if2ai");
    let log_dir = if2ai_dir.join("log");

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Failed to create log directory: {}", e);
    }

    // Initialize file logging for backend
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "backend.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the program - store it in a static
    std::mem::forget(_guard);

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("If2Ai backend starting, log directory: {:?}", log_dir);
    // ⭐ Phase 8B.11 fix-debug banner — if you do NOT see this in your
    // terminal after restarting `tauri dev`, the old binary is still
    // running (Vite hot-reload only swaps frontend; Rust changes need
    // a full process restart).
    tracing::info!(
        "⭐⭐⭐ MEMORY TICKER FIX BUILD a89eeec / 8B.11 — chat turns should fire on_turn_complete"
    );

    // Surface memory feature flags at startup so operators can confirm which
    // recall / policy mode the binary actually picked up from settings.json.
    // ConfigLoader is per-cwd, so we use the process cwd here purely for
    // logging — runtime callers re-load with their own scope.
    //
    // Phase 8A.4 — also install the loaded config as the process-global
    // handle returned by `runtime::config::current()` so
    // `runtime::logical_day::get_today` and `runtime::locale::is_zh` pick
    // up user `timezone` / `language` overrides without an app restart.
    {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let cfg = modules::runtime::config::ConfigLoader::default_for(&cwd)
            .load()
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "[memory] failed to load runtime config for feature flags: {e}; using defaults"
                );
                modules::runtime::config::RuntimeConfig::empty()
            });
        let mem = cfg.memory();
        tracing::info!(
            control_plane_v1_enabled = mem.control_plane_v1_enabled(),
            recall_mode = mem.recall_mode().as_str(),
            policy_enforce_mode = mem.policy_enforce_mode().as_str(),
            language = cfg.language(),
            "[memory] feature flags loaded"
        );
        modules::runtime::config::set_current(cfg);
    }

    // Set up cleanup hooks.
    //
    // We chain on top of the default hook so the original panic message,
    // location, and (with RUST_BACKTRACE=1) the stack are still surfaced
    // to stderr / the log file before we run any cleanup.  Replacing it
    // with `Box::new(|_| cleanup_processes())` previously swallowed the
    // panic output entirely, leaving operators with only the cryptic
    // "thread caused non-unwinding panic. aborting." line that comes
    // from `pkill -f if2ai-backend` killing this very process during
    // unwind.  We also no longer pkill ourselves on panic — abort will
    // tear the process down on its own and pkill matches the live PID.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        tracing::error!(panic = %info, "[panic] backend panicked");
    }));

    // Initialize directories
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let if2ai_dir = PathBuf::from(&home).join(".if2ai");

    let sessions_dir = if let Ok(path) = std::env::var("IF2AI_SESSIONS_DIR") {
        PathBuf::from(path)
    } else {
        if2ai_dir.join("sessions")
    };

    let projects_dir = if let Ok(path) = std::env::var("IF2AI_PROJECTS_DIR") {
        PathBuf::from(path)
    } else {
        if2ai_dir.join("projects")
    };

    let session_manager = modules::session::SessionManager::new(sessions_dir, projects_dir.clone());
    let default_tool_context =
        modules::tools::ToolContext::default_for_workdir(std::path::PathBuf::from("."));
    let tool_registry = modules::tools::ToolRegistry::new(std::sync::Arc::new(
        std::sync::Mutex::new(default_tool_context),
    ));
    // Phase 8A §0.5 Δ-2 — single shared ThreatScanner Arc threaded through
    // every memory write path (providers + future pinned/summary stores) so
    // regex compilation cost is paid once and audit emission is uniform.
    let threat_scanner =
        std::sync::Arc::new(modules::memory::security::ThreatScanner::with_builtin_patterns());

    // Phase 8A §Sprint 1 / T-A2 — single shared JobRunner backed by
    // `<data_local>/.if2ai/memory/jobs.db` (separate from `memory.db` to
    // avoid sqlite mutex contention between the recall hot path and
    // background bookkeeping; v2 §0.5 Δ-6 + 8A.2 review checklist #2).
    // `max_retries=3` matches v2 §0.5 Δ-9 `compiler.max_retries`,
    // `max_concurrent=3` matches `compiler.max_concurrent_llm`.
    let memory_root = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("memory");
    let job_runner = std::sync::Arc::new(open_job_runner_with_fallback(&memory_root));

    // Phase 8A.5 — UtilityLlm shim (v2 §0.5 Δ-1).  ProviderManager is
    // not currently bootstrapped in `main.rs` (provider construction
    // lives behind the per-turn `runtime::conversation` path), so the
    // production shim is a `MockUtilityLlm` placeholder that returns
    // the empty string.  This is wired up correctly across `AppState`
    // so subsequent slices (8A.7+) only need to swap the construction
    // here once `ProviderManager` initialisation is centralised.
    // See the slice 8A.5 commit body for the deferral rationale.
    let utility_llm: std::sync::Arc<dyn modules::memory::UtilityLlm> =
        std::sync::Arc::new(modules::memory::MockUtilityLlm::empty());
    tracing::warn!(
        "[init] UtilityLlm bound to MockUtilityLlm placeholder; real provider wiring deferred to slice 8A.7+"
    );

    // Phase 8A.5 — SessionSummaryStore backed by the shared
    // `<memory_root>/memory.db` (same SQLite file as MemoryProvider —
    // they own disjoint tables, so contention is bounded by the per-
    // operation `spawn_blocking` lock).  JSON sidecars land at
    // `<memory_root>/summaries/<session_id>.json` (v2 §0.5 Δ-6).
    // On open failure we fall back to `NullSessionSummaryStore` so the
    // app still boots — summaries simply do not persist.
    let summary_db_path = memory_root.join("memory.db");
    let summary_store: std::sync::Arc<dyn modules::memory::SessionSummaryStore> =
        match modules::memory::SqliteSessionSummaryStore::open(
            &summary_db_path,
            memory_root.clone(),
        ) {
            Ok(store) => {
                tracing::info!(
                    "[init] SessionSummaryStore initialised at {:?}",
                    summary_db_path
                );
                std::sync::Arc::new(store)
            }
            Err(e) => {
                tracing::error!(
                    "[init] SessionSummaryStore failed to open {summary_db_path:?}: {e}; using NullSessionSummaryStore (summaries will not persist)"
                );
                std::sync::Arc::new(modules::memory::NullSessionSummaryStore::new())
            }
        };

    // Phase 8A.7 + 8A.8 — RollingSummarizer wires SessionSummaryStore +
    // UtilityLlm + JobRunner + ThreatScanner.  Held on AppState so the
    // future Phase 8B ticker (TurnHook::on_turn_complete) can invoke
    // `rolling_summary` without re-threading these collaborators.
    let rolling_summarizer = std::sync::Arc::new(modules::memory::summary::RollingSummarizer::new(
        summary_store.clone(),
        utility_llm.clone(),
        job_runner.clone(),
        Some(threat_scanner.clone()),
    ));

    // Phase 8B.1 — `MemoryCompiler` skeleton (Sprint 2 / T-C1).  Real
    // `compile_*` implementations land in 8B.2 (fingerprint cache) +
    // 8B.3 (today / week / longterm) + 8B.4 (facts / assemble).  The
    // process-global `runtime::config::current()` was installed above
    // (line ~372) so the user's `~/.if2ai/memory_config.json::compiler`
    // overrides flow through here at boot.
    let memory_compiler = std::sync::Arc::new(modules::memory::MemoryCompiler::new(
        summary_store.clone(),
        utility_llm.clone(),
        job_runner.clone(),
        modules::runtime::config::current()
            .memory()
            .compiler()
            .clone(),
    ));

    // Phase 8B.6 — MemoryTicker (Sprint 2 / T-D1).  Constructed once
    // here and held on `AppState` so the future
    // `ConversationRuntime::with_turn_hook(state.memory_ticker.clone())`
    // wiring (handled by `commands/agent.rs` once the prior-session
    // diff is un-stashed) can reach it without re-threading the four
    // collaborators.  The `TurnHook` impl is a no-op stub from 8B.6;
    // real `notify_turn` / `notify_session_end` lands in 8B.7.
    // TODO(8B.x): read `TickerConfig` from `MemoryConfigOverrides`
    // instead of hard-coded defaults once the Settings UI lands.
    let memory_ticker = std::sync::Arc::new(modules::memory::MemoryTicker::new(
        rolling_summarizer.clone(),
        memory_compiler.clone(),
        summary_store.clone(),
        modules::memory::TickerConfig::default(),
    ));

    // Phase 8A.9 — PinnedStore backed by the shared
    // `<memory_root>/memory.db` (same SQLite file as the summary store
    // and MemoryProvider; the `pinned_items` table is disjoint from
    // both).  The markdown sidecar lives at `<memory_root>/pinned.md`
    // — single-root simplification per v2 §0.5 Δ-6 (multi-scope-root
    // separation lands in 8B).  On open failure we fall back to
    // `NullPinnedStore` so the app still boots; pinned writes will then
    // surface a `MemoryError::Generic` to callers.
    let pinned_store: std::sync::Arc<dyn modules::memory::PinnedStore> =
        match modules::memory::SqlitePinnedStore::open(
            &summary_db_path,
            memory_root.clone(),
            Some(threat_scanner.clone()),
        ) {
            Ok(store) => {
                tracing::info!(
                    "[init] PinnedStore initialised at {:?} (sidecar {:?}/pinned.md)",
                    summary_db_path,
                    memory_root
                );
                std::sync::Arc::new(store)
            }
            Err(e) => {
                tracing::error!(
                    "[init] PinnedStore failed to open {summary_db_path:?}: {e}; using NullPinnedStore (pin writes will error)"
                );
                std::sync::Arc::new(modules::memory::NullPinnedStore::new())
            }
        };

    // Phase 8A.12 (T-F5) — first-run sentinel: ensure `<memory_root>/pinned.md`
    // exists so `build_memory_injection` does not log a missing-file warning
    // before the 8B compile pipeline ever writes to it.  `SqlitePinnedStore`
    // already creates the parent directory; this only adds an empty file.
    {
        let pinned_md = memory_root.join("pinned.md");
        if !pinned_md.exists() {
            if let Err(e) = std::fs::write(&pinned_md, "") {
                tracing::warn!(
                    path = %pinned_md.display(),
                    error = %e,
                    "[init] failed to touch pinned.md sentinel; build_memory_injection will skip compiled section"
                );
            }
        }
    }

    let memory_provider = create_memory_provider(threat_scanner.clone());
    let scheduler_provider = modules::scheduler::default_scheduler();
    // Phase 7C, slice 7C.1 — resolve the browser profile mode from
    // env / ~/.if2ai/browser.toml so cookies survive across restarts.
    // Default is `PerSessionPersistent`; tests use `Ephemeral` via
    // `BrowserRegistry::for_test`.
    let browser_profile_mode = modules::browser::BrowserProfileMode::from_env_or_config(&if2ai_dir);
    let browser_registry = modules::browser::BrowserRegistry::new(
        if2ai_dir.join("browser-cold-state.json"),
        browser_profile_mode,
        if2ai_dir.clone(),
    );
    modules::tools::register_builtin_tools(
        &tool_registry,
        memory_provider.clone(),
        scheduler_provider,
        browser_registry.clone(),
        pinned_store.clone(),
    );
    let project_manager = modules::projects::ProjectManager::new(projects_dir);

    // Initialize onboarding flow (Phase 6G)
    // OnboardingFlow is a stateless driver; state is loaded lazily via Tauri commands.
    let onboarding_flow = modules::onboarding::flow::OnboardingFlow;

    // Initialize context budget. M3: prefer ~/.if2ai/budget.yaml when present;
    // any read / parse / validation error falls back to the historical default
    // (4000 tokens, 10/20/30/40%).
    let context_budget = {
        let path = dirs::home_dir()
            .map(|h| h.join(".if2ai").join("budget.yaml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".if2ai/budget.yaml"));
        modules::runtime::budget::BudgetConfig::load_or_default(&path)
    };

    // ── Phase 6BW: Persistent memory/learning infrastructure ──

    // TrajectoryManager: persist sessions as ShareGPT JSONL for future RL.
    let trajectories_dir = if2ai_dir.join("trajectories");
    let trajectory_manager =
        match modules::learning::trajectory::TrajectoryManager::new(trajectories_dir.clone()) {
            Ok(tm) => {
                tracing::info!(
                    "[init] TrajectoryManager initialised at {:?}",
                    trajectories_dir
                );
                Some(std::sync::Arc::new(tm))
            }
            Err(e) => {
                tracing::warn!(
                "[init] TrajectoryManager failed to initialise: {e}; trajectory recording disabled"
            );
                None
            }
        };

    // LearningModule: self-model + reflection engine.
    // `LearningModule::new` is async; spin up a temp single-threaded runtime.
    let learning_module: Option<
        std::sync::Arc<tokio::sync::Mutex<modules::learning::LearningModule>>,
    > = {
        let mp = memory_provider.clone();
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => match rt.block_on(modules::learning::LearningModule::new(mp)) {
                Ok(lm) => {
                    tracing::info!("[init] LearningModule initialised");
                    Some(std::sync::Arc::new(tokio::sync::Mutex::new(lm)))
                }
                Err(e) => {
                    tracing::warn!(
                        "[init] LearningModule failed to initialise: {e}; self-learning disabled"
                    );
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    "[init] Could not create tokio runtime for LearningModule init: {e}"
                );
                None
            }
        }
    };

    // ActiveRetrievalManager: shared per-app instance (avoids re-creating config each turn).
    let active_retrieval_manager = Some(std::sync::Arc::new(
        modules::memory::retrieval::ActiveRetrievalManager::with_defaults(),
    ));

    // ── Phase 6E: Agent Loop Harness ──
    // Harness is disabled by default in production. Enable via the
    // IF2AI_HARNESS_ENABLED=1 environment variable or the IPC commands.
    let harness = if std::env::var("IF2AI_HARNESS_ENABLED").as_deref() == Ok("1") {
        let trace_dir = if2ai_dir.join("traces");
        tracing::info!("[init] Harness enabled; traces → {:?}", trace_dir);
        Some(std::sync::Arc::new(modules::harness::HarnessState::new(
            trace_dir,
        )))
    } else {
        None
    };

    // Create app state — includes memory infrastructure, learning, and onboarding flow
    let app_state = AppState::new(commands::AppStateConfig {
        session_manager,
        tool_registry,
        project_manager,
        memory_provider,
        context_budget,
        onboarding_flow,
        trajectory_manager,
        learning_module,
        active_retrieval_manager,
        harness,
        threat_scanner,
        job_runner,
        utility_llm,
        summary_store,
        rolling_summarizer,
        pinned_store,
        memory_compiler,
        memory_ticker,
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        // Browser registry as separate managed state so browser commands can
        // access it without going through AppState.
        .manage(browser_registry)
        .manage({
            // Phase TTS-C.1：lazy provider + IdleEvictor 接入。
            //
            // - 启动不再同步 load 4 个 ONNX session（节省 ~3s 冷启动）。
            // - 第一次 tts_synthesize / tts_warmup_status 时 lazy load。
            // - 5 分钟无请求自动 unload（释放 ~1.5GB RAM），下次请求 lazy reload。
            // - 模型缺失时 fallback 到 MockTtsProvider。
            let factory: std::sync::Arc<dyn Fn() -> Result<
                std::sync::Arc<dyn modules::tts::TtsProvider>,
                modules::tts::error::TtsError,
            > + Send + Sync + 'static> = std::sync::Arc::new(|| {
                let model_root = dirs::home_dir()
                    .map(|h| h.join(".if2ai/models/tts"))
                    .unwrap_or_else(|| std::path::PathBuf::from(".if2ai/models/tts"));
                let manifest_present = model_root
                    .join("MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json")
                    .is_file();
                if manifest_present {
                    let p = modules::tts::provider::OnnxTtsProvider::from_model_dir(
                        &model_root,
                        Some(4),
                    )?;
                    tracing::info!(
                        model_dir = %model_root.display(),
                        "TTS provider loaded (OnnxTtsProvider)"
                    );
                    Ok(std::sync::Arc::new(p) as std::sync::Arc<dyn modules::tts::TtsProvider>)
                } else {
                    tracing::info!(
                        "TTS model not found at {}; using Mock provider",
                        model_root.display()
                    );
                    Ok(std::sync::Arc::new(modules::tts::provider::MockTtsProvider::new())
                        as std::sync::Arc<dyn modules::tts::TtsProvider>)
                }
            });
            let factory_clone = factory.clone();
            let provider_handle = std::sync::Arc::new(commands::ProviderHandle::new(
                move || (factory_clone)(),
                None, // 不预加载
            ));

            // 启动 idle evictor（5 分钟阈值 / 30 秒检查）
            // Phase TTS-D.1：evict 时把 ProviderState 翻成 Evicted。
            let (state_arc, state_changed_at) = provider_handle.state_handle();
            let boot_for_evict = provider_handle.boot();
            let on_evict: std::sync::Arc<dyn Fn() + Send + Sync + 'static> = std::sync::Arc::new(
                move || {
                    let now_ms = boot_for_evict.elapsed().as_millis() as i64;
                    state_changed_at
                        .store(now_ms, std::sync::atomic::Ordering::Relaxed);
                    let state_arc = state_arc.clone();
                    // 在调用线程里 block_on：evictor 自己的 runtime 已就位
                    let _ = tauri::async_runtime::block_on(async move {
                        let mut guard = state_arc.write().await;
                        *guard = commands::tts::ProviderState::Evicted { elapsed_seconds: 0.0 };
                    });
                },
            );
            let evictor = std::sync::Arc::new(
                modules::tts::manager::eviction::IdleEvictor::spawn(
                    modules::tts::manager::eviction::IdleEvictionConfig::default(),
                    provider_handle.slot(),
                    provider_handle.last_use_millis(),
                    provider_handle.boot(),
                    Some(on_evict),
                ),
            );

            commands::TtsState {
                provider: provider_handle,
                warmup: std::sync::Arc::new(modules::tts::manager::warmup::WarmupManager::new()),
                jobs: std::sync::Arc::new(modules::tts::manager::jobs::StreamingJobManager::new()),
                _evictor: Some(evictor),
            }
        })
        .manage(std::sync::Arc::new(tokio::sync::Mutex::new(
            commands::tts_download::TtsDownloadState::default(),
        )))
        .invoke_handler(tauri::generate_handler![
            run_agent_turn,
            start_agent_stream,
            stop_agent_stream,
            respond_permission,
            // Browser control commands (Phase 7B + 7C profile management + 7C.3 takeover)
            get_browser_sessions,
            close_browser_session,
            get_chrome_status,
            request_browser_status,
            list_browser_profiles,
            clear_browser_profile,
            get_browser_settings,
            set_browser_settings,
            request_browser_takeover,
            release_browser_takeover,
            get_browser_action_log,
            list_sessions,
            delete_session,
            rename_session,
            set_session_pinned,
            create_session,
            list_project_sessions,
            create_project,
            list_projects,
            get_project,
            get_session,
            rename_project,
            delete_project,
            open_project_in_finder,
            ensure_default_workdir,
            open_directory_path,
            pick_folder_dialog,
            list_directory_preview,
            read_file_preview,
            write_file_contents,
            create_permanent_worktree,
            open_settings_window,
            close_settings_window,
            open_browser_viewer_window,
            navigate_viewer_window,
            browser_viewer_go_back,
            browser_viewer_go_forward,
            browser_viewer_reload,
            focus_main_window_and_prefill_prompt,
            execute_tool,
            fetch_skills_market_audits,
            list_tools,
            get_tool_definitions,
            list_toolsets,
            parse_slash_command,
            list_slash_commands,
            suggest_slash_commands,
            execute_slash_command,
            resolve_skill_slash,
            list_skills,
            list_agents,
            // Memory Browser commands
            memory_recall,
            memory_delete,
            memory_export,
            memory_purge,
            memory_clear_all,
            // Memory promotion (session → project → global)
            memory_promotion_candidates,
            memory_promote,
            memory_demote,
            // Per-session memory toggle (Phase 8A.4 / v2 §Sprint 1 / T-A4)
            memory_session_set_enabled,
            // Memory compile pipeline (Phase 8B.5 / T-C5 / v2 §0.5 Δ-11)
            memory_compile_now,
            memory_compiled_read,
            memory_compiled_clear,
            // Session summary timeline (Phase 8B.11 / T-UI-3 / v2 §0.5 Δ-11)
            memory_summaries_list,
            // Pinned-memory commands (Phase 8A.10 / T-F3 / v2 §0.5 Δ-11)
            pinned_get,
            pinned_add,
            pinned_delete,
            pinned_reorder,
            // Skills Hub CLI commands
            hub_browse,
            hub_search,
            hub_inspect,
            hub_check,
            hub_install,
            hub_update,
            hub_audit,
            hub_uninstall,
            hub_publish,
            hub_snapshot_export,
            hub_snapshot_import,
            hub_tap_add,
            hub_tap_remove,
            hub_tap_list,
            // Web search configuration
            get_web_search_config,
            upsert_web_search_provider,
            remove_web_search_provider,
            reorder_web_search_providers,
            validate_web_search_key,
            // Memory settings
            get_memory_config,
            set_memory_config,
            export_trajectories,
            // Trajectory introspection (H6) — async commands from modules/commands/trajectory.rs
            modules::commands::trajectory::get_trajectory_count,
            modules::commands::trajectory::get_trajectory_path,
            // Onboarding & Configuration Platform (Phase 6G)
            // onboarding.rs (5 commands)
            onboarding_get_state,
            onboarding_next_step,
            onboarding_prev_step,
            onboarding_complete,
            security_confirm,
            // system_check.rs (5 commands)
            system_check_run,
            embedded_model_download,
            embedded_model_progress,
            get_model_config,
            set_model_config,
            // provider.rs (9 commands)
            provider_list,
            provider_configure,
            provider_configure_with_models,
            provider_get_config,
            provider_get_configured_models,
            provider_get_all_configured_models,
            provider_list_configured,
            provider_test,
            provider_list_models,
            model_select,
            model_test,
            // Multi-model management (5 commands)
            model_list_available,
            model_get_active,
            model_set_active,
            model_get_role_config,
            model_set_role_config,
            // channel.rs (4 commands)
            channel_list,
            channel_configure,
            channel_test,
            channel_list_configured,
            // activation.rs (5 commands; M2.5 added activation_get_status)
            activation_validate,
            activation_start,
            activation_test_message,
            activation_complete,
            activation_get_status,
            // request_intelligence.rs (M2.6 — deterministic classify)
            request_intelligence_classify,
            // config.rs (4 commands)
            config_load,
            config_save,
            config_validate,
            config_reset_onboarding,
            // Harness Control IPC (Phase 6E)
            get_harness_status,
            start_harness_recording,
            stop_harness_recording,
            get_session_telemetry,
            get_all_session_telemetry,
            // TTS commands (Phase TTS-5)
            tts_health,
            tts_warmup_status,
            tts_start_warmup,
            // User-tunable TTS settings (Settings UI)
            get_tts_settings,
            set_tts_settings,
            // TTS Profiles (named voice + settings recipes)
            list_tts_profiles,
            save_tts_profile,
            delete_tts_profile,
            set_default_tts_profile,
            tts_synthesize,
            tts_stream_start,
            tts_stream_status,
            tts_stream_result,
            tts_stream_close,
            tts_demo_audio,
            tts_list_voices,
            tts_split_text,
            // TTS-D / P0：Voice asset registry + preview
            tts_list_voice_assets,
            tts_voice_audio,
            tts_preview_voice,
            // TTS-E.1：用户上传 / 删除 / 重命名 自定义声纹
            tts_upload_user_voice,
            tts_delete_user_voice,
            tts_rename_user_voice,
            // TTS-E / P2：Voice preview cache
            tts_warm_voice_preview,
            tts_cached_voice_preview,
            // TTS-E / P3：STT（Whisper local + Groq cloud + OpenFlow SenseVoice local）
            stt_model_status,
            stt_download_whisper_model,
            stt_download_openflow_model,
            stt_transcribe,
            stt_get_settings,
            stt_save_settings,
            // TTS model download
            tts_model_status,
            tts_model_download_start,
            tts_model_download_status,
        ])
        .setup(|app| {
            // Inject AppHandle into BrowserRegistry so the browser tool can emit
            // "browser-status" Tauri events to the frontend BrowserCard.
            {
                let registry = app
                    .state::<std::sync::Arc<modules::browser::BrowserRegistry>>()
                    .inner()
                    .clone();
                registry.set_app_handle(app.handle().clone());
            }

            // Register the same AppHandle with MemoryAuditEmitter so memory
            // lifecycle events (memory_captured / memory_write_decision /
            // memory_persisted / memory_recall_served / memory_rejected /
            // memory_promoted) are forwarded to the frontend `memory_event`
            // channel for `MemoryChip` / `MemoryWriteCard` consumption.
            modules::memory::audit::register_app_handle(app.handle().clone());

            // Phase 8B.9 (T-D4) — kick off the MemoryTicker startup
            // hook: recover_unsummarized() catches up dirty sessions
            // from the previous boot, then a backup
            // tokio::time::interval drives `maybe_run_daily` every
            // `daily_check_interval_secs` so a long-idle agent still
            // hits the daily compile cycle.  Spawned on the Tauri
            // async runtime so the setup closure stays synchronous.
            {
                let ticker_for_start = app
                    .state::<AppState>()
                    .inner()
                    .memory_ticker
                    .clone();
                tauri::async_runtime::spawn(async move {
                    let scope = modules::memory::scope::MemoryExecutionScope::global();
                    ticker_for_start.start(scope).await;
                });
            }

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

            // Create system tray menu
            let show_item = MenuItem::with_id(app, "show", "Show If2Ai", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // Build system tray
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("If2Ai - AI Agent Desktop")
                .on_menu_event(
                    |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| match event.id.as_ref()
                    {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            cleanup_processes();
                            app.exit(0);
                        }
                        _ => {}
                    },
                )
                .build(app)?;

            // Hide window on close button instead of exiting
            // SAFETY: get_webview_window returns Some in setup, and run() error is unrecoverable
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

            Ok(())
        })
        // SAFETY: run() error is unrecoverable for a desktop app
        .run(tauri::generate_context!())
        .map_err(|e| tracing::error!("Tauri application exited with error: {e}"))
        .ok();
}
