//! Open Flow SenseVoice ONNX 本地 ASR 后端（vendored from https://github.com/jqlong17/open-flow，MIT）。
//!
//! Open Flow 用 SenseVoice-Small ONNX（FunASR/Alibaba）做中文/多语言 ASR：
//! - **量化版** ~230 MB（默认）
//! - **FP16 版** ~450 MB（精度更高）
//!
//! 子模块：
//! - `preprocess`：FBank + LFR + CMVN（与 FunASR WavFrontend 对齐）
//! - `onnx_inference`：ONNX Runtime session
//! - `decoder`：CTC 贪婪解码
//! - `engine`：对外 API，封装为 `OpenFlowAsrEngine::transcribe(pcm_f32, sr)`
//!
//! License attribution: 本目录 100% 移植自 open-flow `src/asr/`，遵守 MIT 协议。
//! 完整 LICENSE 见 docs/third-party-licenses/open-flow-LICENSE.md。

pub mod decoder;
pub mod downloader;
pub mod engine;
pub mod onnx_inference;
pub mod preprocess;

pub use downloader::{download_all, SenseVoicePreset};
pub use engine::{default_sensevoice_dir, model_is_ready, OpenFlowAsrEngine};
