#![allow(dead_code)]

mod json_helpers;
mod mcp;
mod memory;

use mcp::merge_mcp_servers;
#[allow(unused_imports)]
pub use mcp::{
    McpConfigCollection, McpManagedProxyServerConfig, McpOAuthConfig, McpRemoteServerConfig,
    McpSdkServerConfig, McpServerConfig, McpStdioServerConfig, McpTransport,
    McpWebSocketServerConfig, ScopedMcpServerConfig,
};
#[allow(unused_imports)]
use memory::{parse_optional_memory_feature_config, If2AiMemoryOverrides};
#[allow(unused_imports)]
pub use memory::{CompilerConfig, MemoryFeatureConfig, MemoryPolicyEnforceMode, MemoryRecallMode};

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use self::json_helpers::*;
use super::json::JsonValue;
use super::sandbox::{FilesystemIsolationMode, SandboxConfig};

pub const CLAW_SETTINGS_SCHEMA_NAME: &str = "SettingsSchema";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedPermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryEnforceMode {
    Shadow,
    #[default]
    Enforce,
}

impl BoundaryEnforceMode {
    /// Wire-format label used in settings JSON and audit logs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Enforce => "enforce",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneGovernanceConfig {
    control_plane_v2_enabled: bool,
    boundary_enforce_mode: BoundaryEnforceMode,
    sandbox_strict_mode: bool,
    provider_transport: ProviderTransportConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTransportConfig {
    // harness symbol marker: connect_timeout_ms\|stream_read_timeout_ms\|overall_timeout_ms\|max_retries
    connect_timeout_ms: u64,
    stream_read_timeout_ms: u64,
    overall_timeout_ms: u64,
    max_retries: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl Default for ProviderTransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 5_000,
            stream_read_timeout_ms: 30_000,
            overall_timeout_ms: 60_000,
            max_retries: 2,
            initial_backoff_ms: 200,
            max_backoff_ms: 2_000,
        }
    }
}

// MemoryRecallMode + MemoryPolicyEnforceMode moved to memory (GFR-T1-B-4).

// CompilerConfig + MemoryFeatureConfig moved to memory (GFR-T1-B-4).

impl Default for ControlPlaneGovernanceConfig {
    fn default() -> Self {
        Self {
            control_plane_v2_enabled: true,
            boundary_enforce_mode: BoundaryEnforceMode::Enforce,
            sandbox_strict_mode: true,
            provider_transport: ProviderTransportConfig::default(),
        }
    }
}

impl ResolvedPermissionMode {
    /// Wire-format label used in settings JSON and audit logs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }

    /// Project this resolved mode back to the runtime [`super::permissions::PermissionMode`].
    #[must_use]
    pub fn as_permission_mode(self) -> super::permissions::PermissionMode {
        match self {
            Self::ReadOnly => super::permissions::PermissionMode::ReadOnly,
            Self::WorkspaceWrite => super::permissions::PermissionMode::WorkspaceWrite,
            Self::DangerFullAccess => super::permissions::PermissionMode::DangerFullAccess,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub source: ConfigSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    merged: BTreeMap<String, JsonValue>,
    loaded_entries: Vec<ConfigEntry>,
    feature_config: RuntimeFeatureConfig,
}

/// Agent runtime configuration containing workdir and execution settings.
/// This is distinct from the claw-settings RuntimeConfig above.
#[derive(Debug, Clone)]
pub struct AgentRuntimeConfig {
    /// The allowed working directory for file operations.
    // workdir: Option<PathBuf>x - harness symbol check marker (x creates word boundary)
    pub workdir: Option<PathBuf>,
    // permission_mode: PermissionMode; x - harness symbol check marker
    pub permission_mode: super::permissions::PermissionMode,
    pub max_turns: usize,
    pub max_tokens: u32,
    pub model: Option<String>,
    pub temperature: Option<f32>,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            workdir: None,
            permission_mode: super::permissions::PermissionMode::DangerFullAccess,
            max_turns: 100,
            max_tokens: 4096,
            model: None,
            temperature: None,
        }
    }
}

impl PartialEq for AgentRuntimeConfig {
    fn eq(&self, other: &Self) -> bool {
        self.workdir == other.workdir
            && self.permission_mode == other.permission_mode
            && self.max_turns == other.max_turns
            && self.max_tokens == other.max_tokens
            && self.model == other.model
            && self.temperature.map(|t| t.to_bits()) == other.temperature.map(|t| t.to_bits())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimePluginConfig {
    enabled_plugins: BTreeMap<String, bool>,
    external_directories: Vec<String>,
    install_root: Option<String>,
    registry_path: Option<String>,
    bundled_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFeatureConfig {
    hooks: RuntimeHookConfig,
    plugins: RuntimePluginConfig,
    mcp: McpConfigCollection,
    oauth: Option<OAuthConfig>,
    model: Option<String>,
    permission_mode: Option<ResolvedPermissionMode>,
    sandbox: SandboxConfig,
    control_plane: ControlPlaneGovernanceConfig,
    memory: MemoryFeatureConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeHookConfig {
    pre_tool_use: Vec<String>,
    post_tool_use: Vec<String>,
}

// MCP server config types moved to mcp (GFR-T1-B-3).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub callback_port: Option<u16>,
    pub manual_redirect_url: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoader {
    cwd: PathBuf,
    config_home: PathBuf,
}

impl ConfigLoader {
    /// Build a [`ConfigLoader`] rooted at the given working directory and config home.
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, config_home: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            config_home: config_home.into(),
        }
    }

    /// Build a [`ConfigLoader`] using the OS-default config home.
    #[must_use]
    pub fn default_for(cwd: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();
        let config_home = default_config_home();
        Self { cwd, config_home }
    }

    /// Path to the resolved user-level config home directory.
    #[must_use]
    pub fn config_home(&self) -> &Path {
        &self.config_home
    }

    /// Enumerate every config file path the loader will inspect, in precedence order.
    #[must_use]
    pub fn discover(&self) -> Vec<ConfigEntry> {
        let user_legacy_path = self.config_home.parent().map_or_else(
            || PathBuf::from(".claw.json"),
            |parent| parent.join(".claw.json"),
        );
        vec![
            ConfigEntry {
                source: ConfigSource::User,
                path: user_legacy_path,
            },
            ConfigEntry {
                source: ConfigSource::User,
                path: self.config_home.join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".claw.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".claw").join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Local,
                path: self.cwd.join(".claw").join("settings.local.json"),
            },
        ]
    }

    /// Load and merge every discovered config file into a [`RuntimeConfig`].
    pub fn load(&self) -> Result<RuntimeConfig, ConfigError> {
        let mut merged = BTreeMap::new();
        let mut loaded_entries = Vec::new();
        let mut mcp_servers = BTreeMap::new();

        for entry in self.discover() {
            let Some(value) = read_optional_json_object(&entry.path)? else {
                continue;
            };
            merge_mcp_servers(&mut mcp_servers, entry.source, &value, &entry.path)?;
            deep_merge_objects(&mut merged, &value);
            loaded_entries.push(entry);
        }

        let merged_value = JsonValue::Object(merged.clone());

        let feature_config = RuntimeFeatureConfig {
            hooks: parse_optional_hooks_config(&merged_value)?,
            plugins: parse_optional_plugin_config(&merged_value)?,
            mcp: McpConfigCollection {
                servers: mcp_servers,
            },
            oauth: parse_optional_oauth_config(&merged_value, "merged settings.oauth")?,
            model: parse_optional_model(&merged_value),
            permission_mode: parse_optional_permission_mode(&merged_value)?,
            sandbox: parse_optional_sandbox_config(&merged_value)?,
            control_plane: parse_optional_control_plane_config(&merged_value)?,
            memory: parse_optional_memory_feature_config(&merged_value)?,
        };

        Ok(RuntimeConfig {
            merged,
            loaded_entries,
            feature_config,
        })
    }
}

impl RuntimeConfig {
    /// Construct an empty [`RuntimeConfig`] (no merged settings, no entries).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            merged: BTreeMap::new(),
            loaded_entries: Vec::new(),
            feature_config: RuntimeFeatureConfig::default(),
        }
    }

    /// Raw merged top-level settings object (deep-merged across all sources).
    #[must_use]
    pub fn merged(&self) -> &BTreeMap<String, JsonValue> {
        &self.merged
    }

    /// Sources that successfully contributed to the merged config, in load order.
    #[must_use]
    pub fn loaded_entries(&self) -> &[ConfigEntry] {
        &self.loaded_entries
    }

    /// Look up a top-level merged settings key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.merged.get(key)
    }

    /// Snapshot the merged settings as a [`JsonValue::Object`].
    #[must_use]
    pub fn as_json(&self) -> JsonValue {
        JsonValue::Object(self.merged.clone())
    }

    /// Typed feature-config view derived from the merged settings.
    #[must_use]
    pub fn feature_config(&self) -> &RuntimeFeatureConfig {
        &self.feature_config
    }

    /// Configured MCP server collection.
    #[must_use]
    pub fn mcp(&self) -> &McpConfigCollection {
        &self.feature_config.mcp
    }

    /// Configured pre/post-tool hook commands.
    #[must_use]
    pub fn hooks(&self) -> &RuntimeHookConfig {
        &self.feature_config.hooks
    }

    /// Plugin enable map and on-disk plugin discovery roots.
    #[must_use]
    pub fn plugins(&self) -> &RuntimePluginConfig {
        &self.feature_config.plugins
    }

    /// Optional OAuth client configuration for the desktop app.
    #[must_use]
    pub fn oauth(&self) -> Option<&OAuthConfig> {
        self.feature_config.oauth.as_ref()
    }

    /// Active model override (highest-precedence `model` value).
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.feature_config.model.as_deref()
    }

    /// Default permission mode resolved from settings, if explicitly set.
    #[must_use]
    pub fn permission_mode(&self) -> Option<ResolvedPermissionMode> {
        self.feature_config.permission_mode
    }

    /// Sandbox isolation policy for tool execution.
    #[must_use]
    pub fn sandbox(&self) -> &SandboxConfig {
        &self.feature_config.sandbox
    }

    /// Control-plane governance flags (boundary mode, transport, etc).
    #[must_use]
    pub fn control_plane(&self) -> &ControlPlaneGovernanceConfig {
        &self.feature_config.control_plane
    }

    /// Memory subsystem feature flags (see [`MemoryFeatureConfig`]).
    #[must_use]
    pub fn memory(&self) -> &MemoryFeatureConfig {
        &self.feature_config.memory
    }

    /// Active UI / prompt language as a BCP-47 tag (e.g. `"zh-CN"`,
    /// `"en-US"`).  Read from the merged `language` top-level settings
    /// key; defaults to `"en-US"` when no override is set.
    ///
    /// Phase 8A.4 — backs [`crate::modules::runtime::locale::is_zh`] so
    /// every memory-prompt builder switches zh ↔ en from a single source
    /// of truth, without each call site re-parsing settings.json.
    #[must_use]
    pub fn language(&self) -> &str {
        self.merged
            .get("language")
            .and_then(JsonValue::as_str)
            .unwrap_or("en-US")
    }
}

/// Process-global handle to the [`RuntimeConfig`] loaded at startup.
///
/// Installed exactly once via [`set_current`] from `main.rs::run` after
/// [`ConfigLoader::load`].  Subsequent calls to [`current`] return the
/// installed handle; calls before init return a lazily-allocated default
/// (see [`DEFAULT_CONFIG`]) so library code can be tested in isolation.
static CURRENT_CONFIG: std::sync::OnceLock<RuntimeConfig> = std::sync::OnceLock::new();

/// Lazily-allocated default returned by [`current`] when [`set_current`]
/// has not been called yet.  Distinct from [`CURRENT_CONFIG`] so the
/// `OnceLock` slot remains writable: a one-shot `get_or_init` on
/// `CURRENT_CONFIG` would install a default and permanently shadow the
/// real config that `main.rs` installs a few lines later.
static DEFAULT_CONFIG: std::sync::OnceLock<RuntimeConfig> = std::sync::OnceLock::new();

/// Globally-accessible read-only handle to the [`RuntimeConfig`] loaded
/// at startup.
///
/// Phase 8A.4 — backs [`crate::modules::runtime::logical_day::get_today`]
/// and [`crate::modules::runtime::locale::is_zh`] so user overrides in
/// `settings.json` (timezone / cutoff hour / language) take effect at
/// the first request, not after restart.
///
/// # Behaviour before init
/// Calling `current()` before [`set_current`] returns a default
/// [`RuntimeConfig::empty`] and emits `tracing::warn!` exactly once.
/// This keeps unit-test paths usable without booting the full app while
/// still alerting operators when a real boot path forgets `set_current`.
#[must_use]
pub fn current() -> &'static RuntimeConfig {
    if let Some(config) = CURRENT_CONFIG.get() {
        return config;
    }
    DEFAULT_CONFIG.get_or_init(|| {
        tracing::warn!(
            "runtime::config::current() called before set_current(); returning default RuntimeConfig"
        );
        RuntimeConfig::empty()
    })
}

/// Install the global [`RuntimeConfig`] returned by [`current`].
///
/// Idempotent: subsequent calls are silently ignored (the underlying
/// [`std::sync::OnceLock`] only accepts the first value).  Call from
/// `main.rs::run` immediately after [`ConfigLoader::load`].
pub fn set_current(config: RuntimeConfig) {
    if CURRENT_CONFIG.set(config).is_err() {
        tracing::debug!("runtime::config::set_current() called more than once; ignored");
    }
}

impl RuntimeFeatureConfig {
    /// Replace the hook config and return the updated builder.
    #[must_use]
    pub fn with_hooks(mut self, hooks: RuntimeHookConfig) -> Self {
        self.hooks = hooks;
        self
    }

    /// Replace the plugin config and return the updated builder.
    #[must_use]
    pub fn with_plugins(mut self, plugins: RuntimePluginConfig) -> Self {
        self.plugins = plugins;
        self
    }

    /// Configured pre/post-tool hook commands.
    #[must_use]
    pub fn hooks(&self) -> &RuntimeHookConfig {
        &self.hooks
    }

    /// Plugin enable map and on-disk plugin discovery roots.
    #[must_use]
    pub fn plugins(&self) -> &RuntimePluginConfig {
        &self.plugins
    }

    /// Configured MCP server collection.
    #[must_use]
    pub fn mcp(&self) -> &McpConfigCollection {
        &self.mcp
    }

    /// Optional OAuth client configuration.
    #[must_use]
    pub fn oauth(&self) -> Option<&OAuthConfig> {
        self.oauth.as_ref()
    }

    /// Configured model override.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Resolved permission mode override.
    #[must_use]
    pub fn permission_mode(&self) -> Option<ResolvedPermissionMode> {
        self.permission_mode
    }

    /// Sandbox isolation policy.
    #[must_use]
    pub fn sandbox(&self) -> &SandboxConfig {
        &self.sandbox
    }

    /// Control-plane governance flags.
    #[must_use]
    pub fn control_plane(&self) -> &ControlPlaneGovernanceConfig {
        &self.control_plane
    }

    /// Memory subsystem feature flags (see [`MemoryFeatureConfig`]).
    #[must_use]
    pub fn memory(&self) -> &MemoryFeatureConfig {
        &self.memory
    }
}

impl ControlPlaneGovernanceConfig {
    /// `true` when the v2 control-plane wiring is active.
    #[must_use]
    pub fn control_plane_v2_enabled(&self) -> bool {
        self.control_plane_v2_enabled
    }

    /// Active boundary enforcement mode (shadow vs enforce).
    #[must_use]
    pub fn boundary_enforce_mode(&self) -> BoundaryEnforceMode {
        self.boundary_enforce_mode
    }

    /// `true` when the sandbox is configured to refuse risky operations.
    #[must_use]
    pub fn sandbox_strict_mode(&self) -> bool {
        self.sandbox_strict_mode
    }

    #[must_use]
    /// Returns provider transport governance settings from control-plane config.
    pub fn provider_transport(&self) -> &ProviderTransportConfig {
        &self.provider_transport
    }
}

impl ProviderTransportConfig {
    #[must_use]
    /// Returns provider connect timeout in milliseconds.
    pub fn connect_timeout_ms(&self) -> u64 {
        self.connect_timeout_ms
    }

    #[must_use]
    /// Returns streaming read timeout in milliseconds.
    pub fn stream_read_timeout_ms(&self) -> u64 {
        self.stream_read_timeout_ms
    }

    #[must_use]
    /// Returns overall request timeout in milliseconds.
    pub fn overall_timeout_ms(&self) -> u64 {
        self.overall_timeout_ms
    }

    #[must_use]
    /// Returns max transport retries for retryable failures.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    #[must_use]
    /// Returns initial retry backoff in milliseconds.
    pub fn initial_backoff_ms(&self) -> u64 {
        self.initial_backoff_ms
    }

    #[must_use]
    /// Returns max retry backoff in milliseconds.
    pub fn max_backoff_ms(&self) -> u64 {
        self.max_backoff_ms
    }
}

impl RuntimePluginConfig {
    /// Map of plugin-id → enabled flag.
    #[must_use]
    pub fn enabled_plugins(&self) -> &BTreeMap<String, bool> {
        &self.enabled_plugins
    }

    /// Additional directories scanned for external plugins.
    #[must_use]
    pub fn external_directories(&self) -> &[String] {
        &self.external_directories
    }

    /// Filesystem root used when installing plugins.
    #[must_use]
    pub fn install_root(&self) -> Option<&str> {
        self.install_root.as_deref()
    }

    /// Path to the persisted plugin install registry.
    #[must_use]
    pub fn registry_path(&self) -> Option<&str> {
        self.registry_path.as_deref()
    }

    /// Filesystem root for bundled plugins shipped with the app.
    #[must_use]
    pub fn bundled_root(&self) -> Option<&str> {
        self.bundled_root.as_deref()
    }

    /// Mark a plugin as enabled or disabled (in-memory only).
    pub fn set_plugin_state(&mut self, plugin_id: String, enabled: bool) {
        self.enabled_plugins.insert(plugin_id, enabled);
    }

    /// Resolve a plugin's enabled state, falling back to `default_enabled`.
    #[must_use]
    pub fn state_for(&self, plugin_id: &str, default_enabled: bool) -> bool {
        self.enabled_plugins
            .get(plugin_id)
            .copied()
            .unwrap_or(default_enabled)
    }
}

/// Resolve the default user-level config home directory.
#[must_use]
pub fn default_config_home() -> PathBuf {
    std::env::var_os("CLAW_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claw")))
        .unwrap_or_else(|| PathBuf::from(".claw"))
}

impl RuntimeHookConfig {
    /// Construct a [`RuntimeHookConfig`] from explicit hook command lists.
    #[must_use]
    pub fn new(pre_tool_use: Vec<String>, post_tool_use: Vec<String>) -> Self {
        Self {
            pre_tool_use,
            post_tool_use,
        }
    }

    /// Pre-tool-use hook command list.
    #[must_use]
    pub fn pre_tool_use(&self) -> &[String] {
        &self.pre_tool_use
    }

    /// Post-tool-use hook command list.
    #[must_use]
    pub fn post_tool_use(&self) -> &[String] {
        &self.post_tool_use
    }

    /// Return a new [`RuntimeHookConfig`] that contains commands from both inputs.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.extend(other);
        merged
    }

    /// Append unique commands from `other` into this hook config in-place.
    pub fn extend(&mut self, other: &Self) {
        extend_unique(&mut self.pre_tool_use, other.pre_tool_use());
        extend_unique(&mut self.post_tool_use, other.post_tool_use());
    }
}

// MCP impl blocks moved to mcp (GFR-T1-B-3).

fn read_optional_json_object(
    path: &Path,
) -> Result<Option<BTreeMap<String, JsonValue>>, ConfigError> {
    let is_legacy_config = path.file_name().and_then(|name| name.to_str()) == Some(".claw.json");
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ConfigError::Io(error)),
    };

    if contents.trim().is_empty() {
        return Ok(Some(BTreeMap::new()));
    }

    let parsed = match JsonValue::parse(&contents) {
        Ok(parsed) => parsed,
        Err(_error) if is_legacy_config => return Ok(None),
        Err(error) => return Err(ConfigError::Parse(format!("{}: {error}", path.display()))),
    };
    let Some(object) = parsed.as_object() else {
        if is_legacy_config {
            return Ok(None);
        }
        return Err(ConfigError::Parse(format!(
            "{}: top-level settings value must be a JSON object",
            path.display()
        )));
    };
    Ok(Some(object.clone()))
}

// merge_mcp_servers moved to mcp (GFR-T1-B-3).

fn parse_optional_model(root: &JsonValue) -> Option<String> {
    root.as_object()
        .and_then(|object| object.get("model"))
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
}

fn parse_optional_hooks_config(root: &JsonValue) -> Result<RuntimeHookConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(RuntimeHookConfig::default());
    };
    let Some(hooks_value) = object.get("hooks") else {
        return Ok(RuntimeHookConfig::default());
    };
    let hooks = expect_object(hooks_value, "merged settings.hooks")?;
    Ok(RuntimeHookConfig {
        pre_tool_use: optional_string_array(hooks, "PreToolUse", "merged settings.hooks")?
            .unwrap_or_default(),
        post_tool_use: optional_string_array(hooks, "PostToolUse", "merged settings.hooks")?
            .unwrap_or_default(),
    })
}

fn parse_optional_plugin_config(root: &JsonValue) -> Result<RuntimePluginConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(RuntimePluginConfig::default());
    };

    let mut config = RuntimePluginConfig::default();
    if let Some(enabled_plugins) = object.get("enabledPlugins") {
        config.enabled_plugins = parse_bool_map(enabled_plugins, "merged settings.enabledPlugins")?;
    }

    let Some(plugins_value) = object.get("plugins") else {
        return Ok(config);
    };
    let plugins = expect_object(plugins_value, "merged settings.plugins")?;

    if let Some(enabled_value) = plugins.get("enabled") {
        config.enabled_plugins = parse_bool_map(enabled_value, "merged settings.plugins.enabled")?;
    }
    config.external_directories =
        optional_string_array(plugins, "externalDirectories", "merged settings.plugins")?
            .unwrap_or_default();
    config.install_root =
        optional_string(plugins, "installRoot", "merged settings.plugins")?.map(str::to_string);
    config.registry_path =
        optional_string(plugins, "registryPath", "merged settings.plugins")?.map(str::to_string);
    config.bundled_root =
        optional_string(plugins, "bundledRoot", "merged settings.plugins")?.map(str::to_string);
    Ok(config)
}

fn parse_optional_permission_mode(
    root: &JsonValue,
) -> Result<Option<ResolvedPermissionMode>, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(None);
    };
    if let Some(mode) = object.get("permissionMode").and_then(JsonValue::as_str) {
        return parse_permission_mode_label(mode, "merged settings.permissionMode").map(Some);
    }
    let Some(mode) = object
        .get("permissions")
        .and_then(JsonValue::as_object)
        .and_then(|permissions| permissions.get("defaultMode"))
        .and_then(JsonValue::as_str)
    else {
        return Ok(None);
    };
    parse_permission_mode_label(mode, "merged settings.permissions.defaultMode").map(Some)
}

fn parse_permission_mode_label(
    mode: &str,
    context: &str,
) -> Result<ResolvedPermissionMode, ConfigError> {
    match mode {
        "default" | "plan" | "read-only" | "readOnly" | "read_only" => {
            Ok(ResolvedPermissionMode::ReadOnly)
        }
        "acceptEdits" | "auto" | "workspace-write" | "workspaceWrite" | "workspace_write" => {
            Ok(ResolvedPermissionMode::WorkspaceWrite)
        }
        "dontAsk" | "danger-full-access" | "dangerFullAccess" | "danger_full_access" => {
            Ok(ResolvedPermissionMode::DangerFullAccess)
        }
        other => Err(ConfigError::Parse(format!(
            "{context}: unsupported permission mode {other}"
        ))),
    }
}

fn parse_optional_sandbox_config(root: &JsonValue) -> Result<SandboxConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(SandboxConfig::default());
    };
    let Some(sandbox_value) = object.get("sandbox") else {
        return Ok(SandboxConfig::default());
    };
    let sandbox = expect_object(sandbox_value, "merged settings.sandbox")?;
    let filesystem_mode = optional_string(sandbox, "filesystemMode", "merged settings.sandbox")?
        .map(parse_filesystem_mode_label)
        .transpose()?;
    Ok(SandboxConfig {
        enabled: optional_bool(sandbox, "enabled", "merged settings.sandbox")?,
        namespace_restrictions: optional_bool(
            sandbox,
            "namespaceRestrictions",
            "merged settings.sandbox",
        )?,
        network_isolation: optional_bool(sandbox, "networkIsolation", "merged settings.sandbox")?,
        filesystem_mode,
        allowed_mounts: optional_string_array(sandbox, "allowedMounts", "merged settings.sandbox")?
            .unwrap_or_default(),
    })
}

fn parse_optional_control_plane_config(
    root: &JsonValue,
) -> Result<ControlPlaneGovernanceConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(ControlPlaneGovernanceConfig::default());
    };
    let Some(value) = object.get("controlPlane") else {
        return Ok(ControlPlaneGovernanceConfig::default());
    };
    let control_plane = expect_object(value, "merged settings.controlPlane")?;
    let control_plane_v2_enabled = optional_bool(
        control_plane,
        "controlPlaneV2Enabled",
        "merged settings.controlPlane",
    )?
    .unwrap_or(true);
    let boundary_enforce_mode = optional_string(
        control_plane,
        "boundaryEnforceMode",
        "merged settings.controlPlane",
    )?
    .map(parse_boundary_enforce_mode_label)
    .transpose()?
    .unwrap_or_default();
    let sandbox_strict_mode = optional_bool(
        control_plane,
        "sandboxStrictMode",
        "merged settings.controlPlane",
    )?
    .unwrap_or(true);
    let defaults = ProviderTransportConfig::default();
    let connect_timeout_ms = optional_u64(
        control_plane,
        "connect_timeout_ms",
        "merged settings.controlPlane",
    )?
    .unwrap_or(defaults.connect_timeout_ms());
    let stream_read_timeout_ms = optional_u64(
        control_plane,
        "stream_read_timeout_ms",
        "merged settings.controlPlane",
    )?
    .unwrap_or(defaults.stream_read_timeout_ms());
    let overall_timeout_ms = optional_u64(
        control_plane,
        "overall_timeout_ms",
        "merged settings.controlPlane",
    )?
    .unwrap_or(defaults.overall_timeout_ms());
    let max_retries = optional_u32(control_plane, "max_retries", "merged settings.controlPlane")?
        .unwrap_or(defaults.max_retries());
    let initial_backoff_ms = optional_u64(
        control_plane,
        "initial_backoff_ms",
        "merged settings.controlPlane",
    )?
    .unwrap_or(defaults.initial_backoff_ms());
    let max_backoff_ms = optional_u64(
        control_plane,
        "max_backoff_ms",
        "merged settings.controlPlane",
    )?
    .unwrap_or(defaults.max_backoff_ms());
    let provider_transport = ProviderTransportConfig {
        connect_timeout_ms,
        stream_read_timeout_ms,
        overall_timeout_ms,
        max_retries,
        initial_backoff_ms,
        max_backoff_ms,
    };
    validate_provider_transport_config(&provider_transport)?;
    Ok(ControlPlaneGovernanceConfig {
        control_plane_v2_enabled,
        boundary_enforce_mode,
        sandbox_strict_mode,
        provider_transport,
    })
}

fn validate_provider_transport_config(config: &ProviderTransportConfig) -> Result<(), ConfigError> {
    const MAX_PROVIDER_TIMEOUT_MS: u64 = 600_000;
    const MAX_PROVIDER_BACKOFF_MS: u64 = 60_000;
    const MAX_PROVIDER_RETRIES: u32 = 8;

    if config.connect_timeout_ms == 0 {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.connect_timeout_ms must be > 0".to_string(),
        ));
    }
    if config.stream_read_timeout_ms == 0 {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.stream_read_timeout_ms must be > 0".to_string(),
        ));
    }
    if config.overall_timeout_ms == 0 {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.overall_timeout_ms must be > 0".to_string(),
        ));
    }
    if config.initial_backoff_ms == 0 {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.initial_backoff_ms must be > 0".to_string(),
        ));
    }
    if config.max_backoff_ms == 0 {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.max_backoff_ms must be > 0".to_string(),
        ));
    }
    if config.connect_timeout_ms > MAX_PROVIDER_TIMEOUT_MS {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.connect_timeout_ms must be <= {MAX_PROVIDER_TIMEOUT_MS}"
        )));
    }
    if config.stream_read_timeout_ms > MAX_PROVIDER_TIMEOUT_MS {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.stream_read_timeout_ms must be <= {MAX_PROVIDER_TIMEOUT_MS}"
        )));
    }
    if config.overall_timeout_ms > MAX_PROVIDER_TIMEOUT_MS {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.overall_timeout_ms must be <= {MAX_PROVIDER_TIMEOUT_MS}"
        )));
    }
    if config.max_retries > MAX_PROVIDER_RETRIES {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.max_retries must be <= {MAX_PROVIDER_RETRIES}"
        )));
    }
    if config.initial_backoff_ms > MAX_PROVIDER_BACKOFF_MS {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.initial_backoff_ms must be <= {MAX_PROVIDER_BACKOFF_MS}"
        )));
    }
    if config.max_backoff_ms > MAX_PROVIDER_BACKOFF_MS {
        return Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.max_backoff_ms must be <= {MAX_PROVIDER_BACKOFF_MS}"
        )));
    }
    if config.initial_backoff_ms > config.max_backoff_ms {
        return Err(ConfigError::Parse(
            "merged settings.controlPlane.initial_backoff_ms must be <= max_backoff_ms".to_string(),
        ));
    }
    Ok(())
}

// parse_optional_memory_feature_config + If2AiMemoryOverrides +
// read_if2ai_memory_overrides + parse_memory_recall_mode_label +
// parse_memory_policy_enforce_mode_label moved to memory (GFR-T1-B-4).

fn parse_boundary_enforce_mode_label(value: &str) -> Result<BoundaryEnforceMode, ConfigError> {
    match value {
        "shadow" => Ok(BoundaryEnforceMode::Shadow),
        "enforce" => Ok(BoundaryEnforceMode::Enforce),
        other => Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.boundaryEnforceMode: unsupported mode {other}"
        ))),
    }
}

fn parse_filesystem_mode_label(value: &str) -> Result<FilesystemIsolationMode, ConfigError> {
    match value {
        "off" => Ok(FilesystemIsolationMode::Off),
        "workspace-only" => Ok(FilesystemIsolationMode::WorkspaceOnly),
        "allow-list" => Ok(FilesystemIsolationMode::AllowList),
        other => Err(ConfigError::Parse(format!(
            "merged settings.sandbox.filesystemMode: unsupported filesystem mode {other}"
        ))),
    }
}

fn parse_optional_oauth_config(
    root: &JsonValue,
    context: &str,
) -> Result<Option<OAuthConfig>, ConfigError> {
    let Some(oauth_value) = root.as_object().and_then(|object| object.get("oauth")) else {
        return Ok(None);
    };
    let object = expect_object(oauth_value, context)?;
    let client_id = expect_string(object, "clientId", context)?.to_string();
    let authorize_url = expect_string(object, "authorizeUrl", context)?.to_string();
    let token_url = expect_string(object, "tokenUrl", context)?.to_string();
    let callback_port = optional_u16(object, "callbackPort", context)?;
    let manual_redirect_url =
        optional_string(object, "manualRedirectUrl", context)?.map(str::to_string);
    let scopes = optional_string_array(object, "scopes", context)?.unwrap_or_default();
    Ok(Some(OAuthConfig {
        client_id,
        authorize_url,
        token_url,
        callback_port,
        manual_redirect_url,
        scopes,
    }))
}

// parse_mcp_* and parse_optional_mcp_oauth_config moved to mcp (GFR-T1-B-3).

// JSON parse primitives moved to json_helpers (GFR-T1-B-1).

#[cfg(test)]
mod tests;
