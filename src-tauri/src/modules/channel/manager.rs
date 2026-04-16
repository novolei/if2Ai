//! Channel lifecycle manager — dynamic start/stop for all platform adapters.
//!
//! The `ChannelManager` is responsible for:
//! - Registering `ChannelAdapter` instances for each configured platform
//! - Starting/stopping polling loops for Polling-mode channels
//! - Managing `CancellationToken` per platform for clean shutdown
//! - Tracking active tasks via `DashMap` for concurrent access
//!
//! Per ADR-014 Section 18.1, this enables the dynamic channel lifecycle
//! management where platforms can be started/stopped at runtime.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::adapter::{AdapterRef, ChannelAdapter, MessageHandler};
use super::types::{ChannelEnvelope, ConnectMode};

/// Channel lifecycle manager — manages all platform adapters and their
/// polling/running tasks.
///
/// # Thread Safety
///
/// `ChannelManager` uses `DashMap` for both `adapters` and `tasks`,
/// allowing concurrent registration, start, and stop operations.
///
/// # Lifecycle
///
/// ```text
/// 1. register_adapter(adapter)  — add platform adapter
/// 2. start_platform_polling()   — spawn polling task with CancellationToken
/// 3. ... messages flow ...
/// 4. stop_platform()            — cancel token, await task completion
/// 5. stop_all()                 — shutdown all platforms (app exit)
/// ```
pub struct ChannelManager {
    /// Registered platform adapters, keyed by platform ID.
    adapters: DashMap<String, AdapterRef>,
    /// Active polling tasks, keyed by platform ID.
    /// Each entry holds the `JoinHandle` and its `CancellationToken`.
    tasks: DashMap<String, (JoinHandle<()>, CancellationToken)>,
    /// Global message handler invoked by all polling adapters.
    handler: Option<MessageHandler>,
}

impl ChannelManager {
    /// Create a new, empty `ChannelManager`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adapters: DashMap::new(),
            tasks: DashMap::new(),
            handler: None,
        }
    }

    /// Create a new `ChannelManager` with a global message handler.
    ///
    /// The handler is invoked for every message received by any polling
    /// adapter managed by this manager.
    #[must_use]
    pub fn with_handler(handler: MessageHandler) -> Self {
        Self {
            adapters: DashMap::new(),
            tasks: DashMap::new(),
            handler: Some(handler),
        }
    }

    /// Register a platform adapter.
    ///
    /// If an adapter with the same platform ID already exists, it is replaced.
    /// Registering does NOT automatically start polling — call
    /// `start_platform_polling` or `start_all_polling` for that.
    pub fn register_adapter(&self, adapter: AdapterRef) {
        let id = adapter.platform_id().to_string();
        info!(platform = %id, mode = ?adapter.connect_mode(), "registering channel adapter");
        self.adapters.insert(id, adapter);
    }

    /// Unregister a platform adapter. Stops its polling task if running.
    pub fn unregister_adapter(&self, platform: &str) {
        if self.adapters.remove(platform).is_some() {
            info!(platform = %platform, "unregistering channel adapter");
            self.stop_platform(platform);
        }
    }

    /// Check if a platform adapter is registered.
    #[must_use]
    pub fn is_registered(&self, platform: &str) -> bool {
        self.adapters.contains_key(platform)
    }

    /// Get a reference to a registered adapter, if any.
    #[must_use]
    pub fn get_adapter(&self, platform: &str) -> Option<AdapterRef> {
        self.adapters
            .get(platform)
            .map(|entry| Arc::clone(entry.value()))
    }

    /// List all registered platform IDs.
    #[must_use]
    pub fn list_platforms(&self) -> Vec<String> {
        self.adapters.iter().map(|e| e.key().clone()).collect()
    }

    /// Start polling for a specific platform.
    ///
    /// Only applies to adapters with `ConnectMode::Polling`.
    /// If the platform is already running, it is stopped first (restart).
    ///
    /// # Arguments
    ///
    /// * `platform` — Platform ID (must be registered)
    /// * `adapter` — The adapter to run (should match the platform ID)
    pub fn start_platform_polling(&self, platform: &str, adapter: AdapterRef) {
        // Stop existing task if running
        self.stop_platform(platform);

        let handler = self.handler.clone();
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let platform_id = platform.to_string();
        let log_id = platform_id.clone();

        let handle = tokio::spawn(async move {
            let on_message: MessageHandler = Arc::new(move |envelope: ChannelEnvelope| {
                if let Some(ref h) = handler {
                    h(envelope);
                }
            });

            adapter.run_polling(on_message, cancel_clone).await;
            info!(platform = %platform_id, "polling task exited");
        });

        info!(platform = %log_id, "started platform polling");
        self.tasks.insert(log_id, (handle, cancel));
    }

    /// Start polling for all registered Polling-mode platforms.
    ///
    /// Iterates over all registered adapters and starts polling for each
    /// one that uses `ConnectMode::Polling`.
    pub fn start_all_polling(&self) {
        let adapters: Vec<_> = self
            .adapters
            .iter()
            .filter(|e| matches!(e.value().connect_mode(), ConnectMode::Polling))
            .map(|e| (e.key().clone(), Arc::clone(e.value())))
            .collect();

        for (id, adapter) in adapters {
            self.start_platform_polling(&id, adapter);
        }
    }

    /// Stop a specific platform's polling task.
    ///
    /// Cancels the platform's `CancellationToken` and awaits the task
    /// completion. If the platform is not running, this is a no-op.
    pub fn stop_platform(&self, platform: &str) {
        if let Some((_key, (handle, cancel))) = self.tasks.remove(platform) {
            info!(platform = %platform, "stopping platform polling");
            cancel.cancel();
            // Spawn a detached abort to avoid blocking the caller
            tokio::spawn(async move {
                let _ = handle.await;
            });
        }
    }

    /// Stop all running platform polling tasks.
    ///
    /// Called during application shutdown. Cancels all tokens and
    /// spawns detached abort handlers.
    pub fn stop_all(&self) {
        let platforms: Vec<String> = self.tasks.iter().map(|e| e.key().clone()).collect();
        for platform in platforms {
            self.stop_platform(&platform);
        }
        info!("all channel polling tasks stopped");
    }

    /// Check if a specific platform's polling is currently running.
    #[must_use]
    pub fn is_running(&self, platform: &str) -> bool {
        self.tasks.contains_key(platform)
    }

    /// Return the number of currently running polling tasks.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::channel::types::PlatformConfig;
    use crate::modules::config::ChannelConfig;

    /// Minimal mock adapter for testing.
    struct MockAdapter {
        id: String,
        mode: ConnectMode,
    }

    #[async_trait::async_trait]
    impl ChannelAdapter for MockAdapter {
        fn platform_id(&self) -> &str {
            &self.id
        }

        fn connect_mode(&self) -> ConnectMode {
            self.mode.clone()
        }

        fn test_connection(
            &self,
            _config: &PlatformConfig,
        ) -> Result<crate::modules::provider::types::TestResult, String> {
            Ok(crate::modules::provider::types::TestResult {
                success: true,
                message: "ok".to_string(),
                latency_ms: Some(1),
                details: None,
            })
        }

        fn build_config(&self, _creds: serde_json::Value) -> Result<ChannelConfig, String> {
            Ok(ChannelConfig {
                channel_id: self.id.clone(),
                display_name: self.id.clone(),
                bot_token: None,
                app_secret: None,
                webhook_url: None,
            })
        }

        async fn run_polling(&self, _on_message: MessageHandler, cancel: CancellationToken) {
            cancel.cancelled().await;
        }
    }

    fn make_mock(id: &str, mode: ConnectMode) -> AdapterRef {
        Arc::new(MockAdapter {
            id: id.to_string(),
            mode,
        })
    }

    #[test]
    fn test_manager_new_is_empty() {
        let manager = ChannelManager::new();
        assert!(manager.list_platforms().is_empty());
        assert_eq!(manager.running_count(), 0);
    }

    #[test]
    fn test_register_and_list() {
        let manager = ChannelManager::new();
        manager.register_adapter(make_mock("telegram", ConnectMode::Polling));
        manager.register_adapter(make_mock("feishu", ConnectMode::Polling));

        let platforms = manager.list_platforms();
        assert_eq!(platforms.len(), 2);
        assert!(platforms.contains(&"telegram".to_string()));
        assert!(platforms.contains(&"feishu".to_string()));
    }

    #[test]
    fn test_register_replaces_existing() {
        let manager = ChannelManager::new();
        manager.register_adapter(make_mock("telegram", ConnectMode::Polling));
        manager.register_adapter(make_mock("telegram", ConnectMode::Webhook));

        let platforms = manager.list_platforms();
        assert_eq!(platforms.len(), 1);
    }

    #[test]
    fn test_unregister() {
        let manager = ChannelManager::new();
        manager.register_adapter(make_mock("telegram", ConnectMode::Polling));
        assert!(manager.is_registered("telegram"));

        manager.unregister_adapter("telegram");
        assert!(!manager.is_registered("telegram"));
    }

    #[test]
    fn test_get_adapter() {
        let manager = ChannelManager::new();
        manager.register_adapter(make_mock("feishu", ConnectMode::Polling));

        let adapter = manager.get_adapter("feishu");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().platform_id(), "feishu");

        assert!(manager.get_adapter("nonexistent").is_none());
    }

    #[test]
    fn test_is_running_initially_false() {
        let manager = ChannelManager::new();
        assert!(!manager.is_running("telegram"));
    }

    #[tokio::test]
    async fn test_start_and_stop_platform() {
        let manager = ChannelManager::new();
        let adapter = make_mock("telegram", ConnectMode::Polling);
        manager.register_adapter(adapter.clone());

        manager.start_platform_polling("telegram", adapter);
        assert!(manager.is_running("telegram"));
        assert_eq!(manager.running_count(), 1);

        manager.stop_platform("telegram");
        assert!(!manager.is_running("telegram"));
    }

    #[tokio::test]
    async fn test_start_all_polling() {
        let manager = ChannelManager::new();

        // Register polling adapters
        let telegram = make_mock("telegram", ConnectMode::Polling);
        let feishu = make_mock("feishu", ConnectMode::Polling);
        manager.register_adapter(telegram.clone());
        manager.register_adapter(feishu.clone());

        // Register a webhook adapter (should not start)
        let slack = make_mock("slack", ConnectMode::Webhook);
        manager.register_adapter(slack.clone());

        manager.start_all_polling();

        // Should have started telegram and feishu, but not slack
        assert!(manager.is_running("telegram"));
        assert!(manager.is_running("feishu"));
        assert!(!manager.is_running("slack"));
        assert_eq!(manager.running_count(), 2);

        // Clean up
        manager.stop_all();
    }

    #[tokio::test]
    async fn test_stop_all() {
        let manager = ChannelManager::new();

        let telegram = make_mock("telegram", ConnectMode::Polling);
        let feishu = make_mock("feishu", ConnectMode::Polling);
        manager.register_adapter(telegram.clone());
        manager.register_adapter(feishu.clone());

        manager.start_all_polling();
        assert_eq!(manager.running_count(), 2);

        manager.stop_all();
        assert_eq!(manager.running_count(), 0);
    }

    #[tokio::test]
    async fn test_restart_platform() {
        let manager = ChannelManager::new();
        let adapter = make_mock("telegram", ConnectMode::Polling);
        manager.register_adapter(adapter.clone());

        manager.start_platform_polling("telegram", adapter.clone());
        assert!(manager.is_running("telegram"));

        // Starting again should stop the old one first (restart)
        manager.start_platform_polling("telegram", adapter);
        assert!(manager.is_running("telegram"));
        assert_eq!(manager.running_count(), 1);

        manager.stop_platform("telegram");
    }

    #[test]
    fn test_manager_with_handler() {
        let handler: MessageHandler = Arc::new(|_envelope: ChannelEnvelope| {});
        let manager = ChannelManager::with_handler(handler);
        assert!(manager.handler.is_some());
    }
}
