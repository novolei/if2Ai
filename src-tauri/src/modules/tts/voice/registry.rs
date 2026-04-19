//! Voice asset registry —— Phase TTS-D / P0。
//!
//! 统一管理 3 类语音来源：
//! 1. **Builtin (manifest)**：18 个 voice 自带 prebaked `prompt_audio_codes`
//!    （在 `~/.if2ai/models/tts/.../browser_poc_manifest.json`），合成时直接用
//!    codes，**不需要 wav 文件**。但**预览试听**功能（"我想听听 Junhao 长什么
//!    样"）需要原始 wav，从 `resource_dir/voices/` 读。
//! 2. **Bundled custom**：打包到 release 的 `resources/voices/*.{wav,mp3}`，
//!    比如自定义的 `sjl.mp3`。这类 voice **没有 manifest 里的 prebaked
//!    codes**，每次合成现场用 codec.encode 生成（成本 ~150ms / 首次）。
//! 3. **User uploaded**：用户运行时上传到 `app_data_dir/.if2ai/voices/*.{wav,mp3}`
//!    （以后做"声纹库管理"用）。
//!
//! ## 公开 API
//!
//! - [`VoiceAsset`] —— 语音资产元数据（id / display_name / kind / audio_path）
//! - [`VoiceRegistry::scan`] —— 扫描所有来源构建注册表
//! - [`VoiceRegistry::find`] —— 按 id 查
//! - [`VoiceRegistry::list`] —— 列出所有
//!
//! ## 路径解析
//!
//! - resource_dir / voices：通过 `tauri::Manager::path().resource_dir()` 拿到
//! - app data dir：`tauri::Manager::path().app_data_dir()` + `voices/`

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 语音资产来源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKind {
    /// MOSS-TTS-Nano manifest 内置 18 voice（合成走 prebaked codes）。
    Builtin,
    /// 应用打包带的自定义 voice（合成走 codec.encode 现场编码）。
    Bundled,
    /// 用户上传的 voice。
    UserUploaded,
}

/// 一个可用的语音资产。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceAsset {
    /// 唯一 id：builtin 用 voice name（"Junhao"），bundled/user 用文件 stem（"sjl"）。
    pub id: String,
    /// 展示名（manifest 里的 display_name 或文件 stem 美化后）。
    pub display_name: String,
    /// 来源类型。
    pub kind: VoiceKind,
    /// 用于试听的原始音频路径（builtin 可能为 None — 表示 release 没带这个 demo wav）。
    pub audio_path: Option<PathBuf>,
    /// 可选的简介。
    pub description: Option<String>,
    /// 可选的语言标签（"zh" / "en" / "jp" 等，从文件名首段推导）。
    pub language: Option<String>,
}

impl VoiceAsset {
    /// 是否能试听（audio_path 存在且文件存在）。
    pub fn is_previewable(&self) -> bool {
        self.audio_path
            .as_ref()
            .map(|p| p.exists())
            .unwrap_or(false)
    }
}

/// 全局语音注册表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VoiceRegistry {
    pub voices: Vec<VoiceAsset>,
}

impl VoiceRegistry {
    /// 扫描所有来源构建注册表。
    ///
    /// # 参数
    /// - `manifest_voices`：manifest 里 18 builtin voice 的 (id, display_name, optional zh语言标签)
    /// - `resource_voice_dir`：`<resources>/voices/` 绝对路径（由 Tauri manager 提供）
    /// - `user_voice_dir`：`<app_data>/voices/` 绝对路径
    /// - `model_demo_audio_dir`：可选，`~/.if2ai/models/tts/voices/` —— 老的 demo wav 兜底
    pub fn scan(
        manifest_voices: &[(&str, &str)],
        resource_voice_dir: Option<&Path>,
        user_voice_dir: Option<&Path>,
        model_demo_audio_dir: Option<&Path>,
    ) -> Self {
        let mut voices: Vec<VoiceAsset> = Vec::new();

        // 1. Builtin —— 必须包含，audio_path 在 resource_voice_dir / model_demo_audio_dir 里找匹配
        for (voice_id, display_name) in manifest_voices {
            let candidate_filenames = guess_builtin_audio_filenames(voice_id);
            let audio_path = candidate_filenames.iter().find_map(|fname| {
                resource_voice_dir
                    .map(|d| d.join(fname))
                    .filter(|p| p.exists())
                    .or_else(|| {
                        model_demo_audio_dir
                            .map(|d| d.join(fname))
                            .filter(|p| p.exists())
                    })
            });
            voices.push(VoiceAsset {
                id: (*voice_id).to_string(),
                display_name: (*display_name).to_string(),
                kind: VoiceKind::Builtin,
                audio_path,
                description: None,
                language: language_from_filename(voice_id),
            });
        }

        // 2. Bundled —— 扫 resource_voice_dir 里**不在 builtin** 列表的 .wav/.mp3
        if let Some(dir) = resource_voice_dir {
            for asset in scan_dir_voices(dir, VoiceKind::Bundled) {
                if !voices.iter().any(|v| v.id == asset.id) {
                    voices.push(asset);
                }
            }
        }

        // 3. UserUploaded
        if let Some(dir) = user_voice_dir {
            for asset in scan_dir_voices(dir, VoiceKind::UserUploaded) {
                if !voices.iter().any(|v| v.id == asset.id) {
                    voices.push(asset);
                }
            }
        }

        Self { voices }
    }

    pub fn find(&self, id: &str) -> Option<&VoiceAsset> {
        self.voices.iter().find(|v| v.id == id)
    }

    pub fn list(&self) -> &[VoiceAsset] {
        &self.voices
    }
}

/// 扫描某目录下的音频文件，构造 [`VoiceAsset`]。文件 stem 作为 id。
fn scan_dir_voices(dir: &Path, kind: VoiceKind) -> Vec<VoiceAsset> {
    let mut out: Vec<VoiceAsset> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "wav" | "mp3" | "flac" | "ogg" | "m4a"
        ) {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("voice")
            .to_string();
        // Phase TTS-E.1：可选 sidecar metadata（用户重命名时写入）
        let display_name =
            read_sidecar_display_name(dir, &stem).unwrap_or_else(|| prettify_id(&stem));
        out.push(VoiceAsset {
            id: stem.clone(),
            display_name,
            kind,
            audio_path: Some(path.clone()),
            description: None,
            language: language_from_filename(&stem),
        });
    }
    out
}

/// 读 `<dir>/<stem>.json` 里的 `display_name` 字段；不存在 / 解析失败返回 None。
fn read_sidecar_display_name(dir: &Path, stem: &str) -> Option<String> {
    let path = dir.join(format!("{stem}.json"));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("display_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 把 zh_1 / en_2 等 stem 美化成 "Zh 1" / "En 2"；保持 "sjl" / "Junhao" 等不变。
fn prettify_id(stem: &str) -> String {
    let lower = stem.to_lowercase();
    if let Some(rest) = lower.strip_prefix("zh_") {
        return format!("ZH {}", rest);
    }
    if let Some(rest) = lower.strip_prefix("en_") {
        return format!("EN {}", rest);
    }
    if let Some(rest) = lower.strip_prefix("jp_") {
        return format!("JP {}", rest);
    }
    stem.to_string()
}

/// 从文件名（stem）首段推导语言："zh_1" → "zh"；其他返回 None。
fn language_from_filename(stem: &str) -> Option<String> {
    let lower = stem.to_lowercase();
    for prefix in &["zh", "en", "jp", "ja", "ko", "fr", "de", "es"] {
        if lower.starts_with(&format!("{}_", prefix)) {
            return Some((*prefix).to_string());
        }
    }
    None
}

/// 给 builtin voice id 猜对应的 demo wav 文件名（按 MOSS demo.jsonl 习惯）。
///
/// 例如 "Junhao" → ["zh_1.wav", "junhao.wav"]。返回多个候选让调用方依次试。
fn guess_builtin_audio_filenames(voice_id: &str) -> Vec<String> {
    let lower = voice_id.to_lowercase();
    let mut out = vec![format!("{lower}.wav"), format!("{lower}.mp3")];
    // MOSS-TTS-Nano 习惯：voice 与某个 demo wav 对应
    let known_mapping: &[(&str, &str)] = &[
        ("junhao", "zh_1.wav"),
        ("zhiming", "zh_2.wav"),
        ("xiaoyu", "zh_3.wav"),
        ("yuewen", "zh_4.wav"),
        ("weiguo", "zh_5.wav"),
        ("lingyu", "zh_6.wav"),
        ("trump", "en_1.wav"),
        ("ava", "en_2.wav"),
        ("bella", "en_3.wav"),
        ("adam", "en_4.wav"),
        ("nathan", "en_5.wav"),
        ("sakura", "jp_1.mp3"),
        ("yui", "jp_2.wav"),
    ];
    if let Some((_, file)) = known_mapping
        .iter()
        .find(|(name, _)| *name == lower.as_str())
    {
        out.insert(0, (*file).to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prettify_id_basic() {
        assert_eq!(prettify_id("zh_1"), "ZH 1");
        assert_eq!(prettify_id("en_3"), "EN 3");
        assert_eq!(prettify_id("sjl"), "sjl");
        assert_eq!(prettify_id("Junhao"), "Junhao");
    }

    #[test]
    fn language_inference() {
        assert_eq!(language_from_filename("zh_1"), Some("zh".into()));
        assert_eq!(language_from_filename("en_5"), Some("en".into()));
        assert_eq!(language_from_filename("sjl"), None);
    }

    #[test]
    fn scan_with_no_dirs_returns_only_builtins() {
        let r = VoiceRegistry::scan(
            &[("Junhao", "Junhao 中文男 A"), ("Trump", "Trump")],
            None,
            None,
            None,
        );
        assert_eq!(r.voices.len(), 2);
        assert!(matches!(r.voices[0].kind, VoiceKind::Builtin));
        assert!(r.voices[0].audio_path.is_none(), "no path provided");
    }

    #[test]
    fn scan_picks_up_bundled_files_in_temp() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sjl.mp3"), b"fake").unwrap();
        std::fs::write(dir.path().join("zh_1.wav"), b"fake").unwrap();
        let r = VoiceRegistry::scan(&[("Junhao", "Junhao")], Some(dir.path()), None, None);
        // Junhao builtin + sjl bundled = 2（zh_1.wav 被 builtin Junhao 收为 audio_path 不再当 bundled）
        assert!(r.voices.iter().any(|v| v.id == "Junhao"));
        assert!(r
            .voices
            .iter()
            .any(|v| v.id == "sjl" && v.kind == VoiceKind::Bundled));
        let junhao = r.find("Junhao").unwrap();
        assert!(
            junhao.audio_path.is_some(),
            "Junhao should pick up zh_1.wav"
        );
    }
}
