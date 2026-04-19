//! Phase TTS-E / P3：Whisper STT 模块。
//!
//! 与 TTS 对称：麦克录音（PCM16LE） → whisper.cpp 转写 → 文本 → 可送给
//! Agent 对话 / 直接触发 TTS 语音回复（STT → Agent → TTS 闭环）。
//!
//! ## 架构
//!
//! ```
//! 前端 MediaRecorder (PCM16/OGG) ──base64──▶ tts_stt_transcribe (Tauri cmd)
//!                                             ├─ 解码 bytes
//!                                             ├─ 重采样到 16kHz mono f32
//!                                             └─ whisper.cpp transcribe
//!                                                    └─▶ 文本 → 前端显示/发送
//! ```
//!
//! ## 模型下载
//!
//! whisper.cpp 使用 `.bin` 格式模型（GGML）。推荐 `ggml-small.en.bin` (~242MB)
//! 或 `ggml-base.bin` (~148MB，多语言）。
//! 下载命令（保存到 `~/.if2ai/models/whisper/`）：
//! ```bash
//! curl -L "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin" \
//!   -o ~/.if2ai/models/whisper/ggml-base.bin
//! ```

#![allow(dead_code)]

pub mod groq;
pub mod openflow;
pub mod settings;

use std::path::{Path, PathBuf};

/// whisper 默认模型目录。
pub fn default_whisper_model_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai/models/whisper")
}

/// 候选 whisper 模型文件名（按优先级：质量 vs 速度 trade-off）。
pub const WHISPER_MODEL_CANDIDATES: &[&str] = &[
    "ggml-small.bin",
    "ggml-small.en.bin",
    "ggml-base.bin",
    "ggml-base.en.bin",
    "ggml-tiny.bin",
    "ggml-tiny.en.bin",
];

/// 找到可用的 whisper 模型文件。
pub fn find_whisper_model() -> Option<PathBuf> {
    let dir = default_whisper_model_dir();
    for name in WHISPER_MODEL_CANDIDATES {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 转写结果。
#[derive(Debug, Clone)]
pub struct TranscribeResult {
    pub text: String,
    /// 语言代码（"zh" / "en" 等）；auto-detect 时由 whisper 推断。
    pub language: String,
    /// 耗时（秒）。
    pub elapsed_seconds: f32,
}

/// PCM 音频 (f32, 16kHz, mono) → 文本。
///
/// `model_path`：whisper GGML `.bin` 文件路径。
/// `pcm_f32`：16kHz mono float32 样本（由调用方负责重采样）。
/// `language`：None = auto-detect；"zh" / "en" 等 = 指定语言（更准/更快）。
///
/// 注：`whisper_rs::WhisperContext` 不是 Send，所以本函数必须在 `spawn_blocking` 内调用。
pub fn transcribe_pcm(
    model_path: &Path,
    pcm_f32: &[f32],
    language: Option<&str>,
) -> Result<TranscribeResult, String> {
    use std::time::Instant;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    if pcm_f32.is_empty() {
        return Err("PCM 样本为空".to_string());
    }

    let start = Instant::now();

    // 创建 ctx（模型加载，Metal 加速在此触发）
    let ctx = WhisperContext::new_with_params(
        model_path.to_str().ok_or("invalid model path")?,
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("whisper 模型加载失败: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if let Some(lang) = language {
        params.set_language(Some(lang));
    }
    params.set_n_threads(4);

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("create_state: {e}"))?;
    state
        .full(params, pcm_f32)
        .map_err(|e| format!("whisper.full: {e}"))?;

    let n_seg = state.full_n_segments();
    let mut text_parts: Vec<String> = Vec::new();
    for i in 0..n_seg {
        if let Some(seg) = state.get_segment(i) {
            text_parts.push(seg.to_string());
        }
    }
    let text = text_parts.join("").trim().to_string();
    // 语言 id（c_int）
    let lang_id = state.full_lang_id_from_state();
    let detected_lang = if lang_id >= 0 {
        // 直接用 lang id → 语言代码字符串（whisper 内置映射）
        // lang_id 0 = "en", 1 = "zh", etc. — 用简单映射
        lang_id_to_code(lang_id)
    } else {
        "?".to_string()
    };

    Ok(TranscribeResult {
        text,
        language: language.map(|s| s.to_string()).unwrap_or(detected_lang),
        elapsed_seconds: start.elapsed().as_secs_f32(),
    })
}

/// 把 whisper lang_id (c_int) 转成 ISO 语言代码（简化映射）。
fn lang_id_to_code(id: std::ffi::c_int) -> String {
    // whisper.cpp lang ids: 0=en, 1=zh, 2=de, 3=es, 4=ru, 5=ko, 6=fr, 7=ja, ...
    let codes = &[
        "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv",
        "it", "id", "hi", "fi", "vi", "he",
    ];
    codes.get(id as usize).copied().unwrap_or("?").to_string()
}
