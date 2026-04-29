//! FEAT-SE-002 — Skill draft deduplication via embedding similarity.
//!
//! Pure-sync layer on top of [`super::SkillDraft`]: callers inject an
//! [`Embedder`] (a sync `fn(&str) -> Vec<f32>`), we cluster drafts
//! whose cosine similarity is `>= DEDUP_SIMILARITY_THRESHOLD`, keep
//! the longest-described draft as the cluster representative, and
//! roll the rest into [`DedupedSkill::aliases`].
//!
//! Sync-by-design (Pack contract): keeps the test surface trivial —
//! a `MockEmbedder` returns canned vectors without `tokio`.

use super::SkillDraft;

/// Cosine threshold above which two drafts are treated as the same
/// skill. Pinned at 0.85 per Pack spec; lower values would over-merge.
pub const DEDUP_SIMILARITY_THRESHOLD: f32 = 0.85;

/// Sync embedding facade. Production callers wrap their async embedder
/// in a tiny adapter (or precompute embeddings before calling
/// [`dedup_drafts`]).
pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Output of [`dedup_drafts`]. Wraps the cluster representative plus
/// every aliased draft name and source-turn evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupedSkill {
    /// Representative draft (the longest description in the cluster).
    pub representative: SkillDraft,
    /// Names of the other drafts that folded into this cluster.
    pub aliases: Vec<String>,
    /// Number of original drafts in this cluster (= 1 + aliases.len()).
    pub cluster_size: usize,
}

impl DedupedSkill {
    fn singleton(draft: SkillDraft) -> Self {
        Self {
            representative: draft,
            aliases: Vec::new(),
            cluster_size: 1,
        }
    }
}

/// Group `drafts` by embedding similarity (>= [`DEDUP_SIMILARITY_THRESHOLD`])
/// and reduce each group to one [`DedupedSkill`]. Order of returned
/// vec mirrors the order in which the *representatives* first appear
/// in the input — stable for tests and downstream UIs.
pub fn dedup_drafts(drafts: Vec<SkillDraft>, embedder: &dyn Embedder) -> Vec<DedupedSkill> {
    if drafts.is_empty() {
        return Vec::new();
    }

    let embeddings: Vec<Vec<f32>> = drafts
        .iter()
        .map(|d| embedder.embed(&format!("{}\n{}", d.description, d.body)))
        .collect();

    // Greedy clustering: walk drafts in order; for each, find the
    // first prior cluster whose representative's similarity >=
    // threshold, otherwise start a new cluster.
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for (idx, vec_a) in embeddings.iter().enumerate() {
        let mut joined = false;
        for cluster in clusters.iter_mut() {
            let rep_idx = cluster[0];
            let sim = cosine_similarity(vec_a, &embeddings[rep_idx]);
            if sim >= DEDUP_SIMILARITY_THRESHOLD {
                cluster.push(idx);
                joined = true;
                break;
            }
        }
        if !joined {
            clusters.push(vec![idx]);
        }
    }

    let mut out: Vec<DedupedSkill> = Vec::with_capacity(clusters.len());
    for cluster in clusters {
        if cluster.len() == 1 {
            out.push(DedupedSkill::singleton(drafts[cluster[0]].clone()));
            continue;
        }
        // Pick the longest description as representative.
        let rep_idx = *cluster
            .iter()
            .max_by_key(|&&i| drafts[i].description.len())
            .expect("non-empty cluster");
        let aliases: Vec<String> = cluster
            .iter()
            .filter(|&&i| i != rep_idx)
            .map(|&i| drafts[i].name.clone())
            .collect();
        out.push(DedupedSkill {
            representative: drafts[rep_idx].clone(),
            cluster_size: cluster.len(),
            aliases,
        });
    }
    out
}

/// Cosine similarity in [-1, 1]. Returns 0.0 on length mismatch or
/// zero-norm vectors so callers don't have to special-case.
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
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_basic_orthogonal() {
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_basic_identical() {
        assert!((cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_handles_length_mismatch() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
