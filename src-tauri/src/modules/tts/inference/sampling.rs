//! Sampling logic for text and audio token generation.
//!
//! Mirrors the sampling helpers from `ort_cpu_runtime.py:76-196`:
//! - `_apply_repetition_penalty` / `_argmax_with_repetition_penalty`
//! - `_softmax`
//! - `_sample_from_scores` (greedy / top-k / top-p / temperature)
//! - `_sample_assistant_text_token`
//! - `_sample_audio_token`
//!
//! ## Sampling Flow
//!
//! 1. **Greedy** (`do_sample=false`): argmax over logits, with optional repetition penalty
//! 2. **Sampling** (`do_sample=true`): apply temperature → top-k mask → top-p mask → softmax → categorical
//! 3. **Text token**: samples from 2 candidates (assistant slot / end token)
//! 4. **Audio token**: applies repetition penalty first, then samples from full vocabulary

#![allow(dead_code)]

use rand::rngs::StdRng;
use rand::Rng;

use crate::modules::tts::config::GenerationParams;
use crate::modules::tts::error::TtsError;

/// Applies repetition penalty to logits for previously generated tokens.
///
/// Mirrors `_apply_repetition_penalty()` from `ort_cpu_runtime.py:76-84`.
///
/// For each unique previous token ID:
/// - If logit < 0: multiply by `repetition_penalty` (pushes it further negative)
/// - If logit >= 0: divide by `repetition_penalty` (reduces its value)
///
/// # Arguments
///
/// * `logits` - Logit scores for each token in the vocabulary.
/// * `previous_token_ids` - Token IDs that have been generated so far.
/// * `repetition_penalty` - Penalty factor (> 1.0 penalizes, 1.0 is no-op).
pub fn apply_repetition_penalty(
    logits: &[f32],
    previous_token_ids: &[u32],
    repetition_penalty: f32,
) -> Vec<f32> {
    if previous_token_ids.is_empty() || (repetition_penalty - 1.0).abs() < f32::EPSILON {
        return logits.to_vec();
    }

    let mut result = logits.to_vec();
    let unique_tokens: Vec<u32> = {
        let mut seen = std::collections::HashSet::new();
        previous_token_ids
            .iter()
            .filter(|&&id| seen.insert(id))
            .copied()
            .collect()
    };

    for &token_id in &unique_tokens {
        let idx = token_id as usize;
        if idx < result.len() {
            let val = result[idx];
            result[idx] = if val < 0.0 {
                val * repetition_penalty
            } else {
                val / repetition_penalty
            };
        }
    }

    result
}

/// Greedy selection (argmax) over logits.
///
/// Mirrors `_argmax()` from `ort_cpu_runtime.py:100`.
///
/// # Arguments
///
/// * `logits` - Logit scores. Must be non-empty.
///
/// # Returns
///
/// Index of the maximum logit value.
pub fn argmax(logits: &[f32]) -> usize {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Greedy selection with repetition penalty applied to seen tokens.
///
/// Mirrors `_argmax_with_repetition_penalty()` from `ort_cpu_runtime.py:87-98`.
///
/// # Arguments
///
/// * `logits` - Logit scores.
/// * `previous_token_set` - Set of previously generated token IDs.
/// * `repetition_penalty` - Penalty factor.
///
/// # Returns
///
/// Index of the maximum (penalized) logit value.
pub fn argmax_with_repetition_penalty(
    logits: &[f32],
    previous_token_set: &std::collections::HashSet<u32>,
    repetition_penalty: f32,
) -> usize {
    let apply_penalty =
        !previous_token_set.is_empty() && (repetition_penalty - 1.0).abs() >= f32::EPSILON;

    let mut best_index = 0;
    let mut best_value = f32::NEG_INFINITY;

    for (index, &value) in logits.iter().enumerate() {
        let score = if apply_penalty && previous_token_set.contains(&(index as u32)) {
            if value < 0.0 {
                value * repetition_penalty
            } else {
                value / repetition_penalty
            }
        } else {
            value
        };

        if score > best_value {
            best_value = score;
            best_index = index;
        }
    }

    best_index
}

/// Numerically stable softmax using exp(x - max(x)).
///
/// Mirrors `_softmax()` from `ort_cpu_runtime.py:101-105`.
///
/// # Arguments
///
/// * `values` - Input values (any real numbers).
///
/// # Returns
///
/// Probability distribution summing to 1.0.
pub fn softmax(values: &[f32]) -> Vec<f32> {
    let max_val = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let shifted: Vec<f64> = values
        .iter()
        .map(|&v| f64::from(v) - f64::from(max_val))
        .collect();
    let exps: Vec<f64> = shifted.iter().map(|&v| v.exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.iter().map(|&e| (e / sum) as f32).collect()
}

/// Samples a token index from logit scores using temperature, top-k, and top-p filtering.
///
/// Mirrors `_sample_from_scores()` from `ort_cpu_runtime.py:108-150`.
///
/// ## Algorithm
///
/// 1. If `do_sample` is false, return argmax (greedy).
/// 2. Divide logits by temperature.
/// 3. Apply top-k filtering: keep only the k highest-scoring tokens.
/// 4. Apply top-p (nucleus) filtering: keep the smallest set of tokens whose cumulative probability exceeds p.
/// 5. Softmax the remaining scores and sample categorically.
///
/// # Arguments
///
/// * `logits` - Raw logit scores.
/// * `params` - Sampling parameters (temperature, top_k, top_p, do_sample).
/// * `rng` - Random number generator for sampling.
///
/// # Returns
///
/// Sampled token index.
pub fn sample_from_scores(
    logits: &[f32],
    params: &SamplingParams,
    rng: &mut StdRng,
) -> Result<usize, TtsError> {
    if !params.do_sample {
        return Ok(argmax(logits));
    }

    if params.temperature <= 0.0 {
        return Err(TtsError::InvalidParam(
            "temperature must be positive when do_sample=true".into(),
        ));
    }

    let n = logits.len();
    let mut scores: Vec<f32> = logits.iter().map(|&v| v / params.temperature).collect();

    // Top-k filtering
    if params.top_k > 0 && (params.top_k as usize) < n {
        let mut sorted = scores.clone();
        sorted.sort_by(|a, b| b.total_cmp(a));
        let threshold = sorted[params.top_k as usize - 1];
        for s in &mut scores {
            if *s < threshold {
                *s = f32::NEG_INFINITY;
            }
        }
    }

    // Top-p (nucleus) filtering
    if params.top_p > 0.0 && params.top_p < 1.0 {
        let mut indexed: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
        indexed.sort_by(|(_, a), (_, b)| b.total_cmp(a));

        let sorted_scores: Vec<f32> = indexed.iter().map(|(_, v)| *v).collect();
        let sorted_probs = softmax(&sorted_scores);

        let mut cumulative = 0.0f64;
        let mut remove_mask = vec![false; indexed.len()];
        for (i, &prob) in sorted_probs.iter().enumerate() {
            cumulative += f64::from(prob);
            if cumulative > f64::from(params.top_p) {
                remove_mask[i] = true;
            }
        }

        // Propagate removal to all lower-ranked tokens
        for i in (1..remove_mask.len()).rev() {
            remove_mask[i] = remove_mask[i] || remove_mask[i - 1];
        }
        // Always keep the top token
        if !remove_mask.is_empty() {
            remove_mask[0] = false;
        }

        for (i, &should_remove) in remove_mask.iter().enumerate() {
            if should_remove {
                scores[indexed[i].0] = f32::NEG_INFINITY;
            }
        }
    }

    // Categorical sampling
    let probabilities = softmax(&scores);
    let mut random_value: f64 = rng.gen();

    for (index, &prob) in probabilities.iter().enumerate() {
        random_value -= f64::from(prob);
        if random_value <= 0.0 {
            return Ok(index);
        }
    }

    // Fallback: if numerical issues prevented selection, return argmax
    Ok(argmax(&scores))
}

/// Sampling parameters extracted from [`GenerationParams`].
///
/// Separates text vs audio sampling config so the same sampling functions
/// can be reused for both token types.
#[derive(Debug, Clone)]
pub struct SamplingParams {
    /// Whether to sample (true) or use greedy argmax (false).
    pub do_sample: bool,
    /// Temperature for scaling logits before sampling.
    pub temperature: f32,
    /// Top-k filtering: keep only the k highest-scoring tokens (0 = disabled).
    pub top_k: u32,
    /// Top-p (nucleus) filtering threshold (1.0 = disabled).
    pub top_p: f32,
}

impl SamplingParams {
    /// Create sampling parameters for text token sampling.
    #[must_use]
    pub fn for_text(gen: &GenerationParams) -> Self {
        Self {
            do_sample: gen.do_sample,
            temperature: gen.text_temperature,
            top_k: gen.text_top_k,
            top_p: gen.text_top_p,
        }
    }

    /// Create sampling parameters for audio token sampling.
    #[must_use]
    pub fn for_audio(gen: &GenerationParams) -> Self {
        Self {
            do_sample: gen.do_sample,
            temperature: gen.audio_temperature,
            top_k: gen.audio_top_k,
            top_p: gen.audio_top_p,
        }
    }
}

/// Samples an assistant text token from text logits.
///
/// Mirrors `_sample_assistant_text_token()` from `ort_cpu_runtime.py:153-175`.
///
/// The text token sampling only considers two candidates:
/// - `audio_assistant_slot_token_id`
/// - `audio_end_token_id`
///
/// # Arguments
///
/// * `text_logits` - Full text logit vector from the decoder.
/// * `audio_assistant_slot_token_id` - The assistant slot token ID.
/// * `audio_end_token_id` - The audio end token ID.
/// * `params` - Text sampling parameters.
/// * `rng` - Random number generator.
///
/// # Returns
///
/// Sampled token ID (either assistant slot or end token).
pub fn sample_assistant_text_token(
    text_logits: &[f32],
    audio_assistant_slot_token_id: u32,
    audio_end_token_id: u32,
    params: &SamplingParams,
    rng: &mut StdRng,
) -> Result<u32, TtsError> {
    let candidate_ids = [audio_assistant_slot_token_id, audio_end_token_id];
    let candidate_scores: Vec<f32> = candidate_ids
        .iter()
        .map(|&id| {
            let idx = id as usize;
            if idx < text_logits.len() {
                text_logits[idx]
            } else {
                f32::NEG_INFINITY
            }
        })
        .collect();

    // Clamp top_k to candidate count (always 2)
    let clamped_params = SamplingParams {
        do_sample: params.do_sample,
        temperature: params.temperature,
        top_k: params.top_k.min(candidate_ids.len() as u32),
        top_p: params.top_p,
    };

    let sampled_index = sample_from_scores(&candidate_scores, &clamped_params, rng)?;
    Ok(candidate_ids[sampled_index])
}

/// Samples an audio token from audio logits.
///
/// Mirrors `_sample_audio_token()` from `ort_cpu_runtime.py:178-196`.
///
/// In greedy mode: uses argmax with repetition penalty.
/// In sampling mode: applies repetition penalty first, then samples.
///
/// # Arguments
///
/// * `audio_logits` - Full audio logit vector from the decoder.
/// * `previous_token_ids` - List of previously generated audio token IDs.
/// * `previous_token_set` - Set version for O(1) lookup.
/// * `params` - Audio sampling parameters.
/// * `repetition_penalty` - Repetition penalty factor.
/// * `rng` - Random number generator.
///
/// # Returns
///
/// Sampled audio token ID.
pub fn sample_audio_token(
    audio_logits: &[f32],
    previous_token_ids: &[u32],
    previous_token_set: &std::collections::HashSet<u32>,
    params: &SamplingParams,
    repetition_penalty: f32,
    rng: &mut StdRng,
) -> Result<u32, TtsError> {
    if !params.do_sample {
        let idx =
            argmax_with_repetition_penalty(audio_logits, previous_token_set, repetition_penalty);
        return Ok(idx as u32);
    }

    let penalized = apply_repetition_penalty(audio_logits, previous_token_ids, repetition_penalty);
    let idx = sample_from_scores(&penalized, params, rng)?;
    Ok(idx as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    // ── argmax tests ──

    #[test]
    fn argmax_returns_index_of_maximum() {
        let logits = vec![0.1, 0.5, 0.3, 0.9, 0.2];
        assert_eq!(argmax(&logits), 3);
    }

    #[test]
    fn argmax_first_element() {
        let logits = vec![5.0, 1.0, 2.0];
        assert_eq!(argmax(&logits), 0);
    }

    #[test]
    fn argmax_single_element() {
        let logits = vec![42.0];
        assert_eq!(argmax(&logits), 0);
    }

    // ── softmax tests ──

    #[test]
    fn softmax_sums_to_one() {
        let values = vec![1.0, 2.0, 3.0];
        let probs = softmax(&values);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn softmax_uniform_input_gives_uniform_output() {
        let values = vec![0.0, 0.0, 0.0];
        let probs = softmax(&values);
        for p in &probs {
            assert!((p - 1.0 / 3.0).abs() < 1e-5);
        }
    }

    #[test]
    fn softmax_large_values_favors_maximum() {
        let values = vec![0.0, 0.0, 100.0];
        let probs = softmax(&values);
        assert!(probs[2] > 0.999);
    }

    // ── repetition penalty tests ──

    #[test]
    fn apply_repetition_penalty_positive_values() {
        let logits = vec![1.0, 2.0, 3.0, 2.0];
        let penalized = apply_repetition_penalty(&logits, &[2], 2.0);
        // Token 2 (value 3.0) should be divided by 2.0
        assert!((penalized[2] - 1.5).abs() < 1e-6);
        // Other values unchanged
        assert!((penalized[0] - 1.0).abs() < 1e-6);
        assert!((penalized[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn apply_repetition_penalty_negative_values() {
        let logits = vec![-1.0, -2.0, -3.0];
        let penalized = apply_repetition_penalty(&logits, &[1], 2.0);
        // Token 1 (value -2.0) should be multiplied by 2.0
        assert!((penalized[1] - (-4.0)).abs() < 1e-6);
        assert!((penalized[0] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn apply_repetition_penalty_noop_at_1_0() {
        let logits = vec![1.0, 2.0, 3.0];
        let penalized = apply_repetition_penalty(&logits, &[1], 1.0);
        assert_eq!(penalized, logits);
    }

    #[test]
    fn apply_repetition_penalty_empty_previous_tokens() {
        let logits = vec![1.0, 2.0, 3.0];
        let penalized = apply_repetition_penalty(&logits, &[], 2.0);
        assert_eq!(penalized, logits);
    }

    #[test]
    fn apply_repetition_penalty_out_of_bounds_ignored() {
        let logits = vec![1.0, 2.0, 3.0];
        let penalized = apply_repetition_penalty(&logits, &[999], 2.0);
        assert_eq!(penalized, logits);
    }

    #[test]
    fn apply_repetition_penalty_duplicates_deduplicated() {
        let logits = vec![1.0, 2.0, 3.0];
        let penalized = apply_repetition_penalty(&logits, &[1, 1, 1], 2.0);
        // Token 1 penalized once, not three times
        assert!((penalized[1] - 1.0).abs() < 1e-6);
    }

    // ── argmax_with_repetition_penalty tests ──

    #[test]
    fn argmax_with_repetition_penalty_penalizes_seen() {
        let logits = vec![1.0, 5.0, 3.0];
        let mut seen = std::collections::HashSet::new();
        seen.insert(1);
        // Token 1 is highest (5.0) but penalized to 2.5, so token 2 (3.0) wins
        assert_eq!(argmax_with_repetition_penalty(&logits, &seen, 2.0), 2);
    }

    #[test]
    fn argmax_with_repetition_penalty_no_penalty_still_argmax() {
        let logits = vec![1.0, 5.0, 3.0];
        let seen = std::collections::HashSet::new();
        assert_eq!(argmax_with_repetition_penalty(&logits, &seen, 1.0), 1);
    }

    // ── sample_from_scores tests ──

    #[test]
    fn sample_greedy_matches_argmax() {
        let logits = vec![0.1, 0.5, 0.3, 0.9, 0.2];
        let params = SamplingParams {
            do_sample: false,
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_from_scores(&logits, &params, &mut rng).unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn sample_top_k_filters_candidates() {
        let logits = vec![10.0, 5.0, 3.0, 1.0, 0.5];
        let params = SamplingParams {
            do_sample: true,
            temperature: 1.0,
            top_k: 2,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        // With top_k=2, only indices 0 and 1 survive; 0 dominates
        let result = sample_from_scores(&logits, &params, &mut rng).unwrap();
        assert!(result < 2);
    }

    #[test]
    fn sample_top_p_filters_tail() {
        let logits = vec![10.0, 0.1, 0.1, 0.1, 0.1];
        let params = SamplingParams {
            do_sample: true,
            temperature: 1.0,
            top_k: 0,
            top_p: 0.5,
        };
        let mut rng = StdRng::seed_from_u64(42);
        // With top_p=0.5, only the dominant token should survive
        let result = sample_from_scores(&logits, &params, &mut rng).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn sample_temperature_spreads_probabilities() {
        let logits = vec![2.0, 1.0];
        let params = SamplingParams {
            do_sample: true,
            temperature: 10.0, // High temperature → near-uniform
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let _ = sample_from_scores(&logits, &params, &mut rng).unwrap();
        // High temperature means either token can be sampled
    }

    #[test]
    fn sample_zero_temperature_returns_error() {
        let logits = vec![1.0, 2.0];
        let params = SamplingParams {
            do_sample: true,
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(sample_from_scores(&logits, &params, &mut rng).is_err());
    }

    #[test]
    fn sample_negative_temperature_returns_error() {
        let logits = vec![1.0, 2.0];
        let params = SamplingParams {
            do_sample: true,
            temperature: -1.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        assert!(sample_from_scores(&logits, &params, &mut rng).is_err());
    }

    // ── sample_assistant_text_token tests ──

    #[test]
    fn assistant_text_token_greedy() {
        let mut logits = vec![0.0; 100];
        logits[10] = 1.0; // assistant slot
        logits[20] = 5.0; // end token
        let params = SamplingParams {
            do_sample: false,
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_assistant_text_token(&logits, 10, 20, &params, &mut rng).unwrap();
        assert_eq!(result, 20);
    }

    #[test]
    fn assistant_text_token_sampling() {
        let mut logits = vec![0.0; 100];
        logits[5] = 10.0; // some other token (should be ignored)
        logits[15] = 3.0; // assistant slot
        logits[25] = 1.0; // end token
        let params = SamplingParams {
            do_sample: true,
            temperature: 0.5,
            top_k: 2,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        // Only candidates are assistant_slot(3.0) and end_token(1.0)
        let result = sample_assistant_text_token(&logits, 15, 25, &params, &mut rng).unwrap();
        assert!(result == 15 || result == 25);
    }

    // ── sample_audio_token tests ──

    #[test]
    fn audio_token_greedy_with_penalty() {
        let mut logits = vec![0.0; 10];
        logits[3] = 5.0; // highest
        logits[7] = 4.0;
        let mut seen = std::collections::HashSet::new();
        seen.insert(3); // token 3 was seen
        let params = SamplingParams {
            do_sample: false,
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_audio_token(&logits, &[3], &seen, &params, 2.0, &mut rng).unwrap();
        // Token 3 penalized from 5.0 to 2.5, so token 7 (4.0) wins
        assert_eq!(result, 7);
    }

    #[test]
    fn audio_token_sampling_with_penalty() {
        let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut seen = std::collections::HashSet::new();
        seen.insert(4); // token 4 (highest) was seen
        let params = SamplingParams {
            do_sample: true,
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_audio_token(&logits, &[4], &seen, &params, 2.0, &mut rng).unwrap();
        // Token 4 penalized but still likely to be sampled with temperature=1.0
        // Just verify it returns a valid index
        assert!(result < 5);
    }

    // ── SamplingParams factory tests ──

    #[test]
    fn for_text_uses_text_params() {
        let gen = GenerationParams {
            text_temperature: 0.5,
            text_top_p: 0.8,
            text_top_k: 10,
            ..GenerationParams::default()
        };
        let params = SamplingParams::for_text(&gen);
        assert!((params.temperature - 0.5).abs() < 1e-6);
        assert!((params.top_p - 0.8).abs() < 1e-6);
        assert_eq!(params.top_k, 10);
    }

    #[test]
    fn for_audio_uses_audio_params() {
        let gen = GenerationParams {
            audio_temperature: 0.6,
            audio_top_p: 0.9,
            audio_top_k: 20,
            ..GenerationParams::default()
        };
        let params = SamplingParams::for_audio(&gen);
        assert!((params.temperature - 0.6).abs() < 1e-6);
        assert!((params.top_p - 0.9).abs() < 1e-6);
        assert_eq!(params.top_k, 20);
    }
}
