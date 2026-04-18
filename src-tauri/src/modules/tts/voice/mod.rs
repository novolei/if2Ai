//! Voice preset system for TTS.
//!
//! Contains all 15 built-in voice definitions and 29 demo entries
//! from the MOSS-TTS-Nano Python reference.

pub mod demo;
pub mod presets;

#[allow(unused_imports)]
pub use demo::{DemoEntry, ALL_DEMOS};
#[allow(unused_imports)]
pub use presets::{get_voice_by_name, list_voice_names, ALL_VOICES, VOICE_COUNT};
