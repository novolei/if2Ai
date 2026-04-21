#![allow(dead_code)]

mod json_helpers;
mod mcp;

use mcp::merge_mcp_servers;
#[allow(unused_imports)]
pub use mcp::{
    McpConfigCollection, McpManagedProxyServerConfig, McpOAuthConfig, McpRemoteServerConfig,
    McpSdkServerConfig, McpServerConfig, McpStdioServerConfig, McpTransport,
    McpWebSocketServerConfig, ScopedMcpServerConfig,
};

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Memory recall strategy (Memory Control Plane v1 feature flag).
///
/// - `Lexical` — keyword / FTS only (legacy fallback).
/// - `Hybrid` — lexical + semantic vector search via `VectorMemoryProvider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryRecallMode {
    Lexical,
    #[default]
    Hybrid,
}

impl MemoryRecallMode {
    /// Wire-format label used in settings JSON and audit logs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Hybrid => "hybrid",
        }
    }
}

/// Memory policy enforcement mode (Memory Control Plane v1 feature flag).
///
/// - `Shadow` — `MemoryPolicyEngine` *evaluates* writes and emits audit
///   events but never blocks the underlying `store()` call.
/// - `Enforce` — `Deny` decisions block writes (returns `Err` to the caller).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryPolicyEnforceMode {
    #[default]
    Shadow,
    Enforce,
}

impl MemoryPolicyEnforceMode {
    /// Wire-format label used in settings JSON and audit logs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Enforce => "enforce",
        }
    }
}

/// Phase 8B.1 — `MemoryCompiler` runtime configuration (Sprint 2 / T-C1).
///
/// All fields use `usize` / `u64` / `u32` so [`CompilerConfig`] cleanly
/// derives `Eq`, which in turn keeps the parent
/// [`MemoryFeatureConfig: Eq`] derive intact.
///
/// The struct is read from `~/.if2ai/memory_config.json` under the
/// `compiler` key (snake_case, per v2 §0.5 Δ-9 — complex structures
/// live in `memory_config.json`, not the camelCase claw `settings.json`).
/// Defaults match the openhanako baseline — see
/// `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
/// §Sprint 2 / T-C1.
///
/// Schema:
/// ```jsonc
/// {
///   "compiler": {
///     "today_max_chars":            500,
///     "week_max_chars":             500,
///     "longterm_max_chars":         300,
///     "facts_max_chars":            200,
///     "daily_check_interval_secs":  3600,
///     "max_concurrent_llm":         3,
///     "max_retries":                3
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerConfig {
    /// Maximum char budget for the LLM-rendered `today.md` body.
    pub today_max_chars: usize,
    /// Maximum char budget for the LLM-rendered `week.md` body.
    pub week_max_chars: usize,
    /// Maximum char budget for the LLM-rendered `longterm.md` body.
    pub longterm_max_chars: usize,
    /// Maximum char budget for the LLM-rendered `facts.md` body.
    pub facts_max_chars: usize,
    /// Backup `tokio::time::interval` period for the ticker's
    /// `do_daily` watchdog (seconds).  Default `3600` (one hour).
    pub daily_check_interval_secs: u64,
    /// Maximum concurrent in-flight `UtilityLlm::complete` calls
    /// dispatched from the four `compile_*` pipelines.
    pub max_concurrent_llm: usize,
    /// Per-job retry budget passed through to
    /// [`crate::modules::memory::JobRunner`] for compile jobs.
    pub max_retries: u32,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            today_max_chars: 500,
            week_max_chars: 500,
            longterm_max_chars: 300,
            facts_max_chars: 200,
            daily_check_interval_secs: 3600,
            max_concurrent_llm: 3,
            max_retries: 3,
        }
    }
}

/// Aggregated memory subsystem feature flags (Memory Control Plane v1).
///
/// Read from `settings.json` under the `memory` key.  Defaults preserve
/// shipping behaviour (control plane on, hybrid recall, shadow policy) so
/// existing installs keep working without configuration changes.
///
/// Schema:
/// ```jsonc
/// {
///   "memory": {
///     "controlPlaneV1Enabled": true,
///     "recallMode": "hybrid",          // "lexical" | "hybrid"
///     "policyEnforceMode": "shadow"    // "shadow"  | "enforce"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryFeatureConfig {
    control_plane_v1_enabled: bool,
    recall_mode: MemoryRecallMode,
    policy_enforce_mode: MemoryPolicyEnforceMode,
    // Phase 8A.3 — IANA timezone name (e.g. `"Asia/Shanghai"`,
    // `"America/New_York"`).  `None` means "no user override" → callers
    // (see `runtime::logical_day::resolve_timezone`) fall back to
    // `chrono_tz::UTC`.  Parse failures also fall back to UTC with a
    // `tracing::warn!` so a typo can never panic the runtime.
    timezone: Option<String>,
    // Phase 8A.3 — Logical-day cutoff hour in LOCAL `timezone` (0-23).
    // Default `4` — a 04:00 boundary keeps "I worked till 03:00 last
    // night" rolled into yesterday's daily aggregations rather than
    // fragmenting a single late-night session across two days.
    logical_day_cutoff_hour: u8,
    // Phase 8A.11 — Master toggle for system-prompt memory injection
    // (pinned + compiled memory + usage rules).  Default `true`.  When
    // `false`, [`crate::modules::memory::build_memory_injection`] is
    // skipped at the call site and the system prompt contains no
    // pinned/compiled sections (per v2 §0.5 Δ-9).
    inject_to_prompt: bool,
    // Phase 8A.11 — Token budget cap for the assembled
    // [`crate::modules::memory::MemoryInjection`] payload (default
    // `2000`).  `u32` (not `usize`) to keep
    // `MemoryFeatureConfig: Eq` — call sites cast to `usize` when
    // forwarding to [`crate::modules::memory::build_memory_injection`].
    max_inject_tokens: u32,
    // Phase 8B.1 — `MemoryCompiler` knobs (T-C1).  All `usize`/`u64`/
    // `u32` so the parent `Eq` derive holds.  Source-of-truth is
    // `~/.if2ai/memory_config.json::compiler` per v2 §0.5 Δ-9.
    compiler: CompilerConfig,
}

impl Default for MemoryFeatureConfig {
    fn default() -> Self {
        Self {
            control_plane_v1_enabled: true,
            recall_mode: MemoryRecallMode::default(),
            policy_enforce_mode: MemoryPolicyEnforceMode::default(),
            timezone: None,
            logical_day_cutoff_hour: 4,
            inject_to_prompt: true,
            max_inject_tokens: 2000,
            compiler: CompilerConfig::default(),
        }
    }
}

impl MemoryFeatureConfig {
    /// `true` when the Memory Control Plane v1 wiring (scope resolver,
    /// policy engine, audit emitter) is active.  When `false` the legacy
    /// scope-less store/recall paths are used.
    #[must_use]
    pub fn control_plane_v1_enabled(&self) -> bool {
        self.control_plane_v1_enabled
    }

    /// Active recall strategy.
    #[must_use]
    pub fn recall_mode(&self) -> MemoryRecallMode {
        self.recall_mode
    }

    /// Active policy enforcement mode.
    #[must_use]
    pub fn policy_enforce_mode(&self) -> MemoryPolicyEnforceMode {
        self.policy_enforce_mode
    }

    /// User-configured IANA timezone name (e.g. `"Asia/Shanghai"`),
    /// or `None` when no override is set.  Parsed by
    /// [`crate::modules::runtime::logical_day::resolve_timezone`] which
    /// falls back to `chrono_tz::UTC` on any failure.
    #[must_use]
    pub fn timezone(&self) -> Option<&str> {
        self.timezone.as_deref()
    }

    /// Logical-day cutoff hour in LOCAL `timezone` (0-23, default `4`).
    /// Used by every daily-aggregation memory pipeline (compile_today,
    /// compile_week, diary writer) so the "day boundary" is consistent
    /// across modules — see
    /// [`crate::modules::runtime::logical_day`].
    #[must_use]
    pub fn logical_day_cutoff_hour(&self) -> u8 {
        self.logical_day_cutoff_hour
    }

    /// Phase 8A.11 — Whether the system prompt should include the
    /// pinned + compiled + rules memory sections produced by
    /// [`crate::modules::memory::build_memory_injection`].  Default
    /// `true`.  Surfaces in the Memory Settings UI as the "Inject
    /// memory into prompt" master toggle (per v2 §0.5 Δ-9).
    #[must_use]
    pub fn inject_to_prompt(&self) -> bool {
        self.inject_to_prompt
    }

    /// Phase 8A.11 — Token budget cap for the memory-injection payload
    /// (default `2000`).  Callers convert this to a char budget via
    /// [`crate::modules::memory::CHARS_PER_TOKEN_ESTIMATE`] before
    /// calling [`crate::modules::memory::build_memory_injection`].
    #[must_use]
    pub fn max_inject_tokens(&self) -> u32 {
        self.max_inject_tokens
    }

    /// Phase 8B.1 — Active [`CompilerConfig`] for the
    /// [`crate::modules::memory::MemoryCompiler`].  Source-of-truth is
    /// `~/.if2ai/memory_config.json::compiler` (per v2 §0.5 Δ-9 — complex
    /// structures live in the if2Ai memory config file, not the
    /// camelCase claw `settings.json`).
    #[must_use]
    pub fn compiler(&self) -> &CompilerConfig {
        &self.compiler
    }
}

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

fn parse_optional_memory_feature_config(
    root: &JsonValue,
) -> Result<MemoryFeatureConfig, ConfigError> {
    // Start from defaults so missing keys at every layer behave deterministically.
    let mut config = MemoryFeatureConfig::default();

    // Layer 1: settings.json `memory.*` keys (camelCase).  This is the
    // historical claw-cli source of truth and keeps backward compatibility.
    if let Some(object) = root.as_object() {
        if let Some(value) = object.get("memory") {
            let memory = expect_object(value, "merged settings.memory")?;
            if let Some(flag) =
                optional_bool(memory, "controlPlaneV1Enabled", "merged settings.memory")?
            {
                config.control_plane_v1_enabled = flag;
            }
            if let Some(label) = optional_string(memory, "recallMode", "merged settings.memory")? {
                config.recall_mode = parse_memory_recall_mode_label(label)?;
            }
            if let Some(label) =
                optional_string(memory, "policyEnforceMode", "merged settings.memory")?
            {
                config.policy_enforce_mode = parse_memory_policy_enforce_mode_label(label)?;
            }
            // Phase 8A.3 — `memory.timezone` (IANA name) and
            // `memory.logicalDayCutoffHour` (0-23) flow through here so
            // both `settings.json` and `memory_config.json` agree.
            if let Some(tz) = optional_string(memory, "timezone", "merged settings.memory")? {
                config.timezone = Some(tz.to_string());
            }
            if let Some(hour) =
                optional_u8(memory, "logicalDayCutoffHour", "merged settings.memory")?
            {
                config.logical_day_cutoff_hour = hour;
            }
            // Phase 8A.11 — `memory.injectToPrompt` (bool) and
            // `memory.maxInjectTokens` (u32) keep settings.json in
            // sync with the Memory Settings UI overrides below.
            if let Some(flag) = optional_bool(memory, "injectToPrompt", "merged settings.memory")? {
                config.inject_to_prompt = flag;
            }
            if let Some(tokens) = optional_u32(memory, "maxInjectTokens", "merged settings.memory")?
            {
                config.max_inject_tokens = tokens;
            }
        }
    }

    // Layer 2 (overrides Layer 1): the if2Ai Memory Settings UI writes to
    // `~/.if2ai/memory_config.json` using snake_case keys.  Overlay any
    // user-set feature flags here so the UI is the immediate source of truth.
    if let Some(overrides) = read_if2ai_memory_overrides() {
        if let Some(flag) = overrides.control_plane_v1_enabled {
            config.control_plane_v1_enabled = flag;
        }
        if let Some(mode) = overrides.recall_mode {
            config.recall_mode = mode;
        }
        if let Some(mode) = overrides.policy_enforce_mode {
            config.policy_enforce_mode = mode;
        }
        if let Some(tz) = overrides.timezone {
            config.timezone = Some(tz);
        }
        if let Some(hour) = overrides.logical_day_cutoff_hour {
            config.logical_day_cutoff_hour = hour;
        }
        if let Some(flag) = overrides.inject_to_prompt {
            config.inject_to_prompt = flag;
        }
        if let Some(tokens) = overrides.max_inject_tokens {
            config.max_inject_tokens = tokens;
        }
        if let Some(compiler) = overrides.compiler {
            config.compiler = compiler;
        }
    }

    Ok(config)
}

/// Subset of `~/.if2ai/memory_config.json` that the runtime reads to discover
/// user-driven feature-flag overrides written by the Memory Settings UI.
///
/// Only the three feature-flag fields are extracted; token-budget percentages
/// remain owned by the legacy `ContextBudget`/`BudgetConfig` path so this
/// loader stays additive and backward-compatible.
#[derive(Debug, Default, Deserialize)]
struct If2AiMemoryOverrides {
    #[serde(default)]
    control_plane_v1_enabled: Option<bool>,
    #[serde(default)]
    recall_mode: Option<MemoryRecallMode>,
    #[serde(default)]
    policy_enforce_mode: Option<MemoryPolicyEnforceMode>,
    /// Phase 8A.3 — IANA timezone name override (e.g. `"Asia/Shanghai"`).
    #[serde(default)]
    timezone: Option<String>,
    /// Phase 8A.3 — Logical-day cutoff hour override (0-23).
    #[serde(default)]
    logical_day_cutoff_hour: Option<u8>,
    /// Phase 8A.11 — Master toggle for system-prompt memory injection.
    #[serde(default)]
    inject_to_prompt: Option<bool>,
    /// Phase 8A.11 — Token budget cap for memory injection payload.
    #[serde(default)]
    max_inject_tokens: Option<u32>,
    /// Phase 8B.1 — `MemoryCompiler` knobs (T-C1).  Per v2 §0.5 Δ-9
    /// the entire `compiler.*` block lives in `memory_config.json`
    /// (not the claw `settings.json` camelCase tree) so the if2Ai
    /// Memory Settings UI is the single source of truth.
    #[serde(default)]
    compiler: Option<CompilerConfig>,
}

fn read_if2ai_memory_overrides() -> Option<If2AiMemoryOverrides> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn parse_memory_recall_mode_label(value: &str) -> Result<MemoryRecallMode, ConfigError> {
    match value {
        "lexical" => Ok(MemoryRecallMode::Lexical),
        "hybrid" => Ok(MemoryRecallMode::Hybrid),
        other => Err(ConfigError::Parse(format!(
            "merged settings.memory.recallMode: unsupported mode {other}"
        ))),
    }
}

fn parse_memory_policy_enforce_mode_label(
    value: &str,
) -> Result<MemoryPolicyEnforceMode, ConfigError> {
    match value {
        "shadow" => Ok(MemoryPolicyEnforceMode::Shadow),
        "enforce" => Ok(MemoryPolicyEnforceMode::Enforce),
        other => Err(ConfigError::Parse(format!(
            "merged settings.memory.policyEnforceMode: unsupported mode {other}"
        ))),
    }
}

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
mod tests {
    use crate::modules::runtime::config::{
        BoundaryEnforceMode, CompilerConfig, ConfigLoader, ConfigSource, McpServerConfig,
        McpTransport, MemoryPolicyEnforceMode, MemoryRecallMode, ResolvedPermissionMode,
        CLAW_SETTINGS_SCHEMA_NAME,
    };
    use crate::modules::runtime::json::JsonValue;
    use crate::modules::runtime::sandbox::FilesystemIsolationMode;
    use std::fs;
    use uuid::Uuid;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("runtime-config-{}", Uuid::new_v4()))
    }

    #[test]
    fn rejects_non_object_settings_files() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(&home).expect("home config dir");
        fs::create_dir_all(&cwd).expect("project dir");
        fs::write(home.join("settings.json"), "[]").expect("write bad settings");

        let error = ConfigLoader::new(&cwd, &home)
            .load()
            .expect_err("config should fail");
        assert!(error
            .to_string()
            .contains("top-level settings value must be a JSON object"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn loads_and_merges_claw_code_config_files_by_precedence() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            home.parent().expect("home parent").join(".claw.json"),
            r#"{"model":"haiku","env":{"A":"1"},"mcpServers":{"home":{"command":"uvx","args":["home"]}}}"#,
        )
        .expect("write user compat config");
        fs::write(
            home.join("settings.json"),
            r#"{"model":"sonnet","env":{"A2":"1"},"hooks":{"PreToolUse":["base"]},"permissions":{"defaultMode":"plan"}}"#,
        )
        .expect("write user settings");
        fs::write(
            cwd.join(".claw.json"),
            r#"{"model":"project-compat","env":{"B":"2"}}"#,
        )
        .expect("write project compat config");
        fs::write(
            cwd.join(".claw").join("settings.json"),
            r#"{"env":{"C":"3"},"hooks":{"PostToolUse":["project"]},"mcpServers":{"project":{"command":"uvx","args":["project"]}}}"#,
        )
        .expect("write project settings");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{"model":"opus","permissionMode":"acceptEdits"}"#,
        )
        .expect("write local settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        assert_eq!(CLAW_SETTINGS_SCHEMA_NAME, "SettingsSchema");
        assert_eq!(loaded.loaded_entries().len(), 5);
        assert_eq!(loaded.loaded_entries()[0].source, ConfigSource::User);
        assert_eq!(
            loaded.get("model"),
            Some(&JsonValue::String("opus".to_string()))
        );
        assert_eq!(loaded.model(), Some("opus"));
        assert_eq!(
            loaded.permission_mode(),
            Some(ResolvedPermissionMode::WorkspaceWrite)
        );
        assert_eq!(
            loaded
                .get("env")
                .and_then(JsonValue::as_object)
                .expect("env object")
                .len(),
            4
        );
        assert!(loaded
            .get("hooks")
            .and_then(JsonValue::as_object)
            .expect("hooks object")
            .contains_key("PreToolUse"));
        assert!(loaded
            .get("hooks")
            .and_then(JsonValue::as_object)
            .expect("hooks object")
            .contains_key("PostToolUse"));
        assert_eq!(loaded.hooks().pre_tool_use(), &["base".to_string()]);
        assert_eq!(loaded.hooks().post_tool_use(), &["project".to_string()]);
        assert!(loaded.mcp().get("home").is_some());
        assert!(loaded.mcp().get("project").is_some());

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_sandbox_config() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "sandbox": {
                "enabled": true,
                "namespaceRestrictions": false,
                "networkIsolation": true,
                "filesystemMode": "allow-list",
                "allowedMounts": ["logs", "tmp/cache"]
              }
            }"#,
        )
        .expect("write local settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        assert_eq!(loaded.sandbox().enabled, Some(true));
        assert_eq!(loaded.sandbox().namespace_restrictions, Some(false));
        assert_eq!(loaded.sandbox().network_isolation, Some(true));
        assert_eq!(
            loaded.sandbox().filesystem_mode,
            Some(FilesystemIsolationMode::AllowList)
        );
        assert_eq!(loaded.sandbox().allowed_mounts, vec!["logs", "tmp/cache"]);

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_typed_mcp_and_oauth_config() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            home.join("settings.json"),
            r#"{
              "mcpServers": {
                "stdio-server": {
                  "command": "uvx",
                  "args": ["mcp-server"],
                  "env": {"TOKEN": "secret"}
                },
                "remote-server": {
                  "type": "http",
                  "url": "https://example.test/mcp",
                  "headers": {"Authorization": "Bearer token"},
                  "headersHelper": "helper.sh",
                  "oauth": {
                    "clientId": "mcp-client",
                    "callbackPort": 7777,
                    "authServerMetadataUrl": "https://issuer.test/.well-known/oauth-authorization-server",
                    "xaa": true
                  }
                }
              },
              "oauth": {
                "clientId": "runtime-client",
                "authorizeUrl": "https://console.test/oauth/authorize",
                "tokenUrl": "https://console.test/oauth/token",
                "callbackPort": 54545,
                "manualRedirectUrl": "https://console.test/oauth/callback",
                "scopes": ["org:read", "user:write"]
              }
            }"#,
        )
        .expect("write user settings");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "mcpServers": {
                "remote-server": {
                  "type": "ws",
                  "url": "wss://override.test/mcp",
                  "headers": {"X-Env": "local"}
                }
              }
            }"#,
        )
        .expect("write local settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        let stdio_server = loaded
            .mcp()
            .get("stdio-server")
            .expect("stdio server should exist");
        assert_eq!(stdio_server.scope, ConfigSource::User);
        assert_eq!(stdio_server.transport(), McpTransport::Stdio);

        let remote_server = loaded
            .mcp()
            .get("remote-server")
            .expect("remote server should exist");
        assert_eq!(remote_server.scope, ConfigSource::Local);
        assert_eq!(remote_server.transport(), McpTransport::Ws);
        match &remote_server.config {
            McpServerConfig::Ws(config) => {
                assert_eq!(config.url, "wss://override.test/mcp");
                assert_eq!(
                    config.headers.get("X-Env").map(String::as_str),
                    Some("local")
                );
            }
            other => panic!("expected ws config, got {other:?}"),
        }

        let oauth = loaded.oauth().expect("oauth config should exist");
        assert_eq!(oauth.client_id, "runtime-client");
        assert_eq!(oauth.callback_port, Some(54_545));
        assert_eq!(oauth.scopes, vec!["org:read", "user:write"]);

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_plugin_config_from_enabled_plugins() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            home.join("settings.json"),
            r#"{
              "enabledPlugins": {
                "tool-guard@builtin": true,
                "sample-plugin@external": false
              }
            }"#,
        )
        .expect("write user settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        assert_eq!(
            loaded.plugins().enabled_plugins().get("tool-guard@builtin"),
            Some(&true)
        );
        assert_eq!(
            loaded
                .plugins()
                .enabled_plugins()
                .get("sample-plugin@external"),
            Some(&false)
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_plugin_config() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            home.join("settings.json"),
            r#"{
              "enabledPlugins": {
                "core-helpers@builtin": true
              },
              "plugins": {
                "externalDirectories": ["./external-plugins"],
                "installRoot": "plugin-cache/installed",
                "registryPath": "plugin-cache/installed.json",
                "bundledRoot": "./bundled-plugins"
              }
            }"#,
        )
        .expect("write plugin settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        assert_eq!(
            loaded
                .plugins()
                .enabled_plugins()
                .get("core-helpers@builtin"),
            Some(&true)
        );
        assert_eq!(
            loaded.plugins().external_directories(),
            &["./external-plugins".to_string()]
        );
        assert_eq!(
            loaded.plugins().install_root(),
            Some("plugin-cache/installed")
        );
        assert_eq!(
            loaded.plugins().registry_path(),
            Some("plugin-cache/installed.json")
        );
        assert_eq!(loaded.plugins().bundled_root(), Some("./bundled-plugins"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn rejects_invalid_mcp_server_shapes() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(&home).expect("home config dir");
        fs::create_dir_all(&cwd).expect("project dir");
        fs::write(
            home.join("settings.json"),
            r#"{"mcpServers":{"broken":{"type":"http","url":123}}}"#,
        )
        .expect("write broken settings");

        let error = ConfigLoader::new(&cwd, &home)
            .load()
            .expect_err("config should fail");
        assert!(error
            .to_string()
            .contains("mcpServers.broken: missing string field url"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_desktop_permission_mode_aliases() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{"permissionMode":"workspaceWrite"}"#,
        )
        .expect("write local settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert_eq!(
            loaded.permission_mode(),
            Some(ResolvedPermissionMode::WorkspaceWrite)
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_control_plane_release_flags() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "controlPlane": {
                "controlPlaneV2Enabled": false,
                "boundaryEnforceMode": "shadow",
                "sandboxStrictMode": false,
                "connect_timeout_ms": 4100,
                "stream_read_timeout_ms": 46000,
                "overall_timeout_ms": 91000,
                "max_retries": 3,
                "initial_backoff_ms": 300,
                "max_backoff_ms": 2400
              }
            }"#,
        )
        .expect("write control plane settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert!(!loaded.control_plane().control_plane_v2_enabled());
        assert_eq!(
            loaded.control_plane().boundary_enforce_mode(),
            BoundaryEnforceMode::Shadow
        );
        assert!(!loaded.control_plane().sandbox_strict_mode());
        assert_eq!(
            loaded
                .control_plane()
                .provider_transport()
                .connect_timeout_ms(),
            4100
        );
        assert_eq!(
            loaded
                .control_plane()
                .provider_transport()
                .stream_read_timeout_ms(),
            46_000
        );
        assert_eq!(
            loaded
                .control_plane()
                .provider_transport()
                .overall_timeout_ms(),
            91_000
        );
        assert_eq!(loaded.control_plane().provider_transport().max_retries(), 3);
        assert_eq!(
            loaded
                .control_plane()
                .provider_transport()
                .initial_backoff_ms(),
            300
        );
        assert_eq!(
            loaded.control_plane().provider_transport().max_backoff_ms(),
            2400
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_memory_feature_flags_with_defaults() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        // No `memory` key anywhere → all three flags use shipping defaults.
        assert!(loaded.memory().control_plane_v1_enabled());
        assert_eq!(loaded.memory().recall_mode(), MemoryRecallMode::Hybrid);
        assert_eq!(
            loaded.memory().policy_enforce_mode(),
            MemoryPolicyEnforceMode::Shadow
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_memory_feature_flags_overrides() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");

        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "memory": {
                "controlPlaneV1Enabled": false,
                "recallMode": "lexical",
                "policyEnforceMode": "enforce"
              }
            }"#,
        )
        .expect("write memory settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");

        assert!(!loaded.memory().control_plane_v1_enabled());
        assert_eq!(loaded.memory().recall_mode(), MemoryRecallMode::Lexical);
        assert_eq!(
            loaded.memory().policy_enforce_mode(),
            MemoryPolicyEnforceMode::Enforce
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_logical_day_overrides_from_settings() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "memory": {
                "timezone": "Asia/Shanghai",
                "logicalDayCutoffHour": 6
              }
            }"#,
        )
        .expect("write memory tz settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert_eq!(loaded.memory().timezone(), Some("Asia/Shanghai"));
        assert_eq!(loaded.memory().logical_day_cutoff_hour(), 6);
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn defaults_logical_day_cutoff_to_4_and_tz_to_none() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert_eq!(loaded.memory().timezone(), None);
        assert_eq!(loaded.memory().logical_day_cutoff_hour(), 4);
        // Phase 8A.11 — memory injection defaults.
        assert!(loaded.memory().inject_to_prompt());
        assert_eq!(loaded.memory().max_inject_tokens(), 2000);
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn parses_memory_inject_overrides_from_settings_json() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "memory": {
                "injectToPrompt": false,
                "maxInjectTokens": 512
              }
            }"#,
        )
        .expect("write memory inject settings");

        let loaded = ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert!(!loaded.memory().inject_to_prompt());
        assert_eq!(loaded.memory().max_inject_tokens(), 512);
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn rejects_invalid_memory_recall_mode_label() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{"memory":{"recallMode":"telepathy"}}"#,
        )
        .expect("write bad memory settings");

        let error = ConfigLoader::new(&cwd, &home)
            .load()
            .expect_err("config should reject");
        assert!(error.to_string().contains("memory.recallMode"));
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn runtime_config_language_default_is_en_us() {
        // Empty config → no `language` key → default `"en-US"`.
        let cfg = super::RuntimeConfig::empty();
        assert_eq!(cfg.language(), "en-US");
    }

    #[test]
    fn runtime_config_language_reads_top_level_key() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{"language":"zh-CN"}"#,
        )
        .expect("write language settings");
        let loaded = super::ConfigLoader::new(&cwd, &home)
            .load()
            .expect("config should load");
        assert_eq!(loaded.language(), "zh-CN");
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn current_returns_default_when_unset_and_set_current_installs_value() {
        // CURRENT_CONFIG is a process-global OnceLock; this test exercises
        // both the pre-init default branch and the set-once branch.  Run
        // serially (cargo's default within the same binary) so other
        // tests do not race on the slot.
        let before = super::current();
        // Default config has empty merged map → language() falls back to "en-US".
        assert_eq!(before.language(), "en-US");

        let mut merged = std::collections::BTreeMap::new();
        merged.insert(
            "language".to_string(),
            super::JsonValue::String("zh-CN".to_string()),
        );
        let configured = super::RuntimeConfig {
            merged,
            loaded_entries: Vec::new(),
            feature_config: super::RuntimeFeatureConfig::default(),
        };
        super::set_current(configured);
        let after = super::current();
        assert_eq!(after.language(), "zh-CN");

        // Idempotent: a 2nd set is silently ignored, value does not change.
        let mut other_merged = std::collections::BTreeMap::new();
        other_merged.insert(
            "language".to_string(),
            super::JsonValue::String("ja-JP".to_string()),
        );
        super::set_current(super::RuntimeConfig {
            merged: other_merged,
            loaded_entries: Vec::new(),
            feature_config: super::RuntimeFeatureConfig::default(),
        });
        assert_eq!(super::current().language(), "zh-CN");
    }

    #[test]
    fn rejects_invalid_provider_transport_config() {
        let root = temp_dir();
        let cwd = root.join("project");
        let home = root.join("home").join(".claw");
        fs::create_dir_all(cwd.join(".claw")).expect("project config dir");
        fs::create_dir_all(&home).expect("home config dir");
        fs::write(
            cwd.join(".claw").join("settings.local.json"),
            r#"{
              "controlPlane": {
                "connect_timeout_ms": 0,
                "stream_read_timeout_ms": 5000,
                "overall_timeout_ms": 10000,
                "initial_backoff_ms": 300,
                "max_backoff_ms": 200
              }
            }"#,
        )
        .expect("write invalid control plane settings");

        let error = ConfigLoader::new(&cwd, &home)
            .load()
            .expect_err("config should reject");
        let message = error.to_string();
        assert!(
            message.contains("connect_timeout_ms must be > 0")
                || message.contains("initial_backoff_ms must be <= max_backoff_ms")
        );
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    // ─────────────────────────────────────────────────────────────
    // Phase 8B.1 — `CompilerConfig` defaults + override parsing.
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn compiler_config_default_values() {
        // Sprint 2 / T-C1 baseline; these match the openhanako port
        // referenced from the design doc and are also surfaced by the
        // `MemoryFeatureConfig::default` shipping path.
        let cfg = CompilerConfig::default();
        assert_eq!(cfg.today_max_chars, 500);
        assert_eq!(cfg.week_max_chars, 500);
        assert_eq!(cfg.longterm_max_chars, 300);
        assert_eq!(cfg.facts_max_chars, 200);
        assert_eq!(cfg.daily_check_interval_secs, 3600);
        assert_eq!(cfg.max_concurrent_llm, 3);
        assert_eq!(cfg.max_retries, 3);
    }

    #[test]
    fn memory_feature_config_exposes_compiler_defaults() {
        // The accessor surface that `main.rs` uses to construct the
        // `MemoryCompiler` — guarantees the defaults flow through.
        let mem = super::MemoryFeatureConfig::default();
        assert_eq!(mem.compiler(), &CompilerConfig::default());
    }

    #[test]
    fn memory_compiler_overrides_parses_from_memory_config_json() {
        // Per v2 §0.5 Δ-9 the `compiler.*` block lives in
        // `~/.if2ai/memory_config.json` (snake_case) — verify the
        // overrides struct accepts the schema we'll document for users.
        let raw = r#"{
            "compiler": {
                "today_max_chars": 800,
                "week_max_chars": 700,
                "longterm_max_chars": 400,
                "facts_max_chars": 250,
                "daily_check_interval_secs": 1800,
                "max_concurrent_llm": 5,
                "max_retries": 4
            }
        }"#;
        let parsed: super::If2AiMemoryOverrides =
            serde_json::from_str(raw).expect("overrides parse");
        let compiler = parsed.compiler.expect("compiler block present");
        assert_eq!(compiler.today_max_chars, 800);
        assert_eq!(compiler.week_max_chars, 700);
        assert_eq!(compiler.longterm_max_chars, 400);
        assert_eq!(compiler.facts_max_chars, 250);
        assert_eq!(compiler.daily_check_interval_secs, 1800);
        assert_eq!(compiler.max_concurrent_llm, 5);
        assert_eq!(compiler.max_retries, 4);
    }

    #[test]
    fn memory_compiler_overrides_absent_compiler_keeps_defaults() {
        // When the file exists but has no `compiler` key the loader
        // must leave `MemoryFeatureConfig::default().compiler` intact.
        let raw = r#"{"recall_mode": "hybrid"}"#;
        let parsed: super::If2AiMemoryOverrides =
            serde_json::from_str(raw).expect("overrides parse");
        assert!(parsed.compiler.is_none());
    }
}
