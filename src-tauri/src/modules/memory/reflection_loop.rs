//! MEM-MOD-P5 — Self-Reflection Auto-Loop.
//!
//! Every `reflection_threshold` user turns the [`MemoryTicker`] asks
//! the utility LLM to summarise *what it learned about the user / the
//! task* in this stretch and persists the answer as a `Reflection`-
//! category memory.  These reflections feed:
//!
//! - the per-turn `RetrievedMemory` block (so the LLM remembers
//!   patterns it noticed about the user),
//! - the cross-session persona accumulator landing in MEM-MOD-P7
//!   (`learned_traits`),
//! - the strategy registry's "candidate" pipeline (a future Pack can
//!   trigger automatic strategy promotion when several reflections
//!   converge on the same observation).
//!
//! This module is the **logic** layer:
//! - [`build_reflection_prompt`] renders the (system, user) tuple,
//! - [`synthesize_and_persist`] runs the LLM + writes the memory.
//!
//! [`MemoryTicker`] owns the **scheduling** layer (counter + spawn +
//! re-entrancy gate).  Splitting them keeps the heavy cross-module
//! coupling (LLM + memory provider) out of `ticker/turn_hook.rs`.
//!
//! Failures here are non-fatal — a missed reflection just delays the
//! self-evolution feedback loop by `reflection_threshold` turns.
//! Errors are logged via `tracing::warn!`.

use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryCategory, MemoryError, SharedMemoryProvider};
use crate::modules::runtime::session::ConversationMessage;

/// Maximum length, in chars, of any single message we splice into the
/// reflection prompt.  Cheap protection against pathological huge
/// pastes blowing past the LLM context window.
const PER_MESSAGE_CHAR_CAP: usize = 1200;

/// Build the (system, user) prompt the reflection LLM call sees.
///
/// `recent_messages` should be the last ~10–20 turns the agent has
/// just completed; older history is the rolling-summary's job.
#[must_use]
pub fn build_reflection_prompt(
    session_id: &str,
    recent_messages: &[ConversationMessage],
) -> (String, String) {
    let system = "You are the agent's reflection loop. Read the recent conversation \
and write 1–3 short observations about the USER (preferences, goals, \
recurring patterns) or about your OWN behaviour (mistakes you should \
avoid, recipes that worked).  Output ONLY a single paragraph in the \
agent's first-person voice.  Avoid speculation; cite specifics from \
the transcript.  Skip greetings / pleasantries.  Aim for 60–180 words."
        .to_string();

    let mut user = String::new();
    user.push_str(&format!("SESSION: {session_id}\n\nRECENT TRANSCRIPT:\n"));
    if recent_messages.is_empty() {
        user.push_str("(no messages)\n");
    } else {
        for (i, msg) in recent_messages.iter().enumerate() {
            let body = message_summary(msg);
            user.push_str(&format!("{i}. [{:?}] {body}\n", msg.role));
        }
    }
    user.push_str("\nWrite the reflection paragraph now.");
    (system, user)
}

/// One round of reflection: build the prompt, call the LLM, and persist
/// the answer as a `Reflection`-category memory under
/// `reflection-<session_id>-<unix_secs>` so future `memory_recall`
/// queries can surface it.
///
/// Returns the persisted memory key on success, or `Err` when the LLM
/// call or memory write fails.
pub async fn synthesize_and_persist<L: UtilityLlm + ?Sized>(
    llm: &L,
    memory: &SharedMemoryProvider,
    scope: &MemoryExecutionScope,
    session_id: &str,
    recent_messages: &[ConversationMessage],
) -> Result<String, MemoryError> {
    let (system, user) = build_reflection_prompt(session_id, recent_messages);
    let raw = llm.complete(&system, &user, 384, 0.4).await?;
    let body = raw.trim();
    if body.is_empty() {
        return Err(MemoryError::Generic(
            "reflection LLM returned empty body".into(),
        ));
    }

    let key = format!("reflection-{session_id}-{}", chrono::Utc::now().timestamp());
    memory
        .store_scoped(&key, body, MemoryCategory::Reflection, scope)
        .await?;
    Ok(key)
}

fn message_summary(msg: &ConversationMessage) -> String {
    use crate::modules::runtime::session::ContentBlock;
    let mut text_chunks: Vec<&str> = Vec::new();
    for block in &msg.blocks {
        if let ContentBlock::Text { text } = block {
            text_chunks.push(text.as_str());
        }
    }
    let joined = text_chunks.join(" ");
    if joined.chars().count() > PER_MESSAGE_CHAR_CAP {
        let truncated: String = joined.chars().take(PER_MESSAGE_CHAR_CAP).collect();
        format!("{truncated}…")
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::llm::MockUtilityLlm;
    use crate::modules::memory::SqliteMemoryProvider;
    use crate::modules::runtime::session::ConversationMessage;
    use std::sync::Arc;

    fn user_msg(text: &str) -> ConversationMessage {
        ConversationMessage::user_text(text)
    }

    #[test]
    fn build_prompt_includes_session_and_messages() {
        let msgs = vec![
            user_msg("hi I'm RL, prefer concise replies"),
            user_msg("second turn"),
        ];
        let (sys, usr) = build_reflection_prompt("sess-1", &msgs);
        assert!(sys.contains("first-person"));
        assert!(usr.contains("sess-1"));
        assert!(usr.contains("RL"));
        assert!(usr.contains("User"));
    }

    #[test]
    fn build_prompt_handles_empty_history() {
        let (_, usr) = build_reflection_prompt("sess-2", &[]);
        assert!(usr.contains("(no messages)"));
    }

    #[tokio::test]
    async fn synthesize_persists_reflection_to_memory_provider() {
        let dir = tempfile::tempdir().unwrap();
        let memory: SharedMemoryProvider =
            Arc::new(SqliteMemoryProvider::new(dir.path().join("reflect.db")).unwrap());
        let llm = MockUtilityLlm::new(vec![
            "Observation: the user prefers terse, code-first answers.".to_string(),
        ]);

        let scope = MemoryExecutionScope::global();
        let msgs = vec![user_msg("concise please")];
        let key = synthesize_and_persist(&llm, &memory, &scope, "sess-x", &msgs)
            .await
            .unwrap();

        assert!(key.starts_with("reflection-sess-x-"));
        let stored = memory
            .get_by_key(&key)
            .await
            .unwrap()
            .expect("entry written");
        assert_eq!(stored.category, MemoryCategory::Reflection);
        assert!(stored.content.contains("terse"));
    }

    #[tokio::test]
    async fn synthesize_errors_on_empty_llm_response() {
        let dir = tempfile::tempdir().unwrap();
        let memory: SharedMemoryProvider =
            Arc::new(SqliteMemoryProvider::new(dir.path().join("reflect.db")).unwrap());
        let llm = MockUtilityLlm::new(vec!["   ".to_string()]);
        let scope = MemoryExecutionScope::global();
        let err = synthesize_and_persist(&llm, &memory, &scope, "s", &[])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}
