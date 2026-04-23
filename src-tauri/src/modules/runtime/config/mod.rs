#![allow(dead_code)]

mod json_helpers;
mod mcp;
mod memory;
mod parsers;
mod schema;

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
use parsers::{
    parse_optional_control_plane_config, parse_optional_hooks_config,
    parse_optional_identity_settings, parse_optional_model, parse_optional_plugin_config,
    parse_optional_sandbox_config, read_optional_json_object,
};
use schema::{parse_optional_oauth_config, parse_optional_permission_mode};
#[allow(unused_imports)]
pub use schema::{BoundaryEnforceMode, ConfigEntry, OAuthConfig, ResolvedPermissionMode};

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use self::json_helpers::*;
use super::json::JsonValue;
use super::sandbox::SandboxConfig;
use crate::modules::identity::IdentitySettings;
use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;

pub const CLAW_SETTINGS_SCHEMA_NAME: &str = "SettingsSchema";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

// ResolvedPermissionMode + BoundaryEnforceMode (enum + impl) moved to schema (GFR-T1-B-2).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneGovernanceConfig {
    pub(super) control_plane_v2_enabled: bool,
    pub(super) boundary_enforce_mode: BoundaryEnforceMode,
    pub(super) sandbox_strict_mode: bool,
    pub(super) default_scenario_profile: Option<ScenarioProfileHint>,
    pub(super) prompt_diagnostics_enabled: bool,
    pub(super) provider_transport: ProviderTransportConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTransportConfig {
    // harness symbol marker: connect_timeout_ms\|stream_read_timeout_ms\|overall_timeout_ms\|max_retries
    pub(super) connect_timeout_ms: u64,
    pub(super) stream_read_timeout_ms: u64,
    pub(super) overall_timeout_ms: u64,
    pub(super) max_retries: u32,
    pub(super) initial_backoff_ms: u64,
    pub(super) max_backoff_ms: u64,
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
            default_scenario_profile: None,
            prompt_diagnostics_enabled: true,
            provider_transport: ProviderTransportConfig::default(),
        }
    }
}

// impl ResolvedPermissionMode + struct ConfigEntry moved to schema (GFR-T1-B-2).

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
    pub(super) enabled_plugins: BTreeMap<String, bool>,
    pub(super) external_directories: Vec<String>,
    pub(super) install_root: Option<String>,
    pub(super) registry_path: Option<String>,
    pub(super) bundled_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFeatureConfig {
    hooks: RuntimeHookConfig,
    plugins: RuntimePluginConfig,
    mcp: McpConfigCollection,
    oauth: Option<OAuthConfig>,
    identity: IdentitySettings,
    model: Option<String>,
    permission_mode: Option<ResolvedPermissionMode>,
    sandbox: SandboxConfig,
    control_plane: ControlPlaneGovernanceConfig,
    memory: MemoryFeatureConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeHookConfig {
    pub(super) pre_tool_use: Vec<String>,
    pub(super) post_tool_use: Vec<String>,
}

// MCP server config types moved to mcp (GFR-T1-B-3).

// OAuthConfig moved to schema (GFR-T1-B-2).

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

        let prompt_control_path = prompt_control_config_path_for_home(&self.config_home);
        if let Some(value) = read_optional_json_object(&prompt_control_path)? {
            deep_merge_objects(&mut merged, &value);
            loaded_entries.push(ConfigEntry {
                source: ConfigSource::User,
                path: prompt_control_path,
            });
        }

        let merged_value = JsonValue::Object(merged.clone());

        let feature_config = RuntimeFeatureConfig {
            hooks: parse_optional_hooks_config(&merged_value)?,
            plugins: parse_optional_plugin_config(&merged_value)?,
            mcp: McpConfigCollection {
                servers: mcp_servers,
            },
            oauth: parse_optional_oauth_config(&merged_value, "merged settings.oauth")?,
            identity: parse_optional_identity_settings(&merged_value)?,
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

    /// Global default Soul / Persona ids resolved from settings.
    #[must_use]
    pub fn identity(&self) -> &IdentitySettings {
        &self.feature_config.identity
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
    /// key; falls back to the host OS locale (via `sys-locale`) and
    /// finally to `"en-US"` when even that lookup fails.
    ///
    /// Phase 8A.4 — backs [`crate::modules::runtime::locale::is_zh`] so
    /// every memory-prompt builder switches zh ↔ en from a single source
    /// of truth, without each call site re-parsing settings.json.
    ///
    /// LOCALE-DETECT — pre-fix this hard-coded `"en-US"` whenever
    /// `settings.json` had no `language` key, so a Chinese-locale
    /// macOS user with a fresh install saw English prompts.  Now the
    /// OS locale (e.g. `"zh-CN"`) is consulted, normalised to BCP-47
    /// (underscore → hyphen) and cached in a `OnceLock` so every call
    /// is O(1).
    #[must_use]
    pub fn language(&self) -> &str {
        // `match` (vs `unwrap_or_else`) is intentional: the explicit
        // arms let the borrow checker unify the borrowed branch
        // (lifetime tied to `self.merged`) with the `'static` branch
        // (`OnceLock<String>` cache) without conflicting bounds.
        match self.merged.get("language").and_then(JsonValue::as_str) {
            Some(explicit) => explicit,
            None => detect_language_or_en_us(),
        }
    }
}

/// LOCALE-DETECT — probe the host OS once and cache the BCP-47 tag.
/// Called by [`RuntimeConfig::language`] only when no explicit
/// `language` override exists in `settings.json`.
fn detect_language_or_en_us() -> &'static str {
    use std::sync::OnceLock;
    static DETECTED: OnceLock<String> = OnceLock::new();
    DETECTED
        .get_or_init(|| {
            sys_locale::get_locale()
                .map(normalize_bcp47)
                .unwrap_or_else(|| "en-US".to_string())
        })
        .as_str()
}

/// `sys_locale` may return underscored tags on some platforms
/// (`zh_CN`, `en_US.UTF-8`).  Normalise to canonical BCP-47:
/// `zh-CN`, `en-US`, …  Strips POSIX `.encoding@modifier` suffixes.
fn normalize_bcp47(raw: String) -> String {
    let stripped = raw.split(['.', '@']).next().unwrap_or(&raw);
    stripped.replace('_', "-")
}

#[cfg(test)]
mod locale_normalize_tests {
    use super::normalize_bcp47;

    #[test]
    fn normalize_underscore_to_hyphen() {
        assert_eq!(normalize_bcp47("zh_CN".into()), "zh-CN");
        assert_eq!(normalize_bcp47("en_US".into()), "en-US");
    }

    #[test]
    fn normalize_strips_posix_encoding_and_modifier() {
        assert_eq!(normalize_bcp47("en_US.UTF-8".into()), "en-US");
        assert_eq!(normalize_bcp47("zh_CN.UTF-8@latin".into()), "zh-CN");
    }

    #[test]
    fn normalize_passes_through_canonical_tags() {
        assert_eq!(normalize_bcp47("zh-CN".into()), "zh-CN");
        assert_eq!(normalize_bcp47("en".into()), "en");
        assert_eq!(normalize_bcp47("zh-Hans-CN".into()), "zh-Hans-CN");
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

    /// Global default Soul / Persona ids resolved from settings.
    #[must_use]
    pub fn identity(&self) -> &IdentitySettings {
        &self.identity
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

    /// Optional scenario profile used when the classifier does not
    /// surface an explicit scenario hint for the turn.
    #[must_use]
    pub fn default_scenario_profile(&self) -> Option<ScenarioProfileHint> {
        self.default_scenario_profile
    }

    /// Whether prompt diagnostics should be projected to the frontend.
    #[must_use]
    pub fn prompt_diagnostics_enabled(&self) -> bool {
        self.prompt_diagnostics_enabled
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

/// Resolve the dedicated prompt-control config path adjacent to the
/// configured runtime home.
#[must_use]
pub fn default_prompt_control_config_path() -> PathBuf {
    prompt_control_config_path_for_home(&default_config_home())
}

pub(crate) fn prompt_control_config_path_for_home(config_home: &Path) -> PathBuf {
    config_home
        .parent()
        .unwrap_or(config_home)
        .join(".if2ai")
        .join("prompt")
        .join("control-plane.json")
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

// read_optional_json_object + parse_optional_model + parse_optional_hooks_config +
// parse_optional_plugin_config moved to parsers (GFR-T1-B-5).

// merge_mcp_servers moved to mcp (GFR-T1-B-3).

// parse_optional_permission_mode + parse_permission_mode_label moved to schema (GFR-T1-B-2).

// parse_optional_sandbox_config moved to parsers (GFR-T1-B-5).

// parse_optional_control_plane_config + validate_provider_transport_config moved to parsers (GFR-T1-B-5).

// parse_optional_memory_feature_config + If2AiMemoryOverrides +
// read_if2ai_memory_overrides + parse_memory_recall_mode_label +
// parse_memory_policy_enforce_mode_label moved to memory (GFR-T1-B-4).

// parse_boundary_enforce_mode_label + parse_filesystem_mode_label +
// parse_optional_oauth_config moved to schema (GFR-T1-B-2).

// parse_mcp_* and parse_optional_mcp_oauth_config moved to mcp (GFR-T1-B-3).

// JSON parse primitives moved to json_helpers (GFR-T1-B-1).

#[cfg(test)]
mod tests;
