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
use crate::modules::tts::voice::demo::resolve_demo_audio_path;
use crate::modules::tts::{SynthesisMode, SynthesisParams, TtsProvider};

/// Response from [`tts_health`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsHealthResponse {
    pub status: String,
    pub warmup_state: String,
    pub warmup_progress: f32,
    pub message: String,
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

/// TTS state managed by Tauri.
///
/// Holds the TTS provider, warmup manager, and streaming job manager.
pub struct TtsState {
    pub provider: Arc<dyn TtsProvider>,
    pub warmup: Arc<WarmupManager>,
    pub jobs: Arc<StreamingJobManager>,
}

/// Check TTS system health and model status.
///
/// Returns the current warmup state and whether the system is ready.
#[tauri::command]
pub async fn tts_health(state: tauri::State<'_, TtsState>) -> Result<TtsHealthResponse, String> {
    let snapshot = state.warmup.snapshot().await;
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
    state.warmup.start(state.provider.clone()).await;
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
    state: tauri::State<'_, TtsState>,
    text: String,
    demo_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<SynthesisResponse, String> {
    // Ensure warmup is complete
    state.warmup.ensure_ready(state.provider.clone()).await;

    // Resolve demo audio if demo_id provided
    let resolved_prompt_path = if let Some(ref did) = demo_id {
        resolve_demo_audio_path(did)
    } else {
        prompt_audio_path.map(std::path::PathBuf::from)
    };

    let mode = if resolved_prompt_path.is_some() {
        SynthesisMode::VoiceClone
    } else {
        SynthesisMode::default()
    };

    let synthesis_params = SynthesisParams {
        text,
        mode,
        voice: None,
        prompt_audio_path: resolved_prompt_path,
        prompt_text: None,
        generation: params,
    };

    let result = state
        .provider
        .synthesize(synthesis_params)
        .await
        .map_err(|e| format!("Synthesis failed: {e}"))?;

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
    state: tauri::State<'_, TtsState>,
    text: String,
    demo_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<StreamStartResponse, String> {
    // Ensure warmup is complete
    state.warmup.ensure_ready(state.provider.clone()).await;

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

    // Spawn the streaming synthesis task
    let provider = state.provider.clone();
    let jobs = state.jobs.clone();
    let job_clone = job.clone();
    let resolved_prompt = if let Some(ref did) = demo_id {
        resolve_demo_audio_path(did)
    } else {
        prompt_audio_path.map(std::path::PathBuf::from)
    };

    tokio::spawn(async move {
        run_streaming_synthesis(provider, jobs, job_clone, text, resolved_prompt, params).await;
    });

    let j = job.lock().await;
    Ok(StreamStartResponse {
        stream_id: j.stream_id.clone(),
        sample_rate: j.sample_rate,
        channels: j.channels,
    })
}

/// Internal: run streaming synthesis and update job state.
async fn run_streaming_synthesis(
    provider: Arc<dyn TtsProvider>,
    _jobs: Arc<StreamingJobManager>,
    job: Arc<Mutex<StreamingJob>>,
    text: String,
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

    let mode = if prompt_audio_path.is_some() {
        SynthesisMode::VoiceClone
    } else {
        SynthesisMode::default()
    };

    let synthesis_params = SynthesisParams {
        text,
        mode,
        voice: None,
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

    // Forward audio chunks and update job state
    while let Some(chunk) = audio_rx.recv().await {
        let mut j = job.lock().await;
        j.emitted_audio_seconds = chunk.emitted_audio_seconds;
        j.lead_seconds = chunk.lead_seconds;
        j.current_chunk_index = Some(chunk.chunk_index);
        if j.first_audio_at.is_none() {
            j.first_audio_at = Some(std::time::Instant::now());
        }
        let _ = chunk; // Audio bytes go to Tauri event or client-side consumption
    }

    // Wait for completion
    match synth_handle.await {
        Ok(Ok(stream_result)) => {
            let mut j = job.lock().await;
            j.state = "done".to_string();
            j.run_status = "Complete.".to_string();
            j.completed_at = Some(std::time::Instant::now());
            j.emitted_audio_seconds = stream_result.emitted_audio_seconds;
        }
        Ok(Err(e)) => {
            let mut j = job.lock().await;
            j.state = "failed".to_string();
            j.error = Some(format!("Synthesis error: {e}"));
            j.completed_at = Some(std::time::Instant::now());
        }
        Err(e) => {
            let mut j = job.lock().await;
            j.state = "failed".to_string();
            j.error = Some(format!("Task panic: {e}"));
            j.completed_at = Some(std::time::Instant::now());
        }
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
    Ok(state.provider.list_voices())
}

/// Split text into chunks for voice clone preview.
///
/// Respects the `max_tokens` budget per chunk.
#[tauri::command]
pub async fn tts_split_text(
    state: tauri::State<'_, TtsState>,
    text: String,
    max_tokens: u32,
) -> Result<Vec<String>, String> {
    state
        .provider
        .split_voice_clone_text(&text, max_tokens as usize)
        .map_err(|e| format!("Failed to split text: {e}"))
}
