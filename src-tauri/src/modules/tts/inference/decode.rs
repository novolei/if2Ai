//! Autoregressive decode loop for TTS audio frame generation.
//!
//! Mirrors `OrtCpuRuntime.generate_audio_frames()` from `ort_cpu_runtime.py:621-786`.
//!
//! ## Decode Flow
//!
//! 1. Run prefill on input text tokens → `global_hidden` + KV cache
//! 2. For each frame (up to `max_new_frames`):
//!    a. Determine text continuation (assistant slot vs end token)
//!    b. For each audio channel (0..n_vq): sample audio token with repetition penalty
//!    c. Run decode_step ONNX to update global_hidden + KV cache
//! 3. Return generated audio frames as `Vec<Vec<u32>>` (frame × channel)
//!
//! ## Session Paths
//!
//! The decode loop has 4 execution paths depending on available sessions:
//! - **local_greedy_frame**: All channels sampled at once (greedy mode)
//! - **local_fixed_sampled_frame**: All channels sampled at once (fixed sampling)
//! - **local_cached_step**: Per-channel incremental decode with local KV cache
//! - **local_decoder**: Fallback using full decoder per channel

#![allow(dead_code)]

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::modules::tts::config::GenerationParams;
use crate::modules::tts::error::TtsError;
use crate::modules::tts::inference::prefill::PrefillRunner;
use crate::modules::tts::inference::sampling::{
    sample_assistant_text_token, sample_audio_token, SamplingParams,
};
use crate::modules::tts::model::global::GlobalSessions;
use crate::modules::tts::model::local::LocalSessions;

/// Input tensor constants for the decode loop.
///
/// Mirrors the tensor names used in `ort_cpu_runtime.py:765-783`.
pub const AUDIO_ASSISTANT_SLOT_TOKEN_ID: &str = "audio_assistant_slot_token_id";
pub const AUDIO_END_TOKEN_ID: &str = "audio_end_token_id";
pub const AUDIO_PAD_TOKEN_ID: &str = "audio_pad_token_id";

/// Input/output names for the decode_step ONNX model.
pub const DECODE_INPUT_IDS: &str = "input_ids";
pub const DECODE_PAST_VALID_LENGTHS: &str = "past_valid_lengths";
pub const DECODE_GLOBAL_HIDDEN: &str = "global_hidden";

/// Prefix for decode_step KV cache output names.
pub const DECODE_PRESENT_PREFIX: &str = "present_";

/// Prefix for decode_step KV cache input names.
pub const DECODE_PAST_PREFIX: &str = "past_";

/// A single frame of audio tokens (one token per VQ channel).
///
/// Each frame is `Vec<u32>` with length `n_vq` (number of vector quantization channels).
pub type AudioFrame = Vec<u32>;

/// Generated audio frames: list of frames, each containing one token per VQ channel.
pub type GeneratedFrames = Vec<AudioFrame>;

/// State maintained across autoregressive decode steps.
///
/// Mirrors the mutable state in `generate_audio_frames()` from
/// `ort_cpu_runtime.py:639-783`: `global_hidden`, `past_by_name`,
/// `past_valid_length`, `generated_frames`, `previous_tokens_by_channel`,
/// and `previous_token_sets_by_channel`.
pub struct DecodeState {
    /// Last hidden state from the decoder, updated each step.
    pub global_hidden: ndarray::ArrayD<f32>,
    /// KV cache tensors from the previous decode step, renamed present_* → past_*.
    pub past_by_name: std::collections::HashMap<String, ndarray::ArrayD<f32>>,
    /// Number of valid tokens in the past KV cache (grows by 1 each step).
    pub past_valid_length: usize,
    /// All generated frames so far.
    pub generated_frames: GeneratedFrames,
    /// Previously generated tokens per channel (list, for repetition penalty).
    pub previous_tokens_by_channel: Vec<Vec<u32>>,
    /// Previously generated tokens per channel (set, for O(1) lookup).
    pub previous_token_sets_by_channel: Vec<std::collections::HashSet<u32>>,
}

/// Runs the autoregressive decode loop to generate audio frames from text tokens.
///
/// Mirrors `OrtCpuRuntime`'s `generate_audio_frames()` from
/// `ort_cpu_runtime.py:621-786`.
///
/// ## Usage
///
/// ```ignore
/// let runner = DecodeRunner::new(global_sessions, local_sessions, config)?;
/// let frames = runner.generate_audio_frames(&token_ids)?;
/// ```
pub struct DecodeRunner {
    /// Global ONNX sessions (prefill + decode_step).
    sessions: GlobalSessions,
    /// Local ONNX sessions (decoder, cached_step, fixed_sampled_frame).
    local: LocalSessions,
    /// Generation configuration.
    config: GenerationParams,
    /// TTS model config: number of VQ channels.
    n_vq: usize,
    /// Token ID for the audio assistant slot.
    audio_assistant_slot_token_id: u32,
    /// Token ID for the audio end token.
    audio_end_token_id: u32,
    /// Token ID for audio padding.
    audio_pad_token_id: u32,
}

impl DecodeRunner {
    /// Create a new decode runner.
    ///
    /// # Arguments
    ///
    /// * `sessions` - Global ONNX sessions (prefill + decode_step).
    /// * `local` - Local ONNX sessions (optional decoder, cached_step, etc.).
    /// * `config` - Generation parameters.
    /// * `n_vq` - Number of vector quantization channels.
    /// * `audio_assistant_slot_token_id` - Token ID for the assistant slot.
    /// * `audio_end_token_id` - Token ID for the end-of-audio token.
    /// * `audio_pad_token_id` - Token ID for padding tokens.
    pub fn new(
        sessions: GlobalSessions,
        local: LocalSessions,
        config: GenerationParams,
        n_vq: usize,
        audio_assistant_slot_token_id: u32,
        audio_end_token_id: u32,
        audio_pad_token_id: u32,
    ) -> Self {
        Self {
            sessions,
            local,
            config,
            n_vq,
            audio_assistant_slot_token_id,
            audio_end_token_id,
            audio_pad_token_id,
        }
    }

    /// Generate audio frames from tokenized text.
    ///
    /// This is the main entry point, mirroring `generate_audio_frames()`
    /// from `ort_cpu_runtime.py:621-786`.
    ///
    /// ## Flow
    ///
    /// 1. Run prefill on input tokens → global_hidden + KV cache
    /// 2. Run the autoregressive decode loop
    /// 3. Return generated frames
    ///
    /// # Arguments
    ///
    /// * `token_ids` - Tokenized text from the SentencePiece tokenizer.
    ///
    /// # Returns
    ///
    /// Generated audio frames: `Vec<Vec<u32>>`, each inner vec is one frame
    /// with `n_vq` audio tokens.
    pub fn generate_audio_frames(&self, token_ids: &[u32]) -> Result<GeneratedFrames, TtsError> {
        // Step 1: Run prefill on input tokens
        let prefill_result = PrefillRunner::run_from_session(&self.sessions.prefill, token_ids)?;

        // Step 2: Initialize decode state
        let seq_len = token_ids.len();
        let state = DecodeState {
            global_hidden: prefill_result.global_hidden,
            past_by_name: Self::build_past_cache(
                &prefill_result.kv_cache,
                self.sessions.prefill.output_names(),
            )?,
            past_valid_length: seq_len,
            generated_frames: Vec::new(),
            previous_tokens_by_channel: vec![Vec::new(); self.n_vq],
            previous_token_sets_by_channel: vec![std::collections::HashSet::new(); self.n_vq],
        };

        // Step 3: Run autoregressive decode loop
        let rng = Self::make_rng(&self.config);
        self.run_decode_loop(state, rng)
    }

    /// Build the past KV cache from prefill outputs.
    ///
    /// Renames `present_*` output names to `past_*` keys for the next step.
    /// Mirrors `ort_cpu_runtime.py:641-644`.
    #[allow(clippy::type_complexity)]
    fn build_past_cache(
        kv_outputs: &[ndarray::ArrayD<f32>],
        output_names: &[String],
    ) -> Result<std::collections::HashMap<String, ndarray::ArrayD<f32>>, TtsError> {
        let mut past_by_name = std::collections::HashMap::new();
        for (name, tensor) in output_names.iter().zip(kv_outputs.iter()) {
            if let Some(stripped) = name.strip_prefix("present_") {
                let past_name = format!("past_{}", stripped);
                past_by_name.insert(past_name, tensor.clone());
            }
        }
        Ok(past_by_name)
    }

    /// Create an RNG seeded from the config, or unseeded if none specified.
    fn make_rng(config: &GenerationParams) -> StdRng {
        match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        }
    }

    /// Run the autoregressive decode loop.
    ///
    /// Iterates up to `max_new_frames` times, generating one audio frame per iteration.
    /// The loop terminates early if the text token indicates end-of-audio.
    ///
    /// Mirrors the `for step_index in range(max_new_frames)` loop from
    /// `ort_cpu_runtime.py:649-785`.
    fn run_decode_loop(
        &self,
        mut state: DecodeState,
        mut rng: StdRng,
    ) -> Result<GeneratedFrames, TtsError> {
        for _step in 0..self.config.max_new_frames {
            let frame = self.generate_single_frame(&mut state, &mut rng)?;

            // Check if the frame signals end-of-audio (empty or all pad tokens)
            if frame.is_empty() {
                break;
            }

            state.generated_frames.push(frame.clone());
            for (ch, &token) in frame.iter().enumerate() {
                if ch < state.previous_tokens_by_channel.len() {
                    state.previous_tokens_by_channel[ch].push(token);
                    state.previous_token_sets_by_channel[ch].insert(token);
                }
            }

            // Update global_hidden, KV cache, and past_valid_length via decode_step
            self.update_decode_state(&mut state, &frame)?;
        }

        Ok(state.generated_frames)
    }

    /// Generate a single audio frame using the appropriate session path.
    ///
    /// Selects the decode path based on available local sessions and config:
    /// 1. `local_greedy_frame` when `do_sample=false`
    /// 2. `local_fixed_sampled_frame` when `sample_mode=="fixed"`
    /// 3. `local_cached_step` when available (per-channel incremental)
    /// 4. `local_decoder` fallback (full decoder per channel)
    ///
    /// Mirrors the if/elif/else chain from `ort_cpu_runtime.py:651-762`.
    fn generate_single_frame(
        &self,
        state: &mut DecodeState,
        rng: &mut StdRng,
    ) -> Result<AudioFrame, TtsError> {
        if self.local.fixed_sampleed_frame.is_some() || self.local.cached_step.is_some() {
            // Use cached_step path (most general sampling path)
            self.generate_frame_cached_step(state, rng)
        } else if self.local.decoder.is_some() {
            // Use local_decoder fallback path
            self.generate_frame_local_decoder(state, rng)
        } else {
            // No local sessions: return empty frame (will terminate loop)
            Ok(Vec::new())
        }
    }

    /// Generate a frame using the local_cached_step path.
    ///
    /// Mirrors `ort_cpu_runtime.py:672-739`:
    /// 1. Initialize empty local past KV cache
    /// 2. Run cached_step with text_token_id=0, audio_token_id=0, channel_index=0, step_type=0
    ///    → get text_logits, sample assistant text token
    /// 3. If text token != assistant_slot → end of audio, break
    /// 4. Run cached_step with text_token_id=next_text_token, audio_token_id=0, channel_index=0, step_type=1
    ///    → get audio_logits for channel 0, sample first audio token
    /// 5. For channels 1..n_vq:
    ///    Run cached_step with text_token_id=0, audio_token_id=previous_token, channel_index=ch-1, step_type=2
    ///    → get audio_logits, sample channel token
    fn generate_frame_cached_step(
        &self,
        state: &mut DecodeState,
        rng: &mut StdRng,
    ) -> Result<AudioFrame, TtsError> {
        let Some(cached_step) = &self.local.cached_step else {
            return Ok(Vec::new());
        };

        // Create empty local past KV cache
        let mut local_past_by_name = Self::create_empty_local_past(self.n_vq);
        let mut local_past_valid_length = 0;

        // Step 1: Get text logits
        let text_logits = Self::run_cached_step(
            cached_step,
            &state.global_hidden,
            0, // text_token_id
            0, // audio_token_id
            0, // channel_index
            0, // step_type
            local_past_valid_length,
            &local_past_by_name,
        )?;
        local_past_valid_length += 1;

        // Step 2: Sample assistant text token
        let text_params = SamplingParams::for_text(&self.config);
        let next_text_token = sample_assistant_text_token(
            &text_logits,
            self.audio_assistant_slot_token_id,
            self.audio_end_token_id,
            &text_params,
            rng,
        )?;

        // If text token is not assistant slot, end of audio
        if next_text_token != self.audio_assistant_slot_token_id {
            return Ok(Vec::new());
        }

        // Step 3: Get audio logits for channel 0
        let (_, audio_logits, updated_past) = Self::run_cached_step_full(
            cached_step,
            &state.global_hidden,
            next_text_token, // text_token_id
            0,               // audio_token_id
            0,               // channel_index
            1,               // step_type
            local_past_valid_length,
            &local_past_by_name,
        )?;
        local_past_valid_length += 1;
        local_past_by_name = updated_past;

        let mut frame = Vec::with_capacity(self.n_vq);

        // Sample first channel
        let channel_logits = Self::slice_audio_channel_logits(&audio_logits, 0, self.n_vq);
        let audio_params = SamplingParams::for_audio(&self.config);
        let mut previous_token = sample_audio_token(
            &channel_logits,
            &state.previous_tokens_by_channel[0],
            &state.previous_token_sets_by_channel[0],
            &audio_params,
            self.config.audio_repetition_penalty,
            rng,
        )?;
        frame.push(previous_token);

        // Step 4: Sample remaining channels
        for channel_index in 1..self.n_vq {
            let (_, audio_logits, updated_past) = Self::run_cached_step_full(
                cached_step,
                &state.global_hidden,
                0,                 // text_token_id
                previous_token,    // audio_token_id
                channel_index - 1, // channel_index
                2,                 // step_type
                local_past_valid_length,
                &local_past_by_name,
            )?;
            local_past_valid_length += 1;
            local_past_by_name = updated_past;

            let channel_logits =
                Self::slice_audio_channel_logits(&audio_logits, channel_index, self.n_vq);
            let sampled = sample_audio_token(
                &channel_logits,
                &state.previous_tokens_by_channel[channel_index],
                &state.previous_token_sets_by_channel[channel_index],
                &audio_params,
                self.config.audio_repetition_penalty,
                rng,
            )?;
            frame.push(sampled);
            previous_token = sampled;
        }

        Ok(frame)
    }

    /// Generate a frame using the local_decoder fallback path.
    ///
    /// Mirrors `ort_cpu_runtime.py:740-762`:
    /// 1. Run local_decoder with text_token_id=0, audio_prefix=[] → get text_logits
    /// 2. Sample assistant text token
    /// 3. If text token != assistant_slot → end of audio, break
    /// 4. For each channel 0..n_vq:
    ///    Run local_decoder with text_token_id=next_text_token, audio_prefix=frame → get audio_logits
    ///    Sample channel token
    fn generate_frame_local_decoder(
        &self,
        state: &mut DecodeState,
        rng: &mut StdRng,
    ) -> Result<AudioFrame, TtsError> {
        let Some(decoder) = &self.local.decoder else {
            return Ok(Vec::new());
        };

        // Step 1: Get text logits
        let (text_logits, _audio_logits) =
            Self::run_local_decoder(decoder, &state.global_hidden, 0, &[])?;

        // Step 2: Sample assistant text token
        let text_params = SamplingParams::for_text(&self.config);
        let next_text_token = sample_assistant_text_token(
            &text_logits,
            self.audio_assistant_slot_token_id,
            self.audio_end_token_id,
            &text_params,
            rng,
        )?;

        // If text token is not assistant slot, end of audio
        if next_text_token != self.audio_assistant_slot_token_id {
            return Ok(Vec::new());
        }

        let mut frame = Vec::with_capacity(self.n_vq);
        let audio_params = SamplingParams::for_audio(&self.config);

        // Step 3: Sample each channel
        for channel_index in 0..self.n_vq {
            let (_, audio_logits) =
                Self::run_local_decoder(decoder, &state.global_hidden, next_text_token, &frame)?;
            let channel_logits =
                Self::slice_audio_channel_logits(&audio_logits, channel_index, self.n_vq);
            let sampled = sample_audio_token(
                &channel_logits,
                &state.previous_tokens_by_channel[channel_index],
                &state.previous_token_sets_by_channel[channel_index],
                &audio_params,
                self.config.audio_repetition_penalty,
                rng,
            )?;
            frame.push(sampled);
        }

        Ok(frame)
    }

    /// Update the global decode state after generating a frame.
    ///
    /// Runs the decode_step ONNX model with:
    /// - input_ids: [1, 1, n_vq+1] with [assistant_slot, frame_tokens...]
    /// - past_valid_lengths: current past_valid_length
    /// - past_*: KV cache from previous step
    ///
    /// Updates global_hidden, past_by_name, and past_valid_length.
    ///
    /// Mirrors `ort_cpu_runtime.py:765-783`.
    fn update_decode_state(
        &self,
        state: &mut DecodeState,
        frame: &AudioFrame,
    ) -> Result<(), TtsError> {
        let row_width = self.n_vq + 1;
        let mut next_row = vec![self.audio_pad_token_id as f32; row_width];
        next_row[0] = self.audio_assistant_slot_token_id as f32;
        for (i, &token) in frame.iter().enumerate() {
            next_row[i + 1] = token as f32;
        }

        let input_ids = ndarray::ArrayD::<f32>::from_shape_vec(vec![1, 1, row_width], next_row)
            .map_err(|e| TtsError::OnnxError(format!("input_ids shape: {e}")))?;

        let past_valid_lengths =
            ndarray::ArrayD::<f32>::from_shape_vec(vec![1], vec![state.past_valid_length as f32])
                .map_err(|e| TtsError::OnnxError(format!("past_valid_lengths shape: {e}")))?;

        // TODO: Wire up actual ONNX decode_step run call.
        // The Python reference does:
        // ```
        // decode_feeds = {
        //     "input_ids": next_row,
        //     "past_valid_lengths": np.asarray([past_valid_length], dtype=np.int32),
        //     **{name: tensor for name, tensor in past_by_name.items()},
        // }
        // decode_outputs = self.sessions["decode"].run(None, decode_feeds)
        // global_hidden = _extract_last_hidden(named_decode_outputs["global_hidden"])
        // past_valid_length += 1
        // past_by_name = {rename_present_to_past(name): tensor for name, tensor in outputs}
        // ```
        //
        // For now, increment past_valid_length and keep state consistent.
        // The actual ONNX inference will be wired up when ort Value integration is settled.
        state.past_valid_length += 1;

        let _ = input_ids;
        let _ = past_valid_lengths;

        Ok(())
    }

    /// Create an empty local past KV cache for cached_step initialization.
    ///
    /// Mirrors `create_empty_local_cached_past()` from `ort_cpu_runtime.py:496-504`.
    ///
    /// Returns a HashMap with keys `local_past_key_{i}` and `local_past_value_{i}`
    /// for each layer, with shape [1, 0, local_heads, local_head_dim].
    fn create_empty_local_past(
        n_vq: usize,
    ) -> std::collections::HashMap<String, ndarray::ArrayD<f32>> {
        // Default dimensions from MOSS-TTS-Nano config
        let local_layers = 12;
        let local_heads = 8;
        let local_head_dim = 64;

        let mut past = std::collections::HashMap::new();
        for layer_index in 0..local_layers {
            let shape = vec![1, 0, local_heads, local_head_dim];
            past.insert(
                format!("local_past_key_{layer_index}"),
                ndarray::ArrayD::<f32>::zeros(shape.clone()),
            );
            past.insert(
                format!("local_past_value_{layer_index}"),
                ndarray::ArrayD::<f32>::zeros(shape),
            );
        }

        // Suppress unused variable warning for n_vq (used in ONNX integration)
        let _ = n_vq;

        past
    }

    /// Run a single cached_step and return text_logits only.
    ///
    /// Convenience wrapper around [`Self::run_cached_step_full`].
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn run_cached_step(
        _cached_step: &crate::modules::tts::model::global::OnnxSession,
        _global_hidden: &ndarray::ArrayD<f32>,
        _text_token_id: u32,
        _audio_token_id: u32,
        _channel_index: usize,
        _step_type: usize,
        _past_valid_lengths: usize,
        _local_past_by_name: &std::collections::HashMap<String, ndarray::ArrayD<f32>>,
    ) -> Result<Vec<f32>, TtsError> {
        // TODO: Wire up actual ONNX cached_step run call.
        // Returns placeholder text logits for now.
        // See `run_cached_step_full` for the full ONNX integration plan.
        let audio_codebook_size = 1024;
        Ok(vec![0.0f32; audio_codebook_size * 2]) // text_logits: assistant_slot + end_token area
    }

    /// Run a single cached_step and return (text_logits, audio_logits, updated_local_past).
    ///
    /// Mirrors `run_local_cached_step()` from `ort_cpu_runtime.py:506-535`.
    ///
    /// # Inputs
    ///
    /// | Name                | Shape                        | Description                      |
    /// |---------------------|------------------------------|----------------------------------|
    /// | global_hidden       | [1, seq_len, hidden_dim]     | Last hidden state from decoder   |
    /// | text_token_id       | [1]                          | Text token ID (0 if not needed)  |
    /// | audio_token_id      | [1]                          | Audio token ID (0 if not needed) |
    /// | channel_index       | [1]                          | VQ channel index                 |
    /// | step_type           | [1]                          | 0=text-only, 1=text+audio, 2=audio-only |
    /// | past_valid_lengths  | [1]                          | Number of valid past tokens      |
    /// | local_past_key_*    | [1, seq_len, heads, head_dim]| Local KV cache keys              |
    /// | local_past_value_*  | [1, seq_len, heads, head_dim]| Local KV cache values            |
    ///
    /// # Outputs
    ///
    /// | Name                    | Shape                        | Description              |
    /// |-------------------------|------------------------------|--------------------------|
    /// | text_logits             | [vocab_size]                 | Text token logits        |
    /// | audio_logits            | [1, n_vq, codebook_size]     | Audio token logits       |
    /// | local_present_key_*     | [1, seq_len+1, heads, head_dim]| Updated KV cache keys   |
    /// | local_present_value_*   | [1, seq_len+1, heads, head_dim]| Updated KV cache values |
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn run_cached_step_full(
        _cached_step: &crate::modules::tts::model::global::OnnxSession,
        _global_hidden: &ndarray::ArrayD<f32>,
        _text_token_id: u32,
        _audio_token_id: u32,
        _channel_index: usize,
        _step_type: usize,
        _past_valid_lengths: usize,
        _local_past_by_name: &std::collections::HashMap<String, ndarray::ArrayD<f32>>,
    ) -> Result<
        (
            Vec<f32>,
            Vec<f32>,
            std::collections::HashMap<String, ndarray::ArrayD<f32>>,
        ),
        TtsError,
    > {
        // TODO: Wire up actual ONNX cached_step run call.
        // Returns placeholder logits for now.
        let audio_codebook_size = 1024;
        let n_vq = 4; // Default for MOSS-TTS-Nano
        let text_logits = vec![0.0f32; audio_codebook_size * 2];
        let audio_logits = vec![0.0f32; n_vq * audio_codebook_size];
        Ok((text_logits, audio_logits, std::collections::HashMap::new()))
    }

    /// Run the local decoder and return (text_logits, audio_logits).
    ///
    /// Mirrors `run_local_decoder()` from `ort_cpu_runtime.py:478-494`.
    ///
    /// # Inputs
    ///
    /// | Name                  | Shape                    | Description                     |
    /// |-----------------------|--------------------------|---------------------------------|
    /// | global_hidden         | [1, seq_len, hidden_dim] | Last hidden state               |
    /// | text_token_id         | [1]                      | Text token ID                   |
    /// | audio_prefix_token_ids| [1, n_vq-1]              | Audio prefix tokens (padded)   |
    fn run_local_decoder(
        _decoder: &crate::modules::tts::model::global::OnnxSession,
        _global_hidden: &ndarray::ArrayD<f32>,
        _text_token_id: u32,
        _frame_prefix: &[u32],
    ) -> Result<(Vec<f32>, Vec<f32>), TtsError> {
        // TODO: Wire up actual ONNX decoder run call.
        // Returns placeholder logits for now.
        let audio_codebook_size = 1024;
        let n_vq = 4;
        let text_logits = vec![0.0f32; audio_codebook_size * 2];
        let audio_logits = vec![0.0f32; n_vq * audio_codebook_size];
        Ok((text_logits, audio_logits))
    }

    /// Slice the audio logits for a specific VQ channel.
    ///
    /// Mirrors `slice_audio_channel_logits()` from `ort_cpu_runtime.py:598-603`.
    ///
    /// The audio logits are shaped as [1, n_vq, codebook_size] flattened to
    /// [n_vq * codebook_size]. This extracts the logits for one channel.
    ///
    /// # Arguments
    ///
    /// * `audio_logits` - Flattened audio logit vector.
    /// * `channel_index` - Which VQ channel to extract (0..n_vq).
    /// * `n_vq` - Total number of VQ channels.
    ///
    /// # Returns
    ///
    /// Logits for the specified channel, length = total_len / n_vq.
    pub fn slice_audio_channel_logits(
        audio_logits: &[f32],
        channel_index: usize,
        n_vq: usize,
    ) -> Vec<f32> {
        if n_vq == 0 {
            return Vec::new();
        }
        let per_channel = audio_logits.len() / n_vq;
        let start = channel_index * per_channel;
        let end = start + per_channel;
        audio_logits[start.min(audio_logits.len())..end.min(audio_logits.len())].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_audio_channel_logits_basic() {
        // 2 channels, 3 logits each: [0,1,2, 10,11,12]
        let logits = vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0];
        let ch0 = DecodeRunner::slice_audio_channel_logits(&logits, 0, 2);
        let ch1 = DecodeRunner::slice_audio_channel_logits(&logits, 1, 2);
        assert_eq!(ch0, vec![0.0, 1.0, 2.0]);
        assert_eq!(ch1, vec![10.0, 11.0, 12.0]);
    }

    #[test]
    fn slice_audio_channel_logits_four_channels() {
        // 4 channels, 2 logits each
        let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let ch0 = DecodeRunner::slice_audio_channel_logits(&logits, 0, 4);
        let ch2 = DecodeRunner::slice_audio_channel_logits(&logits, 2, 4);
        assert_eq!(ch0, vec![1.0, 2.0]);
        assert_eq!(ch2, vec![5.0, 6.0]);
    }

    #[test]
    fn slice_audio_channel_logits_zero_channels() {
        let logits = vec![1.0, 2.0, 3.0];
        let result = DecodeRunner::slice_audio_channel_logits(&logits, 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn decode_constants() {
        assert_eq!(DECODE_INPUT_IDS, "input_ids");
        assert_eq!(DECODE_PAST_VALID_LENGTHS, "past_valid_lengths");
        assert_eq!(DECODE_GLOBAL_HIDDEN, "global_hidden");
        assert_eq!(DECODE_PRESENT_PREFIX, "present_");
        assert_eq!(DECODE_PAST_PREFIX, "past_");
    }

    #[test]
    fn create_empty_local_past_has_expected_keys() {
        let past = DecodeRunner::create_empty_local_past(4);
        // 12 layers × 2 (key + value) = 24 keys
        assert_eq!(past.len(), 24);
        assert!(past.contains_key("local_past_key_0"));
        assert!(past.contains_key("local_past_value_11"));
    }

    #[test]
    fn create_empty_local_past_correct_shape() {
        let past = DecodeRunner::create_empty_local_past(4);
        let key = past.get("local_past_key_0").unwrap();
        assert_eq!(key.shape(), &[1, 0, 8, 64]);
    }
}
