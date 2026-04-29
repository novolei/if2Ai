//! FEAT-DK-003 — Auto-contribute domain knowledge candidates from
//! conversation history.
//!
//! Flow:
//!
//! 1. [`extract_domain_knowledge_candidates`] — async; condenses
//!    `Vec<InputMessage>` into a single LLM prompt asking for one
//!    JSON `DomainKnowledgeEntry`-shaped suggestion. LLM error /
//!    empty / unparseable response → empty vec (graceful).
//! 2. [`verify_knowledge_safety`] — sync; wraps the candidate as a
//!    surrogate `SkillDraft` and runs `evaluate_constitution`. Any
//!    rule fire → `safe = false` with rule ids in `violations`.
//!
//! Pack contract: draft-only — nothing is upserted into the
//! `KnowledgeStore`, no work_loop / stream_finalize integration. A
//! future wiring Pack does the persistence.

use serde::{Deserialize, Serialize};

use super::{
    DomainKnowledgeEntry, DomainKnowledgeKind, KnowledgeAuthor, SelectorEntry, SelectorStability,
};
use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::memory::UtilityLlm;
use crate::modules::skills::guard::evaluate_constitution;
use crate::modules::skills::sedimentation::SkillDraft;

/// Per-call LLM token ceiling — keeps utility cost predictable.
const CONTRIBUTOR_MAX_TOKENS: u32 = 320;
const CONTRIBUTOR_TEMPERATURE: f32 = 0.1;

/// Minimum number of messages before we even attempt extraction.
/// Below this the prompt has nothing to summarize over.
const MIN_HISTORY_LEN: usize = 1;

/// Constitution-check verdict for one candidate entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeVerdict {
    pub entry_id: String,
    pub safe: bool,
    pub violations: Vec<String>,
}

const CONTRIBUTOR_SYSTEM_PROMPT: &str = concat!(
    "Inspect the conversation excerpt and propose ONE concrete domain ",
    "knowledge entry the agent should remember.\n",
    "Reply with ONLY a JSON object — no markdown fences, no commentary:\n",
    "{\"kind\":\"website_domain|interaction_primitive|task_sop\",",
    "\"domain\":\"<host or task slug>\",",
    "\"selector\":\"<selector or N/A>\",",
    "\"purpose\":\"<one line>\",",
    "\"gotcha\":\"<known pitfall or empty>\"}",
);

/// Extract zero-or-more candidate `DomainKnowledgeEntry` values from
/// the supplied conversation history.
pub async fn extract_domain_knowledge_candidates(
    history: &[InputMessage],
    llm: &dyn UtilityLlm,
) -> Vec<DomainKnowledgeEntry> {
    if history.len() < MIN_HISTORY_LEN {
        return Vec::new();
    }
    let user_prompt = render_history_excerpt(history);
    let raw = match llm
        .complete(
            CONTRIBUTOR_SYSTEM_PROMPT,
            &user_prompt,
            CONTRIBUTOR_MAX_TOKENS,
            CONTRIBUTOR_TEMPERATURE,
        )
        .await
    {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => return Vec::new(),
        Err(err) => {
            tracing::warn!(
                error = %err,
                "[domain_knowledge.contributor] LLM error; degrading to empty candidates"
            );
            return Vec::new();
        }
    };
    match parse_llm_json(&raw) {
        Some(entry) => vec![entry],
        None => {
            tracing::debug!(
                raw_len = raw.len(),
                "[domain_knowledge.contributor] failed to parse LLM JSON"
            );
            Vec::new()
        }
    }
}

/// Run the SE-003 constitution rules against a candidate. Constructs
/// a surrogate `SkillDraft` from the entry payload so we re-use the
/// existing rule engine without duplicating regex.
#[must_use]
pub fn verify_knowledge_safety(entry: &DomainKnowledgeEntry) -> KnowledgeVerdict {
    let surrogate = entry_as_skill_draft(entry);
    let violations = evaluate_constitution(&surrogate);
    KnowledgeVerdict {
        entry_id: entry.id.clone(),
        safe: violations.is_empty(),
        violations: violations
            .into_iter()
            .map(|v| v.rule_id.to_string())
            .collect(),
    }
}

fn entry_as_skill_draft(entry: &DomainKnowledgeEntry) -> SkillDraft {
    let body = match &entry.kind {
        DomainKnowledgeKind::WebsiteDomain {
            domain,
            selectors,
            gotchas,
            url_patterns,
        } => {
            let mut s = format!("# Website {domain}\n\n## URL patterns\n");
            for u in url_patterns {
                s.push_str(&format!("- {u}\n"));
            }
            s.push_str("\n## Selectors\n");
            for sel in selectors {
                s.push_str(&format!("- `{}` — {}\n", sel.selector, sel.purpose));
            }
            s.push_str("\n## Gotchas\n");
            for g in gotchas {
                s.push_str(&format!("- {g}\n"));
            }
            s
        }
        DomainKnowledgeKind::InteractionPrimitive {
            category,
            problem_statement,
            solution_code,
            ..
        } => format!(
            "# Interaction: {category}\n\n## Problem\n{problem_statement}\n\n## Solution\n```\n{solution_code}\n```\n"
        ),
        DomainKnowledgeKind::TaskSOP {
            task_type,
            execution_steps,
            ..
        } => {
            let mut s = format!("# SOP: {task_type}\n\n## Steps\n");
            for step in execution_steps {
                s.push_str(&format!("- {} :: {}\n", step.tool_name, step.description));
            }
            s
        }
    };
    SkillDraft {
        name: format!("dk-{}", entry.id),
        description: format!("Auto-contributed {} entry", entry.kind.label()),
        body: format!(
            "---\nname: \"dk-{}\"\ndescription: \"surrogate\"\n---\n{}",
            entry.id, body
        ),
        source_turns: Vec::new(),
        tool_sequence: Vec::new(),
    }
}

fn render_history_excerpt(history: &[InputMessage]) -> String {
    let mut out = String::new();
    let take_from = history.len().saturating_sub(8);
    for msg in &history[take_from..] {
        out.push_str(&format!("[{}] ", msg.role));
        for block in &msg.content {
            match block {
                InputContentBlock::Text { text } => {
                    let snippet: String = text.chars().take(160).collect();
                    out.push_str(&snippet);
                    out.push(' ');
                }
                InputContentBlock::ToolUse { name, input, .. } => {
                    out.push_str(&format!("→{name} {} ", input));
                }
                _ => {}
            }
        }
        out.push('\n');
    }
    out
}

fn parse_llm_json(raw: &str) -> Option<DomainKnowledgeEntry> {
    let json_text = extract_first_json_object(raw)?;
    let value: serde_json::Value = serde_json::from_str(&json_text).ok()?;
    let kind_label = value.get("kind")?.as_str()?;
    let domain = value
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let selector = value
        .get("selector")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let purpose = value
        .get("purpose")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let gotcha = value
        .get("gotcha")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let kind = match kind_label {
        "website_domain" => DomainKnowledgeKind::WebsiteDomain {
            domain: domain.clone(),
            url_patterns: vec![format!("https://{domain}/*")],
            selectors: vec![SelectorEntry {
                selector,
                purpose,
                stability: SelectorStability::Untested,
                last_verified: None,
            }],
            gotchas: if gotcha.is_empty() {
                Vec::new()
            } else {
                vec![gotcha]
            },
        },
        "interaction_primitive" => DomainKnowledgeKind::InteractionPrimitive {
            category: domain,
            problem_statement: purpose,
            solution_code: selector,
            tradeoffs: if gotcha.is_empty() {
                Vec::new()
            } else {
                vec![gotcha]
            },
        },
        "task_sop" => DomainKnowledgeKind::TaskSOP {
            task_type: domain,
            prerequisites: Vec::new(),
            key_pitfalls: if gotcha.is_empty() {
                Vec::new()
            } else {
                vec![gotcha]
            },
            execution_steps: Vec::new(),
        },
        _ => return None,
    };

    Some(DomainKnowledgeEntry::new(
        kind,
        KnowledgeAuthor::Agent {
            session_id: "auto-contrib".to_string(),
        },
        0.5,
    ))
}

fn extract_first_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (idx, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..start + idx + ch.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
