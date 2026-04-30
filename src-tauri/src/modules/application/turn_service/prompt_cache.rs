//! System prompt cache keyed by skill + tool fingerprint.
//!
//! Avoids rebuilding the full prompt plan every loop iteration when the
//! active skills and registered tools have not changed.
//!
//! Mirrors Steward's `cached_prompt` on `ReasoningContext`. The cache is
//! per-turn (lives inside the local stream-loop state), not global.
//!
//! ## Wiring status (Steward-Alignment Task 3.2 / S3-S4)
//!
//! Prior-art investigation (see Task 3.2 report) found that if2Ai already
//! computes a `tool_pool_schema_hash_for_stream` for tracing/telemetry but
//! has **no** prompt-plan caching layer — every loop iteration calls
//! [`build_prompt_plan`](super::build_prompt_plan) from scratch.
//!
//! The cache *type* lives here today, but is **not yet wired** into the
//! legacy `run_stream_task_body`. That body will be rewritten into
//! `run_agentic_loop` in Session 5 (Task 5.1), which is the natural home
//! for the iteration-scoped cache. Wiring inline now would create
//! throwaway code (same justification as Task 2.2's `force_text`
//! deferral). When `run_agentic_loop` lands, it should:
//!
//! 1. Hold a `Option<CachedSystemPrompt>` across iterations.
//! 2. At the top of each iteration, compute a [`PromptFingerprint`] from
//!    the current active skill ids + registered tool names.
//! 3. Reuse the cached `content` on hit; otherwise call
//!    `build_prompt_plan` and store the result.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
    pub content: String,
    pub fingerprint: PromptFingerprint,
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
}
