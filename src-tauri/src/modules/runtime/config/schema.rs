//! Schema simple types: permission mode + boundary mode + config entry +
//! OAuth + their JSON parsers.
//!
//! Extracted from `runtime/config/mod.rs` in GFR-T1-B-2 (pure structural
//! move; struct/enum fields and function bodies byte-identical).

use std::path::PathBuf;

use super::super::json::JsonValue;
use super::super::sandbox::FilesystemIsolationMode;
use super::json_helpers::{
    expect_object, expect_string, optional_string, optional_string_array, optional_u16,
};
use super::{ConfigError, ConfigSource};

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
    pub fn as_permission_mode(self) -> super::super::permissions::PermissionMode {
        match self {
            Self::ReadOnly => super::super::permissions::PermissionMode::ReadOnly,
            Self::WorkspaceWrite => super::super::permissions::PermissionMode::WorkspaceWrite,
            Self::DangerFullAccess => super::super::permissions::PermissionMode::DangerFullAccess,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub source: ConfigSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub callback_port: Option<u16>,
    pub manual_redirect_url: Option<String>,
    pub scopes: Vec<String>,
}

pub(super) fn parse_optional_permission_mode(
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

pub(super) fn parse_boundary_enforce_mode_label(
    value: &str,
) -> Result<BoundaryEnforceMode, ConfigError> {
    match value {
        "shadow" => Ok(BoundaryEnforceMode::Shadow),
        "enforce" => Ok(BoundaryEnforceMode::Enforce),
        other => Err(ConfigError::Parse(format!(
            "merged settings.controlPlane.boundaryEnforceMode: unsupported mode {other}"
        ))),
    }
}

pub(super) fn parse_filesystem_mode_label(
    value: &str,
) -> Result<FilesystemIsolationMode, ConfigError> {
    match value {
        "off" => Ok(FilesystemIsolationMode::Off),
        "workspace-only" => Ok(FilesystemIsolationMode::WorkspaceOnly),
        "allow-list" => Ok(FilesystemIsolationMode::AllowList),
        other => Err(ConfigError::Parse(format!(
            "merged settings.sandbox.filesystemMode: unsupported filesystem mode {other}"
        ))),
    }
}

pub(super) fn parse_optional_oauth_config(
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
