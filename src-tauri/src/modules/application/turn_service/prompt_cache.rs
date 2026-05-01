//! System prompt cache keyed by skill + tool fingerprint.
//!
//! Avoids rebuilding the full prompt plan every loop iteration when the
//! active skills and registered tools have not changed.
//!
//! Mirrors Steward's `cached_prompt` on `ReasoningContext`. The cache is
//! per-turn (lives inside the local stream-loop state), not global.
//!
//! ## Wiring status
//!
//! Wired into `iteration_preflight` (stream_iteration.rs) via
//! `StreamLoopState::prompt_cache`. Each iteration computes a
//! [`PromptFingerprint`] from the current tool pool names and compares
//! against the cached fingerprint:
//!
//! 1. **Cache hit** — reuses the cached `system_prompt` + `tool_defs`
//!    without recomputation. A tracing log records the hit and
//!    estimated token savings.
//! 2. **Cache miss** — rebuilds the prompt plan, stores the new
//!    fingerprint + content, and logs the miss.
//! 3. **Forced refresh** — every [`FORCE_REFRESH_INTERVAL`] iterations
//!    the cache is unconditionally invalidated to guard against stale
//!    state (mirrors GenericAgent's periodic-refresh design).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::modules::api::ToolDefinition;

/// Force a cache rebuild every N iterations to prevent stale state.
/// Mirrors GenericAgent's periodic refresh design.
pub const FORCE_REFRESH_INTERVAL: usize = 10;

/// Stable, order-independent fingerprint over the inputs that determine
/// the system prompt: the active skill set and the registered tool set.
///
/// Two `PromptFingerprint` values compare equal iff the (sorted) skill
/// ids and (sorted) tool names hash identically. This is intentionally
/// coarser than a full content hash — it captures exactly what the
/// agentic loop needs to decide "can I reuse last iteration's prompt?"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptFingerprint {
    skill_hash: u64,
    tool_hash: u64,
}

impl PromptFingerprint {
    /// Compute a fingerprint from the active skill ids and tool names.
    /// Inputs are sorted before hashing so order does not matter.
    pub fn compute(skill_ids: &[&str], tool_names: &[&str]) -> Self {
        let mut s: Vec<&str> = skill_ids.to_vec();
        s.sort_unstable();
        let mut t: Vec<&str> = tool_names.to_vec();
        t.sort_unstable();

        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        let skill_hash = h.finish();

        let mut h = DefaultHasher::new();
        t.hash(&mut h);
        let tool_hash = h.finish();

        Self {
            skill_hash,
            tool_hash,
        }
    }
}

/// A cached system prompt plus the fingerprint of the inputs that
/// produced it. Stored per-turn by the agentic loop; compare incoming
/// fingerprints with [`PromptFingerprint`]'s `Eq` impl to decide
/// whether to reuse `content` or rebuild.
#[derive(Debug, Clone)]
pub struct CachedSystemPrompt {
    /// The system prompt text that was built for this fingerprint.
    pub content: String,
    /// Tool definitions snapshot corresponding to this fingerprint.
    pub tool_defs: Vec<ToolDefinition>,
    /// The fingerprint that produced this cache entry.
    pub fingerprint: PromptFingerprint,
    /// The iteration at which this cache entry was last refreshed.
    pub cached_at_iteration: usize,
}

impl CachedSystemPrompt {
    /// Returns `true` when the cache entry should be forcibly refreshed,
    /// either because the fingerprint changed or because the entry has
    /// lived past [`FORCE_REFRESH_INTERVAL`] iterations.
    pub fn should_refresh(&self, current_fp: &PromptFingerprint, current_iter: usize) -> bool {
        self.fingerprint != *current_fp
            || current_iter.saturating_sub(self.cached_at_iteration) >= FORCE_REFRESH_INTERVAL
    }

    /// Rough estimate of cached token savings. Uses a 4-chars-per-token
    /// heuristic on the system prompt plus ~60 tokens per tool definition
    /// (name + description + schema overhead).
    pub fn estimated_token_savings(&self) -> usize {
        let prompt_tokens = self.content.len() / 4;
        let tool_tokens = self.tool_defs.len() * 60;
        prompt_tokens + tool_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_resistance_smoke() {
        assert_ne!(
            PromptFingerprint::compute(&["a"], &["x"]),
            PromptFingerprint::compute(&["aa"], &["x"]),
        );
    }

    #[test]
    fn should_refresh_on_fingerprint_change() {
        let fp_a = PromptFingerprint::compute(&[], &["tool_a"]);
        let fp_b = PromptFingerprint::compute(&[], &["tool_b"]);
        let cached = CachedSystemPrompt {
            content: "system".to_string(),
            tool_defs: vec![],
            fingerprint: fp_a,
            cached_at_iteration: 1,
        };
        // Different fingerprint → should refresh.
        assert!(cached.should_refresh(&fp_b, 2));
    }

    #[test]
    fn should_refresh_after_force_interval() {
        let fp = PromptFingerprint::compute(&[], &["tool_a"]);
        let cached = CachedSystemPrompt {
            content: "system".to_string(),
            tool_defs: vec![],
            fingerprint: fp.clone(),
            cached_at_iteration: 1,
        };
        // Same fingerprint, within interval → no refresh.
        assert!(!cached.should_refresh(&fp, 5));
        // Same fingerprint, at interval boundary → refresh.
        assert!(cached.should_refresh(&fp, 1 + FORCE_REFRESH_INTERVAL));
    }

    #[test]
    fn estimated_token_savings_basic() {
        let cached = CachedSystemPrompt {
            content: "a".repeat(400), // 400 chars ≈ 100 tokens
            tool_defs: vec![
                ToolDefinition {
                    name: "t1".to_string(),
                    description: None,
                    input_schema: serde_json::json!({}),
                },
                ToolDefinition {
                    name: "t2".to_string(),
                    description: None,
                    input_schema: serde_json::json!({}),
                },
            ],
            fingerprint: PromptFingerprint::compute(&[], &["t1", "t2"]),
            cached_at_iteration: 0,
        };
        // 400/4 + 2*60 = 100 + 120 = 220
        assert_eq!(cached.estimated_token_savings(), 220);
    }
}
