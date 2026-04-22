//! Settings Tauri commands.
//!
//! Provides configuration for the memory subsystem and prompt control plane.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::State;

use crate::commands::AppState;
use crate::modules::identity::{
    apply_identity_customization_pack, normalize_identity_customization_pack,
    read_identity_customization_pack, write_identity_customization_pack, CustomPersonaDefinition,
    IdentityCustomizationPack, IdentityRegistry, PersonaCustomization, SoulCustomization,
};
use crate::modules::runtime::config::{default_prompt_control_config_path, ConfigLoader};
use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;

/// Memory recall mode — selects between lexical-only and hybrid (vector +
/// FTS + episodic) retrieval pipelines.  Mirrors the Rust runtime
/// `MemoryRecallMode` enum so the frontend can drive the same feature flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryRecallModeSetting {
    Lexical,
    #[default]
    Hybrid,
}

/// Memory write policy enforce mode — `shadow` audits decisions without
/// blocking, `enforce` rejects denied writes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryPolicyEnforceModeSetting {
    #[default]
    Shadow,
    Enforce,
}

/// User-selectable default scenario profile for prompt control.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PromptScenarioProfileSetting {
    Chat,
    Coding,
    Research,
    Planning,
    Review,
}

impl From<PromptScenarioProfileSetting> for ScenarioProfileHint {
    fn from(value: PromptScenarioProfileSetting) -> Self {
        match value {
            PromptScenarioProfileSetting::Chat => Self::Chat,
            PromptScenarioProfileSetting::Coding => Self::Coding,
            PromptScenarioProfileSetting::Research => Self::Research,
            PromptScenarioProfileSetting::Planning => Self::Planning,
            PromptScenarioProfileSetting::Review => Self::Review,
        }
    }
}

impl From<ScenarioProfileHint> for PromptScenarioProfileSetting {
    fn from(value: ScenarioProfileHint) -> Self {
        match value {
            ScenarioProfileHint::Chat => Self::Chat,
            ScenarioProfileHint::Coding => Self::Coding,
            ScenarioProfileHint::Research => Self::Research,
            ScenarioProfileHint::Planning => Self::Planning,
            ScenarioProfileHint::Review => Self::Review,
        }
    }
}

/// Memory configuration returned by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Total token budget (default 4000)
    pub total_tokens: usize,
    /// System prompt slot percentage (0-100)
    pub system_pct: u8,
    /// Episodic memory slot percentage (0-100)
    pub episodic_pct: u8,
    /// Semantic memory slot percentage (0-100)
    pub semantic_pct: u8,
    /// Working memory slot percentage (0-100)
    pub working_pct: u8,
    /// Number of trajectories recorded
    pub trajectory_count: usize,
    /// Memory Control Plane V1 — master kill-switch for the new memory pipeline.
    pub control_plane_v1_enabled: bool,
    /// Recall mode (`lexical` keeps the legacy SQL search; `hybrid` enables
    /// vector + FTS + episodic fusion).
    pub recall_mode: MemoryRecallModeSetting,
    /// Policy enforcement mode (`shadow` audits only, `enforce` blocks
    /// denied writes).
    pub policy_enforce_mode: MemoryPolicyEnforceModeSetting,
    /// Promotion thresholds — surfaced so the Memory Settings UI can tune
    /// when the background scanner recommends `session→project` and
    /// `project→global` upgrades.  See
    /// [`crate::modules::memory::promotion::PromotionThresholds`].
    pub promotion: crate::modules::memory::promotion::PromotionThresholds,
    /// MEM-MOD-PD0 — IANA timezone name (e.g. `"Asia/Shanghai"`) used by
    /// the logical-day pipeline. `None` means "no override" → falls back
    /// to UTC. Surfaced so the Memory Settings UI can let the user pick
    /// their local zone without hand-editing JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// MEM-MOD-PD0 — Logical-day cutoff hour in LOCAL `timezone`
    /// (0-23, default `4`). A 04:00 boundary keeps "I worked till 03:00
    /// last night" rolled into yesterday's daily aggregations.
    pub logical_day_cutoff_hour: u8,
    /// MEM-MOD-PD0 (B-fix) — Read-only. Detected host OS timezone so
    /// the Memory Settings UI can render "留空 = 跟随系统 (Asia/Shanghai)"
    /// instead of the misleading "= UTC". `None` only when the OS probe
    /// failed (rare), in which case the UI falls back to "UTC" copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_os_timezone: Option<String>,
}

/// Configuration to persist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfigInput {
    pub total_tokens: usize,
    pub system_pct: u8,
    pub episodic_pct: u8,
    pub semantic_pct: u8,
    pub working_pct: u8,
    /// Optional so older clients that don't ship feature-flag UI keep working.
    #[serde(default)]
    pub control_plane_v1_enabled: Option<bool>,
    #[serde(default)]
    pub recall_mode: Option<MemoryRecallModeSetting>,
    #[serde(default)]
    pub policy_enforce_mode: Option<MemoryPolicyEnforceModeSetting>,
    /// Optional so older clients without the promotion-tuning UI keep
    /// working; backend falls back to
    /// [`crate::modules::memory::promotion::PromotionThresholds::default`].
    #[serde(default)]
    pub promotion: Option<crate::modules::memory::promotion::PromotionThresholds>,
    /// MEM-MOD-PD0 — IANA timezone override.  `None` means "do not
    /// touch the persisted value" so older clients that don't ship the
    /// PD0 UI keep their existing zone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// MEM-MOD-PD0 — Logical-day cutoff hour override (0-23).
    #[serde(default)]
    pub logical_day_cutoff_hour: Option<u8>,
}

/// Structured prompt control settings exposed to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptControlSettings {
    /// `None` means auto mode — let request intelligence pick the scenario.
    pub default_scenario_profile: Option<PromptScenarioProfileSetting>,
    /// Selected global Soul id for prompt identity resolution.
    pub default_soul_id: Option<String>,
    /// Selected global Persona id for prompt identity resolution.
    pub default_persona_id: Option<String>,
    /// Optional global agent name; persists across all personas.
    pub agent_name: Option<String>,
    /// Optional name the user wants the agent to call them.
    pub user_name: Option<String>,
    /// Whether prompt diagnostics summaries are emitted to the frontend.
    pub prompt_diagnostics_enabled: bool,
}

/// Persisted prompt control settings input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptControlSettingsInput {
    #[serde(default)]
    pub default_scenario_profile: Option<PromptScenarioProfileSetting>,
    #[serde(default)]
    pub default_soul_id: Option<String>,
    #[serde(default)]
    pub default_persona_id: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default = "default_prompt_diagnostics_enabled")]
    pub prompt_diagnostics_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptControlSoulOption {
    pub id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub mission: String,
    pub core_principles: Vec<String>,
    pub decision_contract: String,
    pub non_negotiables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptControlPersonaOption {
    pub id: String,
    pub soul_id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub tone_rules: Vec<String>,
    pub collaboration_rules: Vec<String>,
    pub output_preferences: Vec<String>,
    /// Avatar id for user-defined personas (matches the bundled
    /// portrait library on the frontend). `None` for built-in personas
    /// — frontend falls back to lookup by persona id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptControlCatalog {
    pub souls: Vec<PromptControlSoulOption>,
    pub personas: Vec<PromptControlPersonaOption>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityCustomizationPackDto {
    #[serde(default)]
    pub souls: BTreeMap<String, SoulCustomization>,
    #[serde(default)]
    pub personas: BTreeMap<String, PersonaCustomization>,
    #[serde(default)]
    pub custom_personas: BTreeMap<String, CustomPersonaDefinition>,
}

fn default_prompt_diagnostics_enabled() -> bool {
    true
}

fn read_persisted_config() -> Option<MemoryConfigInput> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_persisted_config(cfg: &MemoryConfigInput) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let dir = std::path::Path::new(&home).join(".if2ai");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("memory_config.json");
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn read_prompt_control_settings_file() -> Result<Option<Value>, String> {
    let path = default_prompt_control_config_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn write_prompt_control_settings_file(
    input: &PromptControlSettingsInput,
) -> Result<PromptControlSettings, String> {
    let path = default_prompt_control_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut root = match read_prompt_control_settings_file()? {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err(format!("{} must contain a JSON object", path.display()));
        }
        None => Map::new(),
    };

    let mut control_plane = match root.remove("controlPlane") {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err(format!(
                "{} controlPlane must contain a JSON object",
                path.display()
            ));
        }
        None => Map::new(),
    };

    match input.default_scenario_profile {
        Some(profile) => {
            let value = serde_json::to_value(profile).map_err(|e| e.to_string())?;
            control_plane.insert("defaultScenarioProfile".to_string(), value);
        }
        None => {
            control_plane.remove("defaultScenarioProfile");
        }
    }
    control_plane.insert(
        "promptDiagnosticsEnabled".to_string(),
        Value::Bool(input.prompt_diagnostics_enabled),
    );

    let mut identity = match root.remove("identity") {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err(format!(
                "{} identity must contain a JSON object",
                path.display()
            ));
        }
        None => Map::new(),
    };
    match input.default_soul_id.as_deref() {
        Some(soul_id) if !soul_id.is_empty() => {
            identity.insert(
                "defaultSoulId".to_string(),
                Value::String(soul_id.to_string()),
            );
        }
        _ => {
            identity.remove("defaultSoulId");
        }
    }
    match input.default_persona_id.as_deref() {
        Some(persona_id) if !persona_id.is_empty() => {
            identity.insert(
                "defaultPersonaId".to_string(),
                Value::String(persona_id.to_string()),
            );
        }
        _ => {
            identity.remove("defaultPersonaId");
        }
    }
    match input.agent_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => {
            identity.insert("agentName".to_string(), Value::String(name.to_string()));
        }
        _ => {
            identity.remove("agentName");
        }
    }
    match input.user_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => {
            identity.insert("userName".to_string(), Value::String(name.to_string()));
        }
        _ => {
            identity.remove("userName");
        }
    }

    root.insert("controlPlane".to_string(), Value::Object(control_plane));
    root.insert("identity".to_string(), Value::Object(identity));
    let json = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    Ok(PromptControlSettings {
        default_scenario_profile: input.default_scenario_profile,
        default_soul_id: input.default_soul_id.clone(),
        default_persona_id: input.default_persona_id.clone(),
        agent_name: input
            .agent_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string),
        user_name: input
            .user_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string),
        prompt_diagnostics_enabled: input.prompt_diagnostics_enabled,
    })
}

fn count_trajectories() -> usize {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home).join(".if2ai/trajectories");
    match std::fs::read_dir(&path) {
        Ok(entries) => entries
            .filter(|e| {
                e.as_ref()
                    .map(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
                    .unwrap_or(false)
            })
            .count(),
        Err(_) => 0,
    }
}

/// Get the current memory configuration.
///
/// Resolves feature-flag values from the persisted config first, falling
/// back to safe defaults (`control_plane_v1_enabled = true`, recall =
/// `Hybrid`, policy = `Shadow`) when older configs are read.
#[tauri::command]
pub fn get_memory_config(state: State<'_, AppState>) -> MemoryConfig {
    let persisted = read_persisted_config();
    let budget = &state.context_budget;

    let config = persisted.unwrap_or(MemoryConfigInput {
        total_tokens: budget.total,
        system_pct: (budget.system_pct * 100.0) as u8,
        episodic_pct: (budget.episodic_pct * 100.0) as u8,
        semantic_pct: (budget.semantic_pct * 100.0) as u8,
        working_pct: (budget.working_pct * 100.0) as u8,
        control_plane_v1_enabled: None,
        recall_mode: None,
        policy_enforce_mode: None,
        promotion: None,
        timezone: None,
        logical_day_cutoff_hour: None,
    });

    let memory_runtime = crate::modules::runtime::config::current().memory();
    let detected_os_timezone =
        crate::modules::runtime::logical_day::detect_os_timezone().map(|tz| tz.name().to_string());
    MemoryConfig {
        total_tokens: config.total_tokens,
        system_pct: config.system_pct,
        episodic_pct: config.episodic_pct,
        semantic_pct: config.semantic_pct,
        working_pct: config.working_pct,
        trajectory_count: count_trajectories(),
        control_plane_v1_enabled: config.control_plane_v1_enabled.unwrap_or(true),
        recall_mode: config.recall_mode.unwrap_or_default(),
        policy_enforce_mode: config.policy_enforce_mode.unwrap_or_default(),
        promotion: config.promotion.unwrap_or_default(),
        timezone: config
            .timezone
            .clone()
            .or_else(|| memory_runtime.timezone().map(str::to_string)),
        logical_day_cutoff_hour: config
            .logical_day_cutoff_hour
            .unwrap_or_else(|| memory_runtime.logical_day_cutoff_hour()),
        detected_os_timezone,
    }
}

/// Save memory configuration.
#[tauri::command]
pub fn set_memory_config(
    _state: State<'_, AppState>,
    config: MemoryConfigInput,
) -> Result<MemoryConfig, String> {
    let total: u16 = config.system_pct as u16
        + config.episodic_pct as u16
        + config.semantic_pct as u16
        + config.working_pct as u16;
    if total != 100 {
        return Err(format!("Slot percentages must sum to 100%, got {}%", total));
    }

    // Surface validation errors instead of silently writing a malformed
    // promotion config — Memory Settings UI relies on the error string to
    // highlight the bad field.
    if let Some(promo) = &config.promotion {
        promo
            .validate()
            .map_err(|msg| format!("promotion thresholds invalid: {msg}"))?;
    }

    if let Some(hour) = config.logical_day_cutoff_hour {
        if hour > 23 {
            return Err(format!(
                "logical_day_cutoff_hour must be in 0..=23, got {hour}"
            ));
        }
    }
    if let Some(tz) = &config.timezone {
        if !tz.is_empty() && <chrono_tz::Tz as std::str::FromStr>::from_str(tz).is_err() {
            return Err(format!("unknown IANA timezone: {tz}"));
        }
    }

    write_persisted_config(&config)?;

    // Note: `runtime::config::current()` is a `OnceLock` snapshot taken
    // at boot, so timezone / cutoff_hour / feature-flag changes only take
    // effect on the next app restart. The Memory Settings UI surfaces an
    // amber "需要重启" banner for these fields. ContextBudget is similarly
    // restart-bound (shared via Arc).

    let normalized_tz = config
        .timezone
        .clone()
        .and_then(|t| if t.is_empty() { None } else { Some(t) });

    Ok(MemoryConfig {
        total_tokens: config.total_tokens,
        system_pct: config.system_pct,
        episodic_pct: config.episodic_pct,
        semantic_pct: config.semantic_pct,
        working_pct: config.working_pct,
        trajectory_count: count_trajectories(),
        control_plane_v1_enabled: config.control_plane_v1_enabled.unwrap_or(true),
        recall_mode: config.recall_mode.unwrap_or_default(),
        policy_enforce_mode: config.policy_enforce_mode.unwrap_or_default(),
        promotion: config.promotion.unwrap_or_default(),
        timezone: normalized_tz,
        logical_day_cutoff_hour: config.logical_day_cutoff_hour.unwrap_or(4),
        detected_os_timezone: crate::modules::runtime::logical_day::detect_os_timezone()
            .map(|tz| tz.name().to_string()),
    })
}

/// Read the effective prompt control settings, including defaults.
#[tauri::command]
pub fn get_prompt_control_settings() -> Result<PromptControlSettings, String> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let config = ConfigLoader::default_for(cwd)
        .load()
        .map_err(|e| e.to_string())?;

    Ok(PromptControlSettings {
        default_scenario_profile: config
            .control_plane()
            .default_scenario_profile()
            .map(PromptScenarioProfileSetting::from),
        default_soul_id: config.identity().default_soul_id.clone(),
        default_persona_id: config.identity().default_persona_id.clone(),
        agent_name: config.identity().agent_name.clone(),
        user_name: config.identity().user_name.clone(),
        prompt_diagnostics_enabled: config.control_plane().prompt_diagnostics_enabled(),
    })
}

/// Persist prompt control settings to `~/.if2ai/prompt/control-plane.json`.
///
/// Validation runs against the **effective** registry (built-in merged
/// with user-defined personas from identity-pack.json), so persisting a
/// user-created persona as the global default succeeds without
/// requiring a separate validation surface.
#[tauri::command]
pub fn set_prompt_control_settings(
    request: PromptControlSettingsInput,
) -> Result<PromptControlSettings, String> {
    let builtin = IdentityRegistry::builtin();
    // Best-effort: if the pack is unreadable we fall back to built-in
    // only and risk a false-negative for custom personas, but never
    // crash the save. The user can always re-save once the pack is
    // readable again.
    let pack = read_identity_customization_pack().unwrap_or_default();
    let registry = apply_identity_customization_pack(&builtin, &pack);

    if let Some(soul_id) = request.default_soul_id.as_deref() {
        if !soul_id.is_empty() && registry.soul(soul_id).is_none() {
            return Err(format!("unknown soul id: {soul_id}"));
        }
    }
    if let Some(persona_id) = request.default_persona_id.as_deref() {
        if !persona_id.is_empty() && registry.persona(persona_id).is_none() {
            return Err(format!("unknown persona id: {persona_id}"));
        }
    }
    if let (Some(soul_id), Some(persona_id)) = (
        request.default_soul_id.as_deref(),
        request.default_persona_id.as_deref(),
    ) {
        if !soul_id.is_empty() && !persona_id.is_empty() {
            if let Some(persona) = registry.persona(persona_id) {
                if persona.soul_id != soul_id {
                    return Err(format!(
                        "persona `{persona_id}` does not belong to soul `{soul_id}`"
                    ));
                }
            }
        }
    }
    write_prompt_control_settings_file(&request)
}

/// Return the effective Soul / Persona catalog for the prompt control
/// panel. This includes built-in entries plus any user-defined personas
/// from `~/.if2ai/prompt/identity-pack.json` (custom_personas section)
/// so newly created personas surface in the UI without needing a
/// separate listing endpoint.
#[tauri::command]
pub fn get_prompt_control_catalog() -> PromptControlCatalog {
    let builtin = IdentityRegistry::builtin();
    // Best-effort: a malformed pack must not blank the catalog —
    // fall back to built-in only on read error.
    let pack = read_identity_customization_pack().unwrap_or_default();
    let registry = apply_identity_customization_pack(&builtin, &pack);
    PromptControlCatalog {
        souls: registry
            .souls()
            .map(|soul| PromptControlSoulOption {
                id: soul.id.clone(),
                version: soul.version.clone(),
                name: soul.name.clone(),
                summary: soul.summary.clone(),
                mission: soul.mission.clone(),
                core_principles: soul.core_principles.clone(),
                decision_contract: soul.decision_contract.clone(),
                non_negotiables: soul.non_negotiables.clone(),
            })
            .collect(),
        personas: registry
            .personas()
            .map(|persona| PromptControlPersonaOption {
                id: persona.id.clone(),
                soul_id: persona.soul_id.clone(),
                version: persona.version.clone(),
                name: persona.name.clone(),
                summary: persona.summary.clone(),
                tone_rules: persona.tone_rules.clone(),
                collaboration_rules: persona.collaboration_rules.clone(),
                output_preferences: persona.output_preferences.clone(),
                avatar_id: persona.avatar_id.clone(),
            })
            .collect(),
    }
}

/// Load the user-editable identity customization pack from `~/.if2ai/prompt/identity-pack.json`.
#[tauri::command]
pub fn get_identity_customization_pack() -> Result<IdentityCustomizationPackDto, String> {
    let pack = read_identity_customization_pack()?;
    Ok(IdentityCustomizationPackDto {
        souls: pack.souls,
        personas: pack.personas,
        custom_personas: pack.custom_personas,
    })
}

/// Persist the identity customization pack after validating known ids.
#[tauri::command]
pub fn set_identity_customization_pack(
    request: IdentityCustomizationPackDto,
) -> Result<IdentityCustomizationPackDto, String> {
    let registry = IdentityRegistry::builtin();
    for soul_id in request.souls.keys() {
        if registry.soul(soul_id).is_none() {
            return Err(format!("unknown soul id: {soul_id}"));
        }
    }
    for persona_id in request.personas.keys() {
        if registry.persona(persona_id).is_none() {
            return Err(format!("unknown persona id: {persona_id}"));
        }
    }
    // Validate user-defined personas: their id must not collide with
    // built-in ones, and the soul they hang under must exist.
    for (persona_id, custom) in &request.custom_personas {
        if registry.persona(persona_id).is_some() {
            return Err(format!(
                "custom persona id `{persona_id}` collides with a built-in persona"
            ));
        }
        if registry.soul(&custom.soul_id).is_none() {
            return Err(format!(
                "custom persona `{persona_id}` references unknown soul `{}`",
                custom.soul_id
            ));
        }
    }

    let normalized = normalize_identity_customization_pack(IdentityCustomizationPack {
        souls: request.souls,
        personas: request.personas,
        custom_personas: request.custom_personas,
    });
    let saved = write_identity_customization_pack(&normalized)?;
    Ok(IdentityCustomizationPackDto {
        souls: saved.souls,
        personas: saved.personas,
        custom_personas: saved.custom_personas,
    })
}

/// Export trajectories to a user-selected directory.
#[tauri::command]
pub fn export_trajectories() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let traj_dir = std::path::Path::new(&home).join(".if2ai/trajectories");
    let export_dir = traj_dir.join("export");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

    // Copy all .jsonl files to the export directory
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&traj_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                if let Some(name) = path.file_name() {
                    let dest = export_dir.join(name);
                    std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
        }
    }

    Ok(format!(
        "已导出 {} 个轨迹文件到 {}",
        count,
        export_dir.display()
    ))
}
