use std::path::PathBuf;
use std::sync::Arc;

use crate::commands;
use crate::modules;

mod app;
pub mod daydream_config;
mod memory;
mod migration;
mod runtime;

/// Filesystem paths resolved once during backend startup.
pub struct BootPaths {
    pub if2ai_dir: PathBuf,
    pub log_dir: PathBuf,
    pub memory_root: PathBuf,
    pub sessions_dir: PathBuf,
    pub projects_dir: PathBuf,
}

/// P0-2 — Future LLM provider composition seam.
///
/// Today resilience (`provider::resilience::stream_message_with_resilience`)
/// is invoked at call sites in `turn_service`. As we migrate to the
/// `LlmProvider` trait introduced in [`provider::llm_provider`], this
/// helper will assemble the decorator chain top-down based on env /
/// settings:
///
/// ```text
/// Failover( Retry( CircuitBreaker( CompletionCache( RawClient ))))
/// ```
///
/// Until each layer is migrated, the function is intentionally
/// unimplemented; call sites continue to use the historical helpers.
#[allow(dead_code)]
pub fn compose_llm_provider_chain_unimplemented() {
    // Marker for the M-pack that will own the composition.
}

/// Core application bootstrap result consumed by `main.rs`.
pub struct AppBootstrap {
    pub app_state_config: commands::AppStateConfig,
    pub browser_registry: Arc<modules::browser::BrowserRegistry>,
    pub memory_ticker: Arc<modules::memory::MemoryTicker>,
    /// MEM-MOD-P7 — `Some` when the LearnedTraitsStore opened
    /// successfully.  Forwarded into AppState via
    /// `with_learned_traits` after construction.
    pub learned_traits: Option<modules::memory::learned_traits::LearnedTraitsStore>,
}

/// Resolve all process-level data directories used by the desktop host.
///
/// MEM-MOD-PATH-FIX — `memory_root` now derives from `~/.if2ai`
/// (single root for the entire app); `dirs::data_local_dir()` was the
/// pre-fix legacy location and is no longer consulted by new boots.
/// One-shot migration of legacy `~/Library/Application Support/.if2ai`
/// is handled by [`migration::migrate_legacy_data_dir`] which `main.rs`
/// invokes before [`build_app_bootstrap`].
#[must_use]
pub fn resolve_boot_paths() -> BootPaths {
    let paths = resolve_boot_paths_from_inputs(
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| String::from("."))),
        std::env::var("IF2AI_SESSIONS_DIR").ok().map(PathBuf::from),
        std::env::var("IF2AI_PROJECTS_DIR").ok().map(PathBuf::from),
    );
    tracing::info!(
        build_profile = if cfg!(debug_assertions) { "debug" } else { "release" },
        bundle_version = env!("CARGO_PKG_VERSION"),
        config_root = %paths.if2ai_dir.display(),
        memory_root = %paths.memory_root.display(),
        app_data_dir = %dirs::data_local_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unavailable".to_string()),
        argv0 = %std::env::args().next().unwrap_or_else(|| "unknown".to_string()),
        "[init] resolved desktop storage roots"
    );
    paths
}

pub use app::build_app_bootstrap;
pub use migration::migrate_legacy_data_dir;
pub use runtime::initialize_process_runtime;

fn resolve_boot_paths_from_inputs(
    home_dir: PathBuf,
    sessions_override: Option<PathBuf>,
    projects_override: Option<PathBuf>,
) -> BootPaths {
    let if2ai_dir = home_dir.join(".if2ai");
    let log_dir = if2ai_dir.join("log");
    let memory_root = if2ai_dir.join("memory");
    let sessions_dir = sessions_override.unwrap_or_else(|| if2ai_dir.join("sessions"));
    let projects_dir = projects_override.unwrap_or_else(|| if2ai_dir.join("projects"));

    BootPaths {
        if2ai_dir,
        log_dir,
        memory_root,
        sessions_dir,
        projects_dir,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_boot_paths_from_inputs;
    use std::path::PathBuf;

    #[test]
    fn boot_paths_default_to_if2ai_subdirectories() {
        let paths = resolve_boot_paths_from_inputs(PathBuf::from("/tmp/if2ai-home"), None, None);

        assert_eq!(paths.if2ai_dir, PathBuf::from("/tmp/if2ai-home/.if2ai"));
        assert_eq!(paths.log_dir, PathBuf::from("/tmp/if2ai-home/.if2ai/log"));
        // MEM-MOD-PATH-FIX — memory_root now under ~/.if2ai (NOT
        // data_local_dir).  Single-root model post-migration.
        assert_eq!(
            paths.memory_root,
            PathBuf::from("/tmp/if2ai-home/.if2ai/memory")
        );
        assert_eq!(
            paths.sessions_dir,
            PathBuf::from("/tmp/if2ai-home/.if2ai/sessions")
        );
        assert_eq!(
            paths.projects_dir,
            PathBuf::from("/tmp/if2ai-home/.if2ai/projects")
        );
    }

    #[test]
    fn boot_paths_honor_explicit_session_and_project_overrides() {
        let paths = resolve_boot_paths_from_inputs(
            PathBuf::from("/tmp/if2ai-home"),
            Some(PathBuf::from("/tmp/custom-sessions")),
            Some(PathBuf::from("/tmp/custom-projects")),
        );

        assert_eq!(paths.sessions_dir, PathBuf::from("/tmp/custom-sessions"));
        assert_eq!(paths.projects_dir, PathBuf::from("/tmp/custom-projects"));
        assert_eq!(
            paths.memory_root,
            PathBuf::from("/tmp/if2ai-home/.if2ai/memory")
        );
    }
}
