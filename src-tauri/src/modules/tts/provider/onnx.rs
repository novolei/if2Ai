//! ONNX Runtime TTS provider — MOSS-TTS-Nano native synthesis.
//!
//! ## Phase TTS-A 完整实现（A.1~A.7）
//!
//! 本文件经过两轮重构（A.1~A.5 + A.6/A.7 spike）后，已是端到端真实推理：
//! - ✅ Manifest 三件套解析（A.2）
//! - ✅ SentencePiece tokenizer（A.3）
//! - ✅ Reference audio symphonia + rubato（A.4）
//! - ✅ build_voice_clone_request_rows 协议（A.5）
//! - ✅ Prefill / decode_step 真 ort `Session::run` + KV cache 来回喂（A.6）
//! - ✅ 4 条 sample-mode 真 run（greedy / fixed / full / fallback）（A.6）
//! - ✅ Codec encode + decode_full 真 run（A.7）
//! - ✅ Codec streaming session + 自适应 batch budget（A.7）
//!
//! 所有"占位 / 静音"代码已清除；每条路径要么 1:1 调用 ONNX，要么返回结构化
//! `TtsError`。
//!
//! ## 异步策略
//!
//! ONNX `Session::run` 是同步 CPU 计算（可能 100ms-2s），按 CLAUDE.md 规则放
//! `tokio::task::spawn_blocking` 避免阻塞 runtime。`Mutex<OnnxTtsProvider>`
//! 在 [`crate::commands::tts`] 层包装，本文件只关心单次同步推理。

#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::modules::tts::audio::reference::load_reference_audio;
use crate::modules::tts::audio::streaming_decoder::CodecStreamingDecodeSession;
use crate::modules::tts::audio::wav::wav_encode;
use crate::modules::tts::config::GenerationParams;
use crate::modules::tts::error::TtsError;
use crate::modules::tts::inference::request_builder::{
    build_voice_clone_request_rows, VoiceCloneRequestRows,
};
use crate::modules::tts::inference::runner::generate_audio_frames;
use crate::modules::tts::inference::stream_budget::resolve_decode_frame_budget;
use crate::modules::tts::manifest::ManifestBundle;
use crate::modules::tts::model::codec::{interleave_channels, CodecSessions};
use crate::modules::tts::model::global::{GlobalSessions, DEFAULT_THREAD_COUNT};
use crate::modules::tts::model::local::LocalSessions;
use crate::modules::tts::text::normalize_tts_text;
use crate::modules::tts::text::tokenizer::TtsTokenizer;
use crate::modules::tts::voice::presets::{list_voice_names, DEFAULT_VOICE_NAME};
use crate::modules::tts::{
    AudioSink, StreamResult, SynthesisMode, SynthesisParams, SynthesisResult, TtsProvider,
    VoicePreset, WarmupResult,
};

/// ONNX-backed TTS provider —— MOSS-TTS-Nano 真实推理实现。
///
/// ## 并发策略（Phase TTS-E / P2）
///
/// 内部用 `Arc<Mutex<ProviderInner>>` 保护 ONNX sessions（`Session::run` 需 `&mut`）。
/// 额外加 `Semaphore` 把同时持锁数量限制在 [`MAX_CONCURRENT_STREAMS`] = 2：
/// - 1 个可以分配给 chat 自动 TTS（agentVoiceBridge）
/// - 1 个可以分配给手动 Generate 请求
/// - 第 3 个及以上请求在信号量队列中等待；`queue_position` 通过 `pending.available_permits()`
///   反推得出，暴露在 tts_stream_status 里。
///
/// 内存：2 个并发流共享同一套 ONNX sessions，真正的并发 decode 要 2 倍 KV cache 内存；
/// 选 N=2 在 M-series 8GB 机器上可用，N>2 可能 OOM。
pub const MAX_CONCURRENT_STREAMS: usize = 2;

pub struct OnnxTtsProvider {
    inner: Arc<Mutex<ProviderInner>>,
    /// Phase TTS-E / P2：并发信号量；同时只允许 MAX_CONCURRENT_STREAMS 个合成请求持锁。
    semaphore: Arc<tokio::sync::Semaphore>,
}

struct ProviderInner {
    manifest: ManifestBundle,
    sessions: GlobalSessions,
    local: LocalSessions,
    codec: CodecSessions,
    streaming: CodecStreamingDecodeSession,
    /// Phase TTS-C.3：tokenizer 用 Arc 共享给后台心跳 ticker。
    tokenizer: Arc<TtsTokenizer>,
    /// Phase TTS-C.3：心跳 ticker 句柄；provider drop 时 ticker 自动 abort。
    _health_ticker: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for ProviderInner {
    fn drop(&mut self) {
        if let Some(t) = self._health_ticker.take() {
            t.abort();
        }
    }
}

impl OnnxTtsProvider {
    /// 从模型根目录构造 provider。
    pub fn from_model_dir(model_dir: &Path, thread_count: Option<usize>) -> Result<Self, TtsError> {
        let threads = thread_count.unwrap_or(DEFAULT_THREAD_COUNT);

        let manifest = ManifestBundle::load(model_dir)?;

        let prefill_path = manifest.tts_onnx_path("prefill")?;
        let decode_path = manifest.tts_onnx_path("decode_step")?;
        let sessions = GlobalSessions::load_from_paths(&prefill_path, &decode_path, threads)?;

        let local = LocalSessions::load_from_manifest(&manifest, threads)?;
        let codec = CodecSessions::load_from_manifest(&manifest, threads)?;
        let streaming = CodecStreamingDecodeSession::new(&manifest.codec_meta);

        let tokenizer_path = manifest.tokenizer_path()?;
        let tokenizer = Arc::new(TtsTokenizer::load(&tokenizer_path)?);

        // Phase TTS-C.3：60 秒主动 ping 心跳；如果调用 from_model_dir 时不在 tokio
        // runtime 内（main.rs setup 阶段），fall back 到 None（被动 respawn 仍可用）。
        let health_ticker = tokio::runtime::Handle::try_current().ok().map(|_| {
            tokenizer
                .clone()
                .spawn_health_ticker(std::time::Duration::from_secs(60))
        });

        Ok(Self {
            inner: Arc::new(Mutex::new(ProviderInner {
                manifest,
                sessions,
                local,
                codec,
                streaming,
                tokenizer,
                _health_ticker: health_ticker,
            })),
            semaphore: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_STREAMS)),
        })
    }

    /// 兼容旧签名（test 调用）。第一个参数应为模型根目录。
    pub fn from_dirs(
        tts_model_dir: &Path,
        _audio_tokenizer_dir: &Path,
        _tokenizer_path: &Path,
    ) -> Result<Self, TtsError> {
        let model_root = tts_model_dir.parent().unwrap_or(tts_model_dir);
        Self::from_model_dir(model_root, None)
    }
}

// ── 内部纯函数（不持锁，参数显式传入）───────────────────────────────────────

/// 解析 prompt audio codes：用户传文件 → codec_encode；否则查 builtin voice。
fn resolve_prompt_audio_codes(
    inner: &mut ProviderInner,
    voice: Option<&str>,
    prompt_audio_path: Option<&Path>,
) -> Result<Vec<Vec<i32>>, TtsError> {
    if let Some(path) = prompt_audio_path {
        let target_sr = inner.manifest.codec_config().sample_rate;
        let target_ch = inner.manifest.codec_config().channels;
        let waveform = load_reference_audio(path, target_sr, target_ch)?;
        let waveform_arr = waveform.into_channel_major_ndarray();
        let nq = inner.manifest.codec_config().num_quantizers;
        return inner.codec.encode(waveform_arr, nq);
    }

    let resolved_voice = voice.unwrap_or_else(|| {
        inner
            .manifest
            .builtin_voices()
            .first()
            .map(|v| v.voice.as_str())
            .unwrap_or("")
    });
    let voice_row = inner
        .manifest
        .builtin_voices()
        .iter()
        .find(|v| v.voice == resolved_voice)
        .ok_or_else(|| {
            TtsError::VoiceNotFound(
                resolved_voice.to_string(),
                inner
                    .manifest
                    .builtin_voices()
                    .iter()
                    .map(|v| v.voice.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        })?;
    if voice_row.prompt_audio_codes.is_empty() {
        return Err(TtsError::VoiceNotFound(
            resolved_voice.to_string(),
            "builtin voice 缺 prompt_audio_codes".into(),
        ));
    }
    Ok(voice_row.prompt_audio_codes.clone())
}

/// 构造一次单 chunk 的合成请求 rows + 执行 generate_audio_frames + decode_full。
/// 返回 (audio_codes, channel-major PCM, valid_samples, elapsed_seconds)。
fn run_single_chunk(
    inner: &mut ProviderInner,
    text: &str,
    prompt_audio_codes: &[Vec<i32>],
    generation: &GenerationParams,
    seed: Option<u64>,
) -> Result<(Vec<Vec<i32>>, Vec<Vec<f32>>, usize, f32), TtsError> {
    let start = std::time::Instant::now();

    // tokenize
    let token_ids_u32 = inner.tokenizer.encode(text)?;
    let text_token_ids: Vec<i32> = token_ids_u32.into_iter().map(|x| x as i32).collect();

    // 协议组装
    let request: VoiceCloneRequestRows =
        build_voice_clone_request_rows(&inner.manifest, prompt_audio_codes, &text_token_ids);

    // 自回归生成
    let frames = generate_audio_frames(
        &inner.manifest,
        &mut inner.sessions,
        &mut inner.local,
        generation,
        &request,
        seed,
        None,
    )?;

    // codec full decode
    let nq = inner.manifest.codec_config().num_quantizers;
    let (per_channel, valid) = inner.codec.decode_full(&frames, nq)?;

    let elapsed = start.elapsed().as_secs_f32();
    Ok((frames, per_channel, valid, elapsed))
}

/// 流式版本：用 codec_decode_step state machine + 自适应 budget 边生成边送 sink。
fn run_single_chunk_stream(
    inner: &mut ProviderInner,
    text: &str,
    prompt_audio_codes: &[Vec<i32>],
    generation: &GenerationParams,
    seed: Option<u64>,
    chunk_index_offset: usize,
    on_pcm: &mut dyn FnMut(usize, Vec<f32>) -> Result<(), TtsError>,
) -> Result<(usize, f32), TtsError> {
    let start = std::time::Instant::now();
    let token_ids_u32 = inner.tokenizer.encode(text)?;
    let text_token_ids: Vec<i32> = token_ids_u32.into_iter().map(|x| x as i32).collect();
    let request =
        build_voice_clone_request_rows(&inner.manifest, prompt_audio_codes, &text_token_ids);

    inner.streaming.reset();
    let mut emitted_samples: u64 = 0;
    let mut first_audio_at: Option<std::time::Instant> = None;
    let mut pending_frames: Vec<Vec<i32>> = Vec::new();
    let sample_rate = inner.manifest.codec_config().sample_rate;

    // 我们需要在 callback 里访问 inner.codec / inner.streaming，所以让 generate_audio_frames
    // 持有 &mut sessions/local；codec + streaming 通过 raw pointer trick 不可行（生命周期不允许）。
    // 解决：先收集 frames 到 Vec，再分批 streaming decode。这与 Python 的 on_frame
    // 行为略不同，但结果（PCM 总量）等价。
    let frames = generate_audio_frames(
        &inner.manifest,
        &mut inner.sessions,
        &mut inner.local,
        generation,
        &request,
        seed,
        None,
    )?;

    // 把 frames 按自适应预算 1/2/4/8 切片送 streaming codec
    let mut cursor = 0usize;
    let mut chunk_index = chunk_index_offset;
    while cursor < frames.len() {
        let budget = resolve_decode_frame_budget(emitted_samples, sample_rate, first_audio_at);
        let take = budget.min(frames.len() - cursor);
        let batch = &frames[cursor..cursor + take];
        cursor += take;
        pending_frames.extend_from_slice(batch);

        let decoded = inner.codec.run_streaming_frames(
            &mut inner.streaming,
            &pending_frames,
            &inner.manifest.codec_meta,
        )?;
        pending_frames.clear();

        if let Some((per_channel, valid)) = decoded {
            if valid > 0 {
                let interleaved = interleave_channels(&per_channel);
                emitted_samples += valid as u64;
                if first_audio_at.is_none() {
                    first_audio_at = Some(std::time::Instant::now());
                }
                chunk_index += 1;
                on_pcm(chunk_index, interleaved)?;
            }
        }
    }

    Ok((chunk_index, start.elapsed().as_secs_f32()))
}

#[async_trait::async_trait]
impl TtsProvider for OnnxTtsProvider {
    async fn synthesize(&self, params: SynthesisParams) -> Result<SynthesisResult, TtsError> {
        if params.text.is_empty() {
            return Err(TtsError::EmptyText);
        }
        let normalized = normalize_tts_text(&params.text);
        let inner_arc = self.inner.clone();
        let voice_clone = params.voice.clone();

        // Phase TTS-E / P2：Semaphore 控制并发数（最多 MAX_CONCURRENT_STREAMS 同时合成）
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| TtsError::SynthesisFailed("semaphore closed".into()))?;

        // 真推理走 spawn_blocking（CPU 重活，避免阻塞 tokio runtime）
        let outcome: Result<(SynthesisResult, ()), TtsError> =
            tokio::task::spawn_blocking(move || {
                let mut guard = inner_arc.lock().map_err(|e| {
                    TtsError::SynthesisFailed(format!("provider mutex poisoned: {e}"))
                })?;

                let prompt_codes = resolve_prompt_audio_codes(
                    &mut guard,
                    voice_clone.as_deref(),
                    params.prompt_audio_path.as_deref(),
                )?;

                let max_tokens = params.generation.voice_clone_max_text_tokens as usize;
                let chunks = crate::modules::tts::text::chunker::split_text_into_chunks(
                    &guard.tokenizer,
                    &normalized,
                    max_tokens,
                )?;
                let chunks: Vec<String> = if chunks.is_empty() {
                    vec![normalized.clone()]
                } else {
                    chunks
                };

                let sample_rate = guard.manifest.codec_config().sample_rate;
                let channels = guard.manifest.codec_config().channels;

                let mut all_pcm: Vec<f32> = Vec::new();
                let mut total_elapsed = 0.0f32;
                let chunk_count = chunks.len();
                for (idx, chunk) in chunks.iter().enumerate() {
                    let (_codes, per_channel, _valid, elapsed) = run_single_chunk(
                        &mut guard,
                        chunk,
                        &prompt_codes,
                        &params.generation,
                        params.generation.seed,
                    )?;
                    all_pcm.extend(interleave_channels(&per_channel));
                    total_elapsed += elapsed;
                    // chunk 间停顿（最后一段不加），与 Python 保持一致
                    if idx < chunk_count - 1 {
                        let pause_secs =
                            crate::modules::tts::text::chunker::estimate_inter_chunk_pause_seconds(
                                chunk,
                            );
                        let pause_samples =
                            (sample_rate as f32 * pause_secs).round() as usize * channels as usize;
                        all_pcm.extend(std::iter::repeat(0.0f32).take(pause_samples));
                    }
                }
                let wav_bytes = wav_encode(&all_pcm, sample_rate, channels)?;

                let frames_total = if channels > 0 {
                    all_pcm.len() / channels as usize
                } else {
                    0
                };
                let duration = if sample_rate > 0 {
                    frames_total as f32 / sample_rate as f32
                } else {
                    0.0
                };
                let voice_name = voice_clone.unwrap_or_else(|| DEFAULT_VOICE_NAME.to_string());

                Ok((
                    SynthesisResult {
                        audio_bytes: wav_bytes,
                        sample_rate,
                        channels,
                        duration_seconds: duration,
                        voice: voice_name,
                        text_chunks: chunks,
                        elapsed_seconds: total_elapsed,
                        normalized_text: normalized,
                    },
                    (),
                ))
            })
            .await
            .map_err(|e| TtsError::SynthesisFailed(format!("spawn_blocking: {e}")))?;

        outcome.map(|(r, _)| r)
    }

    async fn synthesize_stream(
        &self,
        params: SynthesisParams,
        sink: Arc<dyn AudioSink>,
    ) -> Result<StreamResult, TtsError> {
        if params.text.is_empty() {
            return Err(TtsError::EmptyText);
        }
        let normalized = normalize_tts_text(&params.text);
        let inner_arc = self.inner.clone();
        let voice_clone = params.voice.clone();

        // 流式：在 spawn_blocking 内跑 ONNX，PCM 通过 channel 转发到 async 上下文
        // 再把每块送到 sink.on_audio。
        // 首条消息是 StreamMeta（sample_rate, channels）让 async 侧不再硬编码 48k/2。
        // 后续消息是 (chunk_index, pcm_f32)。
        enum StreamEvent {
            Meta { sample_rate: u32, channels: u16 },
            Pcm { idx: usize, pcm: Vec<f32> },
        }
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
        let prompt_audio_path = params.prompt_audio_path.clone();
        let generation = params.generation.clone();
        let normalized_for_blocking = normalized.clone();
        let stream_start = std::time::Instant::now();

        // Phase TTS-E / P2：Semaphore（streaming 路径同样限流）
        let _stream_permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| TtsError::SynthesisFailed("semaphore closed".into()))?;

        let join: tokio::task::JoinHandle<Result<StreamResult, TtsError>> =
            tokio::task::spawn_blocking(move || -> Result<StreamResult, TtsError> {
                let mut guard = inner_arc.lock().map_err(|e| {
                    TtsError::SynthesisFailed(format!("provider mutex poisoned: {e}"))
                })?;
                let start = std::time::Instant::now();

                let prompt_codes = resolve_prompt_audio_codes(
                    &mut guard,
                    voice_clone.as_deref(),
                    prompt_audio_path.as_deref(),
                )?;
                let max_tokens = generation.voice_clone_max_text_tokens as usize;
                let chunks = crate::modules::tts::text::chunker::split_text_into_chunks(
                    &guard.tokenizer,
                    &normalized_for_blocking,
                    max_tokens,
                )?;
                let chunks: Vec<String> = if chunks.is_empty() {
                    vec![normalized_for_blocking.clone()]
                } else {
                    chunks
                };

                let sample_rate = guard.manifest.codec_config().sample_rate;
                let channels_u16 = guard.manifest.codec_config().channels;
                let channels = channels_u16 as usize;

                // 首条 Meta 让 async 侧立刻拿到 sample_rate/channels（去掉 hardcode 48k/2）
                if tx
                    .send(StreamEvent::Meta {
                        sample_rate,
                        channels: channels_u16,
                    })
                    .is_err()
                {
                    return Err(TtsError::StreamClosed(
                        "sink receiver dropped before meta".into(),
                    ));
                }

                let mut chunk_index_offset: usize = 0;
                let total_emitted_samples = std::cell::Cell::<u64>::new(0);
                let tx_ref = &tx;
                let total_ref = &total_emitted_samples;
                let mut pcm_pump = move |idx: usize, pcm: Vec<f32>| -> Result<(), TtsError> {
                    let len = pcm.len();
                    if tx_ref.send(StreamEvent::Pcm { idx, pcm }).is_err() {
                        return Err(TtsError::StreamClosed("sink receiver dropped".into()));
                    }
                    let frames = if channels > 0 { len / channels } else { len };
                    total_ref.set(total_ref.get() + frames as u64);
                    Ok(())
                };

                for chunk in &chunks {
                    let (next_idx, _elapsed) = run_single_chunk_stream(
                        &mut guard,
                        chunk,
                        &prompt_codes,
                        &generation,
                        generation.seed,
                        chunk_index_offset,
                        &mut pcm_pump,
                    )?;
                    chunk_index_offset = next_idx;
                }
                let total_emitted_samples = total_emitted_samples.get();

                let elapsed = start.elapsed().as_secs_f32();
                let emitted_seconds = if sample_rate > 0 {
                    total_emitted_samples as f32 / sample_rate as f32
                } else {
                    0.0
                };
                let rtf = if elapsed > 0.0 {
                    emitted_seconds / elapsed
                } else {
                    0.0
                };
                let result = StreamResult {
                    audio_path: None,
                    sample_rate,
                    channels: channels_u16,
                    voice: voice_clone.unwrap_or_else(|| DEFAULT_VOICE_NAME.to_string()),
                    text_chunks: chunks,
                    elapsed_seconds: elapsed,
                    emitted_audio_seconds: emitted_seconds,
                    lead_seconds: (emitted_seconds - elapsed).max(0.0),
                    first_audio_latency_seconds: 0.0, // 由 async 侧填充
                    realtime_factor: rtf,
                };
                drop(tx);
                Ok(result)
            });

        // async 侧：消费 channel → 调用 sink。Meta 一定先到，后续是 PCM。
        let mut chunk_emitted_seconds = 0.0f32;
        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut first_audio_at: Option<std::time::Instant> = None;
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::Meta {
                    sample_rate: sr,
                    channels: ch,
                } => {
                    sample_rate = sr;
                    channels = ch;
                }
                StreamEvent::Pcm { idx, pcm } => {
                    if first_audio_at.is_none() {
                        first_audio_at = Some(std::time::Instant::now());
                    }
                    let mut bytes = Vec::with_capacity(pcm.len() * 2);
                    for s in &pcm {
                        let clamped = s.clamp(-1.0, 1.0);
                        let v = (clamped * i16::MAX as f32) as i16;
                        bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    let ch_count = channels.max(1) as usize;
                    let frames = pcm.len() / ch_count;
                    let chunk_secs = if sample_rate > 0 {
                        frames as f32 / sample_rate as f32
                    } else {
                        0.0
                    };
                    chunk_emitted_seconds += chunk_secs;
                    let elapsed = stream_start.elapsed().as_secs_f32();
                    let lead = chunk_emitted_seconds - elapsed;
                    sink.on_audio(crate::modules::tts::AudioChunk {
                        pcm_data: bytes,
                        sample_rate,
                        channels,
                        chunk_index: idx,
                        is_pause: false,
                        emitted_audio_seconds: chunk_emitted_seconds,
                        lead_seconds: lead,
                    })
                    .await;
                }
            }
        }

        let mut result = join
            .await
            .map_err(|e| TtsError::SynthesisFailed(format!("stream join: {e}")))??;
        // 用 async 侧观察到的首音延迟覆写（更准确：包含 channel 跨线程开销）
        if let Some(first) = first_audio_at {
            result.first_audio_latency_seconds =
                first.saturating_duration_since(stream_start).as_secs_f32();
        }
        sink.on_complete(result.clone()).await;
        Ok(result)
    }

    async fn warmup(&self) -> Result<WarmupResult, TtsError> {
        let params = SynthesisParams {
            text: crate::modules::tts::config::WARMUP_TEXT.to_string(),
            mode: SynthesisMode::VoiceClone,
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams {
                max_new_frames: crate::modules::tts::config::WARMUP_MAX_FRAMES,
                ..Default::default()
            },
        };
        let result = self.synthesize(params).await?;
        Ok(WarmupResult {
            elapsed_seconds: result.elapsed_seconds,
            device: "cpu".to_string(),
        })
    }

    fn split_voice_clone_text(
        &self,
        text: &str,
        max_tokens: usize,
    ) -> Result<Vec<String>, TtsError> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| TtsError::SynthesisFailed(format!("provider mutex poisoned: {e}")))?;
        crate::modules::tts::text::chunker::split_text_into_chunks(
            &guard.tokenizer,
            text,
            max_tokens,
        )
    }

    fn list_voices(&self) -> Vec<String> {
        if let Ok(guard) = self.inner.lock() {
            let manifest_voices: Vec<String> = guard
                .manifest
                .builtin_voices()
                .iter()
                .map(|v| v.voice.clone())
                .collect();
            if !manifest_voices.is_empty() {
                return manifest_voices;
            }
        }
        list_voice_names().into_iter().map(String::from).collect()
    }

    fn get_voice(&self, _name: &str) -> Option<&VoicePreset> {
        // VoicePreset 是 owned 数据；manifest 里是 BuiltinVoice，签名不一致。
        // 命令侧应改用 list_voices() / manifest 直接访问；保留 None 兼容。
        None
    }

    fn default_voice(&self) -> &VoicePreset {
        static DEFAULT: std::sync::OnceLock<VoicePreset> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(|| {
            VoicePreset::new(DEFAULT_VOICE_NAME, "Default voice", "wav", Vec::new())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_model_dir_missing_manifest_returns_error() {
        let result = OnnxTtsProvider::from_model_dir(Path::new("/nonexistent"), None);
        match result {
            Err(TtsError::ManifestNotFound(_)) => {}
            Err(other) => panic!("expected ManifestNotFound, got {other:?}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }
}
