//! TTS Profiles — named bundles of (voice + settings + text postprocess) the
//! user can switch between with a single click.
//!
//! ## Why profiles?
//!
//! MOSS-TTS-Nano is a pure voice-clone model: it has no `emotion` /
//! `style` token.  Perceived emotion is 90 % driven by which reference
//! audio is selected and 10 % by sampler params.  So instead of giving
//! users a *sea of knobs*, we let them name and reuse complete recipes:
//!
//! - "客服温柔"  = 温柔参考音 + Natural preset + 0.95× speed + soften punctuation
//! - "冷静播报"  = 标准男声 + Precise preset + 1.05× speed
//! - "快速速览"  = 任意音 + Balanced + 1.35× speed
//!
//! Profiles compose orthogonally with the persisted [`TtsSettings`] —
//! a profile *contains* a `TtsSettings` snapshot, and switching profile
//! is equivalent to atomically updating the active voice + settings.
//!
//! ## Storage
//!
//! `~/.if2ai/tts_profiles.json`:
//!
//! ```json
//! {
//!   "default_profile_id": "builtin-default",
//!   "profiles": [
//!     { "id": "builtin-default", "name": "默认对话", ... },
//!     { "id": "user-uuid",       "name": "我的客服", ... }
//!   ]
//! }
//! ```
//!
//! On first load (file missing), [`TtsProfileBook::seed_builtin`] writes
//! 6 curated profiles so the user has something to play with immediately.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::modules::tts::settings::{TtsQualityPreset, TtsSettings};

const PROFILE_FILENAME: &str = "tts_profiles.json";

/// Builtin profile id prefix.  Builtin profiles are *protected* — the UI
/// won't let users delete them, but they can be duplicated and edited.
pub const BUILTIN_ID_PREFIX: &str = "builtin-";

/// Lightweight text post-processing flags.  These are the "fake emotion"
/// hooks since MOSS-TTS-Nano has no real emotion control: by manipulating
/// punctuation and inserting natural fillers we can shift perceived
/// rhythm without touching the model at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TtsTextPostprocess {
    /// Replace sentence-final periods with commas to soften the cadence.
    /// Use for "温柔/陪伴" profiles.
    #[serde(default)]
    pub soften_punctuation: bool,
    /// Append a trailing "…" to every sentence so the prosody trails off
    /// instead of dropping hard.  Use for "沉浸朗读" / 故事 profiles.
    #[serde(default)]
    pub add_trailing_dots: bool,
}

/// One named TTS recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsProfile {
    /// Stable id; builtin profiles use `BUILTIN_ID_PREFIX` so the UI
    /// can identify and protect them.  User profiles use a UUID.
    pub id: String,
    /// Display name shown in the chat-side picker and Settings list.
    pub name: String,
    /// Optional one-line description.
    #[serde(default)]
    pub description: String,
    /// Voice asset id (matches `TtsVoiceAsset.id` from the registry).
    /// Empty string = "fall back to whatever the user last picked
    /// outside this profile".  Builtin profiles set this to a known
    /// bundled voice; users editing a profile can pick any voice.
    pub voice_id: String,
    /// Generation / playback settings snapshot.
    #[serde(flatten)]
    pub settings: TtsSettings,
    /// Text post-processing flags.
    #[serde(default)]
    pub postprocess: TtsTextPostprocess,
    /// True if this profile is shipped with the app (read-only metadata).
    #[serde(default)]
    pub is_builtin: bool,
}

impl TtsProfile {
    /// Construct a fresh user profile cloning everything from `src`
    /// except `id` (regenerated) and `is_builtin` (always false).
    pub fn duplicate_as_user(src: &TtsProfile, new_id: String, new_name: String) -> Self {
        Self {
            id: new_id,
            name: new_name,
            description: src.description.clone(),
            voice_id: src.voice_id.clone(),
            settings: src.settings.clone(),
            postprocess: src.postprocess.clone(),
            is_builtin: false,
        }
    }

    /// Apply this profile's [`postprocess`] flags to a piece of text.
    /// Pure / cheap; safe to call on every chunk.
    #[must_use]
    pub fn apply_postprocess(&self, text: &str) -> String {
        let mut out = text.to_string();
        if self.postprocess.soften_punctuation {
            out = soften_chinese_periods(&out);
        }
        if self.postprocess.add_trailing_dots {
            out = append_trailing_ellipsis(&out);
        }
        out
    }
}

fn soften_chinese_periods(s: &str) -> String {
    // Replace 。 with 、 and . with , — keeps the segmentation but the
    // model produces a softer, less terminal contour.
    s.replace('。', "，").replace('.', ",")
}

fn append_trailing_ellipsis(s: &str) -> String {
    let trimmed = s.trim_end();
    if trimmed.ends_with('…') || trimmed.ends_with("...") {
        return s.to_string();
    }
    format!("{trimmed}…")
}

/// On-disk container for the profile book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsProfileBook {
    /// Currently selected profile id.  Always exists in `profiles`; if
    /// load detects a stale id it is reset to the first profile.
    pub default_profile_id: String,
    pub profiles: Vec<TtsProfile>,
}

impl TtsProfileBook {
    /// Load the profile book from disk, seeding builtins on first run.
    /// Always returns a valid book — corrupt JSON is logged and replaced.
    #[must_use]
    pub fn load(if2ai_home: &Path) -> Self {
        let path = if2ai_home.join(PROFILE_FILENAME);
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut book) = serde_json::from_str::<TtsProfileBook>(&raw) {
                book.normalize();
                return book;
            }
        }
        let mut seeded = Self::seed_builtin();
        let _ = seeded.save(if2ai_home);
        seeded
    }

    /// Persist to `<if2ai_home>/tts_profiles.json` (pretty JSON).
    pub fn save(&self, if2ai_home: &Path) -> std::io::Result<()> {
        fs::create_dir_all(if2ai_home)?;
        let path = if2ai_home.join(PROFILE_FILENAME);
        let body = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(&path, body)
    }

    /// Insert a new profile or update an existing one (matched by id).
    /// Returns the resulting profile (with `is_builtin` enforced false
    /// for user-created profiles).
    pub fn upsert(&mut self, mut profile: TtsProfile) -> TtsProfile {
        // Builtin protection: the UI may *clone* a builtin then save —
        // the clone must always be a user profile regardless of any
        // stale `is_builtin: true` from the source.
        if !profile.id.starts_with(BUILTIN_ID_PREFIX) {
            profile.is_builtin = false;
        }
        profile.settings.clamp();
        if let Some(slot) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            // Builtins: only `name` / `description` / `voice_id` /
            // `settings` / `postprocess` may change; `is_builtin` and
            // `id` stay frozen.
            if slot.is_builtin {
                slot.name = profile.name.clone();
                slot.description = profile.description.clone();
                slot.voice_id = profile.voice_id.clone();
                slot.settings = profile.settings.clone();
                slot.postprocess = profile.postprocess.clone();
                return slot.clone();
            }
            *slot = profile.clone();
            return profile;
        }
        self.profiles.push(profile.clone());
        profile
    }

    /// Delete by id.  Builtins cannot be deleted — returns `false`.
    /// If the deleted profile was the default, falls back to the first
    /// remaining profile.
    pub fn delete(&mut self, id: &str) -> bool {
        let Some(pos) = self.profiles.iter().position(|p| p.id == id) else {
            return false;
        };
        if self.profiles[pos].is_builtin {
            return false;
        }
        self.profiles.remove(pos);
        if self.default_profile_id == id {
            self.default_profile_id = self
                .profiles
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
        true
    }

    /// Set the default profile id.  Returns `false` if the id is unknown.
    pub fn set_default(&mut self, id: &str) -> bool {
        if !self.profiles.iter().any(|p| p.id == id) {
            return false;
        }
        self.default_profile_id = id.to_string();
        true
    }

    /// Resolve the currently-default profile.
    #[must_use]
    pub fn default_profile(&self) -> Option<&TtsProfile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.default_profile_id)
            .or_else(|| self.profiles.first())
    }

    /// Resolve any profile by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TtsProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Repair invariants after deserialization: ensure `default_profile_id`
    /// resolves; clamp every profile's settings; mark builtin ids as
    /// builtin even if the on-disk flag was tampered with.
    fn normalize(&mut self) {
        for p in &mut self.profiles {
            p.settings.clamp();
            if p.id.starts_with(BUILTIN_ID_PREFIX) {
                p.is_builtin = true;
            }
        }
        if !self.profiles.iter().any(|p| p.id == self.default_profile_id) {
            self.default_profile_id = self
                .profiles
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
    }

    /// 6 curated profiles shipped with the app.  Voice ids reference
    /// bundled voice assets shipped under `src-tauri/resources/voices/`.
    /// The chat-side picker is responsible for falling back gracefully
    /// when a referenced voice id is absent on this install.
    #[must_use]
    pub fn seed_builtin() -> Self {
        let profiles = vec![
            TtsProfile {
                id: "builtin-default".into(),
                name: "默认对话".into(),
                description: "通用 AI 朗读，自然 + 1.0× 语速。".into(),
                voice_id: "zh_1".into(),
                settings: TtsSettings {
                    playback_rate: 1.0,
                    quality: TtsQualityPreset::Natural,
                    max_new_frames: 375,
                    audio_repetition_penalty: 1.2,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess::default(),
                is_builtin: true,
            },
            TtsProfile {
                id: "builtin-soft-service".into(),
                name: "温柔客服".into(),
                description: "温柔女声 · 0.95× 慢节奏 · 句号软化为逗号，听感更柔和。".into(),
                voice_id: "zh_1".into(),
                settings: TtsSettings {
                    playback_rate: 0.95,
                    quality: TtsQualityPreset::Natural,
                    max_new_frames: 500,
                    audio_repetition_penalty: 1.3,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess {
                    soften_punctuation: true,
                    add_trailing_dots: false,
                },
                is_builtin: true,
            },
            TtsProfile {
                id: "builtin-calm-narrator".into(),
                name: "冷静播报".into(),
                description: "标准男声 · Precise 预设 · 1.05× 紧凑节奏，适合新闻 / 文档朗读。".into(),
                voice_id: "Junhao".into(),
                settings: TtsSettings {
                    playback_rate: 1.05,
                    quality: TtsQualityPreset::Precise,
                    max_new_frames: 500,
                    audio_repetition_penalty: 1.35,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess::default(),
                is_builtin: true,
            },
            TtsProfile {
                id: "builtin-quick-skim".into(),
                name: "快速速览".into(),
                description: "1.35× 快速朗读 · 短帧上限 · 适合刷长文。".into(),
                voice_id: "zh_1".into(),
                settings: TtsSettings {
                    playback_rate: 1.35,
                    quality: TtsQualityPreset::Balanced,
                    max_new_frames: 250,
                    audio_repetition_penalty: 1.2,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess::default(),
                is_builtin: true,
            },
            TtsProfile {
                id: "builtin-immersive-reader".into(),
                name: "沉浸朗读".into(),
                description: "0.92× 慢速 · 句尾加省略号 · 适合小说 / 故事场景。".into(),
                voice_id: "Junhao".into(),
                settings: TtsSettings {
                    playback_rate: 0.92,
                    quality: TtsQualityPreset::Natural,
                    max_new_frames: 600,
                    audio_repetition_penalty: 1.25,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess {
                    soften_punctuation: false,
                    add_trailing_dots: true,
                },
                is_builtin: true,
            },
            TtsProfile {
                id: "builtin-energetic".into(),
                name: "激动播报".into(),
                description: "高温度采样 · 1.1× 快节奏 · 适合体育 / 营销 / 短视频解说。".into(),
                voice_id: "Junhao".into(),
                settings: TtsSettings {
                    playback_rate: 1.1,
                    quality: TtsQualityPreset::Natural,
                    max_new_frames: 500,
                    audio_repetition_penalty: 1.15,
                    seed: None,
                    enable_robust_normalization: true,
                },
                postprocess: TtsTextPostprocess::default(),
                is_builtin: true,
            },
        ];
        Self {
            default_profile_id: "builtin-default".into(),
            profiles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn seed_returns_six_protected_builtins() {
        let book = TtsProfileBook::seed_builtin();
        assert_eq!(book.profiles.len(), 6);
        assert!(book.profiles.iter().all(|p| p.is_builtin));
        assert_eq!(book.default_profile_id, "builtin-default");
    }

    #[test]
    fn first_load_writes_seed_to_disk() {
        let tmp = TempDir::new().unwrap();
        let book = TtsProfileBook::load(tmp.path());
        assert_eq!(book.profiles.len(), 6);
        assert!(tmp.path().join("tts_profiles.json").exists());
    }

    #[test]
    fn save_then_load_roundtrips_user_profile() {
        let tmp = TempDir::new().unwrap();
        let mut book = TtsProfileBook::seed_builtin();
        let mine = TtsProfile {
            id: "user-1".into(),
            name: "我的客服".into(),
            description: "test".into(),
            voice_id: "Trump".into(),
            settings: TtsSettings {
                playback_rate: 0.85,
                quality: TtsQualityPreset::Precise,
                max_new_frames: 400,
                audio_repetition_penalty: 1.4,
                seed: Some(7),
                enable_robust_normalization: false,
            },
            postprocess: TtsTextPostprocess {
                soften_punctuation: true,
                add_trailing_dots: false,
            },
            is_builtin: false,
        };
        book.upsert(mine.clone());
        book.save(tmp.path()).unwrap();
        let back = TtsProfileBook::load(tmp.path());
        let same = back.get("user-1").unwrap();
        assert_eq!(same, &mine);
    }

    #[test]
    fn cannot_delete_builtin() {
        let mut book = TtsProfileBook::seed_builtin();
        let ok = book.delete("builtin-default");
        assert!(!ok);
        assert!(book.get("builtin-default").is_some());
    }

    #[test]
    fn delete_user_profile_clears_default_when_was_default() {
        let mut book = TtsProfileBook::seed_builtin();
        book.upsert(TtsProfile {
            id: "user-x".into(),
            name: "x".into(),
            description: "".into(),
            voice_id: "zh_1".into(),
            settings: TtsSettings::default(),
            postprocess: TtsTextPostprocess::default(),
            is_builtin: false,
        });
        book.set_default("user-x");
        assert!(book.delete("user-x"));
        assert_ne!(book.default_profile_id, "user-x");
        assert!(book.get(&book.default_profile_id).is_some());
    }

    #[test]
    fn upsert_preserves_builtin_flag_and_id() {
        let mut book = TtsProfileBook::seed_builtin();
        let mut tweaked = book.get("builtin-default").unwrap().clone();
        tweaked.is_builtin = false; // attacker tries to "demote" a builtin
        tweaked.name = "Hacked".into();
        book.upsert(tweaked);
        let after = book.get("builtin-default").unwrap();
        assert!(after.is_builtin);
        assert_eq!(after.name, "Hacked"); // name still updates
    }

    #[test]
    fn set_default_rejects_unknown_id() {
        let mut book = TtsProfileBook::seed_builtin();
        assert!(!book.set_default("does-not-exist"));
        assert_eq!(book.default_profile_id, "builtin-default");
    }

    #[test]
    fn normalize_repairs_stale_default_id() {
        let mut book = TtsProfileBook {
            default_profile_id: "ghost".into(),
            profiles: TtsProfileBook::seed_builtin().profiles,
        };
        book.normalize();
        assert_eq!(book.default_profile_id, "builtin-default");
    }

    #[test]
    fn postprocess_soften_replaces_periods() {
        let p = TtsProfile {
            id: "u".into(),
            name: "n".into(),
            description: "".into(),
            voice_id: "v".into(),
            settings: TtsSettings::default(),
            postprocess: TtsTextPostprocess {
                soften_punctuation: true,
                add_trailing_dots: false,
            },
            is_builtin: false,
        };
        assert_eq!(p.apply_postprocess("你好。今天好。"), "你好，今天好，");
    }

    #[test]
    fn postprocess_trailing_dots_appends_once() {
        let p = TtsProfile {
            id: "u".into(),
            name: "n".into(),
            description: "".into(),
            voice_id: "v".into(),
            settings: TtsSettings::default(),
            postprocess: TtsTextPostprocess {
                soften_punctuation: false,
                add_trailing_dots: true,
            },
            is_builtin: false,
        };
        assert_eq!(p.apply_postprocess("你好"), "你好…");
        assert_eq!(p.apply_postprocess("你好…"), "你好…");
    }

    #[test]
    fn duplicate_as_user_drops_builtin_flag() {
        let book = TtsProfileBook::seed_builtin();
        let src = book.get("builtin-default").unwrap();
        let dup = TtsProfile::duplicate_as_user(src, "user-clone".into(), "我的副本".into());
        assert!(!dup.is_builtin);
        assert_eq!(dup.voice_id, src.voice_id);
        assert_eq!(dup.settings, src.settings);
    }
}
