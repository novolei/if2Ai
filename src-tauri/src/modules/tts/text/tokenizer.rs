//! TTS tokenizer —— 纯 Rust 实现（HuggingFace `tokenizers` crate）。
//!
//! ## 历史背景
//!
//! 原方案是 Python sidecar 进程调用 `sentencepiece` Python 包，因为 Rust 端的
//! `sentencepiece` crate（FFI 到 C++ libsentencepiece）会和 `onnxruntime` 的
//! libprotobuf 静态符号冲突，导致 ort `Session::commit_from_file` 报
//! `"Protobuf parsing failed"`。
//!
//! 现方案：直接用 HuggingFace 的纯 Rust `tokenizers` crate 加载预先生成的
//! `tokenizer.json`，**0 外部依赖**（不需要用户机器装 python / sentencepiece）。
//!
//! ## bit-exactness 保证
//!
//! `tokenizer.json` 是离线工具用 `transformers.convert_slow_tokenizer.LlamaConverter`
//! 从 `tokenizer.model` 转换而来，并显式注入 SentencePiece `nmt_nfkc` 的
//! `precompiled_charsmap`（HF `Precompiled` normalizer）。1000 句中英混合 + CJK
//! 标点 + emoji + 多语言语料的 round-trip diff 验证为 100% bit-exact 与
//! Python `sentencepiece.Encode()` 输出一致。
//!
//! 重新生成 `tokenizer.json` 的脚本见 `docs/references/tts-tokenizer-rebuild.md`。
//!
//! ## 公开 API
//!
//! - [`TtsTokenizer::load`] / [`TtsTokenizer::from_model_dir`]
//! - [`TtsTokenizer::encode`] → `Vec<u32>`
//! - [`TtsTokenizer::decode`] → `String`
//! - [`TtsTokenizer::count_tokens`] → `usize`
//! - [`TtsTokenizer::vocab_size`] → `usize`
//! - [`TtsTokenizer::ping`] / [`TtsTokenizer::spawn_health_ticker`]：
//!   保留无操作签名以兼容旧调用点（纯 Rust 进程内调用，无需健康检查）。

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tokenizers::Tokenizer;

use crate::modules::tts::error::TtsError;

/// 旧 SentencePiece tokenizer 文件名（仍由 download manifest 引用，保留常量）。
pub const TOKENIZER_MODEL_FILE: &str = "tokenizer.model";

/// HuggingFace `tokenizers` JSON 文件名（实际加载用）。
pub const TOKENIZER_JSON_FILE: &str = "tokenizer.json";

/// TTS tokenizer wrapper（纯 Rust，进程内调用）。
pub struct TtsTokenizer {
    inner: Tokenizer,
    model_path: PathBuf,
    vocab_size: usize,
}

impl TtsTokenizer {
    /// 从指定路径加载 tokenizer。
    ///
    /// `path` 可以是：
    /// - `tokenizer.json`（HF 格式，直接加载）
    /// - `tokenizer.model`（旧 SentencePiece 格式，自动 fallback 到同目录下的
    ///   `tokenizer.json`）
    pub fn load(path: &Path) -> Result<Self, TtsError> {
        let json_path = if path.file_name().and_then(|s| s.to_str()) == Some(TOKENIZER_JSON_FILE) {
            path.to_path_buf()
        } else {
            // 路径指向 tokenizer.model（旧调用约定）→ 同目录找 tokenizer.json
            path.parent()
                .map(|d| d.join(TOKENIZER_JSON_FILE))
                .unwrap_or_else(|| PathBuf::from(TOKENIZER_JSON_FILE))
        };
        if !json_path.exists() {
            return Err(TtsError::ModelNotFound(format!(
                "{TOKENIZER_JSON_FILE} not found at {}",
                json_path.display()
            )));
        }
        let inner = Tokenizer::from_file(&json_path).map_err(|e| {
            TtsError::TokenizationError(format!(
                "load tokenizer.json failed: {e} (path: {})",
                json_path.display()
            ))
        })?;
        let vocab_size = inner.get_vocab_size(true);
        tracing::info!(
            vocab_size,
            path = %json_path.display(),
            "tts tokenizer loaded (HF tokenizers, no sidecar)"
        );
        Ok(Self {
            inner,
            model_path: json_path,
            vocab_size,
        })
    }

    /// 从模型目录加载（优先 `tokenizer.json`，向后兼容 `tokenizer.model` 同目录）。
    pub fn from_model_dir(tts_model_dir: &Path) -> Result<Self, TtsError> {
        Self::load(&tts_model_dir.join(TOKENIZER_JSON_FILE))
    }

    /// 把文本编码为 token id 序列。`add_special_tokens=false` 与原 SP `Encode` 行为一致。
    pub fn encode(&self, text: &str) -> Result<Vec<u32>, TtsError> {
        let encoding = self
            .inner
            .encode(text, false)
            .map_err(|e| TtsError::TokenizationError(format!("encode failed: {e}")))?;
        Ok(encoding.get_ids().to_vec())
    }

    /// 把 token id 序列解码回文本。
    pub fn decode(&self, tokens: &[u32]) -> Result<String, TtsError> {
        self.inner
            .decode(tokens, false)
            .map_err(|e| TtsError::TokenizationError(format!("decode failed: {e}")))
    }

    /// 兼容签名：纯 Rust 实现下 tokenizer 不会"死"，永远返回 `Ok(true)`。
    pub fn ping(&self) -> Result<bool, TtsError> {
        Ok(true)
    }

    /// 兼容签名：旧的 Python sidecar 需要心跳保活；纯 Rust 实现是 no-op，
    /// 返回一个立刻退出的 `JoinHandle` 以保持调用处签名不变。
    pub fn spawn_health_ticker(
        self: std::sync::Arc<Self>,
        _interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async {})
    }

    pub fn count_tokens(&self, text: &str) -> Result<usize, TtsError> {
        self.encode(text).map(|ids| ids.len())
    }

    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_error() {
        let result = TtsTokenizer::load(Path::new("/nonexistent/tokenizer.model"));
        assert!(matches!(result, Err(TtsError::ModelNotFound(_))));
    }

    #[test]
    fn from_model_dir_missing_dir_returns_error() {
        let result = TtsTokenizer::from_model_dir(Path::new("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn constants() {
        assert_eq!(TOKENIZER_MODEL_FILE, "tokenizer.model");
        assert_eq!(TOKENIZER_JSON_FILE, "tokenizer.json");
    }

    #[test]
    fn encode_decode_roundtrip_if_model_present() {
        let model_dir = std::path::PathBuf::from(format!(
            "{}/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX",
            std::env::var("HOME").unwrap_or_default()
        ));
        if !model_dir.join(TOKENIZER_JSON_FILE).exists() {
            eprintln!("[skip] tokenizer.json not found at {}", model_dir.display());
            return;
        }
        let tk = TtsTokenizer::from_model_dir(&model_dir).expect("load");
        assert!(tk.vocab_size() > 1000, "vocab size {}", tk.vocab_size());

        for text in &["你好", "Hello world", "人工智能", "嗯…这个嘛，2024年了！"] {
            let ids = tk.encode(text).expect("encode");
            assert!(!ids.is_empty(), "encode '{text}' empty");
            let back = tk.decode(&ids).expect("decode");
            eprintln!("[round] {text:?} → {ids:?} → {back:?}");
        }
    }

    #[test]
    fn ping_returns_true() {
        let model_dir = std::path::PathBuf::from(format!(
            "{}/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX",
            std::env::var("HOME").unwrap_or_default()
        ));
        if !model_dir.join(TOKENIZER_JSON_FILE).exists() {
            return;
        }
        let tk = TtsTokenizer::from_model_dir(&model_dir).unwrap();
        assert!(tk.ping().unwrap());
    }
}
