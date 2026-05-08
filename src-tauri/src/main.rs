#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap;
mod commands;
mod modules;

fn main() {
    let paths = bootstrap::resolve_boot_paths();
    bootstrap::initialize_process_runtime(&paths);

    // MEM-MOD-PATH-FIX — one-shot migration of the legacy
    // `<data_local_dir>/.if2ai/{memory,trajectories,...}` subtrees
    // into `~/.if2ai/`.  Idempotent: a sentinel file makes
    // subsequent boots no-op.  Must run BEFORE `build_app_bootstrap`
    // because that step opens SQLite + LanceDB at the new path.
    if let Err(err) =
        bootstrap::migrate_legacy_data_dir(&paths.if2ai_dir, dirs::data_local_dir().as_deref())
    {
        tracing::warn!(
            error = %err,
            "[migration] legacy data dir migration failed; continuing with possibly empty memory"
        );
    }

    let app_bootstrap = bootstrap::build_app_bootstrap(&paths);
    let host_composition = commands::compose_desktop_host_state(
        app_bootstrap.app_state_config,
        app_bootstrap.learned_traits,
    );
    let daydream_coordinator = host_composition.app_state.daydream_coordinator.clone();

    modules::desktop_host::attach_native_host(
        tauri::Builder::default(),
        host_composition.app_state,
        app_bootstrap.browser_registry,
        app_bootstrap.memory_ticker,
        host_composition.tts_state,
        host_composition.tts_download_state,
    )
    .invoke_handler(crate::if2ai_command_surface!())
    .setup(move |app| {
        modules::desktop_host::setup_desktop_host(app)?;
        let app_handle = app.handle().clone();
        modules::memory::daydream::spawn_poll_loop(daydream_coordinator, move |report| {
            let family = if report.all_succeeded() {
                modules::runtime::contracts::common::daydream_family::COMPLETED
            } else {
                modules::runtime::contracts::common::daydream_family::FAILED
            };
            let _ = modules::runtime::runtime_event::dispatch(
                Some(&app_handle),
                modules::runtime::contracts::common::RuntimeEventType::DaydreamCycle,
                family,
                modules::runtime::contracts::common::CorrelationIds::default(),
                &report,
                None,
            );
        });
        Ok(())
    })
    // SAFETY: run() error is unrecoverable for a desktop app
    .run(tauri::generate_context!())
    .map_err(|e| tracing::error!("Tauri application exited with error: {e}"))
    .ok();
}
