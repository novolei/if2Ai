//! User-tunable TTS settings (Phase TTS UX, exposed via Settings UI).
//!
//! Persists in `~/.if2ai/tts.toml` and is read by `commands/tts.rs` plus
//! the frontend `useAgentVoiceBridge` / `MessageVoiceButton` flows so
//! every TTS code path obeys the same user preferences.
//!
//! # Why a "playback_rate" instead of a model parameter?
//!
//! MOSS-TTS-Nano has no speed-factor knob in its inference signature.
//! Adjusting `audio_temperature` / `repetition_penalty` does change
//! perceived "rhythm" but not actual playback duration.  The simplest
//! and most reliable knob is the frontend's
//! [`AudioBufferSourceNode.playbackRate`] (0.5×-2.0×) — it works
//! immediately, has no audible artifacts inside ±25 % of 1.0×, and
//! matches what every browser-native SpeechSynthesisUtterance.rate
//! does under the hood.  See `useWebAudioStreamPlayer.ts`.
//!
//! # Quality presets
//!
//! [`TtsQualityPreset`] is a 3-way enum the UI surfaces as a dropdown
//! ("Natural / Balanced / Precise").  Each preset maps to a tuned
//! triple of `audio_temperature` / `audio_top_p` / `audio_top_k` that
//! lets non-technical users get a sensible tradeoff without touching
//! the [`crate::modules::tts::config::GenerationParams`] knobs.
//!
//! Power users still have the full schema available via the Advanced
//! section of the page.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::modules::tts::config::GenerationParams;

/// Filename inside `~/.if2ai/`.
const CONFIG_FILENAME: &str = "tts.toml";

/// Hard limits that the UI also enforces; keep in sync with the React
/// page's slider min/max.
pub const PLAYBACK_RATE_MIN: f32 = 0.5;
pub const PLAYBACK_RATE_MAX: f32 = 2.0;
pub const MAX_NEW_FRAMES_MIN: u32 = 64;
pub const MAX_NEW_FRAMES_MAX: u32 = 1500;

/// Quality preset for the audio sampler — surfaced as a dropdown.
///
/// The mapping to [`GenerationParams`] lives in [`Self::apply`]; do
/// not hard-code the numbers in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsQualityPreset {
    /// Slightly higher temperature → more natural / human-sounding
    /// prosody, occasional surprise pauses.  Good for chat / casual
    /// agent replies.
    #[default]
    Natural,
    /// Default MOSS-TTS-Nano settings — balanced between naturalness
    /// and stability.  Recommended for most use cases.
    Balanced,
    /// Lower temperature + tighter top_p → more predictable / robotic
    /// delivery, fewer mispronunciations.  Good for read-aloud /
    /// formal narration.
    Precise,
}

impl TtsQualityPreset {
    /// Project the preset onto the audio-sampler fields of `params`.
    pub fn apply(self, params: &mut GenerationParams) {
        match self {
            Self::Natural => {
                params.audio_temperature = 0.9;
                params.audio_top_p = 0.95;
                params.audio_top_k = 30;
            }
            Self::Balanced => {
                // Same as MOSS-TTS-Nano upstream defaults.
                params.audio_temperature = 0.8;
                params.audio_top_p = 0.95;
                params.audio_top_k = 25;
            }
            Self::Precise => {
                params.audio_temperature = 0.6;
                params.audio_top_p = 0.85;
                params.audio_top_k = 15;
            }
        }
    }
}

/// User-tunable TTS settings.  Mirror of `tts.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsSettings {
    /// Playback speed multiplier applied by the frontend WebAudio
    /// player.  `1.0` = original speed.  `0.5`-`2.0` is the supported
    /// range; values outside are clamped on save.
    pub playback_rate: f32,
    /// Quality preset that controls the audio sampler triple.
    pub quality: TtsQualityPreset,
    /// Hard cap on generated audio frames per chunk.  Lower values =
    /// faster first-byte latency but may truncate long sentences.
    pub max_new_frames: u32,
    /// Repetition penalty for the audio sampler.  Default 1.2.  Bump
    /// to 1.4-1.6 when you hear stuck-loop "the the the…" output.
    pub audio_repetition_penalty: f32,
    /// Optional fixed RNG seed for reproducible output.  `None` =
    /// random (default).
    pub seed: Option<u64>,
    /// When true, runs MOSS-TTS-Nano's robust text normaliser
    /// (handles numbers, dates, units …).  Default true.
    pub enable_robust_normalization: bool,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            playback_rate: 1.0,
            quality: TtsQualityPreset::default(),
            max_new_frames: 375,
            audio_repetition_penalty: 1.2,
            seed: None,
            enable_robust_normalization: true,
        }
    }
}

impl TtsSettings {
    /// Load settings from `<if2ai_home>/tts.toml`, falling back to the
    /// default when the file is missing or malformed.
    #[must_use]
    pub fn load(if2ai_home: &Path) -> Self {
        let path = if2ai_home.join(CONFIG_FILENAME);
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        parse_settings(&raw).unwrap_or_default()
    }

    /// Persist settings to `<if2ai_home>/tts.toml`.
    ///
    /// Out-of-range values are clamped before writing so the on-disk
    /// state always matches what the runtime would honour.
    pub fn save(&self, if2ai_home: &Path) -> std::io::Result<()> {
        fs::create_dir_all(if2ai_home)?;
        let mut clamped = self.clone();
        clamped.clamp();
        let body = format!(
            "# Generated by If2Ai Settings UI.  Hand-edits outside [tts]\n\
             # are preserved; values inside [tts] are overwritten on save.\n\
             \n\
             [tts]\n\
             # Playback speed multiplier (0.5-2.0).  1.0 = original.\n\
             playback_rate = {playback_rate}\n\
             # Sampler quality preset: natural | balanced | precise\n\
             quality = \"{quality}\"\n\
             # Maximum frames generated per chunk (64-1500).\n\
             max_new_frames = {max_new_frames}\n\
             # Audio repetition penalty (>= 1.0).  Bump to 1.4-1.6 to\n\
             # break stuck-loop output.\n\
             audio_repetition_penalty = {audio_repetition_penalty}\n\
             # Fixed RNG seed; comment out for random.\n\
             {seed_line}\n\
             # Enable MOSS-TTS-Nano robust text normaliser.\n\
             enable_robust_normalization = {enable_robust_normalization}\n",
            playback_rate = clamped.playback_rate,
            quality = quality_to_str(clamped.quality),
            max_new_frames = clamped.max_new_frames,
            audio_repetition_penalty = clamped.audio_repetition_penalty,
            seed_line = match clamped.seed {
                Some(s) => format!("seed = {s}"),
                None => "# seed = 12345".to_string(),
            },
            enable_robust_normalization = clamped.enable_robust_normalization,
        );
        let path = if2ai_home.join(CONFIG_FILENAME);
        fs::write(&path, body)
    }

    /// Project the user's settings onto a [`GenerationParams`] starting
    /// from upstream defaults — used by every backend code path that
    /// wants "the user's chosen TTS knobs".
    pub fn apply_to_generation_params(&self, params: &mut GenerationParams) {
        self.quality.apply(params);
        params.max_new_frames = self
            .max_new_frames
            .clamp(MAX_NEW_FRAMES_MIN, MAX_NEW_FRAMES_MAX);
        params.audio_repetition_penalty = self.audio_repetition_penalty.max(1.0);
        params.seed = self.seed;
        params.enable_robust_normalization = self.enable_robust_normalization;
    }

    /// Clamp every field into its supported range.
    pub fn clamp(&mut self) {
        if !self.playback_rate.is_finite() {
            self.playback_rate = 1.0;
        }
        self.playback_rate = self
            .playback_rate
            .clamp(PLAYBACK_RATE_MIN, PLAYBACK_RATE_MAX);
        self.max_new_frames = self
            .max_new_frames
            .clamp(MAX_NEW_FRAMES_MIN, MAX_NEW_FRAMES_MAX);
        if !self.audio_repetition_penalty.is_finite() || self.audio_repetition_penalty < 1.0 {
            self.audio_repetition_penalty = 1.0;
        }
    }
}

fn quality_to_str(q: TtsQualityPreset) -> &'static str {
    match q {
        TtsQualityPreset::Natural => "natural",
        TtsQualityPreset::Balanced => "balanced",
        TtsQualityPreset::Precise => "precise",
    }
}

fn parse_quality(s: &str) -> Option<TtsQualityPreset> {
    match s.trim().to_ascii_lowercase().as_str() {
        "natural" => Some(TtsQualityPreset::Natural),
        "balanced" => Some(TtsQualityPreset::Balanced),
        "precise" => Some(TtsQualityPreset::Precise),
        _ => None,
    }
}

/// Hand-rolled minimal TOML parser scoped to the `[tts]` section.
/// Avoids dragging in the `toml` crate just for one tiny config.
fn parse_settings(raw: &str) -> Option<TtsSettings> {
    let mut s = TtsSettings::default();
    let mut in_tts = false;
    let mut saw_anything = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            let section = rest.trim_end_matches(']').trim();
            in_tts = section.eq_ignore_ascii_case("tts");
            continue;
        }
        if !in_tts {
            continue;
        }
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let (key, value) = trimmed.split_at(eq);
        let key = key.trim().to_ascii_lowercase();
        let raw_value = value.trim_start_matches('=').trim();
        let unquoted = raw_value.trim_matches('"').trim_matches('\'');
        match key.as_str() {
            "playback_rate" => {
                if let Ok(v) = unquoted.parse::<f32>() {
                    s.playback_rate = v;
                    saw_anything = true;
                }
            }
            "quality" => {
                if let Some(q) = parse_quality(unquoted) {
                    s.quality = q;
                    saw_anything = true;
                }
            }
            "max_new_frames" => {
                if let Ok(v) = unquoted.parse::<u32>() {
                    s.max_new_frames = v;
                    saw_anything = true;
                }
            }
            "audio_repetition_penalty" => {
                if let Ok(v) = unquoted.parse::<f32>() {
                    s.audio_repetition_penalty = v;
                    saw_anything = true;
                }
            }
            "seed" => {
                if let Ok(v) = unquoted.parse::<u64>() {
                    s.seed = Some(v);
                    saw_anything = true;
                }
            }
            "enable_robust_normalization" => {
                let v = unquoted.eq_ignore_ascii_case("true");
                s.enable_robust_normalization = v;
                saw_anything = true;
            }
            _ => {}
        }
    }
    if saw_anything {
        s.clamp();
        Some(s)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_are_neutral_speed_and_balanced_quality() {
        let s = TtsSettings::default();
        assert_eq!(s.playback_rate, 1.0);
        assert_eq!(s.quality, TtsQualityPreset::Natural);
        assert_eq!(s.max_new_frames, 375);
    }

    #[test]
    fn clamp_keeps_playback_rate_in_range() {
        let mut s = TtsSettings {
            playback_rate: 5.0,
            ..Default::default()
        };
        s.clamp();
        assert_eq!(s.playback_rate, PLAYBACK_RATE_MAX);

        let mut s = TtsSettings {
            playback_rate: 0.1,
            ..Default::default()
        };
        s.clamp();
        assert_eq!(s.playback_rate, PLAYBACK_RATE_MIN);

        let mut s = TtsSettings {
            playback_rate: f32::NAN,
            ..Default::default()
        };
        s.clamp();
        assert_eq!(s.playback_rate, 1.0);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let s = TtsSettings {
            playback_rate: 1.25,
            quality: TtsQualityPreset::Precise,
            max_new_frames: 500,
            audio_repetition_penalty: 1.4,
            seed: Some(42),
            enable_robust_normalization: false,
        };
        s.save(tmp.path()).unwrap();
        let back = TtsSettings::load(tmp.path());
        assert_eq!(back, s);
    }

    #[test]
    fn load_returns_default_when_missing() {
        let tmp = TempDir::new().unwrap();
        let back = TtsSettings::load(tmp.path());
        assert_eq!(back, TtsSettings::default());
    }

    #[test]
    fn load_clamps_out_of_range_values_from_disk() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tts.toml");
        fs::write(
            &path,
            "[tts]\nplayback_rate = 99.0\nmax_new_frames = 999999\nquality = \"natural\"\n",
        )
        .unwrap();
        let back = TtsSettings::load(tmp.path());
        assert_eq!(back.playback_rate, PLAYBACK_RATE_MAX);
        assert_eq!(back.max_new_frames, MAX_NEW_FRAMES_MAX);
        assert_eq!(back.quality, TtsQualityPreset::Natural);
    }

    #[test]
    fn quality_preset_apply_changes_audio_sampler_triple() {
        let mut p = GenerationParams::default();
        TtsQualityPreset::Precise.apply(&mut p);
        assert_eq!(p.audio_temperature, 0.6);
        assert_eq!(p.audio_top_p, 0.85);
        assert_eq!(p.audio_top_k, 15);

        let mut p = GenerationParams::default();
        TtsQualityPreset::Natural.apply(&mut p);
        assert!(p.audio_temperature > 0.85);
    }

    #[test]
    fn apply_to_generation_params_threads_through_seed_and_normaliser() {
        let s = TtsSettings {
            playback_rate: 1.0,
            quality: TtsQualityPreset::Balanced,
            max_new_frames: 250,
            audio_repetition_penalty: 1.5,
            seed: Some(7),
            enable_robust_normalization: false,
        };
        let mut p = GenerationParams::default();
        s.apply_to_generation_params(&mut p);
        assert_eq!(p.seed, Some(7));
        assert!(!p.enable_robust_normalization);
        assert_eq!(p.audio_repetition_penalty, 1.5);
        assert_eq!(p.max_new_frames, 250);
    }
}
