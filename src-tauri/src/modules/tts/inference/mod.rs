//! Inference pipeline for the TTS model.

#![allow(dead_code)]

pub mod decode;
pub mod prefill;
pub mod sampling;
pub mod streaming;

#[allow(unused_imports)]
pub use decode::{DecodeRunner, DecodeState};
#[allow(unused_imports)]
pub use prefill::PrefillRunner;
#[allow(unused_imports)]
pub use sampling::{
    apply_repetition_penalty, argmax, argmax_with_repetition_penalty, sample_assistant_text_token,
    sample_audio_token, sample_from_scores, softmax, SamplingParams,
};
#[allow(unused_imports)]
pub use streaming::{collect_chunks, compute_lead_seconds, create_audio_channel, ChannelAudioSink};
