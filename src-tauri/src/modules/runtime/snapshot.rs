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

/// Detailed result of a [`FrozenSnapshot::verify_detailed`] call (M6).
///
/// In addition to the boolean outcome, callers receive both hashes and a
/// short, human-readable description of *why* verification failed.  This
/// turns "integrity check failed" log lines into actionable diagnostics
/// without forcing every callsite to re-hash both prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    /// `true` when `current` hashes to the same value as the snapshot.
    pub valid: bool,
    /// Hash captured at snapshot time (always present).
    pub expected_hash: String,
    /// Hash of the prompt currently being verified.
    pub actual_hash: String,
    /// `None` on success.  On failure, a short description such as
    /// `"length differs: snapshot=128 current=256"` or
    /// `"hash mismatch: prompts differ at byte 42"`.
    pub details: Option<String>,
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

    /// Verify that the current prompt matches the snapshot.
    ///
    /// Returns `true` if the current system prompt hash equals the stored
    /// snapshot hash, indicating no modification.  This is the legacy boolean
    /// interface; new callers should prefer [`Self::verify_detailed`] for
    /// richer diagnostics on mismatch.
    pub fn verify(&self, current_prompt: &str) -> bool {
        self.verify_detailed(current_prompt).valid
    }

    /// M6: hash-based verification that returns full diagnostics.
    ///
    /// On mismatch the `details` field carries a short description of the
    /// first observable divergence (length differs, byte position of the
    /// first differing character, or `"hash mismatch"` when the texts are
    /// identical but hashing produces different values — which would
    /// indicate a corrupted snapshot).
    #[must_use]
    pub fn verify_detailed(&self, current_prompt: &str) -> VerifyResult {
        let actual_hash = compute_hash(current_prompt);
        if self.prompt_hash == actual_hash {
            return VerifyResult {
                valid: true,
                expected_hash: self.prompt_hash.clone(),
                actual_hash,
                details: None,
            };
        }

        let snap_len = self.prompt.len();
        let cur_len = current_prompt.len();
        let details = if snap_len != cur_len {
            format!("length differs: snapshot={snap_len} current={cur_len}")
        } else if let Some(idx) = self
            .prompt
            .as_bytes()
            .iter()
            .zip(current_prompt.as_bytes().iter())
            .position(|(a, b)| a != b)
        {
            format!("hash mismatch: prompts differ at byte {idx}")
        } else {
            "hash mismatch: identical bytes but hash differs (corrupted snapshot?)".to_string()
        };

        VerifyResult {
            valid: false,
            expected_hash: self.prompt_hash.clone(),
            actual_hash,
            details: Some(details),
        }
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
    fn verify_detailed_reports_success() {
        let snapshot = FrozenSnapshot::capture("hello");
        let result = snapshot.verify_detailed("hello");
        assert!(result.valid);
        assert!(result.details.is_none());
        assert_eq!(result.expected_hash, result.actual_hash);
    }

    #[test]
    fn verify_detailed_reports_length_diff() {
        let snapshot = FrozenSnapshot::capture("short");
        let result = snapshot.verify_detailed("a much longer prompt");
        assert!(!result.valid);
        let details = result.details.expect("details on failure");
        assert!(details.contains("length differs"), "got {details}");
        assert_ne!(result.expected_hash, result.actual_hash);
    }

    #[test]
    fn verify_detailed_reports_byte_position_when_lengths_match() {
        let snapshot = FrozenSnapshot::capture("hello world!");
        let result = snapshot.verify_detailed("hello WORLD!");
        assert!(!result.valid);
        let details = result.details.expect("details on failure");
        assert!(details.contains("byte"), "got {details}");
    }

    #[test]
    fn token_estimate() {
        let snapshot = FrozenSnapshot::capture("hello world");
        // M2: estimate_tokens now uses cl100k_base BPE; just bound-check.
        let tokens = snapshot.token_estimate();
        assert!(
            (1..=11).contains(&tokens),
            "expected 1..=11 tokens for 'hello world', got {tokens}"
        );
    }
}
