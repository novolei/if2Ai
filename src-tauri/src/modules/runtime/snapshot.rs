//! Frozen system prompt snapshot
//!
//! Captures the system prompt at session load time and provides
//! hash-based verification to detect if the prompt has been modified.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Immutable snapshot of the system prompt taken at session initialization
///
/// Used to verify that the system prompt has not been modified during
/// the session. The hash is computed from the prompt text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenSnapshot {
    /// The captured system prompt text
    pub prompt: String,
    /// SHA-style hash of the prompt for integrity verification
    pub prompt_hash: String,
    /// When this snapshot was captured
    pub created_at: DateTime<Utc>,
    /// Package version at capture time
    pub version: String,
}

impl FrozenSnapshot {
    /// Capture a snapshot of the current system prompt
    pub fn capture(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            prompt_hash: compute_hash(prompt),
            created_at: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Verify that the current prompt matches the snapshot
    ///
    /// Returns true if the current system prompt hash equals the
    /// stored snapshot hash, indicating no modification.
    pub fn verify(&self, current_prompt: &str) -> bool {
        self.prompt_hash == compute_hash(current_prompt)
    }

    /// Estimate token count of the frozen prompt
    pub fn token_estimate(&self) -> usize {
        crate::modules::runtime::budget::estimate_tokens(&self.prompt)
    }
}

/// Compute a deterministic hash of a string for integrity verification
#[must_use]
fn compute_hash(s: &str) -> String {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_stores_prompt_and_hash() {
        let snapshot = FrozenSnapshot::capture("You are a helpful assistant.");
        assert_eq!(snapshot.prompt, "You are a helpful assistant.");
        assert!(!snapshot.prompt_hash.is_empty());
        assert!(!snapshot.version.is_empty());
    }

    #[test]
    fn verify_returns_true_for_matching_prompt() {
        let snapshot = FrozenSnapshot::capture("You are a helpful assistant.");
        assert!(snapshot.verify("You are a helpful assistant."));
    }

    #[test]
    fn verify_returns_false_for_modified_prompt() {
        let snapshot = FrozenSnapshot::capture("You are a helpful assistant.");
        assert!(!snapshot.verify("You are NOT a helpful assistant."));
    }

    #[test]
    fn hash_is_deterministic() {
        let s1 = FrozenSnapshot::capture("test prompt");
        let s2 = FrozenSnapshot::capture("test prompt");
        assert_eq!(s1.prompt_hash, s2.prompt_hash);
    }

    #[test]
    fn different_prompts_produce_different_hashes() {
        let s1 = FrozenSnapshot::capture("prompt A");
        let s2 = FrozenSnapshot::capture("prompt B");
        assert_ne!(s1.prompt_hash, s2.prompt_hash);
    }

    #[test]
    fn token_estimate() {
        let snapshot = FrozenSnapshot::capture("hello world");
        // "hello world" = 11 chars / 4 + 1 = 3
        assert_eq!(snapshot.token_estimate(), 3);
    }
}
