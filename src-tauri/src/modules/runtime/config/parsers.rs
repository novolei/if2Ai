//! Settings JSON parsers + IO helper.
//!
//! Extracted from `runtime/config/mod.rs` in GFR-T1-B-5 (pure
//! structural move; function bodies byte-identical). All parsers
//! consumed by `ConfigLoader::load` (still in mod.rs); the IO helper
//! is also called by `ConfigLoader::load` directly + by
//! `mcp::merge_mcp_servers`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::super::json::JsonValue;
use super::super::sandbox::SandboxConfig;
use super::json_helpers::{
    expect_object, optional_bool, optional_string, optional_string_array, optional_u32,
    optional_u64, parse_bool_map,
};
use super::schema::{parse_boundary_enforce_mode_label, parse_filesystem_mode_label};
use super::{
    ConfigError, ControlPlaneGovernanceConfig, ProviderTransportConfig, RuntimeHookConfig,
    RuntimePluginConfig,
};
use crate::modules::identity::IdentitySettings;
use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;

pub(super) fn read_optional_json_object(
    path: &Path,
) -> Result<Option<BTreeMap<String, JsonValue>>, ConfigError> {
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
        Err(error) => return Err(ConfigError::Parse(format!("{}: {error}", path.display()))),
    };
    let Some(object) = parsed.as_object() else {
        return Err(ConfigError::Parse(format!(
            "{}: top-level settings value must be a JSON object",
            path.display()
        )));
    };
    Ok(Some(object.clone()))
}

pub(super) fn parse_optional_model(root: &JsonValue) -> Option<String> {
    root.as_object()
        .and_then(|object| object.get("model"))
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn parse_optional_identity_settings(
    root: &JsonValue,
) -> Result<IdentitySettings, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(IdentitySettings::default());
    };
    let Some(identity_value) = object.get("identity") else {
        return Ok(IdentitySettings::default());
    };
    let identity = expect_object(identity_value, "merged settings.identity")?;
    Ok(IdentitySettings {
        default_soul_id: optional_string(identity, "defaultSoulId", "merged settings.identity")?
            .map(str::to_string),
        default_persona_id: optional_string(
            identity,
            "defaultPersonaId",
            "merged settings.identity",
        )?
        .map(str::to_string),
        agent_name: optional_string(identity, "agentName", "merged settings.identity")?
            .map(str::to_string),
        user_name: optional_string(identity, "userName", "merged settings.identity")?
            .map(str::to_string),
    })
}

pub(super) fn parse_optional_hooks_config(
    root: &JsonValue,
) -> Result<RuntimeHookConfig, ConfigError> {
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

pub(super) fn parse_optional_plugin_config(
    root: &JsonValue,
) -> Result<RuntimePluginConfig, ConfigError> {
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

pub(super) fn parse_optional_sandbox_config(
    root: &JsonValue,
) -> Result<SandboxConfig, ConfigError> {
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

pub(super) fn parse_optional_control_plane_config(
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
    let default_scenario_profile = optional_string(
        control_plane,
        "defaultScenarioProfile",
        "merged settings.controlPlane",
    )?
    .map(parse_scenario_profile_hint_label)
    .transpose()?;
    let prompt_diagnostics_enabled = optional_bool(
        control_plane,
        "promptDiagnosticsEnabled",
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
        default_scenario_profile,
        prompt_diagnostics_enabled,
        provider_transport,
    })
}

fn parse_scenario_profile_hint_label(label: &str) -> Result<ScenarioProfileHint, ConfigError> {
    match label {
        "chat" => Ok(ScenarioProfileHint::Chat),
        "coding" => Ok(ScenarioProfileHint::Coding),
        "research" => Ok(ScenarioProfileHint::Research),
        "planning" => Ok(ScenarioProfileHint::Planning),
        "review" => Ok(ScenarioProfileHint::Review),
        other => Err(ConfigError::Parse(format!(
            "invalid scenario profile hint `{other}`; expected one of: chat, coding, research, planning, review"
        ))),
    }
}

pub(super) fn validate_provider_transport_config(
    config: &ProviderTransportConfig,
) -> Result<(), ConfigError> {
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
