//! TTS manifest 解析模块。
//!
//! MOSS-TTS-Nano 通过三份 JSON 描述模型：
//! - `browser_poc_manifest.json`：顶层 manifest，含 `model_files`、
//!   `tts_config`（n_vq / 各 audio_*_token_id）、`prompt_templates`、
//!   `builtin_voices`（含 prebaked `prompt_audio_codes`）、`generation_defaults`。
//! - `tts_browser_onnx_meta.json`：ONNX 文件名映射 + `model_config`
//!   （hidden / kv_heads / head_dim / local_layers ...）+ session I/O 名。
//! - `codec_browser_onnx_meta.json`：codec 文件名 + `codec_config`
//!   （sample_rate / channels / num_quantizers）+ `streaming_decode`
//!   （流式 transformer offsets / attention caches 元信息）。
//!
//! 旧版 ONNX-CPU 仓库目录名为 `MOSS-TTS-Nano-ONNX-CPU`，新版使用
//! `MOSS-TTS-Nano-100M-ONNX`，[`MODEL_DIR_ALIAS_MAP`] 负责把旧路径映射到新名。
//!
//! 参考：`MOSS-TTS-Nano-main/ort_cpu_runtime.py:283-345`。

#![allow(dead_code)]

pub mod browser_poc;
pub mod codec_meta;
pub mod tts_meta;

#[allow(unused_imports)]
pub use browser_poc::{
    BrowserPocManifest, BuiltinVoice, GenerationDefaults, PromptTemplates, TtsConfig,
};
#[allow(unused_imports)]
pub use codec_meta::{
    AttentionCacheSpec, CodecConfig, CodecMeta, StreamingDecodeMeta, TransformerOffsetSpec,
};
#[allow(unused_imports)]
pub use tts_meta::{LocalCachedOnnx, ModelConfig, OnnxNames, TtsMeta};

use std::path::{Path, PathBuf};

use crate::modules::tts::error::TtsError;

/// 旧目录名 → 新目录名 alias map（与 Python `MODEL_DIR_ALIAS_MAP` 1:1）。
pub const MODEL_DIR_ALIAS_MAP: &[(&str, &str)] = &[
    ("MOSS-TTS-Nano-ONNX-CPU", "MOSS-TTS-Nano-100M-ONNX"),
    (
        "MOSS-Audio-Tokenizer-Nano-ONNX-CPU",
        "MOSS-Audio-Tokenizer-Nano-ONNX",
    ),
];

/// 顶层 manifest 在模型目录下的候选相对路径（与 Python `MANIFEST_CANDIDATE_RELATIVE_PATHS` 同序）。
pub const MANIFEST_CANDIDATE_RELATIVE_PATHS: &[&str] = &[
    "browser_poc_manifest.json",
    "MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json",
    "MOSS-TTS-Nano-ONNX-CPU/browser_poc_manifest.json",
];

/// 已解析的全部 manifest 三件套。
///
/// 由 [`ManifestBundle::load`] 一次性构造；后续推理代码只读取本结构，
/// 不再直接接触 JSON。
#[derive(Debug, Clone)]
pub struct ManifestBundle {
    /// 顶层 manifest 内容。
    pub manifest: BrowserPocManifest,
    /// TTS meta（由 manifest.model_files.tts_meta 指向）。
    pub tts_meta: TtsMeta,
    /// Codec meta（由 manifest.model_files.codec_meta 指向）。
    pub codec_meta: CodecMeta,
    /// 顶层 manifest 文件所在目录（解析相对路径用）。
    pub manifest_dir: PathBuf,
    /// 用户传入的模型根目录（一般为 `~/.if2ai/models/tts/`）。
    pub model_dir: PathBuf,
}

impl ManifestBundle {
    /// 从模型根目录加载并解析全部三份 JSON。
    ///
    /// 1. 在 `model_dir` 下按 [`MANIFEST_CANDIDATE_RELATIVE_PATHS`] 依次尝试找到
    ///    `browser_poc_manifest.json`。
    /// 2. 由 manifest.`model_files.tts_meta` / `codec_meta` 解析另外两份 JSON。
    ///
    /// 任何一份缺失都会返回 [`TtsError::ManifestNotFound`]，错误信息包含全部 tried paths。
    pub fn load(model_dir: &Path) -> Result<Self, TtsError> {
        let resolved_model_dir = model_dir
            .canonicalize()
            .unwrap_or_else(|_| model_dir.to_path_buf());

        let manifest_path = resolve_manifest_path(&resolved_model_dir)?;
        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| TtsError::ManifestNotFound("manifest path has no parent".to_string()))?
            .to_path_buf();

        let manifest = BrowserPocManifest::load_from_file(&manifest_path)?;

        let tts_meta_path =
            resolve_manifest_relative_path(&manifest_dir, &manifest.model_files.tts_meta)?;
        let tts_meta = TtsMeta::load_from_file(&tts_meta_path)?;

        let codec_meta_path =
            resolve_manifest_relative_path(&manifest_dir, &manifest.model_files.codec_meta)?;
        let codec_meta = CodecMeta::load_from_file(&codec_meta_path)?;

        Ok(Self {
            manifest,
            tts_meta,
            codec_meta,
            manifest_dir,
            model_dir: resolved_model_dir,
        })
    }

    /// 根据 `tts_meta.files["<key>"]` 解析一个绝对 ONNX 文件路径。
    ///
    /// `key` 取值如 `"prefill"` / `"decode_step"` / `"local_decoder"` /
    /// `"local_cached_step"` / `"local_fixed_sampled_frame"` / `"local_greedy_frame"`。
    pub fn tts_onnx_path(&self, key: &str) -> Result<PathBuf, TtsError> {
        let relative =
            self.tts_meta.files.get(key).ok_or_else(|| {
                TtsError::ManifestNotFound(format!("tts_meta.files.{key} 不存在"))
            })?;
        let tts_dir = self
            .tts_meta_dir()
            .ok_or_else(|| TtsError::ManifestNotFound("tts_meta dir resolve 失败".into()))?;
        Ok(resolve_with_alias(&tts_dir, relative))
    }

    /// 根据 `codec_meta.files["<key>"]` 解析一个绝对 codec ONNX 文件路径。
    ///
    /// `key` 取值如 `"encode"` / `"decode_full"` / `"decode_step"`。
    pub fn codec_onnx_path(&self, key: &str) -> Result<PathBuf, TtsError> {
        let relative =
            self.codec_meta.files.get(key).ok_or_else(|| {
                TtsError::ManifestNotFound(format!("codec_meta.files.{key} 不存在"))
            })?;
        let codec_dir = self
            .codec_meta_dir()
            .ok_or_else(|| TtsError::ManifestNotFound("codec_meta dir resolve 失败".into()))?;
        Ok(resolve_with_alias(&codec_dir, relative))
    }

    /// 解析 tokenizer 路径。
    ///
    /// 优先返回 `tokenizer.json`（HF 格式，纯 Rust 加载），向后兼容 `tokenizer.model`
    /// （旧 SP 格式，由 `TtsTokenizer::load` 自动 fallback 到同目录 `.json`）。
    pub fn tokenizer_path(&self) -> Result<PathBuf, TtsError> {
        let relative = self
            .manifest
            .model_files
            .tokenizer_model
            .as_deref()
            .unwrap_or("tokenizer.model");

        // 优先级：manifest_dir/tokenizer.json > tts_meta_dir/tokenizer.json
        //       > manifest_dir/<relative> > tts_meta_dir/tokenizer.model
        let json_candidates = [
            self.manifest_dir.join("tokenizer.json"),
            self.tts_meta_dir()
                .map(|d| d.join("tokenizer.json"))
                .unwrap_or_default(),
        ];
        for cand in &json_candidates {
            if cand.exists() {
                return Ok(cand.clone());
            }
        }

        let resolved = self.manifest_dir.join(relative);
        if resolved.exists() {
            return Ok(resolved);
        }
        if let Some(tts_dir) = self.tts_meta_dir() {
            let candidate = tts_dir.join("tokenizer.model");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(TtsError::ModelNotFound(format!(
            "tokenizer.json/tokenizer.model 未在 manifest_dir / tts_meta_dir 下找到: {}",
            relative
        )))
    }

    /// 顶层 manifest.tts_config 的便捷访问。
    pub fn tts_config(&self) -> &TtsConfig {
        &self.manifest.tts_config
    }

    /// codec_meta.codec_config 的便捷访问。
    pub fn codec_config(&self) -> &CodecConfig {
        &self.codec_meta.codec_config
    }

    /// builtin voices 列表。
    pub fn builtin_voices(&self) -> &[BuiltinVoice] {
        &self.manifest.builtin_voices
    }

    fn tts_meta_dir(&self) -> Option<PathBuf> {
        let relative = &self.manifest.model_files.tts_meta;
        let path = self.manifest_dir.join(relative);
        path.parent().map(Path::to_path_buf)
    }

    fn codec_meta_dir(&self) -> Option<PathBuf> {
        let relative = &self.manifest.model_files.codec_meta;
        let path = self.manifest_dir.join(relative);
        path.parent().map(Path::to_path_buf)
    }
}

/// 在模型根目录下按候选路径找 `browser_poc_manifest.json`。
fn resolve_manifest_path(model_dir: &Path) -> Result<PathBuf, TtsError> {
    let mut tried: Vec<String> = Vec::new();
    for relative in MANIFEST_CANDIDATE_RELATIVE_PATHS {
        let candidate = model_dir.join(relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
        tried.push(candidate.display().to_string());
    }
    Err(TtsError::ManifestNotFound(format!(
        "browser_poc_manifest.json not found. tried: {}",
        tried.join(", ")
    )))
}

/// 解析 manifest 内的相对路径；找不到时尝试用 alias map 重写一次。
///
/// 镜像 Python `OrtCpuRuntime.resolve_manifest_relative_path()`。
fn resolve_manifest_relative_path(
    manifest_dir: &Path,
    relative: &str,
) -> Result<PathBuf, TtsError> {
    let direct = manifest_dir.join(relative);
    if direct.exists() {
        return Ok(direct);
    }
    let normalized = relative.replace('\\', "/");
    for (legacy, canonical) in MODEL_DIR_ALIAS_MAP {
        let fragment = format!("/{legacy}/");
        let probe = format!("/{normalized}/");
        if !probe.contains(&fragment) {
            continue;
        }
        let rewritten = normalized.replace(legacy, canonical);
        let candidate = manifest_dir.join(&rewritten);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(TtsError::ManifestNotFound(format!(
        "无法解析 manifest 相对路径 '{}'（已尝试 alias map）",
        relative
    )))
}

/// 在某目录下解析一个相对路径；不存在时也允许 alias map 重写。
///
/// 与 [`resolve_manifest_relative_path`] 区别：此函数即使最终路径不存在也返回它，
/// 由调用方决定如何处理（例如 ONNX `Session::load` 自己会报 ModelNotFound）。
fn resolve_with_alias(base: &Path, relative: &str) -> PathBuf {
    let direct = base.join(relative);
    if direct.exists() {
        return direct;
    }
    let normalized = relative.replace('\\', "/");
    for (legacy, canonical) in MODEL_DIR_ALIAS_MAP {
        if !normalized.contains(legacy) {
            continue;
        }
        let rewritten = normalized.replace(legacy, canonical);
        let candidate = base.join(&rewritten);
        if candidate.exists() {
            return candidate;
        }
    }
    direct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_manifest_returns_structured_error() {
        let err = ManifestBundle::load(Path::new("/nonexistent/tts/path")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("browser_poc_manifest.json"));
        assert!(msg.contains("tried"));
    }

    #[test]
    fn alias_map_contains_legacy_dirs() {
        let pairs: Vec<&str> = MODEL_DIR_ALIAS_MAP.iter().map(|(a, _)| *a).collect();
        assert!(pairs.contains(&"MOSS-TTS-Nano-ONNX-CPU"));
        assert!(pairs.contains(&"MOSS-Audio-Tokenizer-Nano-ONNX-CPU"));
    }

    #[test]
    fn candidate_paths_include_top_level_and_subdirs() {
        assert_eq!(
            MANIFEST_CANDIDATE_RELATIVE_PATHS[0],
            "browser_poc_manifest.json"
        );
        assert!(MANIFEST_CANDIDATE_RELATIVE_PATHS
            .iter()
            .any(|p| p.contains("MOSS-TTS-Nano-100M-ONNX")));
    }
}
