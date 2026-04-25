//! App updater manifest transport and status evaluation.
//!
//! This module intentionally stops at check-time state. Download / install is
//! a later pack because APP-UPDATER-001 only establishes the transport,
//! release manifest contract, settings state surface, and CI gate.

use serde::{Deserialize, Serialize};

const MANIFEST_SCHEMA: &str = "if2ai.release-manifest";
const MANIFEST_VERSION: u32 = 1;
const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/novolei/if2Ai/releases/latest/download/if2ai-release-manifest.json";

/// Release channel carried by the updater manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Stable,
    Beta,
    Nightly,
}

impl Default for ReleaseChannel {
    fn default() -> Self {
        Self::Stable
    }
}

/// One downloadable artifact declared by the release manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseArtifact {
    pub platform: String,
    pub arch: String,
    pub url: String,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
}

/// Stable updater manifest schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub schema: String,
    pub version: u32,
    pub channel: ReleaseChannel,
    pub latest_version: String,
    #[serde(default)]
    pub release_notes_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<ReleaseArtifact>,
}

/// Status returned by an updater check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdaterCheckStatus {
    NoUpdate,
    UpdateAvailable,
    Failed,
}

/// Check result exposed to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdaterCheckResult {
    pub status: UpdaterCheckStatus,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub manifest_url: Option<String>,
    pub artifact_url: Option<String>,
    pub release_notes_url: Option<String>,
    pub diagnostic: Option<String>,
}

/// Runtime state exposed before any check is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdaterRuntimeStatus {
    Idle,
}

/// Current updater configuration and idle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdaterRuntimeState {
    pub status: UpdaterRuntimeStatus,
    pub current_version: String,
    pub manifest_url: Option<String>,
    pub channel: ReleaseChannel,
}

/// Return the manifest URL configured for this build/runtime.
pub fn configured_manifest_url() -> Option<String> {
    std::env::var("IF2AI_UPDATE_MANIFEST_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(DEFAULT_MANIFEST_URL.to_string()))
}

/// Return the current package version.
#[must_use]
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Build the idle updater state.
#[must_use]
pub fn runtime_state(manifest_url: Option<String>) -> UpdaterRuntimeState {
    UpdaterRuntimeState {
        status: UpdaterRuntimeStatus::Idle,
        current_version: current_version().to_string(),
        manifest_url,
        channel: ReleaseChannel::Stable,
    }
}

/// Fetch and evaluate the release manifest at `manifest_url`.
pub async fn check_manifest_url(current_version: &str, manifest_url: String) -> UpdaterCheckResult {
    let response = match reqwest::get(&manifest_url).await {
        Ok(response) => response,
        Err(error) => {
            return failed(
                current_version,
                Some(manifest_url),
                format!("release manifest request failed: {error}"),
            );
        }
    };

    if !response.status().is_success() {
        return failed(
            current_version,
            Some(manifest_url),
            format!("release manifest returned HTTP {}", response.status()),
        );
    }

    match response.text().await {
        Ok(raw) => evaluate_manifest_json(current_version, &manifest_url, &raw),
        Err(error) => failed(
            current_version,
            Some(manifest_url),
            format!("release manifest body read failed: {error}"),
        ),
    }
}

/// Parse, validate, and evaluate raw manifest JSON.
pub fn evaluate_manifest_json(
    current_version: &str,
    manifest_url: &str,
    raw: &str,
) -> UpdaterCheckResult {
    let manifest = match parse_release_manifest(raw) {
        Ok(manifest) => manifest,
        Err(error) => {
            return failed(current_version, Some(manifest_url.to_string()), error);
        }
    };
    evaluate_manifest(current_version, Some(manifest_url.to_string()), manifest)
}

/// Parse and validate a release manifest.
pub fn parse_release_manifest(raw: &str) -> Result<ReleaseManifest, String> {
    let manifest: ReleaseManifest = serde_json::from_str(raw)
        .map_err(|error| format!("release manifest JSON is invalid: {error}"))?;
    validate_release_manifest(&manifest)?;
    Ok(manifest)
}

/// Validate schema, artifact URLs, and artifact integrity fields.
pub fn validate_release_manifest(manifest: &ReleaseManifest) -> Result<(), String> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!(
            "release manifest schema mismatch: expected {MANIFEST_SCHEMA}, got {}",
            manifest.schema
        ));
    }
    if manifest.version != MANIFEST_VERSION {
        return Err(format!(
            "release manifest version mismatch: expected {MANIFEST_VERSION}, got {}",
            manifest.version
        ));
    }
    if manifest.latest_version.trim().is_empty() {
        return Err("release manifest latest_version is required".to_string());
    }
    if manifest.artifacts.is_empty() {
        return Err("release manifest must include at least one artifact".to_string());
    }

    for artifact in &manifest.artifacts {
        if artifact.platform.trim().is_empty() {
            return Err("release artifact platform is required".to_string());
        }
        if artifact.arch.trim().is_empty() {
            return Err("release artifact arch is required".to_string());
        }
        if !is_https_url(&artifact.url) {
            return Err(format!(
                "release artifact URL must use https://: {}",
                artifact.url
            ));
        }
        let has_signature = artifact
            .signature
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let has_checksum = artifact
            .checksum_sha256
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()));
        if !has_signature && !has_checksum {
            return Err(format!(
                "release artifact {} / {} must include signature or sha256 checksum",
                artifact.platform, artifact.arch
            ));
        }
    }

    Ok(())
}

/// Evaluate a validated manifest against the current version.
#[must_use]
pub fn evaluate_manifest(
    current_version: &str,
    manifest_url: Option<String>,
    manifest: ReleaseManifest,
) -> UpdaterCheckResult {
    let artifact = select_current_artifact(&manifest);
    if artifact.is_none() {
        return failed(
            current_version,
            manifest_url,
            "release manifest has no artifact for this platform".to_string(),
        );
    }

    let artifact_url = artifact.map(|item| item.url.clone());
    let latest_version = manifest.latest_version;
    let release_notes_url = manifest.release_notes_url;
    let status = if version_is_newer(&latest_version, current_version) {
        UpdaterCheckStatus::UpdateAvailable
    } else {
        UpdaterCheckStatus::NoUpdate
    };

    UpdaterCheckResult {
        status,
        current_version: current_version.to_string(),
        latest_version: Some(latest_version),
        manifest_url,
        artifact_url,
        release_notes_url,
        diagnostic: None,
    }
}

fn failed(
    current_version: &str,
    manifest_url: Option<String>,
    diagnostic: String,
) -> UpdaterCheckResult {
    UpdaterCheckResult {
        status: UpdaterCheckStatus::Failed,
        current_version: current_version.to_string(),
        latest_version: None,
        manifest_url,
        artifact_url: None,
        release_notes_url: None,
        diagnostic: Some(diagnostic),
    }
}

fn select_current_artifact(manifest: &ReleaseManifest) -> Option<&ReleaseArtifact> {
    let platform = current_platform();
    let arch = current_arch();
    manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.platform == platform && artifact.arch == arch)
        .or_else(|| {
            manifest
                .artifacts
                .iter()
                .find(|artifact| artifact.platform == platform && artifact.arch == "universal")
        })
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    }
}

fn is_https_url(value: &str) -> bool {
    url::Url::parse(value)
        .map(|url| url.scheme() == "https")
        .unwrap_or(false)
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn parse_version(value: &str) -> Vec<u64> {
    value
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(version: &str) -> String {
        let platform = current_platform();
        format!(
            r#"{{
              "schema": "if2ai.release-manifest",
              "version": 1,
              "channel": "stable",
              "latest_version": "{version}",
              "release_notes_url": "https://example.com/if2ai/releases/v{version}",
              "artifacts": [
                {{
                  "platform": "{platform}",
                  "arch": "universal",
                  "url": "https://example.com/If2Ai_{version}_universal.dmg",
                  "signature": "signed-by-release-ci"
                }}
              ]
            }}"#
        )
    }

    #[test]
    fn manifest_status_matrix() {
        let available = evaluate_manifest_json(
            "0.4.0",
            "https://example.com/manifest.json",
            &manifest_json("0.5.0"),
        );
        assert_eq!(available.status, UpdaterCheckStatus::UpdateAvailable);
        assert_eq!(available.latest_version.as_deref(), Some("0.5.0"));

        let no_update = evaluate_manifest_json(
            "0.5.0",
            "https://example.com/manifest.json",
            &manifest_json("0.5.0"),
        );
        assert_eq!(no_update.status, UpdaterCheckStatus::NoUpdate);

        let failed =
            evaluate_manifest_json("0.5.0", "https://example.com/manifest.json", "{ bad json");
        assert_eq!(failed.status, UpdaterCheckStatus::Failed);
        assert!(failed.diagnostic.unwrap_or_default().contains("invalid"));
    }

    #[test]
    fn release_manifest_validation_rejects_invalid_artifacts() {
        let missing_integrity = r#"{
          "schema": "if2ai.release-manifest",
          "version": 1,
          "channel": "stable",
          "latest_version": "0.5.0",
          "artifacts": [
            {
              "platform": "darwin",
              "arch": "universal",
              "url": "https://example.com/If2Ai.dmg"
            }
          ]
        }"#;
        let err = parse_release_manifest(missing_integrity).unwrap_err();
        assert!(err.contains("signature or sha256 checksum"));

        let http_url = r#"{
          "schema": "if2ai.release-manifest",
          "version": 1,
          "channel": "stable",
          "latest_version": "0.5.0",
          "artifacts": [
            {
              "platform": "darwin",
              "arch": "universal",
              "url": "http://example.com/If2Ai.dmg",
              "signature": "signed"
            }
          ]
        }"#;
        let err = parse_release_manifest(http_url).unwrap_err();
        assert!(err.contains("https://"));
    }
}
