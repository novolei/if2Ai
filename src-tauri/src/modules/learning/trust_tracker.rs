//! Trust Tracker — per-entry trust scoring
//!
//! Maintains trust scores for memory entries based on user feedback.
//! Scores are in [-1.0, 1.0] and adjusted by helpful/unhelpful feedback.
//!
//! # `#![allow(dead_code)]` justification
//! TrustTracker is consumed by the retrieval pipeline to weight results
//! by trustworthiness. It will be wired into the active retrieval flow.

#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Trust feedback for a memory entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustFeedback {
    Helpful,
    Unhelpful,
    Neutral,
}

/// Trust tracker for memory entries
///
/// Maintains per-key trust scores in [-1.0, 1.0].
/// Adjustments:
/// - Helpful: +0.05
/// - Unhelpful: -0.10
/// - Neutral: no change
pub struct TrustTracker {
    scores: RwLock<HashMap<String, f32>>,
}

impl TrustTracker {
    /// Create a new empty trust tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            scores: RwLock::new(HashMap::new()),
        }
    }

    /// Adjust trust for a memory entry based on feedback
    ///
    /// Helpful: +0.05, Unhelpful: -0.10, Neutral: 0.0
    /// Scores are clamped to [-1.0, 1.0].
    pub async fn adjust(&self, key: &str, feedback: TrustFeedback) {
        let delta = match feedback {
            TrustFeedback::Helpful => 0.05,
            TrustFeedback::Unhelpful => -0.10,
            TrustFeedback::Neutral => 0.0,
        };

        let mut scores = self.scores.write().await;
        let entry = scores.entry(key.to_string()).or_insert(0.0);
        *entry = (*entry + delta).clamp(-1.0, 1.0);
    }

    /// Get the current trust score for a key
    ///
    /// Returns 0.0 if the key has no score yet.
    pub async fn get(&self, key: &str) -> f32 {
        self.scores.read().await.get(key).copied().unwrap_or(0.0)
    }

    /// Get trust scores for all keys
    pub async fn get_all(&self) -> HashMap<String, f32> {
        self.scores.read().await.clone()
    }

    /// Remove a key's trust score
    pub async fn remove(&self, key: &str) {
        self.scores.write().await.remove(key);
    }

    /// Clear all trust scores
    pub async fn clear(&self) {
        self.scores.write().await.clear();
    }

    /// Get the number of tracked keys
    pub async fn len(&self) -> usize {
        self.scores.read().await.len()
    }

    /// Check if any keys are tracked
    pub async fn is_empty(&self) -> bool {
        self.scores.read().await.is_empty()
    }
}

impl Default for TrustTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_tracker_is_empty() {
        let tracker = TrustTracker::new();
        assert_eq!(tracker.get("key").await, 0.0);
        assert!(tracker.is_empty().await);
    }

    #[tokio::test]
    async fn helpful_increases_trust() {
        let tracker = TrustTracker::new();
        tracker.adjust("key", TrustFeedback::Helpful).await;
        let score = tracker.get("key").await;
        assert!((score - 0.05).abs() < 0.001);
    }

    #[tokio::test]
    async fn unhelpful_decreases_trust() {
        let tracker = TrustTracker::new();
        tracker.adjust("key", TrustFeedback::Unhelpful).await;
        let score = tracker.get("key").await;
        assert!((score - (-0.10)).abs() < 0.001);
    }

    #[tokio::test]
    async fn neutral_does_not_change() {
        let tracker = TrustTracker::new();
        tracker.adjust("key", TrustFeedback::Neutral).await;
        let score = tracker.get("key").await;
        assert!(score.abs() < 0.001);
    }

    #[tokio::test]
    async fn scores_clamp_to_bounds() {
        let tracker = TrustTracker::new();

        // Clamp to upper bound
        for _ in 0..30 {
            tracker.adjust("key", TrustFeedback::Helpful).await;
        }
        let score = tracker.get("key").await;
        assert!(
            (score - 1.0).abs() < 0.001,
            "score should be clamped to 1.0, got {score}"
        );

        // Clamp to lower bound
        let tracker2 = TrustTracker::new();
        for _ in 0..15 {
            tracker2.adjust("key", TrustFeedback::Unhelpful).await;
        }
        let score = tracker2.get("key").await;
        assert!(
            (score - (-1.0)).abs() < 0.001,
            "score should be clamped to -1.0, got {score}"
        );
    }

    #[tokio::test]
    async fn remove_clears_score() {
        let tracker = TrustTracker::new();
        tracker.adjust("key", TrustFeedback::Helpful).await;
        tracker.remove("key").await;
        assert_eq!(tracker.get("key").await, 0.0);
    }

    #[tokio::test]
    async fn clear_removes_all_scores() {
        let tracker = TrustTracker::new();
        tracker.adjust("a", TrustFeedback::Helpful).await;
        tracker.adjust("b", TrustFeedback::Unhelpful).await;
        tracker.clear().await;
        assert!(tracker.is_empty().await);
    }

    #[tokio::test]
    async fn get_all_returns_all_scores() {
        let tracker = TrustTracker::new();
        tracker.adjust("a", TrustFeedback::Helpful).await;
        tracker.adjust("b", TrustFeedback::Unhelpful).await;

        let all = tracker.get_all().await;
        assert_eq!(all.len(), 2);
        assert!((all["a"] - 0.05).abs() < 0.001);
        assert!((all["b"] - (-0.10)).abs() < 0.001);
    }

    #[tokio::test]
    async fn multiple_adjustments_accumulate() {
        let tracker = TrustTracker::new();
        tracker.adjust("key", TrustFeedback::Helpful).await;
        tracker.adjust("key", TrustFeedback::Helpful).await;
        tracker.adjust("key", TrustFeedback::Unhelpful).await;

        let score = tracker.get("key").await;
        // 0.05 + 0.05 - 0.10 = 0.0
        assert!(score.abs() < 0.001);
    }
}
