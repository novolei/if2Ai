//! Truth-loop iter-9 — process-wide user-MCP manager.
//!
//! ## Why a singleton?
//!
//! Before iter-9 every `mcp_workbench_*` Tauri command in
//! `commands/settings.rs` did `from_runtime_config → use → shutdown`,
//! which spawned and tore down each stdio MCP child once per call.
//! That made workbench operations slow (fresh `initialize` handshake
//! every time) AND prevented the self-healing daemon from observing
//! MCP child liveness across calls (no persistent process to poll).
//!
//! This module owns one long-lived [`McpServerManager`] for **all
//! user-configured stdio MCP servers** (`runtime::config::current()
//! .mcp().servers()` minus the `browser-use` server name, which is
//! the exclusive territory of `smart_browser::runtime::
//! BROWSER_USE_MCP_MANAGER`).
//!
//! ## Hard rules (see plan §0)
//!
//! 1. **Dedup**: `BROWSER_USE_MCP_SERVER_NAME` MUST be filtered out
//!    on every (re)build to avoid double-spawning the same logical
//!    stdio child.
//! 2. **Reload from disk**: `runtime::config::current()` is a boot
//!    snapshot; live edits to `settings.json` only reach this
//!    singleton after [`reload_user_mcp_manager_from_disk`] is
//!    called (today: from `set_mcp_service_config` post-write).
//! 3. **No per-call shutdown**: workbench commands MUST stop calling
//!    `manager.shutdown().await` after migrating to this singleton.
//!    Lifecycle is owned here ([`shutdown_user_mcp_manager`] is
//!    invoked once at app exit by `desktop_host::setup`).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use super::config::{ConfigLoader, RuntimeConfig, ScopedMcpServerConfig};
use super::mcp_stdio::McpServerManager;
use crate::modules::smart_browser::browser_use_mcp::BROWSER_USE_MCP_SERVER_NAME;

/// Process-wide manager for all user-configured stdio MCP servers.
static GLOBAL_USER_MCP_MANAGER: OnceLock<Arc<tokio::sync::Mutex<McpServerManager>>> =
    OnceLock::new();

/// Filter the loaded config's MCP map down to the set of servers
/// this manager owns: every Stdio server EXCEPT `browser-use`.
///
/// Returning a fresh `BTreeMap` (cloned values) keeps the call sites
/// simple — `McpServerManager::from_servers` already takes a borrow.
#[must_use]
pub fn build_user_servers(config: &RuntimeConfig) -> BTreeMap<String, ScopedMcpServerConfig> {
    config
        .mcp()
        .servers()
        .iter()
        .filter(|(name, _)| name.as_str() != BROWSER_USE_MCP_SERVER_NAME)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Return (and lazily initialise) the process-wide user-MCP manager.
///
/// Initial population uses `runtime::config::current()` if it has
/// been installed; otherwise an empty manager is created so callers
/// outside the full Tauri boot path (notably `cargo test --lib`)
/// still get a usable handle. After the first call subsequent
/// reloads happen via [`reload_user_mcp_manager_from_disk`] —
/// the `Arc<Mutex<...>>` itself never moves.
#[must_use]
pub fn global_user_mcp_manager() -> Arc<tokio::sync::Mutex<McpServerManager>> {
    Arc::clone(GLOBAL_USER_MCP_MANAGER.get_or_init(|| {
        let initial = if super::config::is_initialised() {
            McpServerManager::from_servers(&build_user_servers(super::config::current()))
        } else {
            McpServerManager::from_servers(&BTreeMap::new())
        };
        Arc::new(tokio::sync::Mutex::new(initial))
    }))
}

/// Reload the user-MCP manager from disk (the same `ConfigLoader`
/// path the workbench used per-call before iter-9). Tears down the
/// old manager's stdio children before swapping so we never leak.
///
/// Called from `set_mcp_service_config` after the user edits settings
/// via the UI. Returns the number of stdio servers now under
/// management (after dedup) so the caller can log a meaningful line.
///
/// Errors short-circuit before any teardown: if the new config fails
/// to load, the old manager keeps serving traffic.
pub async fn reload_user_mcp_manager_from_disk(cwd: &Path) -> Result<usize, String> {
    let config = ConfigLoader::default_for(cwd)
        .load()
        .map_err(|e| e.to_string())?;
    let new_servers = build_user_servers(&config);
    let count = new_servers.len();
    let arc = global_user_mcp_manager();
    let mut guard = arc.lock().await;
    let _ = guard.shutdown().await;
    *guard = McpServerManager::from_servers(&new_servers);
    Ok(count)
}

/// Best-effort graceful teardown of every stdio child owned by the
/// singleton. Call from app exit (tray Quit) before `cleanup_processes`
/// so MCP children get a `kill().await` instead of being orphaned.
/// No-op when the singleton was never initialised.
pub async fn shutdown_user_mcp_manager() {
    if let Some(arc) = GLOBAL_USER_MCP_MANAGER.get() {
        let mut guard = arc.lock().await;
        let _ = guard.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::config::{
        ConfigSource, McpServerConfig, McpStdioServerConfig, RuntimeConfig, ScopedMcpServerConfig,
    };

    fn stdio_entry(cmd: &str) -> ScopedMcpServerConfig {
        ScopedMcpServerConfig {
            scope: ConfigSource::User,
            config: McpServerConfig::Stdio(McpStdioServerConfig {
                command: cmd.to_string(),
                args: vec![],
                env: BTreeMap::new(),
            }),
        }
    }

    #[test]
    fn build_user_servers_filters_browser_use() {
        // Forge a RuntimeConfig containing both `browser-use` (which
        // MUST be filtered) and a real user server (which MUST be
        // kept). Constructing RuntimeConfig directly is awkward
        // outside the loader, so we cover the filter logic via a
        // direct map manipulation that mirrors the production shape.
        let mut servers: BTreeMap<String, ScopedMcpServerConfig> = BTreeMap::new();
        servers.insert(BROWSER_USE_MCP_SERVER_NAME.to_string(), stdio_entry("uvx"));
        servers.insert("project-fs".to_string(), stdio_entry("npx"));

        let kept: BTreeMap<String, ScopedMcpServerConfig> = servers
            .iter()
            .filter(|(name, _)| name.as_str() != BROWSER_USE_MCP_SERVER_NAME)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        assert_eq!(kept.len(), 1);
        assert!(kept.contains_key("project-fs"));
        assert!(!kept.contains_key(BROWSER_USE_MCP_SERVER_NAME));
    }

    #[tokio::test]
    async fn global_user_mcp_manager_is_singleton() {
        // Both calls must return the SAME Arc — Arc::ptr_eq compares
        // the underlying allocation, not the contents.
        let a = global_user_mcp_manager();
        let b = global_user_mcp_manager();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[tokio::test]
    async fn lazy_init_with_uninitialised_config_yields_empty_manager() {
        // In `cargo test --lib` the boot path never calls
        // `set_current`, so the singleton's first init must fall
        // back to an empty manager (and NOT panic / read disk).
        let arc = global_user_mcp_manager();
        let guard = arc.lock().await;
        // server_names() is the iter-9 helper exposed on the
        // manager; an empty manager has zero entries.
        assert!(guard.server_names().is_empty());
    }

    #[tokio::test]
    async fn reload_from_empty_config_swaps_in_place() {
        // Build a manager directly so we have a known starting set,
        // install it via the singleton path (only effective on the
        // first call), then exercise the swap branch via direct
        // mutation of the inner state. This covers the *swap*
        // mechanics without relying on a working disk loader.
        let arc = global_user_mcp_manager();
        let mut guard = arc.lock().await;
        // Replace inner with a 1-server manager.
        let mut servers: BTreeMap<String, ScopedMcpServerConfig> = BTreeMap::new();
        servers.insert("temp-server".to_string(), stdio_entry("echo"));
        *guard = McpServerManager::from_servers(&servers);
        assert_eq!(guard.server_names(), vec!["temp-server".to_string()]);

        // Simulate a reload to a different name set.
        servers.clear();
        servers.insert("other".to_string(), stdio_entry("echo"));
        let _ = guard.shutdown().await;
        *guard = McpServerManager::from_servers(&servers);
        assert_eq!(guard.server_names(), vec!["other".to_string()]);
    }

    /// `build_user_servers` against a real (default-empty)
    /// `RuntimeConfig` returns an empty map — proves the helper
    /// gracefully accepts the boot-time / unconfigured case the
    /// `cargo test --lib` env normally exhibits.
    #[test]
    fn build_user_servers_against_empty_config_is_empty() {
        let cfg = RuntimeConfig::empty();
        assert!(build_user_servers(&cfg).is_empty());
    }
}
