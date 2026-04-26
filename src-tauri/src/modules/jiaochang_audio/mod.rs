//! Jiaochang audio plugin source sandbox.
//!
//! The renderer never executes source plugin code.  Plugin source manifests are
//! registered here as constrained URL resolvers, and every playable URL request
//! is processed by a Tokio worker before being returned to the frontend.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot, RwLock};

const PLUGIN_EVENT: &str = "jiaochang://audio/plugin-event";
const WORKER_QUEUE: usize = 64;

static RUNTIME: OnceLock<Arc<JiaochangAudioPluginRuntime>> = OnceLock::new();

/// Manifest supplied by a plugin source installer or settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiaochangAudioPluginManifest {
    pub plugin_id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    pub resolver_template: String,
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
    #[serde(default = "default_cache")]
    pub use_cache: bool,
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
        let registry = Arc::new(RwLock::new(HashMap::new()));
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
        Ok(manifest)
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
        emit_event(app, JiaochangAudioPluginEvent::resolving(&plugin_id, &track_id));
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
}

impl Default for JiaochangAudioPluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl JiaochangAudioPluginEvent {
    fn resolving(plugin_id: &str, track_id: &str) -> Self {
        Self::new("resolving", plugin_id, Some(track_id), "plugin worker resolving URL")
    }

    fn resolved(plugin_id: &str, track_id: &str, url: &str) -> Self {
        let mut event = Self::new("resolved", plugin_id, Some(track_id), "plugin worker resolved URL");
        event.url = Some(url.to_string());
        event
    }

    fn cache_hit(plugin_id: &str, track_id: &str, url: &str) -> Self {
        let mut event = Self::new("cache_hit", plugin_id, Some(track_id), "server URL cache hit");
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
        return Err(format!("Plugin source '{}' is disabled", manifest.plugin_id));
    }
    let url = render_template(&manifest.resolver_template, input);
    validate_url_against_manifest(&url, &manifest)?;
    Ok(JiaochangPluginResolvedTrack {
        track_id: input.track_id.clone(),
        plugin_id: input.plugin_id.clone(),
        url,
        quality: input.quality.clone(),
        cache_hit: false,
        resolved_at: Utc::now(),
        evidence: "rust-side plugin worker resolved URL from constrained manifest".to_string(),
    })
}

fn validate_manifest(manifest: &JiaochangAudioPluginManifest) -> Result<(), String> {
    validate_identifier("plugin_id", &manifest.plugin_id)?;
    if manifest.name.trim().is_empty() {
        return Err("Plugin name is required".to_string());
    }
    if manifest.resolver_template.trim().is_empty() {
        return Err("Plugin resolver_template is required".to_string());
    }
    if manifest.allowed_hosts.is_empty() {
        return Err("Plugin allowed_hosts must declare at least one host".to_string());
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
        ("{source_track_id}", input.source_track_id.as_deref().unwrap_or("")),
        ("{quality}", input.quality.as_str()),
        ("{title}", input.title.as_str()),
        ("{artist}", input.artist.as_deref().unwrap_or("")),
    ];
    replacements.iter().fold(template.to_string(), |acc, (key, value)| {
        acc.replace(key, &urlencoding::encode(value))
    })
}

fn validate_url_against_manifest(
    url: &str,
    manifest: &JiaochangAudioPluginManifest,
) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Plugin resolved invalid URL: {e}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err("Plugin resolved URL must use http or https".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Plugin resolved URL has no host".to_string())?;
    if manifest.allowed_hosts.iter().any(|allowed| allowed == host) {
        Ok(())
    } else {
        Err(format!("Plugin host '{host}' is not in allowed_hosts"))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> JiaochangAudioPluginManifest {
        JiaochangAudioPluginManifest {
            plugin_id: "demo.plugin".to_string(),
            name: "Demo".to_string(),
            version: Some("0.1.0".to_string()),
            enabled: true,
            allowed_hosts: vec!["music.example.test".to_string()],
            resolver_template:
                "https://music.example.test/{source}/{source_track_id}?q={quality}".to_string(),
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
