//! Ephemeral per-session undo/redo stacks (transcript snapshots only).
//!
//! Snapshots are kept in memory (not persisted to session JSON). When
//! `IF2AI_CONVERSATION_UNDO` is unset, undo is **enabled**; set to `0` / `false`
//! / `off` to disable.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use super::manager::Session;
use crate::modules::runtime::session::ConversationMessage;

/// Persisted transcript fields we restore on undo/redo.
#[derive(Debug, Clone)]
pub struct TranscriptSnapshot {
    pub messages: Vec<ConversationMessage>,
    pub message_count: usize,
    pub token_count: u64,
}

impl TranscriptSnapshot {
    #[must_use]
    pub fn from_session(s: &Session) -> Self {
        Self {
            messages: s.messages.clone(),
            message_count: s.message_count,
            token_count: s.token_count,
        }
    }

    pub fn apply_to(&self, s: &mut Session) {
        s.messages = self.messages.clone();
        s.message_count = self.message_count;
        s.token_count = self.token_count;
    }
}

#[derive(Debug)]
struct Stacks {
    undo: Vec<TranscriptSnapshot>,
    redo: Vec<TranscriptSnapshot>,
}

impl Default for Stacks {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

/// Process-wide undo registry (shared via `Arc` on [`super::SessionManager`](super::SessionManager)).
pub struct SessionUndoRegistry {
    inner: Mutex<HashMap<String, Stacks>>,
}

impl std::fmt::Debug for SessionUndoRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionUndoRegistry")
            .finish_non_exhaustive()
    }
}

impl Default for SessionUndoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Surface for Tauri / UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationUndoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
}

impl SessionUndoRegistry {
    const MAX_DEPTH: usize = 32;

    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// When `true`, checkpoints and undo/redo apply. Default **on** (env unset).
    #[must_use]
    pub fn enabled() -> bool {
        !std::env::var("IF2AI_CONVERSATION_UNDO")
            .map(|v| {
                let v = v.trim();
                v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")
            })
            .unwrap_or(false)
    }

    /// Push pre-turn transcript; clears redo for classic invalidation.
    pub fn push_checkpoint(&self, session_id: &str, session: &Session) {
        if !Self::enabled() {
            return;
        }
        let snap = TranscriptSnapshot::from_session(session);
        let mut g = self.inner.lock().expect("session_undo poisoned");
        let stacks = g.entry(session_id.to_string()).or_default();
        stacks.undo.push(snap);
        while stacks.undo.len() > Self::MAX_DEPTH {
            stacks.undo.remove(0);
        }
        stacks.redo.clear();
    }

    #[must_use]
    pub fn status(&self, session_id: &str) -> ConversationUndoStatus {
        let g = self.inner.lock().expect("session_undo poisoned");
        let stacks = g.get(session_id);
        ConversationUndoStatus {
            can_undo: stacks.is_some_and(|s| !s.undo.is_empty()),
            can_redo: stacks.is_some_and(|s| !s.redo.is_empty()),
        }
    }

    /// Pop one undo level into `session` (mutates in place). Returns `false` if empty/disabled.
    pub fn undo_into(&self, session_id: &str, session: &mut Session) -> bool {
        if !Self::enabled() {
            return false;
        }
        let mut g = self.inner.lock().expect("session_undo poisoned");
        let stacks = match g.get_mut(session_id) {
            Some(s) => s,
            None => return false,
        };
        let prev = match stacks.undo.pop() {
            Some(p) => p,
            None => return false,
        };
        let cur = TranscriptSnapshot::from_session(session);
        stacks.redo.push(cur);
        while stacks.redo.len() > Self::MAX_DEPTH {
            stacks.redo.remove(0);
        }
        prev.apply_to(session);
        true
    }

    /// Pop one redo level into `session`. Returns `false` if empty/disabled.
    pub fn redo_into(&self, session_id: &str, session: &mut Session) -> bool {
        if !Self::enabled() {
            return false;
        }
        let mut g = self.inner.lock().expect("session_undo poisoned");
        let stacks = match g.get_mut(session_id) {
            Some(s) => s,
            None => return false,
        };
        let next = match stacks.redo.pop() {
            Some(n) => n,
            None => return false,
        };
        let cur = TranscriptSnapshot::from_session(session);
        stacks.undo.push(cur);
        while stacks.undo.len() > Self::MAX_DEPTH {
            stacks.undo.remove(0);
        }
        next.apply_to(session);
        true
    }

    pub fn remove_session(&self, session_id: &str) {
        let mut g = self.inner.lock().expect("session_undo poisoned");
        g.remove(session_id);
    }
}
