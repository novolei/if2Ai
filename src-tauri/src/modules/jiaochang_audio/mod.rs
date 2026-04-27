//! Jiaochang audio plugin source sandbox.
//!
//! The renderer never executes source plugin code.  Plugin source manifests are
//! registered here as constrained URL resolvers, and every playable URL request
//! is processed by a Tokio worker before being returned to the frontend.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};

const PLUGIN_EVENT: &str = "jiaochang://audio/plugin-event";
const WORKER_QUEUE: usize = 64;
const LX_CERU_NODE_WORKER: &str = r#"
import fs from 'node:fs';
import vm from 'node:vm';

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const input = JSON.parse(Buffer.concat(chunks).toString('utf8'));
const rawScript = fs.readFileSync(input.pluginPath, 'utf8');
const handlers = new Map();
const sent = [];
const EVENT_NAMES = { request: 'request', inited: 'inited', updateAlert: 'updateAlert' };

function send(event, payload) {
  sent.push({ event, payload });
}

function on(event, handler) {
  handlers.set(event, handler);
}

function request(url, options, callback) {
  if (typeof options === 'function') {
    callback = options;
    options = {};
  }
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 9000);
  fetch(url, { ...(options || {}), signal: controller.signal })
    .then(async (response) => {
      const text = await response.text();
      let body = text;
      try { body = JSON.parse(text); } catch {}
      callback?.(null, {
        statusCode: response.status,
        status: response.status,
        headers: Object.fromEntries(response.headers.entries()),
        body,
      });
    })
    .catch((error) => callback?.(error))
    .finally(() => clearTimeout(timeout));
  return () => controller.abort();
}

const sandbox = {
  console: { log() {}, warn() {}, error() {} },
  setTimeout,
  clearTimeout,
  Promise,
  URL,
  URLSearchParams,
  encodeURIComponent,
  decodeURIComponent,
  encodeURI,
  decodeURI,
  fetch,
  globalThis: {},
};
sandbox.globalThis = sandbox;
sandbox.window = sandbox;
sandbox.document = {
  getElementsByTagName(name) {
    return name === 'script' ? [{ innerText: rawScript }] : [];
  },
};
sandbox.globalThis.lx = {
  EVENT_NAMES,
  currentScriptInfo: { rawScript },
  request,
  on,
  send,
};
sandbox.lx = sandbox.globalThis.lx;

try {
  vm.runInNewContext(rawScript, sandbox, { filename: input.pluginPath, timeout: 2000 });
  await new Promise((resolve) => setTimeout(resolve, 1200));
  const handler = handlers.get(EVENT_NAMES.request);
  if (!handler) throw new Error('LX/Ceru plugin did not register request handler');
  const result = await handler({
    action: input.action,
    source: input.source,
    info: {
      keyword: input.keyword,
      page: input.page,
      pagesize: input.pageSize,
      type: input.quality,
      musicInfo: input.musicInfo,
    },
  });
  if (input.action === 'musicSearch' || input.action === 'search') {
    process.stdout.write(JSON.stringify({ result, sent }));
    process.exit(0);
  }
  const url = typeof result === 'string' ? result : result?.url || result?.musicUrl || result?.music_url;
  if (!url) throw new Error('LX/Ceru plugin returned no playable URL');
  process.stdout.write(JSON.stringify({ url, sent }));
} catch (error) {
  process.stderr.write(error?.stack || error?.message || String(error));
  process.exit(1);
}
"#;

static RUNTIME: OnceLock<Arc<JiaochangAudioPluginRuntime>> = OnceLock::new();

/// Manifest supplied by a plugin source installer or settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangAudioPluginManifest {
    pub plugin_id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub resolver_template: String,
    #[serde(default)]
    pub compatibility: JiaochangAudioPluginCompatibility,
    #[serde(default)]
    pub providers: Vec<JiaochangAudioPluginProvider>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub local_js_path: Option<String>,
}

/// Compatibility mode for a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JiaochangAudioPluginCompatibility {
    ConstrainedManifest,
    LxCeruJs,
    AuthorizedMultiSource,
}

impl Default for JiaochangAudioPluginCompatibility {
    fn default() -> Self {
        Self::ConstrainedManifest
    }
}

/// A provider inside an authorized multi-source plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangAudioPluginProvider {
    pub provider_id: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    pub resolver_template: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Static inspection result for a local LX/Ceru style JavaScript plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JiaochangAudioPluginInspection {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub detected_kind: JiaochangAudioPluginCompatibility,
    pub name: Option<String>,
    pub version: Option<String>,
    pub homepage: Option<String>,
    pub risk_flags: Vec<String>,
    pub supported_hosts: Vec<String>,
    pub executable_in_renderer: bool,
    pub recommendation: String,
}

/// Track URL resolution request sent from the audio source adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangPluginTrackResolveInput {
    pub track_id: String,
    pub plugin_id: String,
    pub source: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    pub quality: String,
    #[serde(default)]
    pub source_track_id: Option<String>,
    #[serde(default)]
    pub plugin_music_info: Option<serde_json::Value>,
    #[serde(default = "default_cache")]
    pub use_cache: bool,
}

/// Search request sent to an LX/Ceru compatible plugin source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangPluginTrackSearchInput {
    pub plugin_id: String,
    pub source: String,
    pub query: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// Search result normalized from an LX/Ceru compatible plugin source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangPluginSearchedTrack {
    pub track_id: String,
    pub plugin_id: String,
    pub source: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub source_track_id: Option<String>,
    pub music_info: serde_json::Value,
}

/// Resolved playable URL returned to the renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangPluginResolvedTrack {
    pub track_id: String,
    pub plugin_id: String,
    pub url: String,
    pub quality: String,
    pub cache_hit: bool,
    pub resolved_at: DateTime<Utc>,
    pub evidence: String,
}

/// Diagnostic event emitted through the Tauri event bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangAudioPluginEvent {
    pub event_type: String,
    pub plugin_id: String,
    #[serde(default)]
    pub track_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub cache_hit: bool,
    #[serde(default)]
    pub url: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Cache status exposed for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JiaochangAudioPluginCacheInfo {
    pub entries: usize,
}

/// Runtime owning the plugin registry, URL cache and worker queue.
pub struct JiaochangAudioPluginRuntime {
    registry: Arc<RwLock<HashMap<String, JiaochangAudioPluginManifest>>>,
    cache: Arc<RwLock<HashMap<String, JiaochangPluginResolvedTrack>>>,
    tx: mpsc::Sender<ResolveJob>,
}

struct ResolveJob {
    input: JiaochangPluginTrackResolveInput,
    reply: oneshot::Sender<Result<JiaochangPluginResolvedTrack, String>>,
}

impl JiaochangAudioPluginRuntime {
    /// Start the Rust-side worker and return a handle for command calls.
    #[must_use]
    pub fn new() -> Self {
        let registry = Arc::new(RwLock::new(load_manifest_registry().unwrap_or_default()));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let (tx, rx) = mpsc::channel(WORKER_QUEUE);
        tokio::spawn(plugin_worker(rx, registry.clone()));
        Self {
            registry,
            cache,
            tx,
        }
    }

    /// Register or replace a constrained plugin source manifest.
    pub async fn register_plugin(
        &self,
        manifest: JiaochangAudioPluginManifest,
    ) -> Result<JiaochangAudioPluginManifest, String> {
        validate_manifest(&manifest)?;
        self.registry
            .write()
            .await
            .insert(manifest.plugin_id.clone(), manifest.clone());
        self.persist_registry().await?;
        Ok(manifest)
    }

    /// Persistently enable or disable one plugin source.
    pub async fn set_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<JiaochangAudioPluginManifest, String> {
        validate_identifier("plugin_id", plugin_id)?;
        let updated = {
            let mut registry = self.registry.write().await;
            let manifest = registry
                .get_mut(plugin_id)
                .ok_or_else(|| format!("Plugin source '{plugin_id}' is not registered"))?;
            manifest.enabled = enabled;
            manifest.clone()
        };
        self.persist_registry().await?;
        Ok(updated)
    }

    /// Remove one registered plugin source and its cached URLs.
    pub async fn remove_plugin(&self, plugin_id: &str) -> Result<(), String> {
        validate_identifier("plugin_id", plugin_id)?;
        let removed = self.registry.write().await.remove(plugin_id);
        if removed.is_none() {
            return Err(format!("Plugin source '{plugin_id}' is not registered"));
        }
        self.cache
            .write()
            .await
            .retain(|key, _| !key.starts_with(&format!("{plugin_id}::")));
        self.persist_registry().await?;
        Ok(())
    }

    /// Return all registered plugin source manifests.
    pub async fn list_plugins(&self) -> Vec<JiaochangAudioPluginManifest> {
        let mut plugins = self
            .registry
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plugins.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
        plugins
    }

    /// Install the built-in authorized template for five Chinese music providers.
    pub async fn install_authorized_chinese_sources_template(
        &self,
    ) -> Result<JiaochangAudioPluginManifest, String> {
        let manifest = authorized_chinese_sources_template();
        self.register_plugin(manifest).await
    }

    /// Resolve a playable URL through cache first, then the worker.
    pub async fn resolve_track_url(
        &self,
        app: &AppHandle,
        input: JiaochangPluginTrackResolveInput,
    ) -> Result<JiaochangPluginResolvedTrack, String> {
        if input.use_cache {
            let key = cache_key(&input);
            if let Some(cached) = self.cache.read().await.get(&key).cloned() {
                let mut resolved = cached;
                resolved.cache_hit = true;
                emit_event(
                    app,
                    JiaochangAudioPluginEvent::cache_hit(
                        &resolved.plugin_id,
                        &resolved.track_id,
                        &resolved.url,
                    ),
                );
                return Ok(resolved);
            }
        }

        let plugin_id = input.plugin_id.clone();
        let track_id = input.track_id.clone();
        emit_event(
            app,
            JiaochangAudioPluginEvent::resolving(&plugin_id, &track_id),
        );
        let key = cache_key(&input);
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(ResolveJob { input, reply })
            .await
            .map_err(|_| "Jiaochang audio plugin worker is unavailable".to_string())?;
        let resolved = rx
            .await
            .map_err(|_| "Jiaochang audio plugin worker dropped the response".to_string())??;

        if resolved.url.starts_with("http://") || resolved.url.starts_with("https://") {
            self.cache.write().await.insert(key, resolved.clone());
        }
        emit_event(
            app,
            JiaochangAudioPluginEvent::resolved(
                &resolved.plugin_id,
                &resolved.track_id,
                &resolved.url,
            ),
        );
        Ok(resolved)
    }

    /// Search tracks through a registered LX/Ceru compatible plugin source.
    pub async fn search_tracks(
        &self,
        input: JiaochangPluginTrackSearchInput,
    ) -> Result<Vec<JiaochangPluginSearchedTrack>, String> {
        search_lx_ceru_tracks(&self.registry, &input).await
    }

    /// Return cache diagnostics.
    pub async fn cache_info(&self) -> JiaochangAudioPluginCacheInfo {
        JiaochangAudioPluginCacheInfo {
            entries: self.cache.read().await.len(),
        }
    }

    /// Clear resolved URL cache entries.
    pub async fn clear_cache(&self) -> JiaochangAudioPluginCacheInfo {
        self.cache.write().await.clear();
        self.cache_info().await
    }

    async fn persist_registry(&self) -> Result<(), String> {
        let registry = self.registry.read().await;
        let mut manifests = registry.values().cloned().collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
        write_manifest_registry(&manifests)
    }
}

impl Default for JiaochangAudioPluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl JiaochangAudioPluginEvent {
    fn resolving(plugin_id: &str, track_id: &str) -> Self {
        Self::new(
            "resolving",
            plugin_id,
            Some(track_id),
            "plugin worker resolving URL",
        )
    }

    fn resolved(plugin_id: &str, track_id: &str, url: &str) -> Self {
        let mut event = Self::new(
            "resolved",
            plugin_id,
            Some(track_id),
            "plugin worker resolved URL",
        );
        event.url = Some(url.to_string());
        event
    }

    fn cache_hit(plugin_id: &str, track_id: &str, url: &str) -> Self {
        let mut event = Self::new(
            "cache_hit",
            plugin_id,
            Some(track_id),
            "server URL cache hit",
        );
        event.url = Some(url.to_string());
        event.cache_hit = true;
        event
    }

    fn new(event_type: &str, plugin_id: &str, track_id: Option<&str>, message: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            plugin_id: plugin_id.to_string(),
            track_id: track_id.map(str::to_string),
            message: message.to_string(),
            cache_hit: false,
            url: None,
            timestamp: Utc::now(),
        }
    }
}

/// Return the process-wide Jiaochang audio plugin runtime.
pub fn plugin_runtime() -> Arc<JiaochangAudioPluginRuntime> {
    RUNTIME
        .get_or_init(|| Arc::new(JiaochangAudioPluginRuntime::new()))
        .clone()
}

async fn plugin_worker(
    mut rx: mpsc::Receiver<ResolveJob>,
    registry: Arc<RwLock<HashMap<String, JiaochangAudioPluginManifest>>>,
) {
    while let Some(job) = rx.recv().await {
        let result = resolve_in_worker(&registry, &job.input).await;
        let _ = job.reply.send(result);
    }
}

async fn resolve_in_worker(
    registry: &Arc<RwLock<HashMap<String, JiaochangAudioPluginManifest>>>,
    input: &JiaochangPluginTrackResolveInput,
) -> Result<JiaochangPluginResolvedTrack, String> {
    validate_resolve_input(input)?;
    let manifest = registry
        .read()
        .await
        .get(&input.plugin_id)
        .cloned()
        .ok_or_else(|| format!("Plugin source '{}' is not registered", input.plugin_id))?;
    if !manifest.enabled {
        return Err(format!(
            "Plugin source '{}' is disabled",
            manifest.plugin_id
        ));
    }
    if manifest.compatibility == JiaochangAudioPluginCompatibility::LxCeruJs {
        return resolve_lx_ceru_in_worker(&manifest, input).await;
    }
    let route = resolve_provider_route(&manifest, input)?;
    let url = render_template(route.resolver_template, input);
    validate_url_against_hosts(&url, route.allowed_hosts)?;
    Ok(JiaochangPluginResolvedTrack {
        track_id: input.track_id.clone(),
        plugin_id: input.plugin_id.clone(),
        url,
        quality: input.quality.clone(),
        cache_hit: false,
        resolved_at: Utc::now(),
        evidence: format!(
            "rust-side plugin worker resolved URL via {}",
            route.evidence
        ),
    })
}

fn validate_manifest(manifest: &JiaochangAudioPluginManifest) -> Result<(), String> {
    validate_identifier("plugin_id", &manifest.plugin_id)?;
    if manifest.name.trim().is_empty() {
        return Err("Plugin name is required".to_string());
    }
    if manifest.providers.is_empty() {
        if manifest.resolver_template.trim().is_empty() {
            return Err("Plugin resolver_template is required".to_string());
        }
        if manifest.allowed_hosts.is_empty() {
            return Err("Plugin allowed_hosts must declare at least one host".to_string());
        }
    }
    for provider in &manifest.providers {
        validate_identifier("provider_id", &provider.provider_id)?;
        if provider.name.trim().is_empty() {
            return Err("Plugin provider name is required".to_string());
        }
        if provider.resolver_template.trim().is_empty() {
            return Err(format!(
                "Plugin provider '{}' resolver_template is required",
                provider.provider_id
            ));
        }
        if provider.allowed_hosts.is_empty() && manifest.allowed_hosts.is_empty() {
            return Err(format!(
                "Plugin provider '{}' must declare allowed_hosts or inherit manifest allowed_hosts",
                provider.provider_id
            ));
        }
    }
    Ok(())
}

fn validate_resolve_input(input: &JiaochangPluginTrackResolveInput) -> Result<(), String> {
    validate_identifier("plugin_id", &input.plugin_id)?;
    validate_identifier("track_id", &input.track_id)?;
    if input.quality.trim().is_empty() {
        return Err("quality is required".to_string());
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-' | '.'));
    if valid {
        Ok(())
    } else {
        Err(format!("{label} contains unsupported characters"))
    }
}

fn render_template(template: &str, input: &JiaochangPluginTrackResolveInput) -> String {
    let replacements = [
        ("{track_id}", input.track_id.as_str()),
        ("{plugin_id}", input.plugin_id.as_str()),
        ("{source}", input.source.as_str()),
        (
            "{source_track_id}",
            input.source_track_id.as_deref().unwrap_or(""),
        ),
        ("{quality}", input.quality.as_str()),
        ("{title}", input.title.as_str()),
        ("{artist}", input.artist.as_deref().unwrap_or("")),
    ];
    replacements
        .iter()
        .fold(template.to_string(), |acc, (key, value)| {
            acc.replace(key, &urlencoding::encode(value))
        })
}

struct ProviderRoute<'a> {
    resolver_template: &'a str,
    allowed_hosts: Vec<&'a str>,
    evidence: String,
}

fn resolve_provider_route<'a>(
    manifest: &'a JiaochangAudioPluginManifest,
    input: &JiaochangPluginTrackResolveInput,
) -> Result<ProviderRoute<'a>, String> {
    if manifest.providers.is_empty() {
        return Ok(ProviderRoute {
            resolver_template: &manifest.resolver_template,
            allowed_hosts: manifest.allowed_hosts.iter().map(String::as_str).collect(),
            evidence: "constrained manifest template".to_string(),
        });
    }
    let provider = manifest
        .providers
        .iter()
        .find(|item| item.provider_id == input.source)
        .ok_or_else(|| {
            format!(
                "Plugin source '{}' has no provider '{}'",
                manifest.plugin_id, input.source
            )
        })?;
    if !provider.enabled {
        return Err(format!(
            "Plugin provider '{}' is disabled",
            provider.provider_id
        ));
    }
    let mut allowed_hosts = provider
        .allowed_hosts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if allowed_hosts.is_empty() {
        allowed_hosts = manifest.allowed_hosts.iter().map(String::as_str).collect();
    }
    Ok(ProviderRoute {
        resolver_template: &provider.resolver_template,
        allowed_hosts,
        evidence: format!("provider '{}'", provider.provider_id),
    })
}

fn validate_url_against_hosts(url: &str, allowed_hosts: Vec<&str>) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Plugin resolved invalid URL: {e}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err("Plugin resolved URL must use http or https".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Plugin resolved URL has no host".to_string())?;
    if allowed_hosts.contains(&host) {
        Ok(())
    } else {
        Err(format!("Plugin host '{host}' is not in allowed_hosts"))
    }
}

/// Inspect a local JS source plugin without executing it.
pub fn inspect_lx_ceru_plugin_file(path: &Path) -> Result<JiaochangAudioPluginInspection, String> {
    let raw = std::fs::read(path).map_err(|e| format!("failed to read plugin file: {e}"))?;
    let text = String::from_utf8_lossy(&raw);
    let sha256 = format!("{:x}", Sha256::digest(&raw));
    let risk_flags = scan_plugin_risks(&text);
    let supported_hosts = scan_supported_hosts(&text);
    let detected_kind = if text.contains("globalThis.lx")
        || text.contains("lx.currentScriptInfo")
        || text.contains("@name")
    {
        JiaochangAudioPluginCompatibility::LxCeruJs
    } else {
        JiaochangAudioPluginCompatibility::ConstrainedManifest
    };
    let recommendation = if detected_kind == JiaochangAudioPluginCompatibility::LxCeruJs {
        "Register as a quarantined compatibility source; renderer execution is blocked and URL resolution is mediated by the Rust-side worker.".to_string()
    } else {
        "Use the constrained manifest adapter when resolver_template and allowed_hosts are declared.".to_string()
    };
    Ok(JiaochangAudioPluginInspection {
        path: path.display().to_string(),
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("plugin.js")
            .to_string(),
        size_bytes: raw.len() as u64,
        sha256,
        detected_kind,
        name: extract_plugin_meta(&text, "@name"),
        version: extract_plugin_meta(&text, "@version"),
        homepage: extract_plugin_meta(&text, "@homepage"),
        risk_flags,
        supported_hosts,
        executable_in_renderer: false,
        recommendation,
    })
}

/// Register a local LX/Ceru JS plugin as a Rust-side compatibility source.
pub fn manifest_from_lx_ceru_plugin_file(
    path: &Path,
) -> Result<JiaochangAudioPluginManifest, String> {
    let inspection = inspect_lx_ceru_plugin_file(path)?;
    let providers = infer_lx_ceru_providers(path)?;
    let allowed_hosts = infer_lx_ceru_allowed_hosts(path, &inspection);
    Ok(JiaochangAudioPluginManifest {
        plugin_id: format!(
            "lx-ceru.{}",
            sanitize_plugin_id(
                inspection
                    .name
                    .as_deref()
                    .unwrap_or(inspection.file_name.as_str())
            )
        ),
        name: inspection
            .name
            .clone()
            .unwrap_or_else(|| inspection.file_name.clone()),
        version: inspection.version.clone(),
        source_url: Some(format!("file://{}", path.display())),
        signature: Some(format!("sha256:{}", inspection.sha256)),
        enabled: true,
        allowed_hosts,
        resolver_template: String::new(),
        compatibility: JiaochangAudioPluginCompatibility::LxCeruJs,
        providers,
        risk_flags: inspection.risk_flags,
        local_js_path: Some(path.display().to_string()),
    })
}

/// Return the compliant five-provider plugin template.
#[must_use]
pub fn authorized_chinese_sources_template() -> JiaochangAudioPluginManifest {
    let providers = [
        ("kuwo", "酷我音乐"),
        ("kugou", "酷狗音乐"),
        ("qqmusic", "QQ音乐"),
        ("netease", "网易云音乐"),
        ("migu", "咪咕音乐"),
    ]
    .into_iter()
    .map(|(provider_id, name)| JiaochangAudioPluginProvider {
        provider_id: provider_id.to_string(),
        name: name.to_string(),
        enabled: true,
        allowed_hosts: vec!["127.0.0.1".to_string(), "localhost".to_string()],
        resolver_template: format!(
            "http://127.0.0.1:43179/jiaochang/music/{provider_id}/resolve?track_id={{source_track_id}}&quality={{quality}}"
        ),
        actions: vec!["musicUrl".to_string()],
    })
    .collect();
    JiaochangAudioPluginManifest {
        plugin_id: "if2ai.authorized-cn-music".to_string(),
        name: "If2Ai Authorized CN Music Sources".to_string(),
        version: Some("0.1.0".to_string()),
        source_url: Some("local://if2ai/jiaochang/plugins/authorized-cn-music".to_string()),
        signature: None,
        enabled: true,
        allowed_hosts: vec!["127.0.0.1".to_string(), "localhost".to_string()],
        resolver_template: String::new(),
        compatibility: JiaochangAudioPluginCompatibility::AuthorizedMultiSource,
        providers,
        risk_flags: vec![
            "requires-user-authorized-resolver-endpoint".to_string(),
            "does-not-bundle-third-party-catalog".to_string(),
            "does-not-bypass-platform-authorization".to_string(),
        ],
        local_js_path: None,
    }
}

async fn resolve_lx_ceru_in_worker(
    manifest: &JiaochangAudioPluginManifest,
    input: &JiaochangPluginTrackResolveInput,
) -> Result<JiaochangPluginResolvedTrack, String> {
    let plugin_path = manifest
        .local_js_path
        .as_deref()
        .or_else(|| {
            manifest
                .source_url
                .as_deref()
                .and_then(|value| value.strip_prefix("file://"))
        })
        .ok_or_else(|| "LX/Ceru plugin has no local JS path".to_string())?;
    let provider = manifest
        .providers
        .iter()
        .find(|item| item.provider_id == input.source)
        .ok_or_else(|| {
            format!(
                "LX/Ceru plugin '{}' has no provider '{}'",
                manifest.plugin_id, input.source
            )
        })?;
    if !provider.enabled {
        return Err(format!(
            "LX/Ceru provider '{}' is disabled",
            provider.provider_id
        ));
    }
    let worker_input = serde_json::json!({
        "pluginPath": plugin_path,
        "action": "musicUrl",
        "source": input.source,
        "quality": map_lx_quality(&input.quality),
        "musicInfo": input.plugin_music_info.clone().unwrap_or_else(|| serde_json::json!({
            "id": input.source_track_id.as_deref().unwrap_or(input.title.as_str()),
            "songmid": input.source_track_id.as_deref().unwrap_or(input.title.as_str()),
            "hash": input.source_track_id.as_deref().unwrap_or(input.title.as_str()),
            "name": input.title.as_str(),
            "singer": input.artist.as_deref().unwrap_or(""),
        }))
    });
    let output = run_lx_ceru_node_worker(worker_input).await?;
    let url = output
        .get("url")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "LX/Ceru isolate did not return a playable URL".to_string())?;
    validate_playable_url_shape(url)?;
    Ok(JiaochangPluginResolvedTrack {
        track_id: input.track_id.clone(),
        plugin_id: input.plugin_id.clone(),
        url: url.to_string(),
        quality: input.quality.clone(),
        cache_hit: false,
        resolved_at: Utc::now(),
        evidence: format!(
            "rust-side LX/Ceru JS isolate resolved provider '{}'",
            provider.provider_id
        ),
    })
}

async fn search_lx_ceru_tracks(
    registry: &Arc<RwLock<HashMap<String, JiaochangAudioPluginManifest>>>,
    input: &JiaochangPluginTrackSearchInput,
) -> Result<Vec<JiaochangPluginSearchedTrack>, String> {
    validate_identifier("plugin_id", &input.plugin_id)?;
    if input.query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let manifest = registry
        .read()
        .await
        .get(&input.plugin_id)
        .cloned()
        .ok_or_else(|| format!("Plugin source '{}' is not registered", input.plugin_id))?;
    if !manifest.enabled {
        return Err(format!(
            "Plugin source '{}' is disabled",
            manifest.plugin_id
        ));
    }
    if manifest.compatibility != JiaochangAudioPluginCompatibility::LxCeruJs {
        return Err("Plugin search requires an LX/Ceru JS compatible source".to_string());
    }
    let plugin_path = manifest
        .local_js_path
        .as_deref()
        .or_else(|| {
            manifest
                .source_url
                .as_deref()
                .and_then(|value| value.strip_prefix("file://"))
        })
        .ok_or_else(|| "LX/Ceru plugin has no local JS path".to_string())?;
    let provider = manifest
        .providers
        .iter()
        .find(|item| item.provider_id == input.source)
        .ok_or_else(|| {
            format!(
                "LX/Ceru plugin '{}' has no provider '{}'",
                manifest.plugin_id, input.source
            )
        })?;
    if !provider.enabled {
        return Err(format!(
            "LX/Ceru provider '{}' is disabled",
            provider.provider_id
        ));
    }
    if !provider
        .actions
        .iter()
        .any(|action| action == "musicSearch" || action == "search")
    {
        return Err(format!(
            "LX/Ceru provider '{}' does not support musicSearch; use results from a searchable provider first",
            provider.provider_id
        ));
    }
    let worker_input = serde_json::json!({
        "pluginPath": plugin_path,
        "action": "musicSearch",
        "source": input.source,
        "keyword": input.query,
        "page": input.page.max(1),
        "pageSize": input.page_size.clamp(1, 30),
        "quality": "320k",
        "musicInfo": null,
    });
    let output = run_lx_ceru_node_worker(worker_input).await?;
    let result = output
        .get("result")
        .cloned()
        .ok_or_else(|| "LX/Ceru search returned no result payload".to_string())?;
    Ok(normalize_lx_search_result(
        &manifest.plugin_id,
        &input.source,
        &result,
    ))
}

fn normalize_lx_search_result(
    plugin_id: &str,
    source: &str,
    result: &serde_json::Value,
) -> Vec<JiaochangPluginSearchedTrack> {
    let list = result
        .get("list")
        .and_then(serde_json::Value::as_array)
        .or_else(|| result.get("data").and_then(serde_json::Value::as_array))
        .or_else(|| result.as_array());
    let Some(list) = list else {
        return Vec::new();
    };
    list.iter()
        .take(12)
        .enumerate()
        .map(|(index, item)| {
            let title = json_string(item, &["name", "title", "songName"])
                .unwrap_or_else(|| "Unknown Track".to_string());
            let artist = json_string(item, &["singer", "artist", "artists", "author"]);
            let source_track_id = json_string(
                item,
                &["id", "songmid", "songId", "hash", "rid", "mid", "mediaId"],
            );
            JiaochangPluginSearchedTrack {
                track_id: format!(
                    "plugin:{plugin_id}:{source}:{}:{}",
                    source_track_id.as_deref().unwrap_or("result"),
                    index
                ),
                plugin_id: plugin_id.to_string(),
                source: source.to_string(),
                title,
                artist,
                album: json_string(item, &["albumName", "album"]),
                duration: item
                    .get("duration")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
                source_track_id,
                music_info: item.clone(),
            }
        })
        .collect()
}

fn json_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| match item {
            serde_json::Value::String(text) if !text.trim().is_empty() => {
                Some(text.trim().to_string())
            }
            serde_json::Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
    })
}

async fn run_lx_ceru_node_worker(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut child = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(LX_CERU_NODE_WORKER)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("LX/Ceru isolate failed to start node worker: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.to_string().as_bytes())
            .await
            .map_err(|e| format!("LX/Ceru isolate failed to send input: {e}"))?;
    }
    let output = timeout(Duration::from_secs(12), child.wait_with_output())
        .await
        .map_err(|_| {
            "LX/Ceru isolate timed out; plugin may be blocked by anti-debug or network".to_string()
        })?
        .map_err(|e| format!("LX/Ceru isolate failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "LX/Ceru isolate exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).map_err(|e| {
        format!(
            "LX/Ceru isolate returned invalid JSON: {e}; stdout={}",
            stdout.trim()
        )
    })
}

fn validate_playable_url_shape(value: &str) -> Result<(), String> {
    let parsed =
        url::Url::parse(value).map_err(|e| format!("LX/Ceru resolved invalid URL: {e}"))?;
    if parsed.scheme() == "http" || parsed.scheme() == "https" {
        Ok(())
    } else {
        Err("LX/Ceru resolved URL must use http or https".to_string())
    }
}

fn map_lx_quality(quality: &str) -> &'static str {
    match quality {
        "ambient" => "128k",
        "standard" => "320k",
        "high" => "flac",
        "lossless" => "flac24bit",
        _ => "320k",
    }
}

fn infer_lx_ceru_providers(path: &Path) -> Result<Vec<JiaochangAudioPluginProvider>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read plugin file: {e}"))?;
    let mut provider_specs = vec![
        ("wy", "网易云音乐"),
        ("tx", "QQ音乐"),
        ("kw", "酷我音乐"),
        ("kg", "酷狗音乐"),
        ("mg", "咪咕音乐"),
    ];
    if text.contains("QISHUI_SOURCE_ID") || text.contains("\"qsvip\"") || text.contains("汽水VIP")
    {
        provider_specs.push(("qsvip", "汽水VIP"));
    }
    let mut providers = provider_specs
        .iter()
        .filter(|(id, label)| {
            text.contains(&format!("'{id}'"))
                || text.contains(&format!("\"{id}\""))
                || text.contains(label)
        })
        .map(|(provider_id, name)| JiaochangAudioPluginProvider {
            provider_id: (*provider_id).to_string(),
            name: (*name).to_string(),
            enabled: true,
            allowed_hosts: vec![],
            resolver_template: "lx-ceru-js-isolate".to_string(),
            actions: infer_lx_provider_actions(&text, provider_id),
        })
        .collect::<Vec<_>>();
    if providers.is_empty() {
        providers = provider_specs
            .iter()
            .map(|(provider_id, name)| JiaochangAudioPluginProvider {
                provider_id: (*provider_id).to_string(),
                name: (*name).to_string(),
                enabled: true,
                allowed_hosts: vec![],
                resolver_template: "lx-ceru-js-isolate".to_string(),
                actions: infer_lx_provider_actions(&text, provider_id),
            })
            .collect();
    }
    Ok(providers)
}

fn infer_lx_provider_actions(text: &str, provider_id: &str) -> Vec<String> {
    if provider_id == "qsvip" || text.contains(&format!("sourceConfig[{provider_id}")) {
        return vec![
            "musicSearch".to_string(),
            "musicUrl".to_string(),
            "lyric".to_string(),
        ];
    }
    vec!["musicUrl".to_string()]
}

fn infer_lx_ceru_allowed_hosts(
    path: &Path,
    inspection: &JiaochangAudioPluginInspection,
) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut hosts = inspection.supported_hosts.clone();
    for host in [
        "zrcdy.dpdns.org",
        "music-api.gdstudio.xyz",
        "api.vsaa.cn",
        "proxy.qishui.vsaa.cn",
        "sixyin.com",
        "www.sixyin.com",
    ] {
        if text.contains(host) {
            hosts.push(host.to_string());
        }
    }
    hosts.sort();
    hosts.dedup();
    hosts
}

fn sanitize_plugin_id(value: &str) -> String {
    let mut id = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase();
    if id.is_empty() {
        id = "plugin".to_string();
    }
    id
}

fn scan_plugin_risks(text: &str) -> Vec<String> {
    let checks = [
        ("new Function", "dynamic-code:new-function"),
        ("eval(", "dynamic-code:eval"),
        ("debugger", "anti-debug:debugger"),
        ("while(!![])", "anti-debug:infinite-loop-pattern"),
        ("window.document", "dom-shim:window-document"),
        ("localStorage", "renderer-storage:local-storage"),
        ("XMLHttpRequest", "network:xhr"),
        ("fetch(", "network:fetch"),
        ("User-Agent", "network:user-agent-header"),
        ("cookie", "network:cookie"),
    ];
    let mut flags = checks
        .iter()
        .filter(|(needle, _flag)| text.contains(needle))
        .map(|(_needle, flag)| (*flag).to_string())
        .collect::<Vec<_>>();
    if text.lines().any(|line| line.len() > 50_000) {
        flags.push("obfuscation:very-long-line".to_string());
    }
    flags.sort();
    flags.dedup();
    flags
}

fn scan_supported_hosts(text: &str) -> Vec<String> {
    let mut hosts = [
        "sixyin.com",
        "kuwo.cn",
        "kugou.com",
        "qq.com",
        "music.163.com",
        "migu.cn",
        "api.vsaa.cn",
        "proxy.qishui.vsaa.cn",
    ]
    .into_iter()
    .filter(|host| text.contains(host))
    .map(str::to_string)
    .collect::<Vec<_>>();
    hosts.sort();
    hosts.dedup();
    hosts
}

fn extract_plugin_meta(text: &str, key: &str) -> Option<String> {
    text.lines().take(12).find_map(|line| {
        let trimmed = line.trim().trim_start_matches('*').trim();
        trimmed
            .strip_prefix(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn cache_key(input: &JiaochangPluginTrackResolveInput) -> String {
    format!(
        "{}::{}::{}::{}",
        input.plugin_id,
        input.source,
        input.source_track_id.as_deref().unwrap_or(&input.track_id),
        input.quality
    )
}

fn emit_event(app: &AppHandle, event: JiaochangAudioPluginEvent) {
    let _ = app.emit(PLUGIN_EVENT, event);
}

fn default_enabled() -> bool {
    true
}

fn default_cache() -> bool {
    true
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

fn manifest_registry_path() -> PathBuf {
    if let Ok(path) = std::env::var("IF2AI_JIACHANG_AUDIO_PLUGIN_REGISTRY") {
        return PathBuf::from(path);
    }
    #[cfg(test)]
    {
        std::env::temp_dir().join("if2ai-jiaochang-audio-plugin-sources-test.json")
    }
    #[cfg(not(test))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".if2ai")
            .join("jiaochang")
            .join("audio")
            .join("plugin-sources.json")
    }
}

fn load_manifest_registry() -> Result<HashMap<String, JiaochangAudioPluginManifest>, String> {
    let path = manifest_registry_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read plugin source registry: {e}"))?;
    let manifests: Vec<JiaochangAudioPluginManifest> = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse plugin source registry: {e}"))?;
    let mut registry = HashMap::new();
    for manifest in manifests.into_iter().map(refresh_lx_ceru_manifest_shape) {
        validate_manifest(&manifest)?;
        registry.insert(manifest.plugin_id.clone(), manifest);
    }
    Ok(registry)
}

fn refresh_lx_ceru_manifest_shape(
    mut manifest: JiaochangAudioPluginManifest,
) -> JiaochangAudioPluginManifest {
    if manifest.compatibility != JiaochangAudioPluginCompatibility::LxCeruJs {
        return manifest;
    }
    let Some(path) = manifest.local_js_path.clone() else {
        return manifest;
    };
    let path = PathBuf::from(path);
    if let Ok(inferred) = infer_lx_ceru_providers(&path) {
        for provider in inferred {
            if let Some(existing) = manifest
                .providers
                .iter_mut()
                .find(|item| item.provider_id == provider.provider_id)
            {
                if existing.actions.is_empty() {
                    existing.actions = provider.actions;
                }
                continue;
            }
            manifest.providers.push(provider);
        }
    }
    if let Ok(inspection) = inspect_lx_ceru_plugin_file(&path) {
        let mut hosts = manifest.allowed_hosts;
        hosts.extend(infer_lx_ceru_allowed_hosts(&path, &inspection));
        hosts.sort();
        hosts.dedup();
        manifest.allowed_hosts = hosts;
    }
    manifest
}

fn write_manifest_registry(manifests: &[JiaochangAudioPluginManifest]) -> Result<(), String> {
    let path = manifest_registry_path();
    let parent = path
        .parent()
        .ok_or_else(|| "invalid plugin source registry path".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create plugin source registry dir: {e}"))?;
    let json = serde_json::to_string_pretty(manifests)
        .map_err(|e| format!("failed to serialize plugin source registry: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write plugin source registry: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> JiaochangAudioPluginManifest {
        JiaochangAudioPluginManifest {
            plugin_id: "demo.plugin".to_string(),
            name: "Demo".to_string(),
            version: Some("0.1.0".to_string()),
            source_url: Some("https://music.example.test/manifest.json".to_string()),
            signature: Some("sha256-demo".to_string()),
            enabled: true,
            allowed_hosts: vec!["music.example.test".to_string()],
            resolver_template: "https://music.example.test/{source}/{source_track_id}?q={quality}"
                .to_string(),
            compatibility: JiaochangAudioPluginCompatibility::ConstrainedManifest,
            providers: vec![],
            risk_flags: vec![],
            local_js_path: None,
        }
    }

    fn input() -> JiaochangPluginTrackResolveInput {
        JiaochangPluginTrackResolveInput {
            track_id: "track-1".to_string(),
            plugin_id: "demo.plugin".to_string(),
            source: "demo".to_string(),
            title: "Song".to_string(),
            artist: Some("Artist".to_string()),
            quality: "standard".to_string(),
            source_track_id: Some("abc 123".to_string()),
            plugin_music_info: None,
            use_cache: true,
        }
    }

    #[tokio::test]
    async fn worker_resolves_manifest_template_inside_allowed_host() {
        let registry = Arc::new(RwLock::new(HashMap::from([(
            "demo.plugin".to_string(),
            manifest(),
        )])));
        let resolved = resolve_in_worker(&registry, &input()).await.unwrap();
        assert_eq!(
            resolved.url,
            "https://music.example.test/demo/abc%20123?q=standard"
        );
    }

    #[tokio::test]
    async fn worker_rejects_url_outside_manifest_allowed_hosts() {
        let mut bad = manifest();
        bad.resolver_template = "https://evil.example.test/{track_id}".to_string();
        let registry = Arc::new(RwLock::new(HashMap::from([(
            "demo.plugin".to_string(),
            bad,
        )])));
        let error = resolve_in_worker(&registry, &input()).await.unwrap_err();
        assert!(error.contains("allowed_hosts"));
    }

    #[tokio::test]
    async fn runtime_cache_returns_second_resolution_without_worker_round_trip() {
        let runtime = JiaochangAudioPluginRuntime::new();
        runtime.register_plugin(manifest()).await.unwrap();
        let first = runtime.resolve_without_events(input()).await.unwrap();
        let second = runtime.resolve_without_events(input()).await.unwrap();
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
    }

    #[tokio::test]
    async fn runtime_persists_enabled_state_in_registry() {
        let runtime = JiaochangAudioPluginRuntime::new();
        runtime.register_plugin(manifest()).await.unwrap();
        let updated = runtime
            .set_plugin_enabled("demo.plugin", false)
            .await
            .unwrap();

        assert!(!updated.enabled);
        let listed = runtime.list_plugins().await;
        let demo = listed
            .iter()
            .find(|plugin| plugin.plugin_id == "demo.plugin")
            .unwrap();
        assert!(!demo.enabled);
    }

    #[tokio::test]
    async fn authorized_template_routes_each_provider_independently() {
        let manifest = authorized_chinese_sources_template();
        let registry = Arc::new(RwLock::new(HashMap::from([(
            manifest.plugin_id.clone(),
            manifest,
        )])));
        let mut request = input();
        request.plugin_id = "if2ai.authorized-cn-music".to_string();
        request.source = "netease".to_string();

        let resolved = resolve_in_worker(&registry, &request).await.unwrap();

        assert!(resolved.url.contains("/netease/resolve"));
        assert!(resolved.evidence.contains("provider 'netease'"));
    }

    #[test]
    fn inspect_lx_ceru_plugin_file_detects_known_markers_without_execution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.js");
        std::fs::write(
            &path,
            "/*!\n * @name 星海音乐源\n * @version v2.3.0\n * @homepage https://zrcdy.dpdns.org/\n */\nconst x = globalThis.lx.currentScriptInfo.rawScript; new Function('return this'); debugger;",
        )
        .unwrap();

        let inspection = inspect_lx_ceru_plugin_file(&path).unwrap();

        assert_eq!(
            inspection.detected_kind,
            JiaochangAudioPluginCompatibility::LxCeruJs
        );
        assert_eq!(inspection.name.as_deref(), Some("星海音乐源"));
        assert!(inspection
            .risk_flags
            .contains(&"dynamic-code:new-function".to_string()));
        assert!(!inspection.executable_in_renderer);
    }

    #[test]
    fn lx_ceru_import_manifest_preserves_local_path_and_providers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.js");
        std::fs::write(
            &path,
            "/*!\n * @name 星海音乐源\n * @version v2.3.0\n */\n// music.163.com\nconst source = 'kw'; globalThis.lx.on(globalThis.lx.EVENT_NAMES.request, async () => 'https://media.example.test/a.mp3');",
        )
        .unwrap();

        let manifest = manifest_from_lx_ceru_plugin_file(&path).unwrap();

        assert_eq!(
            manifest.compatibility,
            JiaochangAudioPluginCompatibility::LxCeruJs
        );
        assert_eq!(
            manifest.local_js_path.as_deref(),
            Some(path.to_str().unwrap())
        );
        assert!(manifest
            .providers
            .iter()
            .any(|provider| provider.provider_id == "kw"));
        assert!(manifest
            .signature
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[tokio::test]
    async fn lx_ceru_worker_resolves_mock_plugin_without_renderer_execution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mock-lx.js");
        std::fs::write(
            &path,
            "/*!\n * @name Mock LX\n * @version 1.0.0\n */\n// music.163.com\nconst source = 'kw';\nglobalThis.lx.on(globalThis.lx.EVENT_NAMES.request, async ({ action, source, info }) => {\n  if (action !== 'musicUrl') throw new Error('unexpected action');\n  return `https://media.example.test/${source}/${info.type}/${info.musicInfo.id}.mp3`;\n});",
        )
        .unwrap();
        let manifest = manifest_from_lx_ceru_plugin_file(&path).unwrap();
        let registry = Arc::new(RwLock::new(HashMap::from([(
            manifest.plugin_id.clone(),
            manifest,
        )])));
        let mut request = input();
        request.plugin_id = "lx-ceru.mock-lx".to_string();
        request.source = "kw".to_string();
        request.source_track_id = Some("abc123".to_string());

        let resolved = resolve_in_worker(&registry, &request).await.unwrap();

        assert_eq!(
            resolved.url,
            "https://media.example.test/kw/320k/abc123.mp3"
        );
        assert!(resolved.evidence.contains("LX/Ceru JS isolate"));
    }

    #[tokio::test]
    async fn lx_ceru_worker_searches_searchable_provider_before_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mock-search-lx.js");
        std::fs::write(
            &path,
            "/*!\n * @name Mock Search LX\n * @version 1.0.0\n */\nconst QISHUI_SOURCE_ID = 'qsvip';\nglobalThis.lx.send(globalThis.lx.EVENT_NAMES.inited, { sources: { qsvip: { actions: ['musicSearch', 'musicUrl'] } } });\nglobalThis.lx.on(globalThis.lx.EVENT_NAMES.request, async ({ action, info }) => {\n  if (action === 'musicSearch') return { list: [{ id: 'real-id-1', name: info.keyword, singer: 'Singer' }] };\n  if (action === 'musicUrl') return `https://media.example.test/${info.musicInfo.id}.mp3`;\n  throw new Error('unexpected action');\n});",
        )
        .unwrap();
        let manifest = manifest_from_lx_ceru_plugin_file(&path).unwrap();
        let registry = Arc::new(RwLock::new(HashMap::from([(
            manifest.plugin_id.clone(),
            manifest,
        )])));

        let results = search_lx_ceru_tracks(
            &registry,
            &JiaochangPluginTrackSearchInput {
                plugin_id: "lx-ceru.mock-search-lx".to_string(),
                source: "qsvip".to_string(),
                query: "Song".to_string(),
                page: 1,
                page_size: 5,
            },
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_track_id.as_deref(), Some("real-id-1"));
        assert_eq!(results[0].title, "Song");
    }

    impl JiaochangAudioPluginRuntime {
        async fn resolve_without_events(
            &self,
            input: JiaochangPluginTrackResolveInput,
        ) -> Result<JiaochangPluginResolvedTrack, String> {
            if input.use_cache {
                let key = cache_key(&input);
                if let Some(cached) = self.cache.read().await.get(&key).cloned() {
                    return Ok(JiaochangPluginResolvedTrack {
                        cache_hit: true,
                        ..cached
                    });
                }
            }
            let key = cache_key(&input);
            let (reply, rx) = oneshot::channel();
            self.tx
                .send(ResolveJob { input, reply })
                .await
                .map_err(|_| "worker unavailable".to_string())?;
            let resolved = rx.await.map_err(|_| "worker dropped".to_string())??;
            self.cache.write().await.insert(key, resolved.clone());
            Ok(resolved)
        }
    }
}
