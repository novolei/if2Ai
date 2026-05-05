#![allow(dead_code)]

//! Memory candidate extractor + existing-record lookup (Phase M4.1).
//!
//! First real source of [`MemoryWriteCandidate`]s for the
//! [`crate::modules::application::memory_coordinator::MemoryCoordinator::after_turn`]
//! pipeline.  Until M4.1, both production callers
//! (`commands/agent.rs::run_agent_turn` and
//! `commands/agent.rs::start_agent_stream`) passed `Vec::new()` so
//! the typed write-policy / quality-gate / conflict-resolver chain
//! was structurally complete but never observed real data.
//!
//! Honest scope of this module (intentionally narrow):
//!
//! 1. [`extract_memory_store_tool_candidates`] — scan the new
//!    messages produced during a single agent turn for
//!    `ContentBlock::ToolUse { name == "memory_store", input }`
//!    entries and convert each one into a typed
//!    [`MemoryWriteCandidate`].  This is **the most stable, most
//!    controllable** candidate source on the whole codebase: the
//!    arguments are well-formed JSON, the call site is gated by
//!    the existing `MemoryPolicyEngine`, and the user-facing UX
//!    already speaks "memory write" semantics.
//!
//! 2. [`lookup_existing_records_for_candidates`] — for each
//!    extracted candidate, query the
//!    [`crate::modules::memory::MemoryProvider`] for entries that
//!    look like a prior version of the same record and turn them
//!    into [`ExistingRecordRef`] inputs for the conflict resolver.
//!    First cut uses `recall_scoped` with the candidate `key` as
//!    the query string, returns the first hit (if any) as the
//!    "existing record".  This is enough to flip the resolver
//!    from "always NoConflict" to "produces real outcomes for
//!    same-key writes".
//!
//! ## Out of scope (M4-A intentional non-goals)
//!
//! - Reflection-note → candidate adapter, and tool trace extraction
//!   beyond `memory_store` — these belong to later M4 / M5 slices.
//! - Cross-scope dedup, embedding-based similarity, persistent
//!   per-record evidence registry — left for the M3-B+ persistence
//!   wiring referenced in the M3 audit doc.

use std::sync::Arc;

use crate::modules::application::memory_conflict_resolution::ExistingRecordRef;
use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryEntry, MemoryProvider};
use crate::modules::runtime::contracts::memory::{
    MemoryObjectKind, MemoryScope, MemoryWriteCandidate,
};
use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

/// Stable source tag emitted on every candidate this module produces.
/// Pinned so M4 governance traces can filter / count by source.
pub const SOURCE_MEMORY_STORE_TOOL: &str = "memory_store_tool";

/// Source tag for candidates produced by the LLM auto-extraction path.
pub const SOURCE_AUTO_EXTRACT: &str = "auto_extract";

/// Source tag for candidates extracted from assistant output analysis.
pub const SOURCE_ASSISTANT_OUTPUT_EXTRACT: &str = "assistant_output_extract";

/// Maximum content_preview characters surfaced into the candidate.
/// Mirrors the `pending_approval` preview length used by the
/// `memory_store` tool itself so the wire / audit trail stays
/// consistent end-to-end.
const CONTENT_PREVIEW_CHARS: usize = 120;

/// Scan the messages **produced during a single agent turn** for
/// `memory_store` tool invocations and turn each into a typed
/// [`MemoryWriteCandidate`].
///
/// `new_messages` should be the slice of `ConversationMessage`s
/// that were appended during this turn (i.e. the assistant tool
/// calls + any tool result blocks).  Older messages from prior
/// turns MUST be excluded — otherwise the same candidate is
/// re-emitted every turn.
///
/// Honest semantics:
///
/// - We extract `ToolUse` blocks (the assistant's call), not
///   `ToolResult` blocks (the tool's response).  This means the
///   candidate is recorded *intent-side* — the resolver sees what
///   the agent **wanted** to write, regardless of whether the
///   `memory_store` tool's policy engine ultimately accepted /
///   pending'd / denied the write.  The two views are correlated
///   downstream by `evidence_id` (the tool call id).
/// - `object_kind` is derived from the `category` argument when
///   present (`core` / `daily` / `conversation` map to `Fact`,
///   `Preference`, `Episode`, with everything else falling back
///   to `Unknown`).  This is heuristic but stable; M5 may
///   classify upstream.
/// - `scope` is derived from the **caller-supplied execution
///   scope** (most-specific binding wins: session > project >
///   global) — NOT from the tool's args.  The tool itself does
///   the same resolution today.
#[must_use]
pub fn extract_memory_store_tool_candidates(
    new_messages: &[ConversationMessage],
    fallback_scope: &MemoryExecutionScope,
) -> Vec<MemoryWriteCandidate> {
    let mut candidates = Vec::new();
    for msg in new_messages {
        // Only assistant turns can issue tool calls.
        if msg.role != MessageRole::Assistant {
            continue;
        }
        for block in &msg.blocks {
            let ContentBlock::ToolUse { id, name, input } = block else {
                continue;
            };
            if name != "memory_store" {
                continue;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(input).unwrap_or(serde_json::Value::Null);
            let key = parsed
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = parsed
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let category = parsed
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("conversation");

            let object_kind = classify_object_kind(category);
            let scope = candidate_scope_from(fallback_scope);
            let preview = content_preview_for_candidate(&key, &content);

            candidates.push(MemoryWriteCandidate {
                object_kind,
                scope,
                content_preview: preview,
                evidence_id: Some(id.clone()),
                source: SOURCE_MEMORY_STORE_TOOL.to_string(),
            });
        }
    }
    candidates
}

/// Look up an [`ExistingRecordRef`] for each candidate, using the
/// `MemoryProvider` as the backing store.  Returns a vector
/// **parallel to** `candidates` (one slot per candidate, `None`
/// when no prior record was found).
///
/// First-cut strategy:
///
/// 1. Use the candidate `content_preview` prefix (everything up to
///    the first `:` if present, else the whole preview) as the
///    `recall_scoped` query string — this matches the `key`
///    component of the `memory_store` tool's preview format so
///    same-key writes from the same scope return a hit.
/// 2. Take the first returned [`MemoryEntry`] (provider already
///    sorts by recency / relevance).
/// 3. Build a typed [`ExistingRecordRef`] preserving the existing
///    entry's category (mapped to [`MemoryObjectKind`] via the
///    same heuristic as
///    [`extract_memory_store_tool_candidates`]).
///
/// Honest scope: this is intentionally a thin lookup, not a
/// dedup engine.  Embedding similarity, cross-scope crawl, and
/// stable-evidence ranking are M4.4+ work.  The first cut is
/// enough to flip the conflict resolver from
/// `outcome == NoConflict` for **every** candidate to
/// `outcome ∈ {AcceptReplacement, KeepExisting, RequirePrompt,
/// RejectCandidate, NoConflict}` for at least the same-key case.
pub async fn lookup_existing_records_for_candidates(
    provider: &Arc<dyn MemoryProvider>,
    scope: &MemoryExecutionScope,
    candidates: &[MemoryWriteCandidate],
) -> Vec<Option<ExistingRecordRef>> {
    let mut out = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let query = lookup_query_for_candidate(&candidate.content_preview);
        if query.is_empty() {
            out.push(None);
            continue;
        }
        let recall = provider
            .recall_scoped(&query, None, /* limit */ 4, scope)
            .await
            .unwrap_or_default();
        out.push(map_first_hit_to_existing_ref(&recall, candidate));
    }
    out
}

/// Heuristic content-preview format used by both
/// [`extract_memory_store_tool_candidates`] and
/// [`lookup_existing_records_for_candidates`].  Holds the canonical
/// `<key>: <truncated content>` shape so the lookup query can
/// reliably extract the key portion.
fn content_preview_for_candidate(key: &str, content: &str) -> String {
    let mut preview = String::with_capacity(key.len() + 2 + CONTENT_PREVIEW_CHARS);
    if !key.is_empty() {
        preview.push_str(key);
        preview.push_str(": ");
    }
    for (chars, ch) in content.chars().enumerate() {
        if chars >= CONTENT_PREVIEW_CHARS {
            preview.push('…');
            break;
        }
        preview.push(ch);
    }
    preview
}

/// Inverse of the preview format: pull the `key` portion out (the
/// substring before the first `": "`).  Empty if the preview
/// doesn't follow the format (e.g. an external candidate source).
fn lookup_query_for_candidate(preview: &str) -> String {
    if let Some(idx) = preview.find(": ") {
        preview[..idx].trim().to_string()
    } else {
        preview.trim().to_string()
    }
}

/// Map a `memory_store` tool `category` arg to a typed
/// [`MemoryObjectKind`].  Same closed-set mapping as the M3
/// `MemoryObjectKind::Other` fallback path; centralised so the
/// extractor and the lookup agree byte-for-byte.
fn classify_object_kind(category: &str) -> MemoryObjectKind {
    match category {
        "core" => MemoryObjectKind::Fact,
        "daily" => MemoryObjectKind::Preference,
        "conversation" => MemoryObjectKind::Episode,
        _ => MemoryObjectKind::Unknown,
    }
}

/// Translate a most-specific-binding [`MemoryExecutionScope`] into
/// the canonical [`MemoryScope`] the coordinator expects.  Mirrors
/// the [`crate::modules::tools::builtin::memory_store::scope_label`]
/// precedence so candidate scope == final write scope.
fn candidate_scope_from(scope: &MemoryExecutionScope) -> MemoryScope {
    if scope.session_id.is_some() {
        MemoryScope::Session
    } else if scope.project_id.is_some() {
        MemoryScope::Project
    } else {
        MemoryScope::Global
    }
}

/// Maximum characters of assistant text fed into the extraction prompt.
/// Keeps the utility-LLM call within a reasonable token budget.
const ASSISTANT_TEXT_MAX_CHARS: usize = 2000;

/// Minimum assistant text length (in characters) worth analysing.
/// Shorter texts are unlikely to contain meaningful user-profile signals.
const ASSISTANT_TEXT_MIN_CHARS: usize = 20;

/// Extract assistant-message plain text (skipping ToolUse / ToolResult
/// blocks) and concatenate into a single string, truncated to
/// [`ASSISTANT_TEXT_MAX_CHARS`].
fn extract_assistant_text(messages: &[ConversationMessage]) -> String {
    let mut buf = String::new();
    for msg in messages {
        if msg.role != MessageRole::Assistant {
            continue;
        }
        for block in &msg.blocks {
            if let ContentBlock::Text { text } = block {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(text);
            }
            // Skip ToolUse and ToolResult blocks — they carry
            // operational data, not conversational content.
        }
    }
    // Truncate to the budget so the prompt stays compact.
    if buf.len() > ASSISTANT_TEXT_MAX_CHARS {
        let truncation_point = buf
            .char_indices()
            .nth(ASSISTANT_TEXT_MAX_CHARS)
            .map(|(i, _)| i)
            .unwrap_or(buf.len());
        buf.truncate(truncation_point);
    }
    buf
}

/// System prompt used by [`extract_from_assistant_output`] to guide the
/// utility LLM towards user-profile inferences.
const ASSISTANT_EXTRACT_SYSTEM_PROMPT: &str = r#"You are a user-profile inference assistant. Analyze the assistant's response and extract any implicit inferences about the user that are worth remembering.

Return a JSON array where each element has:
- "key": lowercase_snake_case identifier (e.g., "is_developer", "works_on_tauri_project")
- "content": A factual sentence about the user (e.g., "User is working on a Tauri desktop application project")
- "category": one of "core" (identity), "daily" (preferences/habits), "conversation" (contextual)

Rules:
- Extract ONLY inferences about the USER, not about the assistant's actions
- Focus on: user's occupation, skills, projects, goals, technical preferences, communication style
- Ignore: assistant's operational responses, code output, tool result summaries, generic advice
- Only extract facts that are clearly implied or stated, not speculative guesses
- If nothing worth remembering, return []
- Write content in the same language as the assistant's response"#;

/// From Assistant replies, automatically extract inferences about the
/// user's profile (occupation, skills, preferences, projects, etc.).
///
/// Analyses the **plain text** portions of assistant messages
/// (excluding tool-use / tool-result blocks) and uses a utility LLM
/// to identify implicit facts about the user.  This is a complementary
/// extraction path to the explicit `memory_store` tool-call extractor
/// and the user-message extractor.
///
/// `already_stored` carries the `content_preview` strings of candidates
/// that the LLM already explicitly stored via `memory_store` tool calls
/// in this same turn.  The extraction prompt tells the utility LLM to
/// skip facts that overlap with these entries so we avoid duplicates.
///
/// Graceful degradation:
/// - Returns an empty `Vec` when the assistant text is too short.
/// - Returns an empty `Vec` on any LLM error or JSON parse failure.
pub async fn extract_from_assistant_output(
    assistant_messages: &[ConversationMessage],
    llm: &dyn UtilityLlm,
    fallback_scope: &MemoryExecutionScope,
    already_stored: &[String],
) -> Vec<MemoryWriteCandidate> {
    let text = extract_assistant_text(assistant_messages);
    if text.len() < ASSISTANT_TEXT_MIN_CHARS {
        return Vec::new();
    }

    let stored_section = build_already_stored_section(already_stored);
    let user_prompt = format!(
        "Assistant's response:\n{}\n{stored_section}\nExtract user profile inferences as JSON array:",
        text
    );

    let llm_response = match llm
        .complete(ASSISTANT_EXTRACT_SYSTEM_PROMPT, &user_prompt, 512, 0.0)
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            tracing::debug!(
                error = %err,
                "[extract_from_assistant_output] LLM call failed, skipping"
            );
            return Vec::new();
        }
    };

    parse_assistant_extract_response(&llm_response, fallback_scope)
}

/// Parse the LLM JSON response into [`MemoryWriteCandidate`]s.
///
/// Expects a JSON array of `{ "key", "content", "category" }` objects.
/// Invalid / missing fields are silently skipped; a completely
/// unparseable response yields an empty `Vec`.
fn parse_assistant_extract_response(
    raw: &str,
    fallback_scope: &MemoryExecutionScope,
) -> Vec<MemoryWriteCandidate> {
    // The LLM sometimes wraps the array in a code fence — strip it.
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let items: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(err) => {
            tracing::debug!(
                error = %err,
                raw_len = raw.len(),
                "[extract_from_assistant_output] JSON parse failed, returning empty"
            );
            return Vec::new();
        }
    };

    let scope = candidate_scope_from(fallback_scope);
    let mut candidates = Vec::new();
    for item in &items {
        let key = match item.get("key").and_then(|v| v.as_str()) {
            Some(k) if !k.is_empty() => k,
            _ => continue,
        };
        let content = match item.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };
        let category = item
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("conversation");

        let object_kind = classify_object_kind(category);
        let preview = content_preview_for_candidate(key, content);

        candidates.push(MemoryWriteCandidate {
            object_kind,
            scope,
            content_preview: preview,
            evidence_id: None,
            source: SOURCE_ASSISTANT_OUTPUT_EXTRACT.to_string(),
        });
    }
    candidates
}

fn map_first_hit_to_existing_ref(
    hits: &[MemoryEntry],
    candidate: &MemoryWriteCandidate,
) -> Option<ExistingRecordRef> {
    let entry = hits.first()?;
    let object_kind = classify_object_kind(entry.category.as_str());
    let preview = content_preview_for_candidate(&entry.key, &entry.content);
    Some(ExistingRecordRef {
        object_kind,
        scope: candidate.scope,
        content_preview: preview,
        // First-cut: treat any prior record as "stable" with
        // evidence (key was deliberately written via the
        // `memory_store` tool, which already gated through
        // `MemoryPolicyEngine`).  Refinement (real stability
        // signal) lives in M4.4+ when the persistence layer
        // tracks confirmation count.
        stable: true,
        has_evidence: true,
        polarity: None,
    })
}

// ────────────────────────────────────────────────────────────────────────────
// MEM-AUTO-EXTRACT — LLM-based user message extraction
// ────────────────────────────────────────────────────────────────────────────

/// System prompt for the auto-extraction LLM call.
const AUTO_EXTRACT_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Analyze the user's message and extract any personal facts worth remembering long-term.

Return a JSON array where each element has:
- "key": lowercase_snake_case identifier (e.g., "tropical_fish_hobby", "xiaomi_fish_tank")
- "content": A complete sentence describing the fact (e.g., "User enjoys keeping tropical fish")
- "category": one of "core" (identity facts), "daily" (preferences, possessions, habits), "conversation" (contextual facts)

Rules:
- Extract: preferences, hobbies, possessions, devices, pets, family, work, education, habits, goals
- Ignore: questions, commands, technical discussions, temporary requests, code snippets
- If nothing worth remembering, return []
- Keep content concise but self-contained
- Write content in the same language as the user's message"#;

/// Minimum user text length (in chars) below which we skip LLM extraction.
const MIN_USER_TEXT_LEN: usize = 10;

/// Use LLM to extract memorable personal facts from user messages.
///
/// This is a supplementary path to `extract_memory_store_tool_candidates`:
/// even when the agent does not explicitly call `memory_store`, this
/// function can capture preferences, possessions, habits, etc. mentioned
/// by the user.
///
/// `already_stored` carries the `content_preview` strings of candidates
/// that the LLM already explicitly stored via `memory_store` tool calls
/// in this same turn.  The extraction prompt tells the utility LLM to
/// skip facts that overlap with these entries so we avoid duplicates.
///
/// Graceful degradation: returns an empty `Vec` on any LLM or parse
/// failure — never panics, never blocks the main turn flow.
pub async fn extract_from_user_messages(
    user_messages: &[ConversationMessage],
    llm: &dyn UtilityLlm,
    fallback_scope: &MemoryExecutionScope,
    already_stored: &[String],
) -> Vec<MemoryWriteCandidate> {
    // 1. Collect user text blocks.
    let user_text = collect_user_text(user_messages);
    if user_text.len() < MIN_USER_TEXT_LEN {
        return Vec::new();
    }

    // 2. Build the user prompt, including already-stored facts.
    let stored_section = build_already_stored_section(already_stored);
    let user_prompt = format!(
        "User message:\n{}\n{stored_section}\nExtract memorable personal facts as JSON array:",
        user_text
    );

    // 3. Call LLM (max_tokens=512, temperature=0.0).
    let raw_response = match llm
        .complete(AUTO_EXTRACT_SYSTEM_PROMPT, &user_prompt, 512, 0.0)
        .await
    {
        Ok(text) => text,
        Err(e) => {
            tracing::debug!(error = %e, "[auto_extract] LLM call failed, skipping");
            return Vec::new();
        }
    };

    if raw_response.is_empty() {
        return Vec::new();
    }

    // 4. Parse JSON.
    let items = parse_extraction_json(&raw_response);

    // 5. Convert to MemoryWriteCandidate.
    let scope = candidate_scope_from(fallback_scope);
    items
        .into_iter()
        .map(|item| {
            let object_kind = classify_object_kind(&item.category);
            let preview = content_preview_for_candidate(&item.key, &item.content);
            MemoryWriteCandidate {
                object_kind,
                scope,
                content_preview: preview,
                evidence_id: None,
                source: SOURCE_AUTO_EXTRACT.to_string(),
            }
        })
        .collect()
}

/// Build the "already stored facts" section for the extraction prompt.
/// Returns an empty string when `already_stored` is empty so the prompt
/// is unchanged in the common no-overlap case.
fn build_already_stored_section(already_stored: &[String]) -> String {
    if already_stored.is_empty() {
        return String::new();
    }
    let mut section = String::from("\nAlready stored facts (DO NOT extract these again):\n");
    for item in already_stored {
        section.push_str("- ");
        section.push_str(item);
        section.push('\n');
    }
    section
}

/// Concatenate text content from user-role messages.
fn collect_user_text(messages: &[ConversationMessage]) -> String {
    let mut buf = String::new();
    for msg in messages {
        if msg.role != MessageRole::User {
            continue;
        }
        for block in &msg.blocks {
            if let ContentBlock::Text { text } = block {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(text);
            }
        }
    }
    buf
}

/// A single extracted fact from the LLM JSON output.
#[derive(serde::Deserialize)]
struct ExtractedFact {
    #[serde(default)]
    key: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    category: String,
}

/// Parse the LLM response into extracted facts with robust handling
/// of markdown fences and partial failures.
fn parse_extraction_json(raw: &str) -> Vec<ExtractedFact> {
    // Strip markdown code fences if present.
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        // Remove opening fence (```json or ```)
        let after_open = match trimmed.find('\n') {
            Some(idx) => &trimmed[idx + 1..],
            None => return Vec::new(),
        };
        // Remove closing fence
        match after_open.rfind("```") {
            Some(idx) => after_open[..idx].trim(),
            None => after_open.trim(),
        }
    } else {
        trimmed
    };

    serde_json::from_str::<Vec<ExtractedFact>>(json_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

    fn assistant_tool_use(name: &str, input: &str) -> ConversationMessage {
        ConversationMessage {
            role: MessageRole::Assistant,
            blocks: vec![ContentBlock::ToolUse {
                id: "call-1".to_string(),
                name: name.to_string(),
                input: input.to_string(),
            }],
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
            finish_reason: None,
        }
    }

    #[test]
    fn extracts_one_memory_store_call_per_tool_use_block() {
        let scope = MemoryExecutionScope {
            project_id: None,
            session_id: Some("sess-1".to_string()),
            workdir: None,
        };
        let msgs = vec![assistant_tool_use(
            "memory_store",
            r#"{"key":"user_likes_dark_mode","content":"User prefers dark mode in the app","category":"daily"}"#,
        )];
        let cands = extract_memory_store_tool_candidates(&msgs, &scope);
        assert_eq!(cands.len(), 1);
        let c = &cands[0];
        assert_eq!(c.object_kind, MemoryObjectKind::Preference);
        assert_eq!(c.scope, MemoryScope::Session);
        assert_eq!(c.evidence_id.as_deref(), Some("call-1"));
        assert_eq!(c.source, SOURCE_MEMORY_STORE_TOOL);
        assert!(c.content_preview.starts_with("user_likes_dark_mode: "));
    }

    #[test]
    fn skips_non_memory_store_tools_and_user_messages() {
        let scope = MemoryExecutionScope {
            project_id: None,
            session_id: None,
            workdir: None,
        };
        let msgs = vec![
            assistant_tool_use("bash", r#"{"command":"ls"}"#),
            ConversationMessage {
                role: MessageRole::User,
                blocks: vec![ContentBlock::Text {
                    text: "hello".into(),
                }],
                usage: None,
                thinking: None,
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                request_id: None,
                finish_reason: None,
            },
        ];
        assert!(extract_memory_store_tool_candidates(&msgs, &scope).is_empty());
    }

    #[test]
    fn category_to_object_kind_mapping_is_closed() {
        assert_eq!(classify_object_kind("core"), MemoryObjectKind::Fact);
        assert_eq!(classify_object_kind("daily"), MemoryObjectKind::Preference);
        assert_eq!(
            classify_object_kind("conversation"),
            MemoryObjectKind::Episode
        );
        assert_eq!(
            classify_object_kind("custom_xyz"),
            MemoryObjectKind::Unknown
        );
    }

    #[test]
    fn lookup_query_strips_key_prefix() {
        assert_eq!(
            lookup_query_for_candidate("user_pref: prefers dark mode"),
            "user_pref"
        );
        // No `": "` separator → fall back to whole preview.
        assert_eq!(lookup_query_for_candidate("just-a-string"), "just-a-string");
    }
}
