//! FEAT-SE-004 — Skill vector index + retrieval.
//!
//! Indexing: `description` + first [`SKILL_BODY_INDEX_CHARS`] chars
//! of `body` are embedded and stored under `kind = "skill"` in a
//! [`VectorStore`]. Retrieval: query string is embedded and the
//! top-k most-similar skill records are returned with their score.
//!
//! Why a `VectorStore` trait rather than calling `conversation_recall_vector`
//! directly?
//! - Tests stay deterministic + zero-deps (MockVectorStore in this file).
//! - Future Pack can supply a sqlite-backed `SqliteSkillVectorStore`
//!   that delegates to `conversation_recall_vector`'s tables, with
//!   `kind = "skill"` filtering — no schema migration needed.

#![allow(dead_code)]

use async_trait::async_trait;
use serde_json::Value;

use crate::modules::skills::sedimentation::Embedder;

/// Cap on how many chars of the skill body get embedded — keeps
/// embedding cost bounded regardless of how long the markdown gets.
pub const SKILL_BODY_INDEX_CHARS: usize = 500;

/// Stable `kind` discriminator written to every vector row produced
/// by this module. Distinguishes skill records from other vector
/// data living in the same backing store.
pub const SKILL_VECTOR_KIND: &str = "skill";

/// Errors surfaced by [`index_skill`]. Today only one variant — the
/// store rejected the upsert. Wrapped so future Packs can extend
/// without breaking callers.
#[derive(Debug)]
pub enum IndexError {
    Store(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::Store(msg) => write!(f, "vector store upsert failed: {msg}"),
        }
    }
}

impl std::error::Error for IndexError {}

/// One retrieval result. `payload` is the original metadata blob
/// passed to the store (typically `{ "name": ..., "description": ... }`).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillSearchHit {
    pub skill_id: String,
    pub score: f32,
    pub payload: Value,
}

/// Async vector storage abstraction. Production callers wrap
/// `conversation_recall_vector` (sqlite) or lancedb; tests use the
/// in-file [`MockVectorStore`].
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(
        &self,
        id: &str,
        kind: &str,
        vector: Vec<f32>,
        payload: Value,
    ) -> Result<(), String>;

    /// Return up to `top_k` records of the given `kind`, sorted by
    /// similarity to `query_vec` descending. Tuple shape:
    /// `(id, score, payload)`.
    async fn query(
        &self,
        kind: &str,
        query_vec: Vec<f32>,
        top_k: usize,
    ) -> Vec<(String, f32, Value)>;
}

/// Embed + store one skill record. Idempotent: re-indexing the same
/// `skill_id` should overwrite the previous vector (the store
/// implementation owns this guarantee).
pub async fn index_skill(
    skill_id: &str,
    description: &str,
    body: &str,
    embedder: &dyn Embedder,
    store: &dyn VectorStore,
) -> Result<(), IndexError> {
    let body_slice: String = body.chars().take(SKILL_BODY_INDEX_CHARS).collect();
    let payload_text = format!("{description}\n{body_slice}");
    let vector = embedder.embed(&payload_text);
    let payload = serde_json::json!({
        "skill_id": skill_id,
        "description": description,
    });
    store
        .upsert(skill_id, SKILL_VECTOR_KIND, vector, payload)
        .await
        .map_err(IndexError::Store)
}

/// Retrieve the top `top_k` skills most similar to `query`.
pub async fn search_skills(
    query: &str,
    top_k: usize,
    embedder: &dyn Embedder,
    store: &dyn VectorStore,
) -> Vec<SkillSearchHit> {
    if top_k == 0 {
        return Vec::new();
    }
    let query_vec = embedder.embed(query);
    let raw = store.query(SKILL_VECTOR_KIND, query_vec, top_k).await;
    raw.into_iter()
        .map(|(skill_id, score, payload)| SkillSearchHit {
            skill_id,
            score,
            payload,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Test-only in-memory backend
// ---------------------------------------------------------------------------

pub mod mock {
    //! In-memory `VectorStore` impl used by integration tests.

    use super::*;
    use std::sync::Mutex;

    use crate::modules::skills::sedimentation::cosine_similarity;

    pub struct MockVectorStore {
        rows: Mutex<Vec<(String, String, Vec<f32>, Value)>>,
    }

    impl MockVectorStore {
        #[must_use]
        pub fn new() -> Self {
            Self {
                rows: Mutex::new(Vec::new()),
            }
        }

        #[must_use]
        pub fn len(&self) -> usize {
            self.rows.lock().map(|g| g.len()).unwrap_or(0)
        }

        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    impl Default for MockVectorStore {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl VectorStore for MockVectorStore {
        async fn upsert(
            &self,
            id: &str,
            kind: &str,
            vector: Vec<f32>,
            payload: Value,
        ) -> Result<(), String> {
            let mut rows = self.rows.lock().map_err(|e| e.to_string())?;
            // Idempotent: replace by (id, kind) if already present.
            if let Some(existing) = rows.iter_mut().find(|r| r.0 == id && r.1 == kind) {
                *existing = (id.to_string(), kind.to_string(), vector, payload);
            } else {
                rows.push((id.to_string(), kind.to_string(), vector, payload));
            }
            Ok(())
        }

        async fn query(
            &self,
            kind: &str,
            query_vec: Vec<f32>,
            top_k: usize,
        ) -> Vec<(String, f32, Value)> {
            let Ok(rows) = self.rows.lock() else {
                return Vec::new();
            };
            let mut scored: Vec<(String, f32, Value)> = rows
                .iter()
                .filter(|r| r.1 == kind)
                .map(|r| {
                    let score = cosine_similarity(&query_vec, &r.2);
                    (r.0.clone(), score, r.3.clone())
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(top_k);
            scored
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::skills::vector_index::mock::MockVectorStore;

    struct ConstEmbedder(Vec<f32>);

    impl Embedder for ConstEmbedder {
        fn embed(&self, _text: &str) -> Vec<f32> {
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn index_then_search_roundtrip() {
        let store = MockVectorStore::new();
        let embedder = ConstEmbedder(vec![1.0, 0.0, 0.0]);
        index_skill("s1", "desc", "body", &embedder, &store)
            .await
            .unwrap();
        assert_eq!(store.len(), 1);
        let hits = search_skills("q", 5, &embedder, &store).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].skill_id, "s1");
        assert!(hits[0].score > 0.99);
    }

    #[tokio::test]
    async fn upsert_is_idempotent() {
        let store = MockVectorStore::new();
        let embedder = ConstEmbedder(vec![1.0, 0.0]);
        index_skill("s1", "desc1", "body", &embedder, &store)
            .await
            .unwrap();
        index_skill("s1", "desc2", "body", &embedder, &store)
            .await
            .unwrap();
        assert_eq!(store.len(), 1);
    }
}
