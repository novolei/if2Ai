//! App updater manifest transport and status evaluation.
//!
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const MANIFEST_SCHEMA: &str = "if2ai.release-manifest";
const MANIFEST_VERSION: u32 = 1;
const UPDATER_STATE_EVENT: &str = "app-updater://state";
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
    pub artifact_checksum_sha256: Option<String>,
    pub release_notes_url: Option<String>,
    pub diagnostic: Option<String>,
}

/// Status returned by a download-and-open request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdaterDownloadStatus {
    Downloaded,
    Installing,
    NoUpdate,
    Failed,
}

/// Download result exposed to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdaterDownloadResult {
    pub status: UpdaterDownloadStatus,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub artifact_url: Option<String>,
    pub local_path: Option<String>,
    pub checksum_sha256: Option<String>,
    pub diagnostic: Option<String>,
}

/// Runtime status for the productized updater state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdaterRuntimeStatus {
    Idle,
    Checking,
    Available,
    Downloading,
    Downloaded,
    Installing,
    Latest,
    Error,
}

/// User-tunable updater preferences persisted outside release policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdaterPreferences {
    pub auto_check_enabled: bool,
    pub channel: ReleaseChannel,
}

impl Default for UpdaterPreferences {
    fn default() -> Self {
        Self {
            auto_check_enabled: true,
            channel: ReleaseChannel::Stable,
        }
    }
}

/// Current updater state shared with the settings UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdaterRuntimeState {
    pub status: UpdaterRuntimeStatus,
    pub current_version: String,
    pub manifest_url: Option<String>,
    pub channel: ReleaseChannel,
    pub auto_check_enabled: bool,
    pub latest_version: Option<String>,
    pub release_notes_url: Option<String>,
    pub artifact_url: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub checked_at: Option<String>,
    pub diagnostic: Option<String>,
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
    let preferences = load_preferences();
    UpdaterRuntimeState {
        status: UpdaterRuntimeStatus::Idle,
        current_version: current_version().to_string(),
        manifest_url,
        channel: preferences.channel,
        auto_check_enabled: preferences.auto_check_enabled,
        latest_version: None,
        release_notes_url: None,
        artifact_url: None,
        downloaded_bytes: None,
        total_bytes: None,
        checked_at: None,
        diagnostic: None,
    }
}

/// Return the latest persisted updater state, initializing it if needed.
pub fn current_runtime_state() -> UpdaterRuntimeState {
    shared_state()
        .lock()
        .map(|state| state.clone())
        .unwrap_or_else(|_| error_state("failed to acquire updater state lock".to_string()))
}

/// Persist updater preferences and publish the updated state.
pub fn set_preferences(app: &AppHandle, preferences: UpdaterPreferences) -> UpdaterRuntimeState {
    let _ = save_preferences(&preferences);
    let state = mutate_state(|state| {
        state.auto_check_enabled = preferences.auto_check_enabled;
        state.channel = preferences.channel;
        state.clone()
    });
    publish_state(app, &state);
    state
}

/// Check updates through the official Tauri updater plugin.
pub async fn check_signed_update(app: AppHandle) -> UpdaterCheckResult {
    let manifest_url = configured_manifest_url();
    let checking = mutate_state(|state| {
        state.status = UpdaterRuntimeStatus::Checking;
        state.diagnostic = None;
        state.downloaded_bytes = None;
        state.total_bytes = None;
        state.clone()
    });
    publish_state(&app, &checking);

    let update_result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };

    match update_result {
        Ok(Some(update)) => {
            let state = mutate_state(|state| {
                state.status = UpdaterRuntimeStatus::Available;
                state.latest_version = Some(update.version.clone());
                state.release_notes_url = release_notes_url(&update.raw_json);
                state.artifact_url = Some(update.download_url.to_string());
                state.checked_at = Some(chrono::Utc::now().to_rfc3339());
                state.diagnostic = None;
                state.clone()
            });
            publish_state(&app, &state);
            UpdaterCheckResult {
                status: UpdaterCheckStatus::UpdateAvailable,
                current_version: current_version().to_string(),
                latest_version: state.latest_version,
                manifest_url,
                artifact_url: state.artifact_url,
                artifact_checksum_sha256: None,
                release_notes_url: state.release_notes_url,
                diagnostic: None,
            }
        }
        Ok(None) => {
            let state = mutate_state(|state| {
                state.status = UpdaterRuntimeStatus::Latest;
                state.checked_at = Some(chrono::Utc::now().to_rfc3339());
                state.diagnostic = None;
                state.clone()
            });
            publish_state(&app, &state);
            UpdaterCheckResult {
                status: UpdaterCheckStatus::NoUpdate,
                current_version: current_version().to_string(),
                latest_version: None,
                manifest_url,
                artifact_url: None,
                artifact_checksum_sha256: None,
                release_notes_url: None,
                diagnostic: None,
            }
        }
        Err(error) => {
            let diagnostic = format!("signed updater check failed: {error}");
            let state = mutate_state(|state| {
                state.status = UpdaterRuntimeStatus::Error;
                state.checked_at = Some(chrono::Utc::now().to_rfc3339());
                state.diagnostic = Some(diagnostic.clone());
                state.clone()
            });
            publish_state(&app, &state);
            failed(current_version(), manifest_url, diagnostic)
        }
    }
}

/// Download, verify, and install the signed update bundle.
pub async fn download_and_install_signed_update(app: AppHandle) -> UpdaterDownloadResult {
    let update_result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };

    let update = match update_result {
        Ok(Some(update)) => update,
        Ok(None) => {
            let state = mutate_state(|state| {
                state.status = UpdaterRuntimeStatus::Latest;
                state.diagnostic = None;
                state.clone()
            });
            publish_state(&app, &state);
            return UpdaterDownloadResult {
                status: UpdaterDownloadStatus::NoUpdate,
                current_version: current_version().to_string(),
                latest_version: None,
                artifact_url: None,
                local_path: None,
                checksum_sha256: None,
                diagnostic: None,
            };
        }
        Err(error) => {
            return signed_download_failed(&app, format!("signed updater check failed: {error}"));
        }
    };

    let latest_version = update.version.clone();
    let artifact_url = update.download_url.to_string();
    let downloaded = Arc::new(Mutex::new(0_u64));
    let progress_app = app.clone();
    let progress_counter = downloaded.clone();
    let progress_version = latest_version.clone();
    let progress_url = artifact_url.clone();
    let download_result = update
        .download(
            move |chunk_size, total| {
                let downloaded_bytes = progress_counter
                    .lock()
                    .map(|mut value| {
                        *value += chunk_size as u64;
                        *value
                    })
                    .unwrap_or(0);
                let state = mutate_state(|state| {
                    state.status = UpdaterRuntimeStatus::Downloading;
                    state.latest_version = Some(progress_version.clone());
                    state.artifact_url = Some(progress_url.clone());
                    state.downloaded_bytes = Some(downloaded_bytes);
                    state.total_bytes = total;
                    state.diagnostic = None;
                    state.clone()
                });
                publish_state(&progress_app, &state);
            },
            {
                let finish_app = app.clone();
                move || {
                    let state = mutate_state(|state| {
                        state.status = UpdaterRuntimeStatus::Downloaded;
                        state.downloaded_bytes = state.total_bytes.or(state.downloaded_bytes);
                        state.clone()
                    });
                    publish_state(&finish_app, &state);
                }
            },
        )
        .await;

    let bytes = match download_result {
        Ok(bytes) => bytes,
        Err(error) => {
            return signed_download_failed(
                &app,
                format!("signed updater download failed: {error}"),
            );
        }
    };

    let installing = mutate_state(|state| {
        state.status = UpdaterRuntimeStatus::Installing;
        state.latest_version = Some(latest_version.clone());
        state.artifact_url = Some(artifact_url.clone());
        state.diagnostic = None;
        state.clone()
    });
    publish_state(&app, &installing);

    match update.install(bytes) {
        Ok(()) => UpdaterDownloadResult {
            status: UpdaterDownloadStatus::Installing,
            current_version: current_version().to_string(),
            latest_version: Some(latest_version),
            artifact_url: Some(artifact_url),
            local_path: None,
            checksum_sha256: None,
            diagnostic: None,
        },
        Err(error) => {
            signed_download_failed(&app, format!("signed updater install failed: {error}"))
        }
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

/// Download the newest artifact declared by the configured manifest, verify
/// its SHA-256 checksum when present, and open it with the platform handler.
pub async fn download_and_open_update(
    current_version: &str,
    manifest_url: String,
) -> UpdaterDownloadResult {
    let manifest = match fetch_manifest(&manifest_url).await {
        Ok(manifest) => manifest,
        Err(error) => return download_failed(current_version, error),
    };
    if !version_is_newer(&manifest.latest_version, current_version) {
        return UpdaterDownloadResult {
            status: UpdaterDownloadStatus::NoUpdate,
            current_version: current_version.to_string(),
            latest_version: Some(manifest.latest_version),
            artifact_url: None,
            local_path: None,
            checksum_sha256: None,
            diagnostic: None,
        };
    }

    let Some(artifact) = select_current_artifact(&manifest).cloned() else {
        return download_failed(
            current_version,
            "release manifest has no artifact for this platform".to_string(),
        );
    };

    let download_dir = updater_download_dir(&manifest.latest_version);
    if let Err(error) = tokio::fs::create_dir_all(&download_dir).await {
        return download_failed(
            current_version,
            format!("failed to create updater download directory: {error}"),
        );
    }
    let local_path = download_dir.join(artifact_file_name(&artifact.url));
    if let Err(error) = download_artifact(&artifact.url, &local_path).await {
        return download_failed(current_version, error);
    }
    if let Some(expected) = artifact.checksum_sha256.as_deref() {
        match sha256_file(&local_path).await {
            Ok(actual) if actual.eq_ignore_ascii_case(expected) => {}
            Ok(actual) => {
                let _ = tokio::fs::remove_file(&local_path).await;
                return download_failed(
                    current_version,
                    format!("download checksum mismatch: expected={expected}, actual={actual}"),
                );
            }
            Err(error) => return download_failed(current_version, error),
        }
    }
    if let Err(error) = open_path(&local_path) {
        return download_failed(
            current_version,
            format!("downloaded update but failed to open artifact: {error}"),
        );
    }

    UpdaterDownloadResult {
        status: UpdaterDownloadStatus::Downloaded,
        current_version: current_version.to_string(),
        latest_version: Some(manifest.latest_version),
        artifact_url: Some(artifact.url),
        local_path: Some(local_path.display().to_string()),
        checksum_sha256: artifact.checksum_sha256,
        diagnostic: None,
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
    let artifact_checksum_sha256 = artifact.and_then(|item| item.checksum_sha256.clone());
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
        artifact_checksum_sha256,
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
        artifact_checksum_sha256: None,
        release_notes_url: None,
        diagnostic: Some(diagnostic),
    }
}

fn shared_state() -> &'static Mutex<UpdaterRuntimeState> {
    static STATE: once_cell::sync::Lazy<Mutex<UpdaterRuntimeState>> =
        once_cell::sync::Lazy::new(|| Mutex::new(runtime_state(configured_manifest_url())));
    &STATE
}

fn mutate_state<F>(mutator: F) -> UpdaterRuntimeState
where
    F: FnOnce(&mut UpdaterRuntimeState) -> UpdaterRuntimeState,
{
    match shared_state().lock() {
        Ok(mut state) => mutator(&mut state),
        Err(_) => error_state("failed to acquire updater state lock".to_string()),
    }
}

fn publish_state(app: &AppHandle, state: &UpdaterRuntimeState) {
    if let Err(error) = app.emit(UPDATER_STATE_EVENT, state) {
        tracing::warn!(error = %error, "[app-updater] failed to emit state");
    }
}

fn error_state(diagnostic: String) -> UpdaterRuntimeState {
    let mut state = runtime_state(configured_manifest_url());
    state.status = UpdaterRuntimeStatus::Error;
    state.diagnostic = Some(diagnostic);
    state
}

fn signed_download_failed(app: &AppHandle, diagnostic: String) -> UpdaterDownloadResult {
    let state = mutate_state(|state| {
        state.status = UpdaterRuntimeStatus::Error;
        state.diagnostic = Some(diagnostic.clone());
        state.clone()
    });
    publish_state(app, &state);
    UpdaterDownloadResult {
        status: UpdaterDownloadStatus::Failed,
        current_version: current_version().to_string(),
        latest_version: None,
        artifact_url: None,
        local_path: None,
        checksum_sha256: None,
        diagnostic: Some(diagnostic),
    }
}

fn release_notes_url(raw_json: &serde_json::Value) -> Option<String> {
    raw_json
        .get("release_notes_url")
        .or_else(|| raw_json.get("releaseNotesUrl"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn preferences_path() -> PathBuf {
    crate::modules::config::store::if2ai_data_root()
        .join("updater")
        .join("preferences.json")
}

fn load_preferences() -> UpdaterPreferences {
    std::fs::read_to_string(preferences_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<UpdaterPreferences>(&raw).ok())
        .unwrap_or_default()
}

fn save_preferences(preferences: &UpdaterPreferences) -> Result<(), String> {
    let path = preferences_path();
    let parent = path
        .parent()
        .ok_or_else(|| "updater preferences path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create updater preference dir: {error}"))?;
    let raw = serde_json::to_string_pretty(preferences)
        .map_err(|error| format!("failed to encode updater preferences: {error}"))?;
    std::fs::write(path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write updater preferences: {error}"))?;
    Ok(())
}

async fn fetch_manifest(manifest_url: &str) -> Result<ReleaseManifest, String> {
    let response = reqwest::get(manifest_url)
        .await
        .map_err(|error| format!("release manifest request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "release manifest returned HTTP {}",
            response.status()
        ));
    }
    let raw = response
        .text()
        .await
        .map_err(|error| format!("release manifest body read failed: {error}"))?;
    parse_release_manifest(&raw)
}

async fn download_artifact(url: &str, local_path: &Path) -> Result<(), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|error| format!("artifact download request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "artifact download returned HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("artifact body read failed: {error}"))?;
    let parent = local_path
        .parent()
        .ok_or_else(|| "artifact local path has no parent".to_string())?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".if2ai-update-")
        .tempfile_in(parent)
        .map_err(|error| format!("failed to create updater tempfile: {error}"))?;
    tmp.write_all(&bytes)
        .map_err(|error| format!("failed to write updater tempfile: {error}"))?;
    tmp.persist(local_path)
        .map_err(|error| format!("failed to persist updater artifact: {error}"))?;
    Ok(())
}

async fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| format!("failed to read downloaded artifact: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn updater_download_dir(version: &str) -> PathBuf {
    crate::modules::config::store::if2ai_data_root()
        .join("updater")
        .join(version.trim_start_matches('v'))
}

fn artifact_file_name(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "If2Ai-update.dmg".to_string())
}

fn open_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).status()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).status()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).status()?;
    }
    Ok(())
}

fn download_failed(current_version: &str, diagnostic: String) -> UpdaterDownloadResult {
    UpdaterDownloadResult {
        status: UpdaterDownloadStatus::Failed,
        current_version: current_version.to_string(),
        latest_version: None,
        artifact_url: None,
        local_path: None,
        checksum_sha256: None,
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
