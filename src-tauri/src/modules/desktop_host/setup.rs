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
    resolve_bundled_skills(app);
    install_system_tray(app)?;
    install_main_window_policy(app);
    Ok(())
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
