//! Streaming audio delivery for TTS synthesis.
//!
//! Provides async channel-based streaming of PCM audio chunks
//! from the TTS synthesis pipeline to the frontend.
//!
//! Mirrors the streaming patterns in `app.py`'s `/api/generate/stream`
//! endpoint and `ort_cpu_runtime.py`'s `CodecStreamingDecodeSession`.
//!
//! ## Usage
//!
//! ```ignore
//! let (tx, rx) = create_audio_channel();
//! let sink = ChannelAudioSink::new(tx);
//! provider.synthesize_stream(params, Arc::new(sink)).await?;
//! ```

#![allow(dead_code)]

use tokio::sync::mpsc;

use crate::modules::tts::{AudioChunk, AudioSink, StreamResult};

/// Creates a bounded audio channel for streaming.
///
/// Returns a `(sender, receiver)` pair where the sender accepts
/// `AudioChunk` messages and the receiver yields them.
///
/// The channel has a capacity of 32 chunks (~3.2 seconds of audio
/// at 100ms per chunk), providing a balance between latency and
/// buffer stability.
pub fn create_audio_channel() -> (mpsc::Sender<AudioChunk>, mpsc::Receiver<AudioChunk>) {
    mpsc::channel(32)
}

/// Audio sink that forwards chunks to a tokio mpsc channel.
///
/// Implements [`AudioSink`] and sends each audio chunk to the
/// receiver end via an async channel. On complete, the channel
/// is closed.
pub struct ChannelAudioSink {
    sender: mpsc::Sender<AudioChunk>,
    complete_sender: mpsc::Sender<StreamResult>,
}

impl ChannelAudioSink {
    /// Create a new channel audio sink.
    ///
    /// # Arguments
    ///
    /// * `audio_sender` - Channel sender for audio chunks.
    /// * `complete_sender` - Channel sender for the completion result.
    #[must_use]
    pub fn new(
        audio_sender: mpsc::Sender<AudioChunk>,
        complete_sender: mpsc::Sender<StreamResult>,
    ) -> Self {
        Self {
            sender: audio_sender,
            complete_sender,
        }
    }
}

#[async_trait::async_trait]
impl AudioSink for ChannelAudioSink {
    async fn on_audio(&self, chunk: AudioChunk) {
        // If the receiver is dropped, silently drop the chunk.
        let _ = self.sender.send(chunk).await;
    }

    async fn on_complete(&self, result: StreamResult) {
        let _ = self.complete_sender.send(result).await;
    }
}

/// Collects all audio chunks from a receiver into a single PCM buffer.
///
/// Useful for converting streaming output to buffered WAV output.
///
/// # Arguments
///
/// * `receiver` - Channel receiver for audio chunks.
///
/// # Returns
///
/// A tuple of `(all_pcm_bytes, total_chunks)`.
pub async fn collect_chunks(receiver: &mut mpsc::Receiver<AudioChunk>) -> (Vec<u8>, usize) {
    let mut all_bytes = Vec::new();
    let mut count = 0;

    while let Some(chunk) = receiver.recv().await {
        all_bytes.extend_from_slice(&chunk.pcm_data);
        count += 1;
    }

    (all_bytes, count)
}

/// Calculates lead time for streaming playback.
///
/// Mirrors `_compute_stream_lead_seconds()` from
/// `ort_cpu_runtime.py:208-213`.
///
/// Lead time = emitted_audio_seconds - elapsed_wall_time.
/// Positive lead means the stream is ahead of real-time playback.
///
/// # Arguments
///
/// * `emitted_audio_seconds` - Total audio emitted so far in seconds.
/// * `elapsed_seconds` - Wall clock time since stream started.
///
/// # Returns
///
/// Lead time in seconds (can be negative if behind).
#[must_use]
pub fn compute_lead_seconds(emitted_audio_seconds: f32, elapsed_seconds: f32) -> f32 {
    emitted_audio_seconds - elapsed_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn channel_audio_sink_forwards_chunks() {
        let (audio_tx, mut audio_rx) = mpsc::channel(32);
        let (complete_tx, mut complete_rx) = mpsc::channel(1);
        let sink = ChannelAudioSink::new(audio_tx, complete_tx);

        let chunk = AudioChunk {
            pcm_data: vec![0u8; 960],
            sample_rate: 48_000,
            channels: 2,
            chunk_index: 0,
            is_pause: false,
            emitted_audio_seconds: 0.1,
            lead_seconds: 0.0,
        };

        sink.on_audio(chunk.clone()).await;
        let received = audio_rx.recv().await.unwrap();
        assert_eq!(received.pcm_data.len(), 960);

        // Test completion
        let result = StreamResult {
            audio_path: None,
            sample_rate: 48_000,
            channels: 2,
            voice: "test".to_string(),
            text_chunks: vec!["test".to_string()],
            elapsed_seconds: 0.5,
            emitted_audio_seconds: 1.0,
            lead_seconds: 0.5,
            first_audio_latency_seconds: 0.05,
            realtime_factor: 2.0,
        };
        sink.on_complete(result.clone()).await;
        let received_result = complete_rx.recv().await.unwrap();
        assert_eq!(received_result.voice, "test");
    }

    #[tokio::test]
    async fn collect_chunks_aggregates_all_data() {
        let (tx, mut rx) = mpsc::channel(32);

        for i in 0..5 {
            tx.send(AudioChunk {
                pcm_data: vec![i as u8; 100],
                sample_rate: 48_000,
                channels: 2,
                chunk_index: i,
                is_pause: false,
                emitted_audio_seconds: (i + 1) as f32 * 0.1,
                lead_seconds: 0.0,
            })
            .await
            .unwrap();
        }
        drop(tx); // Close the channel

        let (bytes, count) = collect_chunks(&mut rx).await;
        assert_eq!(count, 5);
        assert_eq!(bytes.len(), 500);
    }

    #[test]
    fn compute_lead_seconds_positive() {
        assert!((compute_lead_seconds(1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn compute_lead_seconds_negative() {
        assert!((compute_lead_seconds(0.3, 0.5) - (-0.2)).abs() < 1e-6);
    }
}
