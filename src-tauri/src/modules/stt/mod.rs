//! STT 模块 —— 仅支持 OpenFlow (SenseVoice ONNX)。
//!
//! ## 历史
//!
//! Phase TTS-E 最初支持三个 backend：
//!   - whisper.cpp 本地（Metal 加速）
//!   - Groq Whisper API（云端）
//!   - SenseVoice / OpenFlow（本地 ONNX）
//!
//! Apr 2026 起精简为单 backend = OpenFlow。原因：
//!   - whisper-rs 引入 ~3 min 冷编译开销 + macOS 11.0 deployment target 强约束
//!   - SenseVoice 在中文/英文场景下精度与 whisper-large-v3 相当或更优
//!   - 单 backend 简化用户认知（无需选择 / 无需配 API key）
//!
//! ## 架构
//!
//! ```text
//! 前端 MediaRecorder (PCM16LE) ──base64──▶ stt_transcribe (Tauri cmd)
//!                                            ├─ 解码 bytes → f32
//!                                            └─ OpenFlowAsrEngine.transcribe
//!                                                  ├─ 内部重采样到 16kHz
//!                                                  └─ ONNX 推理
//!                                                       └─▶ 文本 → 前端
//! ```

#![allow(dead_code)]

pub mod openflow;
pub mod settings;

/// STT 转写结果（OpenFlow / SenseVoice engine 共享）。
#[derive(Debug, Clone)]
pub struct TranscribeResult {
    pub text: String,
    /// 语言代码（"zh" / "en" 等）；auto-detect 时由模型推断。
    pub language: String,
    /// 耗时（秒）。
    pub elapsed_seconds: f32,
}
