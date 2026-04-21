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
//! - Assistant-output extraction (`object_kind` inference from
//!   plain text), reflection-note → candidate adapter, and tool
//!   trace extraction beyond `memory_store` — these belong to
//!   later M4 / M5 slices.
//! - Cross-scope dedup, embedding-based similarity, persistent
//!   per-record evidence registry — left for the M3-B+ persistence
//!   wiring referenced in the M3 audit doc.

#![allow(dead_code)]

use std::sync::Arc;

use crate::modules::application::memory_conflict_resolution::ExistingRecordRef;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryEntry, MemoryProvider};
use crate::modules::runtime::contracts::memory::{
    MemoryObjectKind, MemoryScope, MemoryWriteCandidate,
};
use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

/// Stable source tag emitted on every candidate this module produces.
/// Pinned so M4 governance traces can filter / count by source.
pub const SOURCE_MEMORY_STORE_TOOL: &str = "memory_store_tool";

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
    let mut chars = 0usize;
    for ch in content.chars() {
        if chars >= CONTENT_PREVIEW_CHARS {
            preview.push('…');
            break;
        }
        preview.push(ch);
        chars += 1;
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
        let mut msgs = Vec::new();
        msgs.push(assistant_tool_use("bash", r#"{"command":"ls"}"#));
        msgs.push(ConversationMessage {
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
        });
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
