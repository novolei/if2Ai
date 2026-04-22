use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::{AppState, AppStateConfig, ProviderHandle, TtsState};

/// Fully assembled managed state for the native desktop host.
///
/// `main.rs` should build domain dependencies once, then hand the
/// resulting composition to the desktop host builder instead of
/// owning host-specific bootstrap details inline.
pub struct DesktopHostComposition {
    /// Shared application state exposed to the business command layer.
    pub app_state: AppState,
    /// Managed TTS runtime state used by streaming / synthesis commands.
    pub tts_state: TtsState,
    /// Managed download-progress state for TTS model acquisition flows.
    pub tts_download_state: Arc<Mutex<super::tts_download::TtsDownloadState>>,
}

/// Build the managed state required by the native desktop host.
///
/// This is the seam between domain dependency assembly and host
/// registration. `main.rs` provides the already-initialized domain
/// collaborators, and this module materializes the Tauri-managed
/// state objects that the desktop shell owns.
///
/// MEM-MOD-P7 — `learned_traits` is opt-in: when bootstrap successfully
/// opened the SQLite-backed store it is attached to the AppState here
/// via the builder; on failure the AppState ships with `learned_traits =
/// None` and the related IPCs degrade gracefully to "no traits".
#[must_use]
pub fn compose_desktop_host_state(
    app_state_config: AppStateConfig,
    learned_traits: Option<crate::modules::memory::learned_traits::LearnedTraitsStore>,
) -> DesktopHostComposition {
    let mut app_state = AppState::new(app_state_config);
    if let Some(store) = learned_traits {
        app_state = app_state.with_learned_traits(store);
    }
    DesktopHostComposition {
        app_state,
        tts_state: build_tts_state(),
        tts_download_state: Arc::new(Mutex::new(super::tts_download::TtsDownloadState::default())),
    }
}

fn build_tts_state() -> TtsState {
    let factory: Arc<
        dyn Fn() -> Result<
                Arc<dyn crate::modules::tts::TtsProvider>,
                crate::modules::tts::error::TtsError,
            > + Send
            + Sync
            + 'static,
    > = Arc::new(|| {
        let model_root = resolve_tts_model_root();
        let manifest_present = model_root
            .join("MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json")
            .is_file();
        if manifest_present {
            let provider = crate::modules::tts::provider::OnnxTtsProvider::from_model_dir(
                &model_root,
                Some(4),
            )?;
            tracing::info!(
                model_dir = %model_root.display(),
                "TTS provider loaded (OnnxTtsProvider)"
            );
            Ok(Arc::new(provider) as Arc<dyn crate::modules::tts::TtsProvider>)
        } else {
            tracing::info!(
                "TTS model not found at {}; using Mock provider",
                model_root.display()
            );
            Ok(
                Arc::new(crate::modules::tts::provider::MockTtsProvider::new())
                    as Arc<dyn crate::modules::tts::TtsProvider>,
            )
        }
    });

    let factory_clone = factory.clone();
    let provider_handle = Arc::new(ProviderHandle::new(move || (factory_clone)(), None));

    let (state_arc, state_changed_at) = provider_handle.state_handle();
    let boot_for_evict = provider_handle.boot();
    let on_evict: Arc<dyn Fn() + Send + Sync + 'static> = Arc::new(move || {
        let now_ms = boot_for_evict.elapsed().as_millis() as i64;
        state_changed_at.store(now_ms, std::sync::atomic::Ordering::Relaxed);
        let state_arc = state_arc.clone();
        let _ = tauri::async_runtime::block_on(async move {
            let mut guard = state_arc.write().await;
            *guard = super::tts::ProviderState::Evicted {
                elapsed_seconds: 0.0,
            };
        });
    });

    let evictor = Arc::new(crate::modules::tts::manager::eviction::IdleEvictor::spawn(
        crate::modules::tts::manager::eviction::IdleEvictionConfig::default(),
        provider_handle.slot(),
        provider_handle.last_use_millis(),
        provider_handle.boot(),
        Some(on_evict),
    ));

    TtsState {
        provider: provider_handle,
        warmup: Arc::new(crate::modules::tts::manager::warmup::WarmupManager::new()),
        jobs: Arc::new(crate::modules::tts::manager::jobs::StreamingJobManager::new()),
        _evictor: Some(evictor),
    }
}

fn resolve_tts_model_root() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".if2ai/models/tts"))
        .unwrap_or_else(|| PathBuf::from(".if2ai/models/tts"))
}
