//! FastEmbed provider for generating text embeddings
//!
//! Wraps the `fastembed` crate to provide offline, CPU-friendly
//! text embedding with a 384-dimensional model.
//!
//! # `#![allow(dead_code)]` justification
//! These types are infrastructure for future consumers (VectorMemoryProvider,
//! hybrid search). They will be used when the full vector memory pipeline
//! is wired into the agent loop.

#![allow(dead_code)]

use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use thiserror::Error;

use crate::modules::skills::sedimentation::Embedder;
use crate::modules::system_check::model_download::embedded_model_cache_dir;

/// Error type for embedding operations
#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("embedding model failed: {0}")]
    ModelError(String),

    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("empty text provided")]
    EmptyText,
}

/// FastEmbed provider for generating text embeddings
///
/// Uses a 384-dimensional model by default.
/// The model is downloaded on first use and cached locally.
pub struct FastEmbedProvider {
    model: Mutex<TextEmbedding>,
    dimension: usize,
}

impl FastEmbedProvider {
    /// The embedding dimension for the default model
    pub const DIMENSION: usize = 384;

    /// Create a new FastEmbedProvider with the multilingual model
    ///
    /// Uses `multilingual-e5-small` (384d) for multilingual support.
    /// Downloads the model on first use and caches it locally.
    pub fn new() -> Result<Self, EmbeddingError> {
        tracing::info!(
            model = "MultilingualE5Small",
            "initializing FastEmbed embedding model"
        );
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::MultilingualE5Small)
                .with_cache_dir(embedded_model_cache_dir())
                .with_show_download_progress(false),
        )
        .map_err(|e| EmbeddingError::ModelError(e.to_string()))?;
        tracing::info!(
            model = "MultilingualE5Small",
            dimension = Self::DIMENSION,
            "FastEmbed embedding model initialized"
        );

        Ok(Self {
            model: Mutex::new(model),
            dimension: Self::DIMENSION,
        })
    }

    /// Create a provider with a custom model
    pub fn with_model(model: TextEmbedding) -> Self {
        Self {
            model: Mutex::new(model),
            dimension: Self::DIMENSION,
        }
    }

    /// Generate embeddings for a batch of texts (passages)
    pub fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        self.model
            .lock()
            .unwrap()
            .embed(texts, None)
            .map_err(|e| EmbeddingError::ModelError(e.to_string()))
    }

    /// Generate an embedding for a single text
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyText);
        }

        let mut embeddings = self.embed(vec![text])?;
        let embedding = embeddings
            .pop()
            .ok_or_else(|| EmbeddingError::ModelError("no embedding returned".to_string()))?;

        if embedding.len() != self.dimension {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.dimension,
                actual: embedding.len(),
            });
        }

        Ok(embedding)
    }

    /// Returns the embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

impl Embedder for FastEmbedProvider {
    /// Bridge to `embed_one`; returns `Vec::new()` on any error so the
    /// dedup gate degrades gracefully instead of panicking.
    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_one(text).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "FastEmbedProvider::embed failed; returning empty vec");
            Vec::new()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires FastEmbed model download (run with --ignored to execute)"]
    fn provider_dimension() {
        let provider = FastEmbedProvider::new().expect("model should load");
        assert_eq!(provider.dimension(), 384);
    }

    #[test]
    #[ignore = "requires FastEmbed model download (run with --ignored to execute)"]
    fn embed_returns_correct_dimension() {
        let provider = FastEmbedProvider::new().expect("model should load");
        let embedding = provider
            .embed_one("Hello, world!")
            .expect("embed should succeed");
        assert_eq!(embedding.len(), 384);
    }

    #[test]
    #[ignore = "requires FastEmbed model download (run with --ignored to execute)"]
    fn embed_batch_returns_correct_count() {
        let provider = FastEmbedProvider::new().expect("model should load");
        let texts = vec!["Hello", "World", "Test"];
        let embeddings = provider.embed(texts).expect("batch embed should succeed");
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 384);
        }
    }

    #[test]
    #[ignore = "requires FastEmbed model download (run with --ignored to execute)"]
    fn embed_batch_empty_input() {
        let provider = FastEmbedProvider::new().expect("model should load");
        let embeddings = provider.embed(vec![]).expect("empty batch should succeed");
        assert!(embeddings.is_empty());
    }

    #[test]
    #[ignore = "requires FastEmbed model download (run with --ignored to execute)"]
    fn embed_empty_text_error() {
        let provider = FastEmbedProvider::new().expect("model should load");
        let result = provider.embed_one("");
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod embedder_trait_tests {
    use super::*;
    use crate::modules::skills::sedimentation::Embedder;

    /// ConstEmbedder stub verifies the trait is callable.
    #[test]
    fn embed_via_trait_returns_nonempty_or_empty_never_panics() {
        struct ConstEmbedder(Vec<f32>);
        impl Embedder for ConstEmbedder {
            fn embed(&self, _text: &str) -> Vec<f32> {
                self.0.clone()
            }
        }
        let e = ConstEmbedder(vec![1.0, 0.0]);
        assert_eq!(e.embed("hello"), vec![1.0, 0.0]);
    }

    /// FastEmbedProvider must satisfy the Embedder bound at compile time.
    #[test]
    fn fast_embed_provider_satisfies_embedder_bound() {
        fn _accepts_embedder(_e: &dyn Embedder) {}
        // Construct only if model is available; skip otherwise.
        if let Ok(p) = FastEmbedProvider::new() {
            _accepts_embedder(&p);
        }
    }
}
