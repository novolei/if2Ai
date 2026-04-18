//! Context compaction for session management
//!
//! This implementation is adapted from:
//! - iClaw/agent/context_compressor.py (UClaw)
//! - hermes-agent/agent/context_compressor.py
//!
//! Changes from upstream:
//! - Rust async/await instead of Python asyncio
//! - Token counting using simple char-based estimation instead of tiktoken
//! - SessionManager integration for persistence
//! - Weibull decay with configurable lambda/k parameters

#![allow(dead_code)]

use super::session::{ContentBlock, ConversationMessage, MessageRole, Session};

const COMPACT_CONTINUATION_PREAMBLE: &str =
    "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\n";
const COMPACT_RECENT_MESSAGES_NOTE: &str = "Recent messages are preserved verbatim.";
const COMPACT_DIRECT_RESUME_INSTRUCTION: &str = "Continue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, and do not preface with continuation text.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionConfig {
    pub preserve_recent_messages: usize,
    pub max_estimated_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preserve_recent_messages: 4,
            max_estimated_tokens: 10_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionResult {
    pub summary: String,
    pub formatted_summary: String,
    pub compacted_session: Session,
    pub removed_message_count: usize,
}

/// Sum [`estimate_message_tokens`] across every message in the session
/// to produce a coarse running token estimate (used by the compaction
/// trigger and the context-budget header in the chat UI).
#[must_use]
pub fn estimate_session_tokens(session: &Session) -> usize {
    session.messages.iter().map(estimate_message_tokens).sum()
}

/// Estimate token count from a character count.
///
/// Retained as a fallback for legacy call sites that only have a length
/// (e.g. aggregated character counters). Prefer the text-aware
/// [`crate::modules::runtime::budget::estimate_tokens`] when the underlying
/// string is available — it uses `cl100k_base` BPE for accurate counting (M2).
#[must_use]
pub fn estimate_token_count_from_chars(char_count: usize) -> usize {
    char_count / 4 + 1
}

/// Decide whether `session` is large enough to trigger compaction.
///
/// Returns `true` when the messages outside any prior compaction summary
/// exceed both the recent-message buffer and the configured token budget.
#[must_use]
pub fn should_compact(session: &Session, config: CompactionConfig) -> bool {
    let start = compacted_summary_prefix_len(session);
    let compactable = &session.messages[start..];

    compactable.len() > config.preserve_recent_messages
        && compactable
            .iter()
            .map(estimate_message_tokens)
            .sum::<usize>()
            >= config.max_estimated_tokens
}

/// Strip the `<analysis>...</analysis>` block and unwrap the
/// `<summary>...</summary>` block from a model-produced compact summary,
/// returning the user-visible Markdown.
#[must_use]
pub fn format_compact_summary(summary: &str) -> String {
    let without_analysis = strip_tag_block(summary, "analysis");
    let formatted = if let Some(content) = extract_tag_block(&without_analysis, "summary") {
        without_analysis.replace(
            &format!("<summary>{content}</summary>"),
            &format!("Summary:\n{}", content.trim()),
        )
    } else {
        without_analysis
    };

    collapse_blank_lines(&formatted).trim().to_string()
}

/// Build the assistant continuation message that primes the next turn after
/// compaction.  Combines the formatted summary with optional reminders about
/// preserved tail messages and follow-up-question suppression.
#[must_use]
pub fn get_compact_continuation_message(
    summary: &str,
    suppress_follow_up_questions: bool,
    recent_messages_preserved: bool,
) -> String {
    let mut base = format!(
        "{COMPACT_CONTINUATION_PREAMBLE}{}",
        format_compact_summary(summary)
    );

    if recent_messages_preserved {
        base.push_str("\n\n");
        base.push_str(COMPACT_RECENT_MESSAGES_NOTE);
    }

    if suppress_follow_up_questions {
        base.push('\n');
        base.push_str(COMPACT_DIRECT_RESUME_INSTRUCTION);
    }

    base
}

/// Run synchronous compaction over `session`: emit a placeholder summary
/// (the LLM call is performed elsewhere) and return the trimmed session
/// plus the count of removed messages.
#[must_use]
pub fn compact_session(session: &Session, config: CompactionConfig) -> CompactionResult {
    if !should_compact(session, config) {
        return CompactionResult {
            summary: String::new(),
            formatted_summary: String::new(),
            compacted_session: session.clone(),
            removed_message_count: 0,
        };
    }

    let existing_summary = session
        .messages
        .first()
        .and_then(extract_existing_compacted_summary);
    let compacted_prefix_len = usize::from(existing_summary.is_some());
    let keep_from = session
        .messages
        .len()
        .saturating_sub(config.preserve_recent_messages);
    let removed = &session.messages[compacted_prefix_len..keep_from];
    let preserved = session.messages[keep_from..].to_vec();
    let summary =
        merge_compact_summaries(existing_summary.as_deref(), &summarize_messages(removed));
    let formatted_summary = format_compact_summary(&summary);
    let continuation = get_compact_continuation_message(&summary, true, !preserved.is_empty());

    let mut compacted_messages = vec![ConversationMessage {
        role: MessageRole::System,
        blocks: vec![ContentBlock::Text { text: continuation }],
        usage: None,
        thinking: None,
        task_outcome: None,
        degraded_reason: None,
        resume_available: None,
        resume_cursor: None,
        request_id: None,
    }];
    compacted_messages.extend(preserved);

    CompactionResult {
        summary,
        formatted_summary,
        compacted_session: Session {
            version: session.version,
            messages: compacted_messages,
        },
        removed_message_count: removed.len(),
    }
}

fn compacted_summary_prefix_len(session: &Session) -> usize {
    usize::from(
        session
            .messages
            .first()
            .and_then(extract_existing_compacted_summary)
            .is_some(),
    )
}

fn summarize_messages(messages: &[ConversationMessage]) -> String {
    let user_messages = messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .count();
    let assistant_messages = messages
        .iter()
        .filter(|message| message.role == MessageRole::Assistant)
        .count();
    let tool_messages = messages
        .iter()
        .filter(|message| message.role == MessageRole::Tool)
        .count();

    let mut tool_names = messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolUse { name, .. } => Some(name.as_str()),
            ContentBlock::ToolResult { tool_name, .. } => Some(tool_name.as_str()),
            ContentBlock::Text { .. } => None,
        })
        .collect::<Vec<_>>();
    tool_names.sort_unstable();
    tool_names.dedup();

    let mut lines = vec![
        "<summary>".to_string(),
        "Conversation summary:".to_string(),
        format!(
            "- Scope: {} earlier messages compacted (user={}, assistant={}, tool={}).",
            messages.len(),
            user_messages,
            assistant_messages,
            tool_messages
        ),
    ];

    if !tool_names.is_empty() {
        lines.push(format!("- Tools mentioned: {}.", tool_names.join(", ")));
    }

    let recent_user_requests = collect_recent_role_summaries(messages, MessageRole::User, 3);
    if !recent_user_requests.is_empty() {
        lines.push("- Recent user requests:".to_string());
        lines.extend(
            recent_user_requests
                .into_iter()
                .map(|request| format!("  - {request}")),
        );
    }

    let pending_work = infer_pending_work(messages);
    if !pending_work.is_empty() {
        lines.push("- Pending work:".to_string());
        lines.extend(pending_work.into_iter().map(|item| format!("  - {item}")));
    }

    let key_files = collect_key_files(messages);
    if !key_files.is_empty() {
        lines.push(format!("- Key files referenced: {}.", key_files.join(", ")));
    }

    if let Some(current_work) = infer_current_work(messages) {
        lines.push(format!("- Current work: {current_work}"));
    }

    lines.push("- Key timeline:".to_string());
    for message in messages {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        let content = message
            .blocks
            .iter()
            .map(summarize_block)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("  - {role}: {content}"));
    }
    lines.push("</summary>".to_string());
    lines.join("\n")
}

fn merge_compact_summaries(existing_summary: Option<&str>, new_summary: &str) -> String {
    let Some(existing_summary) = existing_summary else {
        return new_summary.to_string();
    };

    let previous_highlights = extract_summary_highlights(existing_summary);
    let new_formatted_summary = format_compact_summary(new_summary);
    let new_highlights = extract_summary_highlights(&new_formatted_summary);
    let new_timeline = extract_summary_timeline(&new_formatted_summary);

    let mut lines = vec!["<summary>".to_string(), "Conversation summary:".to_string()];

    if !previous_highlights.is_empty() {
        lines.push("- Previously compacted context:".to_string());
        lines.extend(
            previous_highlights
                .into_iter()
                .map(|line| format!("  {line}")),
        );
    }

    if !new_highlights.is_empty() {
        lines.push("- Newly compacted context:".to_string());
        lines.extend(new_highlights.into_iter().map(|line| format!("  {line}")));
    }

    if !new_timeline.is_empty() {
        lines.push("- Key timeline:".to_string());
        lines.extend(new_timeline.into_iter().map(|line| format!("  {line}")));
    }

    lines.push("</summary>".to_string());
    lines.join("\n")
}

fn summarize_block(block: &ContentBlock) -> String {
    let raw = match block {
        ContentBlock::Text { text } => text.clone(),
        ContentBlock::ToolUse { name, input, .. } => format!("tool_use {name}({input})"),
        ContentBlock::ToolResult {
            tool_name,
            output,
            is_error,
            ..
        } => format!(
            "tool_result {tool_name}: {}{output}",
            if *is_error { "error " } else { "" }
        ),
    };
    truncate_summary(&raw, 160)
}

fn collect_recent_role_summaries(
    messages: &[ConversationMessage],
    role: MessageRole,
    limit: usize,
) -> Vec<String> {
    messages
        .iter()
        .filter(|message| message.role == role)
        .rev()
        .filter_map(|message| first_text_block(message))
        .take(limit)
        .map(|text| truncate_summary(text, 160))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn infer_pending_work(messages: &[ConversationMessage]) -> Vec<String> {
    messages
        .iter()
        .rev()
        .filter_map(first_text_block)
        .filter(|text| {
            let lowered = text.to_ascii_lowercase();
            lowered.contains("todo")
                || lowered.contains("next")
                || lowered.contains("pending")
                || lowered.contains("follow up")
                || lowered.contains("remaining")
        })
        .take(3)
        .map(|text| truncate_summary(text, 160))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn collect_key_files(messages: &[ConversationMessage]) -> Vec<String> {
    let mut files = messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .map(|block| match block {
            ContentBlock::Text { text } => text.as_str(),
            ContentBlock::ToolUse { input, .. } => input.as_str(),
            ContentBlock::ToolResult { output, .. } => output.as_str(),
        })
        .flat_map(extract_file_candidates)
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files.into_iter().take(8).collect()
}

fn infer_current_work(messages: &[ConversationMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter_map(first_text_block)
        .find(|text| !text.trim().is_empty())
        .map(|text| truncate_summary(text, 200))
}

fn first_text_block(message: &ConversationMessage) -> Option<&str> {
    message.blocks.iter().find_map(|block| match block {
        ContentBlock::Text { text } if !text.trim().is_empty() => Some(text.as_str()),
        ContentBlock::ToolUse { .. }
        | ContentBlock::ToolResult { .. }
        | ContentBlock::Text { .. } => None,
    })
}

fn has_interesting_extension(candidate: &str) -> bool {
    std::path::Path::new(candidate)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["rs", "ts", "tsx", "js", "json", "md"]
                .iter()
                .any(|expected| extension.eq_ignore_ascii_case(expected))
        })
}

fn extract_file_candidates(content: &str) -> Vec<String> {
    content
        .split_whitespace()
        .filter_map(|token| {
            let candidate = token.trim_matches(|char: char| {
                matches!(char, ',' | '.' | ':' | ';' | ')' | '(' | '"' | '\'' | '`')
            });
            if candidate.contains('/') && has_interesting_extension(candidate) {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn truncate_summary(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let mut truncated = content.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}

fn estimate_message_tokens(message: &ConversationMessage) -> usize {
    use crate::modules::runtime::budget::estimate_tokens;

    // M2: use the BPE-backed estimator. Concatenation is fine because BPE token
    // counts are sub-additive: combining two strings can never inflate the count
    // beyond the sum of their individual token counts.
    message
        .blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => estimate_tokens(text),
            ContentBlock::ToolUse { name, input, .. } => {
                estimate_tokens(name) + estimate_tokens(input)
            }
            ContentBlock::ToolResult {
                tool_name, output, ..
            } => estimate_tokens(tool_name) + estimate_tokens(output),
        })
        .sum()
}

fn extract_tag_block(content: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let start_index = content.find(&start)? + start.len();
    let end_index = content[start_index..].find(&end)? + start_index;
    Some(content[start_index..end_index].to_string())
}

fn strip_tag_block(content: &str, tag: &str) -> String {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    if let (Some(start_index), Some(end_index_rel)) = (content.find(&start), content.find(&end)) {
        let end_index = end_index_rel + end.len();
        let mut stripped = String::new();
        stripped.push_str(&content[..start_index]);
        stripped.push_str(&content[end_index..]);
        stripped
    } else {
        content.to_string()
    }
}

fn collapse_blank_lines(content: &str) -> String {
    let mut result = String::new();
    let mut last_blank = false;
    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && last_blank {
            continue;
        }
        result.push_str(line);
        result.push('\n');
        last_blank = is_blank;
    }
    result
}

fn extract_existing_compacted_summary(message: &ConversationMessage) -> Option<String> {
    if message.role != MessageRole::System {
        return None;
    }

    let text = first_text_block(message)?;
    let summary = text.strip_prefix(COMPACT_CONTINUATION_PREAMBLE)?;
    let summary = summary
        .split_once(&format!("\n\n{COMPACT_RECENT_MESSAGES_NOTE}"))
        .map_or(summary, |(value, _)| value);
    let summary = summary
        .split_once(&format!("\n{COMPACT_DIRECT_RESUME_INSTRUCTION}"))
        .map_or(summary, |(value, _)| value);
    Some(summary.trim().to_string())
}

fn extract_summary_highlights(summary: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_timeline = false;

    for line in format_compact_summary(summary).lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed == "Summary:" || trimmed == "Conversation summary:" {
            continue;
        }
        if trimmed == "- Key timeline:" {
            in_timeline = true;
            continue;
        }
        if in_timeline {
            continue;
        }
        lines.push(trimmed.to_string());
    }

    lines
}

fn extract_summary_timeline(summary: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_timeline = false;

    for line in format_compact_summary(summary).lines() {
        let trimmed = line.trim_end();
        if trimmed == "- Key timeline:" {
            in_timeline = true;
            continue;
        }
        if !in_timeline {
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        lines.push(trimmed.to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use crate::modules::runtime::compact::{
        collect_key_files, compact_session, estimate_session_tokens, format_compact_summary,
        get_compact_continuation_message, infer_pending_work, should_compact, CompactionConfig,
    };
    use crate::modules::runtime::session::{
        ContentBlock, ConversationMessage, MessageRole, Session,
    };

    #[test]
    fn formats_compact_summary_like_upstream() {
        let summary = "<analysis>scratch</analysis>\n<summary>Kept work</summary>";
        assert_eq!(format_compact_summary(summary), "Summary:\nKept work");
    }

    #[test]
    fn leaves_small_sessions_unchanged() {
        let session = Session {
            version: 1,
            messages: vec![ConversationMessage::user_text("hello")],
        };

        let result = compact_session(&session, CompactionConfig::default());
        assert_eq!(result.removed_message_count, 0);
        assert_eq!(result.compacted_session, session);
        assert!(result.summary.is_empty());
        assert!(result.formatted_summary.is_empty());
    }

    #[test]
    fn compacts_older_messages_into_a_system_summary() {
        let session = Session {
            version: 1,
            messages: vec![
                ConversationMessage::user_text("one ".repeat(200)),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "two ".repeat(200),
                }]),
                ConversationMessage::tool_result("1", "bash", "ok ".repeat(200), false),
                ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::Text {
                        text: "recent".to_string(),
                    }],
                    thinking: None,
                    usage: None,
                    task_outcome: None,
                    degraded_reason: None,
                    resume_available: None,
                    resume_cursor: None,
                    request_id: None,
                },
            ],
        };

        let result = compact_session(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            },
        );

        assert_eq!(result.removed_message_count, 2);
        assert_eq!(
            result.compacted_session.messages[0].role,
            MessageRole::System
        );
        assert!(matches!(
            &result.compacted_session.messages[0].blocks[0],
            ContentBlock::Text { text } if text.contains("Summary:")
        ));
        assert!(result.formatted_summary.contains("Scope:"));
        assert!(result.formatted_summary.contains("Key timeline:"));
        assert!(should_compact(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            }
        ));
        assert!(
            estimate_session_tokens(&result.compacted_session) < estimate_session_tokens(&session)
        );
    }

    #[test]
    fn keeps_previous_compacted_context_when_compacting_again() {
        let initial_session = Session {
            version: 1,
            messages: vec![
                ConversationMessage::user_text("Investigate rust/crates/runtime/src/compact.rs"),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "I will inspect the compact flow.".to_string(),
                }]),
                ConversationMessage::user_text(
                    "Also update rust/crates/runtime/src/conversation.rs",
                ),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "Next: preserve prior summary context during auto compact.".to_string(),
                }]),
            ],
        };
        let config = CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        };

        let first = compact_session(&initial_session, config);
        let mut follow_up_messages = first.compacted_session.messages.clone();
        follow_up_messages.extend([
            ConversationMessage::user_text("Please add regression tests for compaction."),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "Working on regression coverage now.".to_string(),
            }]),
        ]);

        let second = compact_session(
            &Session {
                version: 1,
                messages: follow_up_messages,
            },
            config,
        );

        assert!(second
            .formatted_summary
            .contains("Previously compacted context:"));
        assert!(second
            .formatted_summary
            .contains("Scope: 2 earlier messages compacted"));
        assert!(second
            .formatted_summary
            .contains("Newly compacted context:"));
        assert!(second
            .formatted_summary
            .contains("Also update rust/crates/runtime/src/conversation.rs"));
        assert!(matches!(
            &second.compacted_session.messages[0].blocks[0],
            ContentBlock::Text { text }
                if text.contains("Previously compacted context:")
                    && text.contains("Newly compacted context:")
        ));
        assert!(matches!(
            &second.compacted_session.messages[1].blocks[0],
            ContentBlock::Text { text } if text.contains("Please add regression tests for compaction.")
        ));
    }

    #[test]
    fn ignores_existing_compacted_summary_when_deciding_to_recompact() {
        let summary = "<summary>Conversation summary:\n- Scope: earlier work preserved.\n- Key timeline:\n  - user: large preserved context\n</summary>";
        let session = Session {
            version: 1,
            messages: vec![
                ConversationMessage {
                    role: MessageRole::System,
                    blocks: vec![ContentBlock::Text {
                        text: get_compact_continuation_message(summary, true, true),
                    }],
                    thinking: None,
                    usage: None,
                    task_outcome: None,
                    degraded_reason: None,
                    resume_available: None,
                    resume_cursor: None,
                    request_id: None,
                },
                ConversationMessage::user_text("tiny"),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "recent".to_string(),
                }]),
            ],
        };

        assert!(!should_compact(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            }
        ));
    }

    #[test]
    fn truncates_long_blocks_in_summary() {
        let summary = super::summarize_block(&ContentBlock::Text {
            text: "x".repeat(400),
        });
        assert!(summary.ends_with('…'));
        assert!(summary.chars().count() <= 161);
    }

    #[test]
    fn extracts_key_files_from_message_content() {
        let files = collect_key_files(&[ConversationMessage::user_text(
            "Update rust/crates/runtime/src/compact.rs and rust/crates/tools/src/lib.rs next.",
        )]);
        assert!(files.contains(&"rust/crates/runtime/src/compact.rs".to_string()));
        assert!(files.contains(&"rust/crates/tools/src/lib.rs".to_string()));
    }

    #[test]
    fn infers_pending_work_from_recent_messages() {
        let pending = infer_pending_work(&[
            ConversationMessage::user_text("done"),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "Next: update tests and follow up on remaining CLI polish.".to_string(),
            }]),
        ]);
        assert_eq!(pending.len(), 1);
        assert!(pending[0].contains("Next: update tests"));
    }
}
