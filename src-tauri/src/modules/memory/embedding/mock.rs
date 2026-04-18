//! Deterministic, zero-dependency embedding provider for tests (H5).
//!
//! [`MockEmbedder`] produces stable 384-dimensional vectors derived purely
//! from the SHA-style hash of the input text. Identical inputs always yield
//! identical vectors and the result is L2-normalised, so it behaves enough
//! like a real embedding for HRR / vector-store unit tests without requiring
//! a 200MB FastEmbed model download.
//!
//! Properties intentionally provided:
//! - **Deterministic** — same text → same vector across runs and platforms.
//! - **Distinct** — different texts almost always yield distinct vectors
//!   (collision probability ≈ 2⁻⁶⁴ via FNV-style hashing of byte chunks).
//! - **Unit-norm** — output is L2-normalised so cosine similarity ≈ dot
//!   product, matching the expectations downstream of FastEmbed.
//!
//! Properties intentionally **not** provided:
//! - Semantic similarity. "cat" and "kitten" hash independently.
//!
//! For semantic tests, use the real `FastEmbedProvider` and gate the test
//! behind `#[ignore]`.

use crate::modules::memory::embedding::fastembed::EmbeddingError;

/// The fixed embedding dimension matched to `FastEmbedProvider::DIMENSION`.
const MOCK_DIMENSION: usize = 384;

/// Deterministic mock embedding provider.
///
/// Mirrors the surface of `FastEmbedProvider` (`embed`, `embed_one`,
/// `dimension`) so tests can swap providers via cfg-gated wiring.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Test-only helper; consumed by hrr/integration.rs tests.
pub struct MockEmbedder;

impl MockEmbedder {
    /// The mock embedding dimension (384, matching FastEmbed multilingual-e5-small).
    #[allow(dead_code)]
    pub const DIMENSION: usize = MOCK_DIMENSION;

    /// Construct a new mock embedder. There is no state, so this is free.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }

    /// Embed a batch of texts, returning a deterministic vector per input.
    #[allow(dead_code)]
    pub fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.into_iter().map(deterministic_vector).collect())
    }

    /// Embed a single text. Returns an error for empty input to mirror
    /// `FastEmbedProvider::embed_one` semantics.
    #[allow(dead_code)]
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyText);
        }
        Ok(deterministic_vector(text))
    }

    /// Returns the embedding dimension (always 384).
    #[allow(dead_code)]
    pub fn dimension(&self) -> usize {
        MOCK_DIMENSION
    }
}

/// Hash `text` into a unit-norm 384-dimensional vector.
///
/// Algorithm:
/// 1. Walk the bytes of `text`, fold them into a 64-bit FNV-1a digest.
/// 2. Use the digest to seed a deterministic LCG, producing each component.
/// 3. L2-normalise the result so cosine ≈ dot.
fn deterministic_vector(text: &str) -> Vec<f32> {
    // FNV-1a 64-bit constants.
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;

    let mut digest = FNV_OFFSET;
    for byte in text.bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(FNV_PRIME);
    }

    // Mix digest with a linear-congruential generator (Numerical Recipes).
    let mut state = digest | 1; // avoid all-zero state
    let mut out = Vec::with_capacity(MOCK_DIMENSION);
    let mut sum_sq = 0.0_f32;
    for _ in 0..MOCK_DIMENSION {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // Map to [-1, 1] using the high 24 bits.
        let bits = (state >> 40) as u32 & 0x00FF_FFFF;
        let v = (bits as f32 / 8_388_608.0_f32) - 1.0_f32;
        sum_sq += v * v;
        out.push(v);
    }

    let norm = sum_sq.sqrt().max(f32::EPSILON);
    for v in &mut out {
        *v /= norm;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_matches_fastembed() {
        assert_eq!(MockEmbedder::new().dimension(), 384);
    }

    #[test]
    fn embed_one_is_deterministic() {
        let m = MockEmbedder::new();
        let a = m.embed_one("hello world").unwrap();
        let b = m.embed_one("hello world").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 384);
    }

    #[test]
    fn distinct_inputs_produce_distinct_vectors() {
        let m = MockEmbedder::new();
        let a = m.embed_one("alpha").unwrap();
        let b = m.embed_one("beta").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn embed_one_rejects_empty() {
        let m = MockEmbedder::new();
        assert!(m.embed_one("").is_err());
    }

    #[test]
    fn output_is_unit_norm() {
        let m = MockEmbedder::new();
        let v = m.embed_one("normalise me").unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "norm = {norm}");
    }

    #[test]
    fn batch_embed_matches_singletons() {
        let m = MockEmbedder::new();
        let batch = m.embed(vec!["x", "y", "z"]).unwrap();
        assert_eq!(batch.len(), 3);
        assert_eq!(batch[0], m.embed_one("x").unwrap());
        assert_eq!(batch[2], m.embed_one("z").unwrap());
    }
}
