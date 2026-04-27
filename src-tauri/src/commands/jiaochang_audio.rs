//! Tauri commands for Jiaochang audio plugin sources.

use crate::modules::jiaochang_audio::{
    inspect_lx_ceru_plugin_file, manifest_from_lx_ceru_plugin_file, plugin_runtime,
    JiaochangAudioPluginCacheInfo, JiaochangAudioPluginInspection, JiaochangAudioPluginManifest,
    JiaochangPluginResolvedTrack, JiaochangPluginSearchedTrack, JiaochangPluginTrackResolveInput,
    JiaochangPluginTrackSearchInput,
};
use std::path::PathBuf;
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

/// Install the built-in authorized multi-source template.
#[tauri::command]
pub async fn jiaochang_audio_plugin_install_authorized_cn_template(
    app: AppHandle,
) -> Result<JiaochangAudioPluginManifest, String> {
    let registered = plugin_runtime()
        .install_authorized_chinese_sources_template()
        .await?;
    app.emit(
        "jiaochang://audio/plugin-event",
        serde_json::json!({
            "event_type": "registered",
            "plugin_id": registered.plugin_id,
            "message": "authorized CN music source template installed",
            "timestamp": chrono::Utc::now(),
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(registered)
}

/// Statically inspect a local LX/Ceru JS plugin without executing it.
#[tauri::command]
pub async fn jiaochang_audio_plugin_inspect_js_file(
    path: String,
) -> Result<JiaochangAudioPluginInspection, String> {
    inspect_lx_ceru_plugin_file(&PathBuf::from(path))
}

/// Import a local LX/Ceru JS plugin as a Rust-side isolate source.
#[tauri::command]
pub async fn jiaochang_audio_plugin_import_lx_ceru_js_file(
    app: AppHandle,
    path: String,
) -> Result<JiaochangAudioPluginManifest, String> {
    let manifest = manifest_from_lx_ceru_plugin_file(&PathBuf::from(path))?;
    let registered = plugin_runtime().register_plugin(manifest).await?;
    app.emit(
        "jiaochang://audio/plugin-event",
        serde_json::json!({
            "event_type": "registered",
            "plugin_id": registered.plugin_id,
            "message": "LX/Ceru JS plugin registered for Rust-side isolate",
            "timestamp": chrono::Utc::now(),
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(registered)
}

/// Persistently enable or disable a registered plugin source.
#[tauri::command]
pub async fn jiaochang_audio_plugin_set_enabled(
    app: AppHandle,
    plugin_id: String,
    enabled: bool,
) -> Result<JiaochangAudioPluginManifest, String> {
    let updated = plugin_runtime()
        .set_plugin_enabled(&plugin_id, enabled)
        .await?;
    app.emit(
        "jiaochang://audio/plugin-event",
        serde_json::json!({
            "event_type": "registered",
            "plugin_id": updated.plugin_id,
            "message": if updated.enabled { "plugin source enabled" } else { "plugin source disabled" },
            "timestamp": chrono::Utc::now(),
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(updated)
}

/// Remove a registered plugin source and clear its cache entries.
#[tauri::command]
pub async fn jiaochang_audio_plugin_remove(
    app: AppHandle,
    plugin_id: String,
) -> Result<(), String> {
    plugin_runtime().remove_plugin(&plugin_id).await?;
    app.emit(
        "jiaochang://audio/plugin-event",
        serde_json::json!({
            "event_type": "removed",
            "plugin_id": plugin_id,
            "message": "plugin source removed",
            "timestamp": chrono::Utc::now(),
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Search plugin tracks through an LX/Ceru compatible Rust-side worker.
#[tauri::command]
pub async fn jiaochang_audio_plugin_search_tracks(
    input: JiaochangPluginTrackSearchInput,
) -> Result<Vec<JiaochangPluginSearchedTrack>, String> {
    plugin_runtime().search_tracks(input).await
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
