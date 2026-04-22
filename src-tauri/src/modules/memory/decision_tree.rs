//! MEM-MOD-P4 — Mem0-style Update Decision Tree.
//!
//! Before persisting a fresh memory, we ask a small "utility LLM" to
//! decide whether this fact is:
//!
//! - **NOOP**:   already covered by an existing memory, nothing to do.
//! - **ADD**:    genuinely new; insert as-is.
//! - **UPDATE**: refines an existing entry (e.g. "now it's blue" vs an
//!               older "the colour is red"); rewrite that entry.
//! - **DELETE**: contradicts an existing fact; the older entry should
//!               be retired (the new content may also be added or
//!               discarded depending on caller policy).
//!
//! Reference: Mem0 paper §3.2 (Memory Update Decision Tree).
//!
//! # Failure modes
//!
//! Both the prompt and the parser are deliberately small.  When the
//! LLM returns garbage, an unknown verb, or an unknown candidate key,
//! we fall back to **ADD**.  The cost of an extra duplicate write is
//! lower than the cost of dropping a real fact.
//!
//! # Wiring
//!
//! This module is logic-only — it has no opinion on *when* to invoke
//! the decision tree.  Production callers (`memory_store` tool) own
//! the feature-flag check (`MemoryFeatureConfig::decision_tree_enabled`)
//! and the candidate-fetch step.  Keeping this layer pure makes the
//! gnarly LLM contract trivially testable from unit tests.

use serde::{Deserialize, Serialize};

use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::{MemoryEntry, MemoryError};

/// One of the four possible actions after the decision tree runs.
///
/// The variant carries the identity of the affected existing memory
/// when relevant so the caller does not need to re-look-up the candidate
/// list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verb")]
pub enum DecisionPlan {
    /// No change needed; the new content is fully implied by an
    /// existing memory.
    NoOp { covered_by: Option<String> },
    /// Persist `content` as a brand-new entry.
    Add,
    /// Replace the `content` of an existing entry identified by `key`.
    Update { existing_key: String },
    /// Retire an existing entry identified by `key` (caller decides
    /// whether to also add the new content).
    Delete { existing_key: String },
}

impl DecisionPlan {
    /// Verb label as it appears on the LLM wire — also used for audit
    /// emission and as the `verb` discriminator in the JSON envelope
    /// returned by `memory_store` when the tree is enabled.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            DecisionPlan::NoOp { .. } => "noop",
            DecisionPlan::Add => "add",
            DecisionPlan::Update { .. } => "update",
            DecisionPlan::Delete { .. } => "delete",
        }
    }
}

/// Render the system + user prompt the LLM sees.
///
/// `candidates` is the existing memories the caller deemed potentially
/// related (typically a `recall(content, None, k)` with k = 5).  The
/// prompt asks the LLM to emit a single JSON object so we don't need a
/// fragile regex parser.
#[must_use]
pub fn build_prompt(content: &str, candidates: &[MemoryEntry]) -> (String, String) {
    let system = "You are a memory-update classifier. Given a NEW FACT and a list of \
EXISTING MEMORIES, return a single JSON object describing the action to take. \
Allowed verbs: \"noop\" (already covered), \"add\" (new), \"update\" (rewrite \
an existing entry), \"delete\" (an existing fact is now wrong). Respond ONLY \
with the JSON object, no prose.

Schema (every field optional unless required by the verb):
  { \"verb\": \"noop\",   \"covered_by\": \"<key>\" }
  { \"verb\": \"add\" }
  { \"verb\": \"update\", \"existing_key\": \"<key>\" }
  { \"verb\": \"delete\", \"existing_key\": \"<key>\" }
"
        .to_string();

    let mut user = String::new();
    user.push_str("NEW FACT:\n");
    user.push_str(content.trim());
    user.push_str("\n\nEXISTING MEMORIES:\n");
    if candidates.is_empty() {
        user.push_str("(none)\n");
    } else {
        for (i, entry) in candidates.iter().enumerate() {
            user.push_str(&format!(
                "{i}. key={}; category={}; content={}\n",
                entry.key,
                entry.category.as_str(),
                entry.content.trim()
            ));
        }
    }
    user.push_str("\nReturn the JSON object now.");
    (system, user)
}

/// Parse a raw LLM response into a [`DecisionPlan`].  Strips common
/// JSON-fence wrappers (\`\`\`json … \`\`\`) before parsing.  Returns
/// `None` if the JSON is invalid, the verb is unknown, or the verb
/// requires an `existing_key` that is missing or unknown.  Callers are
/// expected to fall back to `DecisionPlan::Add` on `None`.
#[must_use]
pub fn parse_decision(raw: &str, candidates: &[MemoryEntry]) -> Option<DecisionPlan> {
    let body = strip_json_fence(raw.trim());
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let verb = parsed.get("verb")?.as_str()?;
    let key_belongs_to_candidates =
        |k: &str| candidates.iter().any(|c| c.key == k);

    match verb {
        "noop" => {
            let covered_by = parsed
                .get("covered_by")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            // NoOp without a target is still legal — the LLM may
            // genuinely think the new fact is uninteresting.
            Some(DecisionPlan::NoOp { covered_by })
        }
        "add" => Some(DecisionPlan::Add),
        "update" => {
            let key = parsed.get("existing_key")?.as_str()?.to_string();
            if key_belongs_to_candidates(&key) {
                Some(DecisionPlan::Update { existing_key: key })
            } else {
                None
            }
        }
        "delete" => {
            let key = parsed.get("existing_key")?.as_str()?.to_string();
            if key_belongs_to_candidates(&key) {
                Some(DecisionPlan::Delete { existing_key: key })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// End-to-end: build the prompt, call the LLM, parse the response,
/// fall back to `Add` on any failure.  Errors from the LLM call
/// itself bubble up so the caller can decide whether to surface a
/// "decision tree unavailable" warning vs silently degrade.
pub async fn decide_via_llm<L: UtilityLlm + ?Sized>(
    llm: &L,
    content: &str,
    candidates: &[MemoryEntry],
) -> Result<DecisionPlan, MemoryError> {
    let (system, user) = build_prompt(content, candidates);
    // 256 tokens is plenty for a single JSON object.  Temp 0 keeps
    // the verb deterministic.
    let raw = llm.complete(&system, &user, 256, 0.0).await?;
    Ok(parse_decision(&raw, candidates).unwrap_or(DecisionPlan::Add))
}

fn strip_json_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(stripped) = s.strip_prefix("```json") {
        return stripped.trim_start_matches('\n').trim_end_matches("```").trim();
    }
    if let Some(stripped) = s.strip_prefix("```") {
        return stripped.trim_start_matches('\n').trim_end_matches("```").trim();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::MemoryCategory;
    use chrono::Utc;

    fn entry(key: &str, content: &str) -> MemoryEntry {
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: MemoryCategory::Conversation,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        }
    }

    #[test]
    fn parses_add_verb() {
        assert_eq!(
            parse_decision(r#"{"verb":"add"}"#, &[]),
            Some(DecisionPlan::Add)
        );
    }

    #[test]
    fn parses_noop_with_covered_by() {
        let cs = [entry("k1", "x")];
        let plan = parse_decision(r#"{"verb":"noop","covered_by":"k1"}"#, &cs).unwrap();
        match plan {
            DecisionPlan::NoOp { covered_by } => assert_eq!(covered_by.as_deref(), Some("k1")),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_update_with_existing_key() {
        let cs = [entry("k1", "x")];
        let plan = parse_decision(r#"{"verb":"update","existing_key":"k1"}"#, &cs).unwrap();
        assert_eq!(
            plan,
            DecisionPlan::Update {
                existing_key: "k1".to_string()
            }
        );
    }

    #[test]
    fn rejects_update_with_unknown_key() {
        let cs = [entry("k1", "x")];
        assert!(parse_decision(r#"{"verb":"update","existing_key":"ghost"}"#, &cs).is_none());
    }

    #[test]
    fn rejects_update_without_existing_key() {
        let cs = [entry("k1", "x")];
        assert!(parse_decision(r#"{"verb":"update"}"#, &cs).is_none());
    }

    #[test]
    fn handles_json_fence_wrapping() {
        let raw = "```json\n{\"verb\":\"add\"}\n```";
        assert_eq!(parse_decision(raw, &[]), Some(DecisionPlan::Add));
    }

    #[test]
    fn unknown_verb_is_none() {
        assert!(parse_decision(r#"{"verb":"surprise"}"#, &[]).is_none());
    }

    #[test]
    fn invalid_json_is_none() {
        assert!(parse_decision("not json at all", &[]).is_none());
    }

    #[test]
    fn build_prompt_includes_candidates() {
        let cs = [entry("k1", "v1"), entry("k2", "v2")];
        let (sys, usr) = build_prompt("new fact", &cs);
        assert!(sys.contains("verb"));
        assert!(usr.contains("new fact"));
        assert!(usr.contains("key=k1"));
        assert!(usr.contains("key=k2"));
    }

    #[test]
    fn build_prompt_handles_empty_candidates() {
        let (_, usr) = build_prompt("solo", &[]);
        assert!(usr.contains("(none)"));
    }

    // End-to-end with MockUtilityLlm to prove the fallback chain.
    #[tokio::test]
    async fn decide_via_llm_falls_back_to_add_on_garbage() {
        use crate::modules::memory::llm::MockUtilityLlm;
        let llm = MockUtilityLlm::new(vec!["??? not json".to_string()]);
        let plan = decide_via_llm(&llm, "fact", &[]).await.unwrap();
        assert_eq!(plan, DecisionPlan::Add);
    }

    #[tokio::test]
    async fn decide_via_llm_returns_parsed_plan_on_valid_json() {
        use crate::modules::memory::llm::MockUtilityLlm;
        let llm = MockUtilityLlm::new(vec![r#"{"verb":"add"}"#.to_string()]);
        let plan = decide_via_llm(&llm, "fact", &[]).await.unwrap();
        assert_eq!(plan, DecisionPlan::Add);
    }
}
