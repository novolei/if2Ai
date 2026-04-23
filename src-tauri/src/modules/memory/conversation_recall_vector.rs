//! P1-7 — Optional FastEmbed-backed vectors for `conversation_recall`
//! hybrid ranking on top of the SQLite FTS5 baseline.
//!
//! Gated by `IF2AI_CONVERSATION_RECALL_VECTOR=1` (default off) so cold
//! boots and unit tests don't pull the embedding model unless explicitly
//! enabled. When the embedder fails to initialise we warn once and
//! degrade to FTS-only for the rest of the process.

use std::sync::{Arc, OnceLock};

use crate::modules::memory::embedding::FastEmbedProvider;

static EMBEDDER: OnceLock<Option<Arc<FastEmbedProvider>>> = OnceLock::new();

/// Returns true when `IF2AI_CONVERSATION_RECALL_VECTOR` is `1`/`true`.
#[must_use]
pub fn vector_hybrid_enabled() -> bool {
    std::env::var("IF2AI_CONVERSATION_RECALL_VECTOR")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn embedder_cached() -> Option<Arc<FastEmbedProvider>> {
    EMBEDDER
        .get_or_init(|| match FastEmbedProvider::new() {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "[conversation_recall] FastEmbed init failed; hybrid vector path disabled"
                );
                None
            }
        })
        .clone()
}

/// `Some(embedder)` when both the env switch is on AND init succeeded.
#[must_use]
pub fn embedder_for_recall() -> Option<Arc<FastEmbedProvider>> {
    if !vector_hybrid_enabled() {
        return None;
    }
    embedder_cached()
}

/// Encode a `Vec<f32>` to little-endian bytes for SQLite BLOB storage.
#[must_use]
pub fn f32_slice_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Decode a little-endian f32 BLOB back to `Vec<f32>`. `None` on bad length.
#[must_use]
pub fn blob_to_f32_slice(blob: &[u8]) -> Option<Vec<f32>> {
    if blob.len() % 4 != 0 {
        return None;
    }
    let mut v = Vec::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        v.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(v)
}

/// Cosine similarity in `[-1, 1]`; returns 0 on degenerate vectors.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom <= f32::EPSILON {
        return 0.0;
    }
    (dot / denom).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trip() {
        let v = vec![0.0_f32, 1.5, -2.25, 3.125];
        let b = f32_slice_to_blob(&v);
        assert_eq!(blob_to_f32_slice(&b).unwrap(), v);
    }

    #[test]
    fn cosine_basic() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        let c = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }
}
