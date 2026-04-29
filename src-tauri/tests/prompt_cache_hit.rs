//! Verifies PromptFingerprint equality semantics for caching the system prompt
//! across iterations of the agent loop.

use if2ai_backend::modules::application::turn_service::prompt_cache::{
    CachedSystemPrompt, PromptFingerprint,
};

#[test]
fn identical_inputs_produce_equal_fingerprints() {
    let fp1 = PromptFingerprint::compute(&["skill-a"], &["bash", "grep"]);
    let fp2 = PromptFingerprint::compute(&["skill-a"], &["bash", "grep"]);
    assert_eq!(fp1, fp2);
}

#[test]
fn different_skill_set_invalidates_fingerprint() {
    let fp1 = PromptFingerprint::compute(&["skill-a"], &["bash"]);
    let fp2 = PromptFingerprint::compute(&["skill-b"], &["bash"]);
    assert_ne!(fp1, fp2);
}

#[test]
fn different_tool_set_invalidates_fingerprint() {
    let fp1 = PromptFingerprint::compute(&["skill-a"], &["bash"]);
    let fp2 = PromptFingerprint::compute(&["skill-a"], &["grep"]);
    assert_ne!(fp1, fp2);
}

#[test]
fn order_independent_for_skills() {
    let fp1 = PromptFingerprint::compute(&["a", "b"], &["x"]);
    let fp2 = PromptFingerprint::compute(&["b", "a"], &["x"]);
    assert_eq!(fp1, fp2);
}

#[test]
fn order_independent_for_tools() {
    let fp1 = PromptFingerprint::compute(&["a"], &["x", "y"]);
    let fp2 = PromptFingerprint::compute(&["a"], &["y", "x"]);
    assert_eq!(fp1, fp2);
}

#[test]
fn cached_prompt_round_trips() {
    let fp = PromptFingerprint::compute(&["skill-a"], &["bash"]);
    let cache = CachedSystemPrompt {
        content: "system text".into(),
        fingerprint: fp.clone(),
    };
    let again = PromptFingerprint::compute(&["skill-a"], &["bash"]);
    assert_eq!(cache.fingerprint, again);
    assert_eq!(cache.content, "system text");
}

#[test]
fn empty_inputs_consistent() {
    assert_eq!(
        PromptFingerprint::compute(&[], &[]),
        PromptFingerprint::compute(&[], &[]),
    );
}
