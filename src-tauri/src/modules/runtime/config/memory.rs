//! Memory subsystem configuration: feature flags + compiler tuning.
//!
//! Extracted from `runtime/config/mod.rs` in GFR-T1-B-4 (pure
//! structural move; struct/enum fields and function bodies
//! byte-identical).

use serde::{Deserialize, Serialize};

use super::super::json::JsonValue;
use super::json_helpers::{
    expect_object, optional_bool, optional_string, optional_u32, optional_u8,
};
use super::ConfigError;

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
    // MEM-MOD-P4 — Mem0-style memory update decision tree.  When `true`,
    // `memory_store` consults a small utility-LLM classifier before
    // persisting a new fact and may turn the write into NOOP / UPDATE /
    // DELETE instead of always ADD.  Default `true`: the prompt now
    // includes specificity-vs-coverage rules that prevent false NOOPs.
    decision_tree_enabled: bool,
    // Whether to run conflict detection when storing a new memory.
    // Independent of the decision tree — checks for contradictions
    // between the incoming fact and existing memories.  Default `true`
    // (the feature is stable enough to be on by default).
    conflict_detection_enabled: bool,
    // MEM-AUTO-EXTRACT — When `true`, the after-turn pipeline uses a
    // utility LLM to extract memorable personal facts from user messages
    // even when the agent did not explicitly call `memory_store`.  This
    // is a supplementary path that runs alongside the existing
    // tool-call-based extraction.  Default `true`.
    auto_extract_enabled: bool,
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
            decision_tree_enabled: true,
            conflict_detection_enabled: true,
            auto_extract_enabled: true,
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

    /// MEM-MOD-P4 — `true` when `memory_store` is allowed to consult
    /// the Mem0-style update decision tree before persisting.  Default
    /// `true` — the prompt includes specificity-vs-coverage rules that
    /// prevent false NOOPs so every `memory_store` call is classified.
    #[must_use]
    pub fn decision_tree_enabled(&self) -> bool {
        self.decision_tree_enabled
    }

    /// Whether conflict detection is enabled for new memory writes.
    /// When `true`, `memory_store` checks incoming facts against existing
    /// memories for contradictions before persisting.  Default `true`.
    #[must_use]
    pub fn conflict_detection_enabled(&self) -> bool {
        self.conflict_detection_enabled
    }

    /// MEM-AUTO-EXTRACT — `true` when the after-turn pipeline should use
    /// a utility LLM to extract memorable personal facts from user
    /// messages, even if the agent did not call `memory_store`.
    /// Default `true`.
    #[must_use]
    pub fn auto_extract_enabled(&self) -> bool {
        self.auto_extract_enabled
    }
}

pub(super) fn parse_optional_memory_feature_config(
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
        if let Some(flag) = overrides.decision_tree_enabled {
            config.decision_tree_enabled = flag;
        }
        if let Some(flag) = overrides.conflict_detection_enabled {
            config.conflict_detection_enabled = flag;
        }
        if let Some(flag) = overrides.auto_extract_enabled {
            config.auto_extract_enabled = flag;
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
pub(super) struct If2AiMemoryOverrides {
    #[serde(default)]
    pub(super) control_plane_v1_enabled: Option<bool>,
    #[serde(default)]
    pub(super) recall_mode: Option<MemoryRecallMode>,
    #[serde(default)]
    pub(super) policy_enforce_mode: Option<MemoryPolicyEnforceMode>,
    /// Phase 8A.3 — IANA timezone name override (e.g. `"Asia/Shanghai"`).
    #[serde(default)]
    pub(super) timezone: Option<String>,
    /// Phase 8A.3 — Logical-day cutoff hour override (0-23).
    #[serde(default)]
    pub(super) logical_day_cutoff_hour: Option<u8>,
    /// Phase 8A.11 — Master toggle for system-prompt memory injection.
    #[serde(default)]
    pub(super) inject_to_prompt: Option<bool>,
    /// Phase 8A.11 — Token budget cap for memory injection payload.
    #[serde(default)]
    pub(super) max_inject_tokens: Option<u32>,
    /// Phase 8B.1 — `MemoryCompiler` knobs (T-C1).  Per v2 §0.5 Δ-9
    /// the entire `compiler.*` block lives in `memory_config.json`
    /// (not the claw `settings.json` camelCase tree) so the if2Ai
    /// Memory Settings UI is the single source of truth.
    #[serde(default)]
    pub(super) compiler: Option<CompilerConfig>,
    /// MEM-MOD-P4 — Mem0-style update decision tree feature flag.
    #[serde(default)]
    pub(super) decision_tree_enabled: Option<bool>,
    /// Whether conflict detection runs when storing a new memory.
    #[serde(default)]
    pub(super) conflict_detection_enabled: Option<bool>,
    /// MEM-AUTO-EXTRACT — toggle LLM-based user-message extraction.
    #[serde(default)]
    pub(super) auto_extract_enabled: Option<bool>,
}

pub(super) fn read_if2ai_memory_overrides() -> Option<If2AiMemoryOverrides> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub(super) fn parse_memory_recall_mode_label(value: &str) -> Result<MemoryRecallMode, ConfigError> {
    match value {
        "lexical" => Ok(MemoryRecallMode::Lexical),
        "hybrid" => Ok(MemoryRecallMode::Hybrid),
        other => Err(ConfigError::Parse(format!(
            "merged settings.memory.recallMode: unsupported mode {other}"
        ))),
    }
}

pub(super) fn parse_memory_policy_enforce_mode_label(
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
