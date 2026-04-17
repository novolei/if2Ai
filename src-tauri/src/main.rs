#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::PathBuf;
use std::process::Command;

use commands::AppState;
use commands::{
    // Onboarding & Configuration Platform (Phase 6G)
    activation_complete,
    activation_start,
    activation_test_message,
    activation_validate,
    channel_configure,
    channel_list,
    channel_list_configured,
    channel_test,
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
    embedded_model_download,
    embedded_model_progress,
    ensure_default_workdir,
    execute_slash_command,
    execute_tool,
    export_trajectories,
    fetch_skills_market_audits,
    focus_main_window_and_prefill_prompt,
    get_memory_config,
    get_model_config,
    get_project,
    get_session,
    get_tool_definitions,
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
    list_directory_preview,
    list_project_sessions,
    list_projects,
    list_sessions,
    list_skills,
    list_slash_commands,
    list_tools,
    list_toolsets,
    memory_delete,
    memory_export,
    memory_purge,
    memory_recall,
    model_get_active,
    model_get_role_config,
    model_list_available,
    model_select,
    model_set_active,
    model_set_role_config,
    model_test,
    onboarding_complete,
    onboarding_get_state,
    onboarding_next_step,
    onboarding_prev_step,
    open_directory_path,
    open_project_in_finder,
    open_settings_window,
    parse_slash_command,
    pick_folder_dialog,
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
    remove_web_search_provider,
    rename_project,
    rename_session,
    reorder_web_search_providers,
    resolve_skill_slash,
    respond_permission,
    run_agent_turn,
    security_confirm,
    set_memory_config,
    set_model_config,
    set_session_pinned,
    start_agent_stream,
    stop_agent_stream,
    suggest_slash_commands,
    system_check_run,
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
fn create_memory_provider() -> modules::memory::SharedMemoryProvider {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create tokio runtime for memory init: {e}, falling back to SQLite"
            );
            return create_sqlite_provider();
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
            let db_path = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".if2ai")
                .join("memory")
                .join("vector_db");

            let config = modules::memory::VectorProviderConfig {
                db_path,
                vector_search_enabled: true,
            };

            modules::memory::VectorMemoryProvider::new(config).await
        })
        .await
    });

    match vector_result {
        Ok(Ok(provider)) => {
            tracing::info!("[memory] VectorMemoryProvider initialized successfully");
            std::sync::Arc::new(provider) as modules::memory::SharedMemoryProvider
        }
        Ok(Err(e)) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider initialization failed: {e}, falling back to SQLite"
            );
            create_sqlite_provider()
        }
        Err(_) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider timed out after 30s, falling back to SQLite"
            );
            create_sqlite_provider()
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
fn create_sqlite_provider() -> modules::memory::SharedMemoryProvider {
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("memory")
        .join("memory.db");
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match modules::memory::SqliteMemoryProvider::new(db_path) {
        Ok(p) => std::sync::Arc::new(p) as modules::memory::SharedMemoryProvider,
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

    // Set up cleanup hooks
    std::panic::set_hook(Box::new(|_| {
        cleanup_processes();
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
    let memory_provider = create_memory_provider();
    let scheduler_provider = modules::scheduler::default_scheduler();
    let browser_registry = modules::browser::BrowserRegistry::new(
        if2ai_dir.join("browser-cold-state.json"),
    );
    modules::tools::register_builtin_tools(
        &tool_registry,
        memory_provider.clone(),
        scheduler_provider,
        browser_registry.clone(),
    );
    let project_manager = modules::projects::ProjectManager::new(projects_dir);

    // Initialize onboarding flow (Phase 6G)
    // OnboardingFlow is a stateless driver; state is loaded lazily via Tauri commands.
    let onboarding_flow = modules::onboarding::flow::OnboardingFlow;

    // Initialize context budget (default: 4000 tokens, 10/20/30/40%)
    let context_budget = modules::runtime::budget::ContextBudget::default();

    // Create app state — now includes memory infrastructure and onboarding flow
    let app_state = AppState::new(
        session_manager,
        tool_registry,
        project_manager,
        memory_provider,
        context_budget,
        onboarding_flow,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            run_agent_turn,
            start_agent_stream,
            stop_agent_stream,
            respond_permission,
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
            // activation.rs (4 commands)
            activation_validate,
            activation_start,
            activation_test_message,
            activation_complete,
            // config.rs (4 commands)
            config_load,
            config_save,
            config_validate,
            config_reset_onboarding,
        ])
        .setup(|app| {
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
