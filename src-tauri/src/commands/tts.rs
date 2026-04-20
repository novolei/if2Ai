//! TTS (Text-to-Speech) Tauri commands.
//!
//! Provides 12 commands mirroring the Python `app.py` endpoints:
//! - `tts_health`: Check TTS system health
//! - `tts_warmup_status`: Get warmup progress/state
//! - `tts_start_warmup`: Trigger async warmup
//! - `tts_synthesize`: Buffered synthesis (returns WAV as base64)
//! - `tts_stream_start`: Start streaming synthesis job
//! - `tts_stream_status`: Poll job status
//! - `tts_stream_result`: Get final job result
//! - `tts_stream_close`: Cancel/close a stream
//! - `tts_demo_audio`: Get demo audio as base64
//! - `tts_list_voices`: List available voice names
//! - `tts_split_text`: Split text for voice clone preview
//!
//! All commands require `TtsState` to be registered via `.manage()`.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::modules::tts::config::GenerationParams;
use crate::modules::tts::manager::jobs::{StreamingJob, StreamingJobManager};
use crate::modules::tts::manager::warmup::WarmupManager;
use crate::modules::tts::voice::demo::{resolve_demo_audio_path, resolve_demo_voice_name};
use crate::modules::tts::voice::registry::{VoiceAsset, VoiceKind, VoiceRegistry};
use crate::modules::tts::{SynthesisMode, SynthesisParams, TtsProvider};
use tauri::Manager;

/// Phase TTS-D / P0：把 voice_id / demo_id / prompt_audio_path 三种输入
/// 路由成 Provider 接受的 (voice, prompt_audio_path) 元组。
///
/// 优先级：voice_id > demo_id > prompt_audio_path
/// - voice_id (builtin) → (Some("Junhao"), None) 走 manifest prebaked codes
/// - voice_id (bundled / user) → (None, Some(<asset_audio_path>)) 走 codec.encode
/// - demo_id → (Some(<demo.voice_name>), None)
/// - prompt_audio_path → (None, Some(<path>))
fn resolve_voice_source(
    app: &tauri::AppHandle,
    voice_id: Option<&str>,
    demo_id: Option<&str>,
    prompt_audio_path: Option<&str>,
) -> (Option<String>, Option<std::path::PathBuf>) {
    if let Some(vid) = voice_id {
        let registry = scan_voice_registry(app);
        if let Some(asset) = registry.find(vid) {
            return match asset.kind {
                VoiceKind::Builtin => (Some(asset.id.clone()), None),
                _ => (None, asset.audio_path.clone()),
            };
        }
        // 未知 voice_id：当作 builtin name 试试（兼容前端传 raw "Junhao"）
        return (Some(vid.to_string()), None);
    }
    if let Some(did) = demo_id {
        return (resolve_demo_voice_name(did), None);
    }
    (None, prompt_audio_path.map(std::path::PathBuf::from))
}

/// Phase TTS-D / P0：扫描 builtin + bundled + user voices 构建注册表。
///
/// `app` 用于拿 resource_dir（打包带的 wav）和 app_data_dir（用户上传）。
fn scan_voice_registry(app: &tauri::AppHandle) -> VoiceRegistry {
    // 1. Builtin voice 列表 —— 没有 OnnxTtsProvider 时提供硬编码兜底
    let builtin: Vec<(&'static str, &'static str)> = vec![
        ("Junhao", "Junhao · 中文男 A"),
        ("Zhiming", "Zhiming · 中文男 B"),
        ("Xiaoyu", "Xiaoyu · 中文女 A"),
        ("Yuewen", "Yuewen · 中文女 B"),
        ("Weiguo", "Weiguo · 中文男 C"),
        ("Lingyu", "Lingyu · 中文女 C"),
        ("Trump", "Trump · 英文男"),
        ("Ava", "Ava · 英文女 A"),
        ("Bella", "Bella · 英文女 B"),
        ("Adam", "Adam · 英文男 A"),
        ("Nathan", "Nathan · 英文男 B"),
        ("Soyo", "Soyo · 日文女"),
        ("Saki", "Saki · 日文女"),
        ("Mortis", "Mortis"),
        ("Umiri", "Umiri"),
        ("Mei", "Mei"),
        ("Anon", "Anon"),
        ("Arisa", "Arisa"),
    ];

    let resource_voice_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|r| r.join("resources").join("voices"));
    // dev 模式下 resource_dir 是项目根，prod 是 .app/Contents/Resources
    let resource_dev_fallback = std::env::current_dir()
        .ok()
        .map(|c| c.join("src-tauri").join("resources").join("voices"));
    let user_voice_dir = app.path().app_data_dir().ok().map(|d| d.join("voices"));
    let model_demo_dir = dirs::home_dir().map(|h| h.join(".if2ai/models/tts/voices"));

    // 优先 resource_dir，dev fallback 兜底
    let primary_resource = resource_voice_dir
        .as_deref()
        .filter(|d| d.exists())
        .or_else(|| resource_dev_fallback.as_deref().filter(|d| d.exists()));

    VoiceRegistry::scan(
        &builtin,
        primary_resource,
        user_voice_dir.as_deref(),
        model_demo_dir.as_deref(),
    )
}

/// Response from [`tts_health`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsHealthResponse {
    pub status: String,
    pub warmup_state: String,
    pub warmup_progress: f32,
    pub message: String,
    /// Phase TTS-D.1：Provider 生命周期状态（前端可据此显示 loading bar 等）。
    pub provider_state: ProviderState,
    /// Phase TTS-E / P2：当前合成/等待请求数（包括 Semaphore 等待中的）。
    pub queue_depth: i32,
}

/// Response from [`tts_warmup_status`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupStatusResponse {
    pub state: String,
    pub progress: f32,
    pub message: String,
    pub error: Option<String>,
}

/// Response from [`tts_synthesize`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResponse {
    /// WAV audio as base64 string.
    pub audio_base64: String,
    /// Sample rate (always 48000).
    pub sample_rate: u32,
    /// Duration in seconds.
    pub duration_seconds: f32,
    /// Voice used.
    pub voice: String,
    /// Text chunks generated.
    pub text_chunks: Vec<String>,
}

/// Response from [`tts_stream_start`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStartResponse {
    pub stream_id: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Response from [`tts_demo_audio`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoAudioResponse {
    pub audio_base64: String,
    pub content_type: String,
}

/// Phase TTS-B.2：每 audio chunk 通过 Tauri event 推前端的 payload。
///
/// 前端用 `listen('tts:stream-chunk', cb)` 订阅；`pcm_base64` 是 PCM16LE 编码
/// 的二进制（每帧 = `channels * 2` 字节），Web Audio API 解码为 Float32 后
/// 通过 AudioBufferSourceNode 链式调度即可 gapless 播放。
#[derive(Debug, Clone, Serialize)]
pub struct TtsStreamChunkEvent {
    pub stream_id: String,
    pub chunk_index: usize,
    pub sample_rate: u32,
    pub channels: u16,
    pub pcm_base64: String,
    pub emitted_audio_seconds: f32,
    pub lead_seconds: f32,
}

/// `tts:stream-end` event payload.
///
/// 在 `run_streaming_synthesis` 完成（成功 / 失败 / panic）后**一定会发**一次。
/// 前端 `useAgentVoiceBridge` 必须等到该事件再启动下一句的 stream，否则
/// `setExpectedStreamId` 切换时会把上一句仍在传输的 chunks 丢弃，导致朗读不完整。
#[derive(Debug, Clone, Serialize)]
pub struct TtsStreamEndEvent {
    pub stream_id: String,
    /// 总输出 PCM 时长（秒）。前端可据此估算"还要播多久"
    pub total_audio_seconds: f32,
    pub success: bool,
    /// 失败时非空
    pub error: Option<String>,
}

/// Phase TTS-D.1：Provider 的生命周期状态机，前端轮询 `tts_health` 观察。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ProviderState {
    /// 还没被任何请求触发过加载（首次启动 / 刚 drop 完）。
    NotLoaded,
    /// 正在 load ONNX sessions + sidecar（典型 ~3s）。
    Loading,
    /// 已加载，可以接受请求。
    Loaded {
        /// 加载完成至今的秒数。
        elapsed_seconds: f32,
    },
    /// 上次 load 失败（model 缺失、ONNX 异常等）。
    Failed { error: String },
    /// IdleEvictor 触发释放 → 等下次 `get()` 重新 load。
    Evicted {
        /// 释放至今的秒数。
        elapsed_seconds: f32,
    },
}

impl Default for ProviderState {
    fn default() -> Self {
        Self::NotLoaded
    }
}

/// Phase TTS-C.1：TTS Provider 的 lazy 生命周期句柄。
///
/// 解决两个问题：
/// 1. **首次启动加快**：不在 main.rs setup 时同步 load 4 个 ONNX session（避免
///    冷启动 +3s）。第一次调 [`ProviderHandle::get`] 时再 lazy 加载。
/// 2. **空闲驱逐**：`IdleEvictor` 5 分钟无请求时把 provider drop（释放
///    ~1.5GB RAM）。下次 `get()` 重新 load。
///
/// 内部结构：
/// - `slot: Arc<RwLock<Option<Arc<dyn TtsProvider>>>>` —— 当前 provider；None
///   表示未加载或被 evict。
/// - `factory: Arc<dyn Fn() -> Result<Arc<dyn TtsProvider>>>` —— 重新构造的
///   工厂闭包（捕获 model_dir / threads 配置）。
/// - `last_use_millis: Arc<AtomicI64>` —— evictor 共享的活跃打卡。
/// - `state: Arc<RwLock<ProviderState>>` —— Phase TTS-D.1，前端可观察。
/// - `queue_depth: Arc<AtomicI32>` —— Phase TTS-E / P2，等待 Semaphore 的请求数。
pub struct ProviderHandle {
    slot: Arc<tokio::sync::RwLock<Option<Arc<dyn TtsProvider>>>>,
    factory: Arc<ProviderFactory>,
    last_use_millis: Arc<std::sync::atomic::AtomicI64>,
    boot: std::time::Instant,
    state: Arc<tokio::sync::RwLock<ProviderState>>,
    /// 上次状态变化的墙钟（boot 起算 ms），用于 elapsed_seconds 计算。
    state_changed_at_ms: Arc<std::sync::atomic::AtomicI64>,
    /// Phase TTS-E / P2：当前在 Semaphore 等待队列中的请求数。
    queue_depth: Arc<std::sync::atomic::AtomicI32>,
}

type ProviderFactory = dyn Fn() -> Result<Arc<dyn TtsProvider>, crate::modules::tts::error::TtsError>
    + Send
    + Sync
    + 'static;

impl ProviderHandle {
    /// 用一个工厂闭包构造句柄；可选传入初始 provider（首次启动若已加载好）。
    pub fn new(
        factory: impl Fn() -> Result<Arc<dyn TtsProvider>, crate::modules::tts::error::TtsError>
            + Send
            + Sync
            + 'static,
        initial: Option<Arc<dyn TtsProvider>>,
    ) -> Self {
        let boot = std::time::Instant::now();
        let initial_state = if initial.is_some() {
            ProviderState::Loaded {
                elapsed_seconds: 0.0,
            }
        } else {
            ProviderState::NotLoaded
        };
        Self {
            slot: Arc::new(tokio::sync::RwLock::new(initial)),
            factory: Arc::new(factory),
            last_use_millis: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            boot,
            state: Arc::new(tokio::sync::RwLock::new(initial_state)),
            state_changed_at_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            queue_depth: Arc::new(std::sync::atomic::AtomicI32::new(0)),
        }
    }

    /// Phase TTS-E / P2：当前等待 Semaphore 或正在合成的请求数。
    pub fn queue_depth(&self) -> i32 {
        self.queue_depth.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 队列进入（provider.get() 返回但 Semaphore permit 未释放前由调用方管）。
    pub fn inc_queue(&self) {
        self.queue_depth
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 队列离开。
    pub fn dec_queue(&self) {
        self.queue_depth
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 拿当前 provider；不在 → 用 factory 现场加载。
    /// 同时把"上次使用时间"打卡到 `last_use_millis`，evictor 用它判 idle。
    pub async fn get(&self) -> Result<Arc<dyn TtsProvider>, String> {
        // 打卡（不等加载完成；evictor 看到非 0 即认为活跃）
        self.last_use_millis.store(
            (self.boot.elapsed().as_millis() as i64).max(1),
            std::sync::atomic::Ordering::Relaxed,
        );
        // Fast path：已加载
        {
            let read = self.slot.read().await;
            if let Some(p) = read.as_ref() {
                return Ok(p.clone());
            }
        }
        // Slow path：lazy load（写锁 + 双重检查）
        let mut write = self.slot.write().await;
        if let Some(p) = write.as_ref() {
            return Ok(p.clone());
        }
        // 标记 Loading 让前端可见
        self.set_state(ProviderState::Loading).await;
        tracing::info!("TTS provider lazy load triggered");
        match (self.factory)() {
            Ok(new_provider) => {
                *write = Some(new_provider.clone());
                self.set_state(ProviderState::Loaded {
                    elapsed_seconds: 0.0,
                })
                .await;
                Ok(new_provider)
            }
            Err(e) => {
                let msg = format!("{e}");
                self.set_state(ProviderState::Failed { error: msg.clone() })
                    .await;
                Err(format!("TTS provider lazy load failed: {msg}"))
            }
        }
    }

    /// 给 IdleEvictor / 测试用：拿 slot Arc 共享（evictor 把 None 写回触发释放）。
    pub fn slot(&self) -> Arc<tokio::sync::RwLock<Option<Arc<dyn TtsProvider>>>> {
        self.slot.clone()
    }

    pub fn last_use_millis(&self) -> Arc<std::sync::atomic::AtomicI64> {
        self.last_use_millis.clone()
    }

    pub fn boot(&self) -> std::time::Instant {
        self.boot
    }

    /// Phase TTS-D.1：拿当前状态快照（含 elapsed_seconds 自动计算）。
    pub async fn current_state(&self) -> ProviderState {
        let s = self.state.read().await.clone();
        let changed_ms = self
            .state_changed_at_ms
            .load(std::sync::atomic::Ordering::Relaxed);
        let now_ms = self.boot.elapsed().as_millis() as i64;
        let elapsed = ((now_ms - changed_ms).max(0) as f32) / 1000.0;
        match s {
            ProviderState::Loaded { .. } => ProviderState::Loaded {
                elapsed_seconds: elapsed,
            },
            ProviderState::Evicted { .. } => ProviderState::Evicted {
                elapsed_seconds: elapsed,
            },
            other => other,
        }
    }

    /// Phase TTS-D.1：暴露 state Arc 给 IdleEvictor 用（evict 时写 Evicted）。
    pub fn state_handle(
        &self,
    ) -> (
        Arc<tokio::sync::RwLock<ProviderState>>,
        Arc<std::sync::atomic::AtomicI64>,
    ) {
        (self.state.clone(), self.state_changed_at_ms.clone())
    }

    async fn set_state(&self, new_state: ProviderState) {
        let now_ms = self.boot.elapsed().as_millis() as i64;
        self.state_changed_at_ms
            .store(now_ms, std::sync::atomic::Ordering::Relaxed);
        let mut guard = self.state.write().await;
        *guard = new_state;
    }
}

/// TTS state managed by Tauri.
///
/// Holds the TTS provider handle, warmup manager, and streaming job manager.
pub struct TtsState {
    pub provider: Arc<ProviderHandle>,
    pub warmup: Arc<WarmupManager>,
    pub jobs: Arc<StreamingJobManager>,
    /// IdleEvictor 句柄保留以维持后台 ticker 生命；drop 时 ticker 自动停。
    pub _evictor: Option<Arc<crate::modules::tts::manager::eviction::IdleEvictor>>,
}

/// Check TTS system health and model status.
///
/// Returns the current warmup state and whether the system is ready.
#[tauri::command]
pub async fn tts_health(state: tauri::State<'_, TtsState>) -> Result<TtsHealthResponse, String> {
    let snapshot = state.warmup.snapshot().await;
    let provider_state = state.provider.current_state().await;
    let queue_depth = state.provider.queue_depth();
    Ok(TtsHealthResponse {
        status: if snapshot.is_ready() {
            "ready"
        } else if snapshot.is_failed() {
            "failed"
        } else {
            "initializing"
        }
        .to_string(),
        warmup_state: snapshot.state.clone(),
        warmup_progress: snapshot.progress,
        message: snapshot.message.clone(),
        provider_state,
        queue_depth,
    })
}

/// Get the current warmup status.
///
/// Returns the warmup state, progress percentage, and status message.
#[tauri::command]
pub async fn tts_warmup_status(
    state: tauri::State<'_, TtsState>,
) -> Result<WarmupStatusResponse, String> {
    let snapshot = state.warmup.snapshot().await;
    Ok(WarmupStatusResponse {
        state: snapshot.state,
        progress: snapshot.progress,
        message: snapshot.message,
        error: snapshot.error,
    })
}

/// Trigger the TTS warmup sequence (runs in background).
///
/// Idempotent: calling multiple times only starts warmup once.
#[tauri::command]
pub async fn tts_start_warmup(state: tauri::State<'_, TtsState>) -> Result<(), String> {
    let provider = state.provider.get().await?;
    state.warmup.start(provider).await;
    Ok(())
}

/// Buffered synthesis — generates complete WAV audio and returns it as base64.
///
/// Mirrors the Python `/api/generate` endpoint.
///
/// # Arguments
///
/// * `text` - Text to synthesize.
/// * `demo_id` - Optional demo ID to use demo prompt audio.
/// * `prompt_audio_path` - Optional path to prompt audio for voice clone.
/// * `params` - Generation parameters (sampling, max frames, etc.).
#[tauri::command]
pub async fn tts_synthesize(
    app: tauri::AppHandle,
    state: tauri::State<'_, TtsState>,
    text: String,
    demo_id: Option<String>,
    voice_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<SynthesisResponse, String> {
    let provider = state.provider.get().await?;
    state.warmup.ensure_ready(provider.clone()).await;
    // Phase TTS-E / P2：queue depth tracking
    state.provider.inc_queue();
    let prov_handle = state.provider.clone();

    // 优先级：voice_id > demo_id > prompt_audio_path
    // - voice_id（builtin / bundled / user）→ registry 解析
    // - demo_id → demo entry 对应 builtin voice name
    // - prompt_audio_path → 直接用文件路径
    let (resolved_voice, resolved_prompt_path) = resolve_voice_source(
        &app,
        voice_id.as_deref(),
        demo_id.as_deref(),
        prompt_audio_path.as_deref(),
    );

    let synthesis_params = SynthesisParams {
        text,
        mode: SynthesisMode::VoiceClone,
        voice: resolved_voice,
        prompt_audio_path: resolved_prompt_path,
        prompt_text: None,
        generation: params,
    };

    let result = provider
        .synthesize(synthesis_params)
        .await
        .map_err(|e| format!("Synthesis failed: {e}"))?;

    prov_handle.dec_queue();
    Ok(SynthesisResponse {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(&result.audio_bytes),
        sample_rate: result.sample_rate,
        duration_seconds: result.duration_seconds,
        voice: result.voice,
        text_chunks: result.text_chunks,
    })
}

/// Start streaming synthesis — returns a stream_id for tracking.
///
/// The stream can be polled for status via `tts_stream_status`.
/// Mirrors the Python `/api/generate-stream/start` endpoint.
///
/// # Arguments
///
/// * `text` - Text to synthesize.
/// * `demo_id` - Optional demo ID to use demo prompt audio.
/// * `prompt_audio_path` - Optional path to prompt audio for voice clone.
/// * `params` - Generation parameters.
#[tauri::command]
pub async fn tts_stream_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, TtsState>,
    text: String,
    demo_id: Option<String>,
    voice_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<StreamStartResponse, String> {
    let provider = state.provider.get().await?;
    state.warmup.ensure_ready(provider.clone()).await;

    // Create streaming job
    let job = state.jobs.create().await;
    let _stream_id = job.lock().await.stream_id.clone();

    // Set up job metadata
    {
        let mut j = job.lock().await;
        j.text_chunks = vec![text.clone()];
        if let Some(ref path) = prompt_audio_path {
            j.prompt_audio_path = Some(path.clone());
        }
    }

    let jobs = state.jobs.clone();
    let job_clone = job.clone();
    let (resolved_voice, resolved_prompt) = resolve_voice_source(
        &app,
        voice_id.as_deref(),
        demo_id.as_deref(),
        prompt_audio_path.as_deref(),
    );
    let app_clone = app.clone();

    tokio::spawn(async move {
        run_streaming_synthesis(
            app_clone,
            provider,
            jobs,
            job_clone,
            text,
            resolved_voice,
            resolved_prompt,
            params,
        )
        .await;
    });

    let j = job.lock().await;
    Ok(StreamStartResponse {
        stream_id: j.stream_id.clone(),
        sample_rate: j.sample_rate,
        channels: j.channels,
    })
}

/// Internal: run streaming synthesis and update job state.
///
/// Phase TTS-B.2: emit `tts:stream-chunk` Tauri event per audio chunk so
/// the frontend's Web Audio API player can schedule gapless playback.
async fn run_streaming_synthesis(
    app: tauri::AppHandle,
    provider: Arc<dyn TtsProvider>,
    _jobs: Arc<StreamingJobManager>,
    job: Arc<Mutex<StreamingJob>>,
    text: String,
    voice: Option<String>,
    prompt_audio_path: Option<std::path::PathBuf>,
    params: GenerationParams,
) {
    use crate::modules::tts::inference::streaming::{create_audio_channel, ChannelAudioSink};

    // Update job state to streaming
    {
        let mut j = job.lock().await;
        j.state = "streaming".to_string();
        j.run_status = "Generating audio...".to_string();
        j.started_at = Some(std::time::Instant::now());
    }

    let synthesis_params = SynthesisParams {
        text,
        mode: SynthesisMode::VoiceClone,
        voice,
        prompt_audio_path,
        prompt_text: None,
        generation: params,
    };

    let (audio_tx, mut audio_rx) = create_audio_channel();
    let (complete_tx, complete_rx) = tokio::sync::mpsc::channel(1);
    let sink = Arc::new(ChannelAudioSink::new(audio_tx, complete_tx));

    let provider_ref = provider.clone();

    // Spawn synthesis task
    let synth_handle =
        tokio::spawn(async move { provider_ref.synthesize_stream(synthesis_params, sink).await });

    let stream_id_for_events = { job.lock().await.stream_id.clone() };

    // Forward audio chunks and update job state. Phase TTS-B.3：从 chunk 拿
    // sample_rate / channels（不再硬编码 48k/2）。Phase TTS-B.2：通过 Tauri
    // event 把 PCM16LE bytes 推到前端 Web Audio API 调度器。
    while let Some(chunk) = audio_rx.recv().await {
        {
            let mut j = job.lock().await;
            if j.first_audio_at.is_none() {
                j.first_audio_at = Some(std::time::Instant::now());
                if chunk.sample_rate > 0 {
                    j.sample_rate = chunk.sample_rate;
                }
                if chunk.channels > 0 {
                    j.channels = chunk.channels;
                }
            }
            j.emitted_audio_seconds = chunk.emitted_audio_seconds;
            j.lead_seconds = chunk.lead_seconds;
            j.current_chunk_index = Some(chunk.chunk_index);
        }

        let payload = TtsStreamChunkEvent {
            stream_id: stream_id_for_events.clone(),
            chunk_index: chunk.chunk_index,
            sample_rate: chunk.sample_rate,
            channels: chunk.channels,
            pcm_base64: base64::engine::general_purpose::STANDARD.encode(&chunk.pcm_data),
            emitted_audio_seconds: chunk.emitted_audio_seconds,
            lead_seconds: chunk.lead_seconds,
        };
        if let Err(e) = tauri::Emitter::emit(&app, "tts:stream-chunk", &payload) {
            tracing::warn!(error = %e, "tts:stream-chunk emit failed");
        }
    }

    // Wait for completion
    let mut total_audio_seconds = 0.0_f32;
    let mut completion_error: Option<String> = None;
    match synth_handle.await {
        Ok(Ok(stream_result)) => {
            total_audio_seconds = stream_result.emitted_audio_seconds;
            let mut j = job.lock().await;
            j.state = "done".to_string();
            j.run_status = "Complete.".to_string();
            j.completed_at = Some(std::time::Instant::now());
            j.emitted_audio_seconds = stream_result.emitted_audio_seconds;
        }
        Ok(Err(e)) => {
            let msg = format!("Synthesis error: {e}");
            completion_error = Some(msg.clone());
            let mut j = job.lock().await;
            j.state = "failed".to_string();
            j.error = Some(msg);
            j.completed_at = Some(std::time::Instant::now());
        }
        Err(e) => {
            let msg = format!("Task panic: {e}");
            completion_error = Some(msg.clone());
            let mut j = job.lock().await;
            j.state = "failed".to_string();
            j.error = Some(msg);
            j.completed_at = Some(std::time::Instant::now());
        }
    }

    // Emit `tts:stream-end` so frontend bridge knows this stream is fully drained
    // and can safely start the next sentence (避免 setExpectedStreamId 把上一句残余 chunks 丢掉)
    let end_payload = TtsStreamEndEvent {
        stream_id: stream_id_for_events.clone(),
        total_audio_seconds,
        success: completion_error.is_none(),
        error: completion_error,
    };
    if let Err(e) = tauri::Emitter::emit(&app, "tts:stream-end", &end_payload) {
        tracing::warn!(error = %e, "tts:stream-end emit failed");
    }

    let _ = complete_rx;
}

/// Get the status of a streaming job.
///
/// Returns a JSON snapshot of the job's current state.
#[tauri::command]
pub async fn tts_stream_status(
    state: tauri::State<'_, TtsState>,
    stream_id: String,
) -> Result<serde_json::Value, String> {
    let job = state
        .jobs
        .get(&stream_id)
        .await
        .ok_or_else(|| format!("Stream '{stream_id}' not found."))?;
    let j = job.lock().await;
    Ok(j.snapshot())
}

/// Get the final result of a streaming job.
///
/// Blocks until the job is done or failed.
#[tauri::command]
pub async fn tts_stream_result(
    state: tauri::State<'_, TtsState>,
    stream_id: String,
) -> Result<serde_json::Value, String> {
    let job = state
        .jobs
        .get(&stream_id)
        .await
        .ok_or_else(|| format!("Stream '{stream_id}' not found."))?;

    // Poll until done or failed
    loop {
        {
            let j = job.lock().await;
            if j.is_done() || j.is_failed() {
                return Ok(j.snapshot());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Close or cancel a streaming job.
#[tauri::command]
pub async fn tts_stream_close(
    state: tauri::State<'_, TtsState>,
    stream_id: String,
) -> Result<serde_json::Value, String> {
    let job = state
        .jobs
        .close(&stream_id)
        .await
        .ok_or_else(|| format!("Stream '{stream_id}' not found."))?;
    let j = job.lock().await;
    Ok(j.snapshot())
}

/// Get demo audio by demo ID as base64.
///
/// Returns the embedded demo audio file (WAV/MP3).
#[tauri::command]
pub async fn tts_demo_audio(
    _state: tauri::State<'_, TtsState>,
    demo_id: String,
) -> Result<DemoAudioResponse, String> {
    let audio_path = resolve_demo_audio_path(&demo_id).ok_or_else(|| {
        format!("Demo '{demo_id}' not found. Use tts_list_voices to see available demos.")
    })?;

    let audio_bytes = std::fs::read(&audio_path).map_err(|e| {
        format!(
            "Failed to read demo audio at '{}': {e}",
            audio_path.display()
        )
    })?;

    let content_type = if audio_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
    {
        "audio/mpeg"
    } else {
        "audio/wav"
    };

    Ok(DemoAudioResponse {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(&audio_bytes),
        content_type: content_type.to_string(),
    })
}

/// List all available voice preset names.
#[tauri::command]
pub async fn tts_list_voices(state: tauri::State<'_, TtsState>) -> Result<Vec<String>, String> {
    let provider = state.provider.get().await?;
    Ok(provider.list_voices())
}

/// Phase TTS-D / P0：列出**所有**语音资产（builtin + bundled + user uploaded）
/// 供前端"Agent 语音设定"面板渲染。
///
/// 返回的 `VoiceAsset` 含 `id` / `display_name` / `kind` / `language` / `is_previewable`。
#[tauri::command]
pub async fn tts_list_voice_assets(app: tauri::AppHandle) -> Result<Vec<VoiceAssetDto>, String> {
    let registry = scan_voice_registry(&app);
    Ok(registry.list().iter().map(VoiceAssetDto::from).collect())
}

/// 给前端用的 DTO（不暴露绝对路径）。
#[derive(Debug, Clone, Serialize)]
pub struct VoiceAssetDto {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub is_previewable: bool,
}

impl From<&VoiceAsset> for VoiceAssetDto {
    fn from(v: &VoiceAsset) -> Self {
        Self {
            id: v.id.clone(),
            display_name: v.display_name.clone(),
            kind: match v.kind {
                VoiceKind::Builtin => "builtin",
                VoiceKind::Bundled => "bundled",
                VoiceKind::UserUploaded => "user",
            }
            .to_string(),
            language: v.language.clone(),
            description: v.description.clone(),
            is_previewable: v.is_previewable(),
        }
    }
}

/// Phase TTS-D / P0：返回某个 voice 的原始 audio 文件（base64），供前端
/// `<audio>` 试听原始 prompt 声音。
#[tauri::command]
pub async fn tts_voice_audio(
    app: tauri::AppHandle,
    voice_id: String,
) -> Result<DemoAudioResponse, String> {
    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&voice_id)
        .ok_or_else(|| format!("Voice '{voice_id}' not found"))?;
    let path = asset
        .audio_path
        .as_ref()
        .ok_or_else(|| format!("Voice '{voice_id}' has no audio file"))?;
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read voice audio at {}: {e}", path.display()))?;
    let content_type = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext.to_ascii_lowercase().as_str() {
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "ogg" => "audio/ogg",
            _ => "audio/wav",
        })
        .unwrap_or("audio/wav")
        .to_string();
    Ok(DemoAudioResponse {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        content_type,
    })
}

/// Phase TTS-D / P0：用某个 voice 合成一段固定预览文本（"你好，我是 X。"），
/// 返回 base64 WAV，供前端"试听 Agent 用这个声音说话"。
#[tauri::command]
pub async fn tts_preview_voice(
    app: tauri::AppHandle,
    state: tauri::State<'_, TtsState>,
    voice_id: String,
    sample_text: Option<String>,
) -> Result<SynthesisResponse, String> {
    let provider = state.provider.get().await?;
    state.warmup.ensure_ready(provider.clone()).await;

    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&voice_id)
        .ok_or_else(|| format!("Voice '{voice_id}' not found"))?;

    // builtin → 走 manifest prebaked codes（voice name）
    // bundled / user → 走 prompt_audio_path（codec.encode）
    let (voice_name, prompt_path) = match asset.kind {
        VoiceKind::Builtin => (Some(asset.id.clone()), None),
        _ => (None, asset.audio_path.clone()),
    };
    if matches!(asset.kind, VoiceKind::Bundled | VoiceKind::UserUploaded) && prompt_path.is_none() {
        return Err(format!("Voice '{voice_id}' has no audio source"));
    }

    let text = sample_text.unwrap_or_else(|| format!("你好，我是 {}。", asset.display_name));
    let params = SynthesisParams {
        text,
        mode: SynthesisMode::VoiceClone,
        voice: voice_name,
        prompt_audio_path: prompt_path,
        prompt_text: None,
        generation: GenerationParams {
            max_new_frames: 200,
            seed: Some(1234),
            ..Default::default()
        },
    };
    let result = provider
        .synthesize(params)
        .await
        .map_err(|e| format!("Preview synthesis failed: {e}"))?;
    Ok(SynthesisResponse {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(&result.audio_bytes),
        sample_rate: result.sample_rate,
        duration_seconds: result.duration_seconds,
        voice: result.voice,
        text_chunks: result.text_chunks,
    })
}

/// Phase TTS-C.2：把任意文本按 voice clone 三段式 chunker 切成 chunks，
/// 供前端预览（无需触发真合成）。
///
/// 内部走 [`crate::modules::tts::TtsProvider::split_voice_clone_text`]，最终
/// 调用 [`crate::modules::tts::text::chunker::split_voice_clone_text`]，
/// 与合成时使用的逻辑完全一致。
#[tauri::command]
pub async fn tts_split_text(
    state: tauri::State<'_, TtsState>,
    text: String,
    max_tokens: u32,
) -> Result<Vec<String>, String> {
    let provider = state.provider.get().await?;
    provider
        .split_voice_clone_text(&text, max_tokens as usize)
        .map_err(|e| format!("Failed to split text: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────
// Phase TTS-E.1：用户上传 / 删除 / 重命名 自定义声纹（saved to app_data_dir/voices/）
// ─────────────────────────────────────────────────────────────────────────

/// 校验上传文件的最大大小（30 MB）—— prompt audio 通常 5-30s，足够。
const MAX_USER_VOICE_BYTES: usize = 30 * 1024 * 1024;

/// 允许的扩展名（小写比较）。
const ALLOWED_VOICE_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "m4a"];

/// 把任意输入字符串转成安全文件 stem：仅保留 ASCII letters/digits/underscore/hyphen。
/// 全空 / 全非法字符 → 返回 "voice"。
fn sanitize_voice_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c == ' ' || c == '.' {
                Some('_')
            } else {
                None
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '-' || c == '_' || c == '.');
    if trimmed.is_empty() {
        "voice".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 拿用户语音目录（`<app_data_dir>/voices/`），需要时自动创建。
fn user_voice_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取 app_data_dir: {e}"))?
        .join("voices");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建用户语音目录 {}: {e}", dir.display()))?;
    Ok(dir)
}

/// 用 symphonia probe 一下文件，确认是个能解码的音频。失败时返回中文错误。
fn validate_audio_bytes(bytes: &[u8], extension: &str) -> Result<(), String> {
    use std::io::Cursor;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("音频校验失败：{e}（请确认是 wav/mp3/flac/ogg/m4a 文件）"))?;
    Ok(())
}

/// 上传响应。
#[derive(Debug, Clone, Serialize)]
pub struct UploadVoiceResponse {
    pub asset: VoiceAssetDto,
    pub saved_path: String,
}

/// Phase TTS-E.1：用户上传一段音频作为自定义声纹。
///
/// - `file_name`：用户给的原始文件名（例如 "myvoice.mp3"），用来推导 id + 扩展名。
/// - `file_bytes`：base64 解码后的二进制；前端用 FileReader 读完转 base64 传过来。
/// - `display_name`：可选，UI 上显示用；默认用 file_name 的 stem 美化。
///
/// # 行为
/// 1. 校验大小 ≤ 30 MB + 扩展名在白名单
/// 2. 用 symphonia 探测确认音频格式
/// 3. 保存到 `<app_data_dir>/voices/<sanitized>.<ext>`（重名加 `_2`、`_3`...）
/// 4. registry 自动 pick up（前端只要重新调 `tts_list_voice_assets` 就有）
#[tauri::command]
pub async fn tts_upload_user_voice(
    app: tauri::AppHandle,
    file_name: String,
    file_bytes_base64: String,
    display_name: Option<String>,
) -> Result<UploadVoiceResponse, String> {
    // 1. 解码 base64
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&file_bytes_base64)
        .map_err(|e| format!("base64 解码失败：{e}"))?;
    if bytes.is_empty() {
        return Err("文件为空".to_string());
    }
    if bytes.len() > MAX_USER_VOICE_BYTES {
        return Err(format!(
            "文件过大（{} MB）；最大 {} MB",
            bytes.len() / (1024 * 1024),
            MAX_USER_VOICE_BYTES / (1024 * 1024)
        ));
    }

    // 2. 解析扩展名
    let raw_ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if !ALLOWED_VOICE_EXTENSIONS.contains(&raw_ext.as_str()) {
        return Err(format!(
            "不支持的扩展名 .{raw_ext}；仅支持 {}",
            ALLOWED_VOICE_EXTENSIONS.join(" / ")
        ));
    }

    // 3. 校验音频格式
    validate_audio_bytes(&bytes, &raw_ext)?;

    // 4. 生成 sanitize 后的文件名 + 处理重名
    let stem_raw = std::path::Path::new(&file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("voice");
    let stem = sanitize_voice_id(stem_raw);
    let dir = user_voice_dir(&app)?;
    let mut final_stem = stem.clone();
    let mut counter = 2;
    while dir.join(format!("{final_stem}.{raw_ext}")).exists() {
        final_stem = format!("{stem}_{counter}");
        counter += 1;
    }
    let final_path = dir.join(format!("{final_stem}.{raw_ext}"));

    // 5. 原子写入：先写 .tmp 再 rename（避免崩溃残留半文件）
    let tmp_path = dir.join(format!("{final_stem}.{raw_ext}.tmp"));
    std::fs::write(&tmp_path, &bytes).map_err(|e| format!("写入临时文件失败：{e}"))?;
    std::fs::rename(&tmp_path, &final_path).map_err(|e| format!("重命名失败：{e}"))?;

    // 6. 可选 display_name 写到 sidecar JSON（方便重命名持久化）
    if let Some(name) = display_name.as_deref().filter(|s| !s.is_empty()) {
        let meta_path = dir.join(format!("{final_stem}.json"));
        let meta = serde_json::json!({ "display_name": name });
        let _ = std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap_or_default(),
        );
    }

    tracing::info!(
        voice_id = final_stem,
        bytes = bytes.len(),
        path = %final_path.display(),
        "user voice uploaded"
    );

    // 7. 重新扫描 registry 并把刚才这条挑出来返给前端
    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&final_stem)
        .ok_or_else(|| format!("voice {final_stem} 上传后未在 registry 找到"))?;

    Ok(UploadVoiceResponse {
        asset: VoiceAssetDto::from(asset),
        saved_path: final_path.to_string_lossy().to_string(),
    })
}

/// Phase TTS-E.1：删除一条用户上传的声纹。
///
/// 仅允许删除 `kind == user` 的 voice。删除文件 + 对应 sidecar metadata。
#[tauri::command]
pub async fn tts_delete_user_voice(app: tauri::AppHandle, voice_id: String) -> Result<(), String> {
    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&voice_id)
        .ok_or_else(|| format!("Voice '{voice_id}' not found"))?;
    if asset.kind != VoiceKind::UserUploaded {
        return Err(format!(
            "拒绝删除：'{voice_id}' 是 {:?} voice，仅允许删除用户上传的声纹",
            asset.kind
        ));
    }
    if let Some(path) = asset.audio_path.as_ref() {
        std::fs::remove_file(path)
            .map_err(|e| format!("删除文件失败：{e} ({})", path.display()))?;
        // sidecar metadata（可能不存在，silent ignore）
        if let Some(parent) = path.parent() {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let _ = std::fs::remove_file(parent.join(format!("{stem}.json")));
            // 删除 preview WAV 缓存
            let _ = std::fs::remove_file(parent.join(format!("{stem}.preview.wav")));
        }
        tracing::info!(voice_id, "user voice deleted");
        Ok(())
    } else {
        Err(format!("Voice '{voice_id}' has no audio file path"))
    }
}

/// Phase TTS-E.1：重命名（仅修改 display_name，文件名不动以保持 voice_id 稳定）。
#[tauri::command]
pub async fn tts_rename_user_voice(
    app: tauri::AppHandle,
    voice_id: String,
    new_display_name: String,
) -> Result<(), String> {
    let trimmed = new_display_name.trim().to_string();
    if trimmed.is_empty() {
        return Err("display_name 不能为空".into());
    }
    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&voice_id)
        .ok_or_else(|| format!("Voice '{voice_id}' not found"))?;
    if asset.kind != VoiceKind::UserUploaded {
        return Err("仅允许重命名用户上传的声纹".into());
    }
    let dir = user_voice_dir(&app)?;
    let meta_path = dir.join(format!("{voice_id}.json"));
    let meta = serde_json::json!({ "display_name": trimmed });
    std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    )
    .map_err(|e| format!("写入 metadata 失败：{e}"))?;
    tracing::info!(voice_id, new_name = %trimmed, "user voice renamed");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────
// Phase TTS-E / P2：Voice 预览缓存（快照 WAV）
// ─────────────────────────────────────────────────────────────────────────

/// preview WAV 保存路径：`<user_voice_dir>/<voice_id>.preview.wav`
fn voice_preview_path(app: &tauri::AppHandle, voice_id: &str) -> Option<std::path::PathBuf> {
    let dir = user_voice_dir(app).ok()?;
    Some(dir.join(format!("{voice_id}.preview.wav")))
}

/// Phase TTS-E.3：为某个 voice 预生成一段 4s WAV 快照并缓存到磁盘。
///
/// - 使用 "你好，欢迎试用 TTS 语音合成功能。" 作为 preview 文本（约 4s）
/// - 生成结果写到 `<user_voice_dir>/<voice_id>.preview.wav`
/// - 后续调用 `tts_voice_audio(voice_id)` 时优先返回 preview；如果 preview
///   存在且 < 24h 则直接 serve，否则用原始 prompt audio
///
/// 前端上传完成后应 fire-and-forget 调本命令。
#[tauri::command]
pub async fn tts_warm_voice_preview(
    app: tauri::AppHandle,
    state: tauri::State<'_, TtsState>,
    voice_id: String,
) -> Result<String, String> {
    let provider = state.provider.get().await?;
    let registry = scan_voice_registry(&app);
    let asset = registry
        .find(&voice_id)
        .ok_or_else(|| format!("Voice '{voice_id}' not found"))?;

    let (voice_name, prompt_path) = match asset.kind {
        VoiceKind::Builtin => (Some(asset.id.clone()), None),
        _ => (None, asset.audio_path.clone()),
    };
    if matches!(asset.kind, VoiceKind::Bundled | VoiceKind::UserUploaded) && prompt_path.is_none() {
        return Err(format!(
            "Voice '{voice_id}' has no audio source for preview"
        ));
    }

    let preview_text = "你好，欢迎试用 TTS 语音合成功能，这是我的声音。".to_string();
    let params = crate::modules::tts::SynthesisParams {
        text: preview_text.clone(),
        mode: crate::modules::tts::SynthesisMode::VoiceClone,
        voice: voice_name,
        prompt_audio_path: prompt_path,
        prompt_text: None,
        generation: GenerationParams {
            max_new_frames: 200,
            seed: Some(42),
            ..Default::default()
        },
    };

    let result = provider
        .synthesize(params)
        .await
        .map_err(|e| format!("Preview synthesis failed: {e}"))?;

    let preview_path = voice_preview_path(&app, &voice_id).ok_or("无法解析 preview 路径")?;
    std::fs::write(&preview_path, &result.audio_bytes)
        .map_err(|e| format!("写入 preview WAV 失败：{e}"))?;

    tracing::info!(
        voice_id,
        duration_s = result.duration_seconds,
        path = %preview_path.display(),
        "voice preview cached"
    );
    Ok(preview_path.to_string_lossy().to_string())
}

/// Phase TTS-E.3：返回某个 voice 的预览 WAV（base64）。
///
/// 优先返回缓存的 preview WAV；缓存不存在时 fallback 到原始 prompt audio。
/// `tts_voice_audio` 用于"原声试听"（返回 prompt 本身）；
/// 本命令用于"合成效果预听"（更能代表实际输出）。
#[tauri::command]
pub async fn tts_cached_voice_preview(
    app: tauri::AppHandle,
    voice_id: String,
) -> Result<DemoAudioResponse, String> {
    // 1. 优先返回 preview WAV（如果存在）
    if let Some(path) = voice_preview_path(&app, &voice_id) {
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| format!("read preview: {e}"))?;
            return Ok(DemoAudioResponse {
                audio_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                content_type: "audio/wav".to_string(),
            });
        }
    }
    // 2. Fallback：返回原始 prompt audio（等同 tts_voice_audio）
    tts_voice_audio(app, voice_id).await
}

// ── User-tunable TTS settings (Settings UI entry point) ─────────────────────

/// `~/.if2ai/` resolver shared by `get_tts_settings` / `set_tts_settings`.
/// Falls back to `.` when `dirs::home_dir()` is unavailable so the
/// command always returns a usable path rather than an error.
fn if2ai_home() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".if2ai")
}

/// Read the persisted TTS settings (`~/.if2ai/tts.toml`).
///
/// Always succeeds — a missing or malformed file is treated as
/// "user has never opened the page" and the default settings are
/// returned.
#[tauri::command]
pub async fn get_tts_settings() -> Result<crate::modules::tts::TtsSettings, String> {
    Ok(crate::modules::tts::TtsSettings::load(&if2ai_home()))
}

/// Persist user-edited TTS settings to `~/.if2ai/tts.toml`.
///
/// Out-of-range values are clamped before writing.  Returns an error
/// only on actual disk failure (permissions, full filesystem, …).
#[tauri::command]
pub async fn set_tts_settings(
    settings: crate::modules::tts::TtsSettings,
) -> Result<(), String> {
    settings
        .save(&if2ai_home())
        .map_err(|e| format!("failed to write tts.toml: {e}"))
}

// ── TTS Profiles (named voice + settings recipes) ─────────────────────────

/// Read the entire profile book.  On first call creates and seeds
/// `~/.if2ai/tts_profiles.json` with 6 builtin profiles.
#[tauri::command]
pub async fn list_tts_profiles() -> Result<crate::modules::tts::TtsProfileBook, String> {
    Ok(crate::modules::tts::TtsProfileBook::load(&if2ai_home()))
}

/// Insert or update a single profile (matched by `id`).  Builtin
/// profiles are partially protected: their `id` and `is_builtin` flag
/// stay frozen, but `name` / `voice_id` / `settings` / `postprocess`
/// may be customised.  Returns the resulting profile.
#[tauri::command]
pub async fn save_tts_profile(
    profile: crate::modules::tts::TtsProfile,
) -> Result<crate::modules::tts::TtsProfile, String> {
    let home = if2ai_home();
    let mut book = crate::modules::tts::TtsProfileBook::load(&home);
    let saved = book.upsert(profile);
    book.save(&home)
        .map_err(|e| format!("failed to write tts_profiles.json: {e}"))?;
    Ok(saved)
}

/// Delete a user-created profile.  Returns an error if `id` references
/// a builtin (which cannot be deleted).
#[tauri::command]
pub async fn delete_tts_profile(id: String) -> Result<(), String> {
    let home = if2ai_home();
    let mut book = crate::modules::tts::TtsProfileBook::load(&home);
    if !book.delete(&id) {
        return Err(format!(
            "profile '{id}' does not exist or is a builtin (use duplicate to customise)"
        ));
    }
    book.save(&home)
        .map_err(|e| format!("failed to write tts_profiles.json: {e}"))
}

/// Set the active default profile id.  Returns an error if `id` is
/// unknown.
#[tauri::command]
pub async fn set_default_tts_profile(id: String) -> Result<(), String> {
    let home = if2ai_home();
    let mut book = crate::modules::tts::TtsProfileBook::load(&home);
    if !book.set_default(&id) {
        return Err(format!("profile '{id}' does not exist"));
    }
    book.save(&home)
        .map_err(|e| format!("failed to write tts_profiles.json: {e}"))
}
