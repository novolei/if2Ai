//! Built-in voice preset definitions.
//!
//! 15 voice presets mirroring `_DEFAULT_VOICE_FILES` from
//! `moss_tts_nano_runtime.py` in the Python reference.
//!
//! Each voice is identified by name and maps to a reference audio
//! file in the `assets/audio/` directory of the MOSS-TTS-Nano repo.

use crate::modules::tts::config::VoicePreset;

/// All 15 voice presets as (name, audio_file, description) tuples.
pub const VOICE_DEFINITIONS: &[(
    /* name */ &str,
    /* audio_file */ &str,
    /* description */ &str,
)] = &[
    ("Junhao", "zh_1.wav", "Chinese male voice A"),
    ("Zhiming", "zh_2.wav", "Chinese male voice B"),
    ("Xiaoyu", "zh_3.wav", "Chinese female voice A"),
    ("Yuewen", "zh_4.wav", "Chinese female voice B"),
    ("Weiguo", "zh_5.wav", "Chinese male voice C"),
    ("Lingyu", "zh_6.wav", "Chinese female voice C"),
    ("Trump", "en_1.wav", "Trump reference voice"),
    ("Ava", "en_2.wav", "English female voice A"),
    ("Bella", "en_3.wav", "English female voice B"),
    ("Adam", "en_4.wav", "English male voice A"),
    ("Nathan", "en_5.wav", "English male voice B"),
    ("Sakura", "jp_1.mp3", "Japanese female voice A"),
    ("Yui", "jp_2.wav", "Japanese female voice B"),
    ("Aoi", "jp_3.wav", "Japanese female voice C"),
    ("Hina", "jp_4.wav", "Japanese female voice D"),
];

/// The default voice name.
pub const DEFAULT_VOICE_NAME: &str = "Junhao";

/// Number of built-in voices.
pub const VOICE_COUNT: usize = VOICE_DEFINITIONS.len();

/// All voice presets as [`VoicePreset`] structs.
///
/// Audio data is empty at compile time — it is populated at runtime
/// by loading files from the model cache or bundled assets.
pub fn all_voices() -> Vec<VoicePreset> {
    VOICE_DEFINITIONS
        .iter()
        .map(|(name, audio_file, description)| {
            let ext = if audio_file.ends_with(".mp3") {
                "mp3"
            } else {
                "wav"
            };
            VoicePreset::new(name, description, ext, Vec::new())
        })
        .collect()
}

/// All voice presets, lazily initialized.
pub static ALL_VOICES: std::sync::OnceLock<Vec<VoicePreset>> = std::sync::OnceLock::new();

/// Returns the list of all voice preset names.
pub fn list_voice_names() -> Vec<&'static str> {
    VOICE_DEFINITIONS.iter().map(|(name, _, _)| *name).collect()
}

/// Get a voice preset by name.
pub fn get_voice_by_name(name: &str) -> Option<VoicePreset> {
    VOICE_DEFINITIONS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(n, audio_file, description)| {
            let ext = if audio_file.ends_with(".mp3") {
                "mp3"
            } else {
                "wav"
            };
            VoicePreset::new(n, description, ext, Vec::new())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_count_matches_python_reference() {
        assert_eq!(
            VOICE_DEFINITIONS.len(),
            15,
            "must match 15 voices from Python reference"
        );
    }

    #[test]
    fn all_voices_returns_correct_count() {
        let voices = all_voices();
        assert_eq!(voices.len(), 15);
    }

    #[test]
    fn list_voice_names_returns_all_names() {
        let names = list_voice_names();
        assert_eq!(names.len(), 15);
        assert!(names.contains(&"Junhao"));
        assert!(names.contains(&"Trump"));
        assert!(names.contains(&"Sakura"));
    }

    #[test]
    fn get_voice_by_name_returns_correct_voice() {
        let voice = get_voice_by_name("Junhao").expect("Junhao should exist");
        assert_eq!(voice.name, "Junhao");
        assert_eq!(voice.description, "Chinese male voice A");
        assert_eq!(voice.audio_ext, "wav");
    }

    #[test]
    fn get_voice_by_name_returns_none_for_unknown() {
        assert!(get_voice_by_name("UnknownVoice").is_none());
    }

    #[test]
    fn sakura_is_mp3_format() {
        let voice = get_voice_by_name("Sakura").expect("Sakura should exist");
        assert!(voice.is_mp3());
    }

    #[test]
    fn default_voice_name_exists() {
        let names = list_voice_names();
        assert!(names.contains(&DEFAULT_VOICE_NAME));
    }
}
