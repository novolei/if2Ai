//! Tauri commands for Jiaochang audio plugin sources.

use crate::modules::jiaochang_audio::{
    plugin_runtime, JiaochangAudioPluginCacheInfo, JiaochangAudioPluginManifest,
    JiaochangPluginResolvedTrack, JiaochangPluginTrackResolveInput,
};
use tauri::{AppHandle, Emitter};

/// Register or replace a Rust-side constrained plugin source manifest.
#[tauri::command]
pub async fn jiaochang_audio_plugin_register(
    app: AppHandle,
    manifest: JiaochangAudioPluginManifest,
) -> Result<JiaochangAudioPluginManifest, String> {
    let registered = plugin_runtime().register_plugin(manifest).await?;
    app.emit(
        "jiaochang://audio/plugin-event",
        serde_json::json!({
            "event_type": "registered",
            "plugin_id": registered.plugin_id,
            "message": "plugin source registered",
            "timestamp": chrono::Utc::now(),
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(registered)
}

/// List plugin source manifests known to the Rust-side worker.
#[tauri::command]
pub async fn jiaochang_audio_plugin_list() -> Result<Vec<JiaochangAudioPluginManifest>, String> {
    Ok(plugin_runtime().list_plugins().await)
}

/// Resolve a playable plugin track URL through cache and the Rust worker.
#[tauri::command]
pub async fn jiaochang_audio_plugin_resolve_track_url(
    app: AppHandle,
    input: JiaochangPluginTrackResolveInput,
) -> Result<JiaochangPluginResolvedTrack, String> {
    plugin_runtime().resolve_track_url(&app, input).await
}

/// Return server-side plugin URL cache diagnostics.
#[tauri::command]
pub async fn jiaochang_audio_plugin_cache_info() -> Result<JiaochangAudioPluginCacheInfo, String> {
    Ok(plugin_runtime().cache_info().await)
}

/// Clear server-side plugin URL cache entries.
#[tauri::command]
pub async fn jiaochang_audio_plugin_cache_clear() -> Result<JiaochangAudioPluginCacheInfo, String> {
    Ok(plugin_runtime().clear_cache().await)
}
