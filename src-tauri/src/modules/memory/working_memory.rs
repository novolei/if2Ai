//! Working memory with sliding window eviction
//!
//! Maintains a bounded window of recent conversation turns,
//! evicting oldest messages when turn count or token budget is exceeded.

use crate::modules::runtime::budget::estimate_tokens;
use crate::modules::runtime::session::ConversationMessage;

/// Working memory uses a sliding window of recent turns
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    /// Recent conversation messages
    pub turns: Vec<ConversationMessage>,
    /// Maximum number of turns to retain (default: 8)
    pub max_turns: usize,
    /// Maximum token budget for working memory (default: 1600)
    pub max_tokens: usize,
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self {
            turns: Vec::new(),
            max_turns: 8,
            max_tokens: 1600,
        }
    }
}

impl WorkingMemory {
    /// Create a new working memory with the given limits
    #[allow(dead_code)] // Public API; callers in streaming path (fix-streaming-parity)
    pub fn new(max_turns: usize, max_tokens: usize) -> Self {
        Self {
            turns: Vec::new(),
            max_turns,
            max_tokens,
        }
    }

    /// Add a message and evict oldest entries if limits are exceeded
    #[allow(dead_code)] // Public API; will be called once streaming path is wired (fix-streaming-parity)
    pub fn push(&mut self, message: ConversationMessage) {
        self.turns.push(message);
        self.evict_if_needed();
    }

    /// Add multiple messages and evict if needed
    pub fn extend(&mut self, messages: impl IntoIterator<Item = ConversationMessage>) {
        self.turns.extend(messages);
        self.evict_if_needed();
    }

    /// Total token count across all retained messages
    pub fn token_count(&self) -> usize {
        self.turns.iter().map(message_token_count).sum()
    }

    /// Current number of retained messages
    #[allow(dead_code)] // Public API; used in tests and future telemetry
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    /// Whether working memory is empty
    #[allow(dead_code)] // Public API; used in tests and future checks
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Get all retained messages
    pub fn messages(&self) -> &[ConversationMessage] {
        &self.turns
    }

    /// Clear all retained messages
    pub fn clear(&mut self) {
        self.turns.clear();
    }

    fn evict_if_needed(&mut self) {
        // Evict by turn count limit
        while self.turns.len() > self.max_turns {
            self.turns.remove(0);
        }
        // Evict by token budget
        while self.token_count() > self.max_tokens && !self.turns.is_empty() {
            self.turns.remove(0);
        }
    }
}

/// Estimate token count for a single conversation message
#[must_use]
pub fn message_token_count(msg: &ConversationMessage) -> usize {
    let mut count = 0;
    for block in &msg.blocks {
        match block {
            crate::modules::runtime::session::ContentBlock::Text { text } => {
                count += estimate_tokens(text);
            }
            crate::modules::runtime::session::ContentBlock::ToolUse {
                id, name, input, ..
            } => {
                count += estimate_tokens(id);
                count += estimate_tokens(name);
                count += estimate_tokens(input);
            }
            crate::modules::runtime::session::ContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                ..
            } => {
                count += estimate_tokens(tool_use_id);
                count += estimate_tokens(tool_name);
                count += estimate_tokens(output);
            }
        }
    }
    if let Some(thinking) = &msg.thinking {
        count += estimate_tokens(thinking);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::session::{ContentBlock, MessageRole};

    fn text_message(role: MessageRole, text: &str) -> ConversationMessage {
        ConversationMessage {
            role,
            blocks: vec![ContentBlock::Text {
                text: text.to_string(),
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
    fn default_limits() {
        let wm = WorkingMemory::default();
        assert_eq!(wm.max_turns, 8);
        assert_eq!(wm.max_tokens, 1600);
    }

    #[test]
    fn push_preserves_order() {
        let mut wm = WorkingMemory::new(10, 10000);
        wm.push(text_message(MessageRole::User, "hello"));
        wm.push(text_message(MessageRole::Assistant, "hi there"));
        assert_eq!(wm.len(), 2);
        assert_eq!(
            wm.messages()[0].blocks[0],
            ContentBlock::Text {
                text: "hello".to_string()
            }
        );
    }

    #[test]
    fn evicts_by_turn_limit() {
        let mut wm = WorkingMemory::new(3, 10000);
        for i in 0..5 {
            wm.push(text_message(MessageRole::User, &format!("msg_{i}")));
        }
        assert_eq!(wm.len(), 3);
        // Oldest messages should be removed
        assert!(wm.messages()[0]
            .blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Text { text } if text == "msg_2")));
    }

    #[test]
    fn evicts_by_token_limit() {
        // 200 token limit — each message is ~50+ chars = ~14 tokens
        let mut wm = WorkingMemory::new(100, 40);
        for i in 0..10 {
            let content = format!("message number {i} with some content");
            wm.push(text_message(MessageRole::User, &content));
        }
        assert!(wm.token_count() <= 40);
    }

    #[test]
    fn token_count_accuracy() {
        let mut wm = WorkingMemory::new(100, 10000);
        wm.push(text_message(MessageRole::User, "hello"));
        wm.push(text_message(MessageRole::Assistant, "world"));
        // M2: estimate_tokens now uses cl100k_base BPE; "hello" and "world"
        // each encode to a single token, so the total falls in [2, 4] depending
        // on whether any role/wrapper tokens are added in the future. We assert
        // a sensible upper bound rather than an exact char/4 value.
        let n = wm.token_count();
        assert!((1..=4).contains(&n), "expected 1..=4 tokens, got {n}");
    }

    #[test]
    fn clear_empties() {
        let mut wm = WorkingMemory::default();
        wm.push(text_message(MessageRole::User, "hello"));
        wm.clear();
        assert!(wm.is_empty());
    }

    #[test]
    fn extend_adds_multiple() {
        let mut wm = WorkingMemory::new(10, 10000);
        let msgs = vec![
            text_message(MessageRole::User, "a"),
            text_message(MessageRole::Assistant, "b"),
        ];
        wm.extend(msgs);
        assert_eq!(wm.len(), 2);
    }
}
