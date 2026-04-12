#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod modules;

use std::path::PathBuf;
use std::process::Command;

use commands::AppState;
use commands::{
    close_settings_window, create_permanent_worktree, create_project, create_session,
    delete_project, delete_session, execute_tool, get_project, get_session, get_tool_definitions,
    list_project_sessions, list_projects, list_sessions, list_tools, open_project_in_finder,
    open_settings_window, rename_project, run_agent_turn, set_session_pinned, start_agent_stream,
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
    modules::tools::register_builtin_tools(&tool_registry);
    let project_manager = modules::projects::ProjectManager::new(projects_dir);

    // Create app state
    let app_state = AppState::new(session_manager, tool_registry, project_manager);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            run_agent_turn,
            start_agent_stream,
            list_sessions,
            delete_session,
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
            create_permanent_worktree,
            open_settings_window,
            close_settings_window,
            execute_tool,
            list_tools,
            get_tool_definitions,
        ])
        .setup(|app| {
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
            let window = app.get_webview_window("main").unwrap();
            let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
            let _ = window.set_background_color(Some(Color(0xf6, 0xf7, 0xf8, 0xff)));
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
