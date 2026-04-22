#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap;
mod commands;
mod modules;

fn main() {
    let paths = bootstrap::resolve_boot_paths();
    bootstrap::initialize_process_runtime(&paths);

    let app_bootstrap = bootstrap::build_app_bootstrap(&paths);
    let host_composition = commands::compose_desktop_host_state(
        app_bootstrap.app_state_config,
        app_bootstrap.learned_traits,
    );

    modules::desktop_host::attach_native_host(
        tauri::Builder::default(),
        host_composition.app_state,
        app_bootstrap.browser_registry,
        app_bootstrap.memory_ticker,
        host_composition.tts_state,
        host_composition.tts_download_state,
    )
    .invoke_handler(crate::if2ai_command_surface!())
    .setup(|app| Ok(modules::desktop_host::setup_desktop_host(app)?))
    // SAFETY: run() error is unrecoverable for a desktop app
    .run(tauri::generate_context!())
    .map_err(|e| tracing::error!("Tauri application exited with error: {e}"))
    .ok();
}
