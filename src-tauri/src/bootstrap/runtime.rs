use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::modules;

pub fn initialize_process_runtime(paths: &super::BootPaths) {
    if let Err(e) = std::fs::create_dir_all(&paths.log_dir) {
        eprintln!("Warning: Failed to create log directory: {}", e);
    }

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &paths.log_dir, "backend.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    std::mem::forget(guard);

    let env_filter = EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into());

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stdout).with_ansi(true))
        .with(env_filter)
        .init();

    tracing::info!("If2Ai backend starting, log directory: {:?}", paths.log_dir);
    tracing::info!(
        "⭐⭐⭐ MEMORY TICKER FIX BUILD a89eeec / 8B.11 — chat turns should fire on_turn_complete"
    );

    install_runtime_config();
    install_panic_hook();
}

fn install_runtime_config() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        tracing::error!(panic = %info, "[panic] backend panicked");
    }));
}
