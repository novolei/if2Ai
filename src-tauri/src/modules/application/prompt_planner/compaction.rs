//! Compaction policy for long coding sessions.
//!
//! MIG-008: Determines when to generate continuation blocks and builds
//! continuation summaries for context preservation.

use super::block::{PromptBlock, PromptBlockKind, PromptBlockSource};

/// Default thresholds for coding session compaction.
const DEFAULT_TOKEN_THRESHOLD: usize = 4000;
const DEFAULT_MESSAGE_THRESHOLD: usize = 16;

/// Compaction policy for determining when to generate continuation blocks.
pub(super) struct CompactionPolicy {
    token_threshold: usize,
    message_threshold: usize,
}

impl CompactionPolicy {
    /// Create a new compaction policy with custom thresholds.
    pub(super) fn new(token_threshold: Option<usize>, message_threshold: Option<usize>) -> Self {
        Self {
            token_threshold: token_threshold.unwrap_or(DEFAULT_TOKEN_THRESHOLD),
            message_threshold: message_threshold.unwrap_or(DEFAULT_MESSAGE_THRESHOLD),
        }
    }

    /// Check if compaction should be triggered based on session metrics.
    pub(super) fn should_compact(
        &self,
        estimated_tokens: Option<usize>,
        message_count: Option<usize>,
    ) -> bool {
        let token_exceeded = estimated_tokens
            .map(|t| t >= self.token_threshold)
            .unwrap_or(false);
        let message_exceeded = message_count
            .map(|m| m >= self.message_threshold)
            .unwrap_or(false);

        token_exceeded || message_exceeded
    }
}

/// Build a continuation block for long coding sessions.
///
/// MIG-008: Generates a continuation block that summarizes the current
/// state of the coding session, including current work, pending tasks,
/// and key files.
///
/// The session_snapshot is expected to be a simple text summary provided
/// by the caller. In a full implementation, this would parse structured
/// session data.
pub(super) fn build_coding_continuation_block(session_snapshot: &str) -> PromptBlock {
    let content = if session_snapshot.is_empty() {
        "# Session Continuation\n\nThis is a continuation of a long coding session. \
         Previous context has been compacted to preserve token budget."
            .to_string()
    } else {
        format!(
            "# Session Continuation\n\n{}\n\n\
             Previous context has been compacted. Focus on the current task.",
            session_snapshot
        )
    };

    PromptBlock {
        id: "coding_continuation".to_string(),
        kind: PromptBlockKind::Continuation,
        title: "coding_continuation".to_string(),
        content,
        source: PromptBlockSource {
            subsystem: "compaction".to_string(),
            reference: None,
        },
        priority: 50,
        is_sensitive: false,
    }
}
