use crate::modules::identity::resolve_identity;
use crate::modules::identity::{IdentityRegistry, IdentitySettings, SessionIdentityOverride};
use crate::modules::runtime::config::{
    mcp_settings_path_for_home, prompt_control_config_path_for_home, BoundaryEnforceMode,
    CompilerConfig, ConfigLoader, ConfigSource, McpServerConfig, McpTransport,
    MemoryPolicyEnforceMode, MemoryRecallMode, ResolvedPermissionMode, IF2AI_SETTINGS_SCHEMA_NAME,
};
use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;
use crate::modules::runtime::json::JsonValue;
use crate::modules::runtime::sandbox::FilesystemIsolationMode;
use std::fs;
use uuid::Uuid;

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("runtime-config-{}", Uuid::new_v4()))
}

struct HomeGuard {
    original: Option<std::ffi::OsString>,
}

impl HomeGuard {
    fn set(path: &std::path::Path) -> Self {
        let original = std::env::var_os("HOME");
        std::env::set_var("HOME", path);
        Self { original }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.original {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }
}

#[test]
fn rejects_non_object_settings_files() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
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
fn loads_and_merges_if2ai_config_files_by_precedence() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    let mcp_settings = mcp_settings_path_for_home(&home);
    fs::create_dir_all(mcp_settings.parent().expect("mcp settings parent"))
        .expect("mcp config dir");

    fs::write(
        &mcp_settings,
        r#"{"mcpServers":{"home":{"command":"uvx","args":["home"]}}}"#,
    )
    .expect("write user mcp settings");
    fs::write(
        home.join("settings.json"),
        r#"{"model":"sonnet","env":{"A2":"1"},"hooks":{"PreToolUse":["base"]},"permissions":{"defaultMode":"plan"}}"#,
    )
    .expect("write user settings");
    fs::write(
        cwd.join(".if2ai").join("settings.json"),
        r#"{"env":{"B":"2","C":"3"},"hooks":{"PostToolUse":["project"]},"mcpServers":{"project":{"command":"uvx","args":["project"]}}}"#,
    )
    .expect("write project settings");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
        r#"{"model":"opus","permissionMode":"acceptEdits"}"#,
    )
    .expect("write local settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(IF2AI_SETTINGS_SCHEMA_NAME, "SettingsSchema");
    assert_eq!(loaded.loaded_entries().len(), 4);
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
        3
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    let mcp_settings = mcp_settings_path_for_home(&home);
    fs::create_dir_all(mcp_settings.parent().expect("mcp settings parent"))
        .expect("mcp config dir");

    fs::write(
        &mcp_settings,
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
          }
        }"#,
    )
    .expect("write user mcp settings");
    fs::write(
        home.join("settings.json"),
        r#"{
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
        cwd.join(".if2ai").join("settings.local.json"),
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
fn config_reads_identity_defaults() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        home.join("settings.json"),
        r#"{
          "identity": {
            "defaultSoulId": "if2ai-core",
            "defaultPersonaId": "staff-architect"
          }
        }"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(
        loaded.identity(),
        &IdentitySettings {
            default_soul_id: Some("if2ai-core".to_string()),
            default_persona_id: Some("staff-architect".to_string()),
            ..IdentitySettings::default()
        }
    );

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn session_override_wins_over_global_default() {
    let registry = IdentityRegistry::builtin();
    let defaults = IdentitySettings {
        default_soul_id: Some("if2ai-core".to_string()),
        default_persona_id: Some("staff-architect".to_string()),
        ..IdentitySettings::default()
    };
    let session = SessionIdentityOverride {
        soul_id: None,
        persona_id: Some("execution-partner".to_string()),
    };

    let resolution = resolve_identity(&registry, &defaults, Some(&session));
    assert_eq!(resolution.resolved.soul_id, "if2ai-core");
    assert_eq!(
        resolution.resolved.persona_id.as_deref(),
        Some("execution-partner")
    );
}

#[test]
fn parses_plugin_config_from_enabled_plugins() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(&home).expect("home config dir");
    fs::create_dir_all(&cwd).expect("project dir");
    let mcp_settings = mcp_settings_path_for_home(&home);
    fs::create_dir_all(mcp_settings.parent().expect("mcp settings parent"))
        .expect("mcp config dir");
    fs::write(
        &mcp_settings,
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    assert_eq!(loaded.control_plane().default_scenario_profile(), None);
    assert!(loaded.control_plane().prompt_diagnostics_enabled());

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn config_reads_prompt_control_defaults() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");
    assert_eq!(loaded.control_plane().default_scenario_profile(), None);
    assert!(loaded.control_plane().prompt_diagnostics_enabled());

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn config_reads_prompt_control_overrides_from_prompt_dir() {
    let root = temp_dir();
    let home_root = root.join("home");
    let cwd = root.join("project");
    let home = home_root.join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    let prompt_control_path = prompt_control_config_path_for_home(&home);
    fs::create_dir_all(
        prompt_control_path
            .parent()
            .expect("prompt control parent should exist"),
    )
    .expect("create prompt config dir");
    fs::write(
        &prompt_control_path,
        r#"{
          "controlPlane": {
            "defaultScenarioProfile": "research",
            "promptDiagnosticsEnabled": false
          }
        }"#,
    )
    .expect("write prompt control settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");
    assert_eq!(
        loaded.control_plane().default_scenario_profile(),
        Some(ScenarioProfileHint::Research)
    );
    assert!(!loaded.control_plane().prompt_diagnostics_enabled());

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_memory_feature_flags_with_defaults() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
fn runtime_config_language_default_falls_back_to_os_or_en_us() {
    // LOCALE-DETECT — pre-fix this expected hard-coded "en-US".
    // Now we probe the host OS first (sys-locale) and fall back to
    // "en-US" only when that fails.  CI / dev hosts give a real
    // locale so we just assert the tag is non-empty BCP-47-shaped.
    let cfg = super::RuntimeConfig::empty();
    let lang = cfg.language();
    assert!(!lang.is_empty(), "language must never be empty");
    assert!(
        !lang.contains('_'),
        "BCP-47 uses hyphens, not underscores; got {lang}"
    );
}

#[test]
fn runtime_config_language_reads_top_level_key() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    // LOCALE-DETECT — empty merged map → language() falls back to OS
    // locale or "en-US"; both are valid non-empty BCP-47.
    assert!(!before.language().is_empty());

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
    let home = root.join("home").join(".if2ai");
    fs::create_dir_all(cwd.join(".if2ai")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");
    fs::write(
        cwd.join(".if2ai").join("settings.local.json"),
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
    let parsed: super::If2AiMemoryOverrides = serde_json::from_str(raw).expect("overrides parse");
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
    let parsed: super::If2AiMemoryOverrides = serde_json::from_str(raw).expect("overrides parse");
    assert!(parsed.compiler.is_none());
}
