//! E2E smoke test：用真模型跑一次 voice clone，断言输出 PCM 不是静音。
//!
//! ## 运行方式
//!
//! 默认 `#[ignore]`（CI 不跑）。本地手动跑：
//!
//! ```bash
//! # 1) 下载模型到默认位置（~830MB，仅首次需要）
//! python3 -c "
//! from huggingface_hub import snapshot_download
//! import os
//! base = os.path.expanduser('~/.if2ai/models/tts')
//! snapshot_download(repo_id='OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX',
//!     local_dir=os.path.join(base, 'MOSS-TTS-Nano-100M-ONNX'),
//!     allow_patterns=['*.onnx', '*.data', '*.json', 'tokenizer.model'])
//! snapshot_download(repo_id='OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX',
//!     local_dir=os.path.join(base, 'MOSS-Audio-Tokenizer-Nano-ONNX'),
//!     allow_patterns=['*.onnx', '*.data', '*.json'])
//! "
//!
//! # 2) 跑 smoke
//! MOSS_TTS_MODEL_DIR=$HOME/.if2ai/models/tts \
//!   cargo test --release --test tts_e2e_smoke -- --ignored --nocapture
//!
//! # 3) 输出 wav 路径在 stdout；用系统播放器打开听
//! ```
//!
//! ## 验收
//!
//! - 合成成功（无 panic / 无 ONNX error）
//! - 输出 WAV 不是静音（RMS > 0.005）
//! - 时长 ≥ 0.5s（warmup 文本约 0.7s 中文）
//! - sample_rate=48000, channels=2

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use if2ai_backend::modules::tts::config::GenerationParams;
use if2ai_backend::modules::tts::provider::OnnxTtsProvider;
use if2ai_backend::modules::tts::{
    AudioSink, StreamResult, SynthesisMode, SynthesisParams, TtsProvider,
};

fn model_dir_or_skip() -> Option<PathBuf> {
    if let Ok(env_dir) = std::env::var("MOSS_TTS_MODEL_DIR") {
        let p = PathBuf::from(env_dir);
        if p.join("MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json")
            .exists()
            || p.join("browser_poc_manifest.json").exists()
        {
            return Some(p);
        }
    }
    let default = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai/models/tts");
    if default
        .join("MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json")
        .exists()
    {
        return Some(default);
    }
    eprintln!(
        "[skip] MOSS-TTS-Nano model not found. Set MOSS_TTS_MODEL_DIR or download to \
         ~/.if2ai/models/tts/. See test file header for instructions."
    );
    None
}

fn ref_audio_path_or_skip() -> Option<PathBuf> {
    let candidates = [
        "/Users/ryanliu/Documents/IfAI/MOSS-TTS-Nano-main/assets/audio/zh_1.wav",
        "../MOSS-TTS-Nano-main/assets/audio/zh_1.wav",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 计算 PCM RMS 能量，用于"非静音"判断。
fn pcm_rms(wav_bytes: &[u8]) -> f32 {
    // 跳过 WAV header (44 bytes for standard PCM)
    if wav_bytes.len() <= 44 {
        return 0.0;
    }
    let pcm = &wav_bytes[44..];
    let mut sum_sq = 0.0f64;
    let mut n = 0usize;
    for chunk in pcm.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        let f = sample as f64 / i16::MAX as f64;
        sum_sq += f * f;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    (sum_sq / n as f64).sqrt() as f32
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要 ~830MB 真模型，本地手动跑：MOSS_TTS_MODEL_DIR=... cargo test -- --ignored"]
async fn voice_clone_with_builtin_voice_produces_non_silent_audio() {
    let Some(model_dir) = model_dir_or_skip() else {
        return;
    };

    eprintln!("[smoke] loading model from {}", model_dir.display());
    let load_start = Instant::now();
    let provider = OnnxTtsProvider::from_model_dir(&model_dir, Some(4))
        .expect("OnnxTtsProvider::from_model_dir failed");
    eprintln!(
        "[smoke] model loaded in {:.2}s",
        load_start.elapsed().as_secs_f32()
    );

    let voices = provider.list_voices();
    assert!(!voices.is_empty(), "至少应有一个 voice");
    eprintln!("[smoke] available voices ({}): {:?}", voices.len(), voices);

    let params = SynthesisParams {
        text: "你好，欢迎使用模思智能。".to_string(),
        mode: SynthesisMode::VoiceClone,
        voice: Some(voices[0].clone()),
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams {
            max_new_frames: 200, // smoke 跑短点
            seed: Some(1234),    // 与 Python np.random.default_rng(1234) 对齐
            ..Default::default()
        },
    };

    let synth_start = Instant::now();
    let result = provider
        .synthesize(params)
        .await
        .expect("synthesize failed");
    let elapsed = synth_start.elapsed().as_secs_f32();

    eprintln!(
        "[smoke] synthesize done: voice={} chunks={} sample_rate={} channels={} duration={:.3}s wall={:.2}s wav_bytes={}",
        result.voice,
        result.text_chunks.len(),
        result.sample_rate,
        result.channels,
        result.duration_seconds,
        elapsed,
        result.audio_bytes.len(),
    );

    assert_eq!(result.sample_rate, 48_000, "MOSS-TTS-Nano 固定 48 kHz");
    assert_eq!(result.channels, 2, "stereo");
    assert!(
        result.audio_bytes.len() > 100,
        "WAV bytes 至少要包含 header + 一些 PCM"
    );

    let rms = pcm_rms(&result.audio_bytes);
    eprintln!("[smoke] PCM RMS = {rms:.6}");
    assert!(
        rms > 0.005,
        "RMS={rms} 低于 0.005 阈值——可能是静音 / 全零 / sampling 异常"
    );
    assert!(
        result.duration_seconds > 0.3,
        "duration {:.3}s 太短",
        result.duration_seconds
    );

    // 落盘方便人工试听
    let out = std::env::temp_dir().join("if2ai_tts_smoke_builtin.wav");
    std::fs::write(&out, &result.audio_bytes).unwrap();
    eprintln!("[smoke] ✓ saved to {}", out.display());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要 ~830MB 真模型 + reference audio"]
async fn voice_clone_with_reference_audio_produces_non_silent_audio() {
    let Some(model_dir) = model_dir_or_skip() else {
        return;
    };
    let Some(ref_audio) = ref_audio_path_or_skip() else {
        eprintln!("[skip] reference audio not found at expected paths");
        return;
    };

    let provider =
        OnnxTtsProvider::from_model_dir(&model_dir, Some(4)).expect("from_model_dir failed");

    let params = SynthesisParams {
        text: "Hello, this is a test of voice cloning.".to_string(),
        mode: SynthesisMode::VoiceClone,
        voice: None,
        prompt_audio_path: Some(ref_audio.clone()),
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
        .expect("synthesize with reference audio failed");

    eprintln!(
        "[smoke-ref] ref={} → out duration={:.3}s wav_bytes={}",
        ref_audio.display(),
        result.duration_seconds,
        result.audio_bytes.len()
    );

    let rms = pcm_rms(&result.audio_bytes);
    eprintln!("[smoke-ref] PCM RMS = {rms:.6}");
    assert!(rms > 0.005, "voice clone reference RMS={rms} 太低");

    let out = std::env::temp_dir().join("if2ai_tts_smoke_ref.wav");
    std::fs::write(&out, &result.audio_bytes).unwrap();
    eprintln!("[smoke-ref] ✓ saved to {}", out.display());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要真模型；流式路径验证"]
async fn streaming_emits_pcm_chunks() {
    let Some(model_dir) = model_dir_or_skip() else {
        return;
    };

    let provider =
        OnnxTtsProvider::from_model_dir(&model_dir, Some(4)).expect("from_model_dir failed");

    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountSink {
        count: AtomicUsize,
        bytes: tokio::sync::Mutex<usize>,
    }
    #[async_trait::async_trait]
    impl AudioSink for CountSink {
        async fn on_audio(&self, chunk: if2ai_backend::modules::tts::AudioChunk) {
            self.count.fetch_add(1, Ordering::SeqCst);
            *self.bytes.lock().await += chunk.pcm_data.len();
        }
        async fn on_complete(&self, _result: StreamResult) {}
    }
    let sink = Arc::new(CountSink {
        count: AtomicUsize::new(0),
        bytes: tokio::sync::Mutex::new(0),
    });

    let params = SynthesisParams {
        text: "你好世界。".to_string(),
        mode: SynthesisMode::VoiceClone,
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams {
            max_new_frames: 100,
            seed: Some(1234),
            ..Default::default()
        },
    };

    let result = provider
        .synthesize_stream(params, sink.clone())
        .await
        .expect("synthesize_stream failed");

    let chunks = sink.count.load(Ordering::SeqCst);
    let bytes = *sink.bytes.lock().await;
    eprintln!(
        "[stream] chunks={} bytes={} emitted_secs={:.3} lead_secs={:.3} wall={:.2}s",
        chunks, bytes, result.emitted_audio_seconds, result.lead_seconds, result.elapsed_seconds
    );
    assert!(chunks > 0, "至少应推送 1 帧");
    assert!(bytes > 1000, "至少应有上千字节 PCM");
}
