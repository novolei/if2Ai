//! Timeline flush helper: collapse accumulated text/thinking into a
//! single assistant ConversationMessage and push it onto the timeline.
//!
//! Extracted from `commands/agent.rs` in GFR-005c (pure structural
//! move; struct fields and function body byte-identical).

use crate::modules::runtime::session::ContentBlock;

#[derive(Debug, Clone)]
pub(crate) struct PersistedTurnOutcome {
    pub(crate) task_outcome: String,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) resume_available: bool,
    pub(crate) resume_cursor: Option<String>,
    pub(crate) request_id: String,
}

pub(crate) fn flush_assistant_timeline_segment(
    timeline_messages: &mut Vec<crate::modules::runtime::session::ConversationMessage>,
    accumulated_text: &mut String,
    accumulated_thinking: &mut String,
    persisted_outcome: Option<&PersistedTurnOutcome>,
) -> bool {
    if accumulated_text.is_empty() && accumulated_thinking.is_empty() {
        return false;
    }

    let text = std::mem::take(accumulated_text);
    let thinking = std::mem::take(accumulated_thinking);

    timeline_messages.push(crate::modules::runtime::session::ConversationMessage {
        role: crate::modules::runtime::session::MessageRole::Assistant,
        blocks: vec![ContentBlock::Text { text }],
        usage: None,
        thinking: if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        },
        task_outcome: persisted_outcome.map(|value| value.task_outcome.clone()),
        degraded_reason: persisted_outcome.and_then(|value| value.degraded_reason.clone()),
        resume_available: persisted_outcome.map(|value| value.resume_available),
        resume_cursor: persisted_outcome.and_then(|value| value.resume_cursor.clone()),
        request_id: persisted_outcome.map(|value| value.request_id.clone()),
        finish_reason: None,
    });

    true
}
