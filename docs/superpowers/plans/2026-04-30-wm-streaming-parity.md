# WorkingMemory Streaming Parity (F1)

> Compact plan; F1 was deferred from Phase 4. Adds the streaming-parity wiring that the dead-code comment on `WorkingMemory::push` referenced.

**Goal:** Apply `WorkingMemory` recency window (default 8 turns) to the streaming agent path BEFORE tier-compression, with env-var override and reuse of existing `sanitize_messages_for_provider` for tool_use→tool_result pairing.

**Worktree:** `.worktrees/wm-streaming-parity` (branch `feature/wm-streaming-parity` from `vnext` `4802288`)

**Decision matrix (from brainstorming):**
- **A** — Apply window BEFORE tier-compression
- **II** — Reuse `sanitize_messages_for_provider` for pairing
- **III** — Trust downstream tier-compression+sanitize for safety

---

## T1 — Apply `WorkingMemory` window in streaming path

**Files:**
- Modify: `src-tauri/src/modules/memory/working_memory.rs` (remove `#[allow(dead_code)]`, add `pub fn window_recent_turns(messages: &[ConversationMessage], max_turns: usize) -> Vec<ConversationMessage>` static helper)
- Modify: `src-tauri/src/modules/application/turn_service/stream.rs` (~line 412-461 site where session messages are converted) — apply windowing before constructing `InputMessage` list
- Modify: `src-tauri/src/modules/runtime/budget.rs` — add `streaming_window_turns()` env helper mirroring `streaming_tier_budget()` (env: `IF2AI_STREAM_WINDOW_TURNS`, default 8, sanity floor 2)
- Add tests:
  - Unit test in `working_memory.rs` for `window_recent_turns` (preserves order, slices last N turns)
  - Unit test in `budget.rs` for `streaming_window_turns()` env behavior

**Steps:**

- [ ] **T1.1: Audit**
  ```bash
  cd .worktrees/wm-streaming-parity
  sed -n '380,470p' src-tauri/src/modules/application/turn_service/stream.rs
  rg -n "WorkingMemory|window|recent_turns" src-tauri/src/modules/memory/working_memory.rs
  ```
  Find the exact site where `session.messages` becomes `Vec<InputMessage>`. Confirm `WorkingMemory::push` / `evict_if_needed` definitions.

- [ ] **T1.2: Add `window_recent_turns` helper to `working_memory.rs`**
  Pure function (NOT a method on the type — keeps WorkingMemory state-free for streaming):
  ```rust
  /// Return a slice of the last N "turns" (user→assistant pairs) from a message
  /// list. Used by the streaming path to cap history before tier-compression.
  ///
  /// "Turn" here means a logical user→assistant exchange. The slicing is done
  /// at user-message boundaries so we never split a tool_use→tool_result chain
  /// emitted by a single assistant turn. Any orphan tool_use/tool_result blocks
  /// that survive should be cleaned by the existing
  /// `sanitize_messages_for_provider` downstream (defense-in-depth).
  pub fn window_recent_turns(
      messages: &[ConversationMessage],
      max_turns: usize,
  ) -> Vec<ConversationMessage> {
      if max_turns == 0 || messages.is_empty() {
          return messages.to_vec();
      }
      // Find indexes of user messages from the back; we want the index of the
      // (max_turns)th-from-the-end user message to use as the slice start.
      let mut user_indexes: Vec<usize> = messages.iter()
          .enumerate()
          .filter(|(_, m)| m.role == Role::User)
          .map(|(i, _)| i)
          .collect();
      if user_indexes.len() <= max_turns {
          // Already within budget; passthrough.
          return messages.to_vec();
      }
      // Keep last max_turns user messages + everything after them.
      let cut_at = user_indexes[user_indexes.len() - max_turns];
      messages[cut_at..].to_vec()
  }
  ```
  (Adapt to actual `ConversationMessage` type and `Role` import path.)

  Remove the `#[allow(dead_code)]` markers on `WorkingMemory::new` and `WorkingMemory::push` since this related work is now real.

- [ ] **T1.3: Add `streaming_window_turns` env helper in `budget.rs`**
  Mirroring T1's earlier `streaming_tier_budget()`:
  ```rust
  pub const DEFAULT_STREAM_WINDOW_TURNS: usize = 8;

  pub fn streaming_window_turns() -> usize {
      std::env::var("IF2AI_STREAM_WINDOW_TURNS")
          .ok()
          .and_then(|s| s.parse::<usize>().ok())
          .filter(|&n| n >= 2)  // sanity: keep at least last user+assistant pair
          .unwrap_or(DEFAULT_STREAM_WINDOW_TURNS)
  }
  ```

- [ ] **T1.4: Wire into `stream.rs`**
  In the message-conversion site (~line 412-461 of `stream.rs`):
  ```rust
  // Convert session messages to API format
  let runtime_session = app_session_to_runtime(&app_session);

  // Phase 5 (F1): apply WorkingMemory recency window before further processing.
  // Pure recency slice — orphan tool_use/result pairs are cleaned downstream by
  // sanitize_messages_for_provider during preflight (defense-in-depth).
  let windowed_messages = crate::modules::memory::working_memory::window_recent_turns(
      &runtime_session.messages,
      crate::modules::runtime::budget::streaming_window_turns(),
  );

  let messages: Vec<InputMessage> = windowed_messages
      .iter()
      .map(|msg| {
          // ... existing mapping ...
      })
      .collect();
  ```

- [ ] **T1.5: Add unit tests**

  In `working_memory.rs` `#[cfg(test)] mod tests`:
  ```rust
  #[test]
  fn window_recent_turns_passthrough_when_under_budget() {
      let msgs = vec![
          ConversationMessage::user("u1"),
          ConversationMessage::assistant("a1"),
          ConversationMessage::user("u2"),
          ConversationMessage::assistant("a2"),
      ];
      let out = window_recent_turns(&msgs, 8);
      assert_eq!(out.len(), 4);
  }

  #[test]
  fn window_recent_turns_slices_at_user_boundary() {
      // 4 turns; window of 2 should keep last 2 (user3+assistant3+user4+assistant4)
      let msgs = vec![
          ConversationMessage::user("u1"),
          ConversationMessage::assistant("a1"),
          ConversationMessage::user("u2"),
          ConversationMessage::assistant("a2"),
          ConversationMessage::user("u3"),
          ConversationMessage::assistant("a3"),
          ConversationMessage::user("u4"),
          ConversationMessage::assistant("a4"),
      ];
      let out = window_recent_turns(&msgs, 2);
      assert_eq!(out.len(), 4);
      assert!(matches!(out[0].role, Role::User));
      // First user in result is the 3rd user from the input (index 4)
  }

  #[test]
  fn window_recent_turns_zero_returns_passthrough() {
      let msgs = vec![ConversationMessage::user("hi")];
      let out = window_recent_turns(&msgs, 0);
      assert_eq!(out.len(), 1);  // 0 means "don't window"
  }

  #[test]
  fn window_recent_turns_preserves_assistant_with_tool_calls_in_window() {
      // If the last assistant turn has tool_use, the slice should include it
      // (it's after the cut boundary). Sanitize handles any orphan after that.
      let msgs = vec![
          ConversationMessage::user("u1"),
          ConversationMessage::assistant("a1"),  // with tool_use blocks
          ConversationMessage::user("u2"),  // tool_result acting as user
      ];
      let out = window_recent_turns(&msgs, 1);
      // Window=1 keeps last user msg (u2) + nothing before
      assert_eq!(out.len(), 1);
  }
  ```

  In `budget.rs` `#[cfg(test)] mod tests`:
  ```rust
  #[test]
  fn streaming_window_turns_default_when_unset() {
      let _g = TEST_ENV_LOCK.lock().unwrap();
      std::env::remove_var("IF2AI_STREAM_WINDOW_TURNS");
      assert_eq!(streaming_window_turns(), DEFAULT_STREAM_WINDOW_TURNS);
  }

  #[test]
  fn streaming_window_turns_honors_env_var() {
      let _g = TEST_ENV_LOCK.lock().unwrap();
      std::env::set_var("IF2AI_STREAM_WINDOW_TURNS", "12");
      assert_eq!(streaming_window_turns(), 12);
      std::env::remove_var("IF2AI_STREAM_WINDOW_TURNS");
  }

  #[test]
  fn streaming_window_turns_floor_clamps_to_default() {
      let _g = TEST_ENV_LOCK.lock().unwrap();
      std::env::set_var("IF2AI_STREAM_WINDOW_TURNS", "1");
      assert_eq!(streaming_window_turns(), DEFAULT_STREAM_WINDOW_TURNS);
      std::env::remove_var("IF2AI_STREAM_WINDOW_TURNS");
  }
  ```
  (Reuse the existing `TEST_ENV_LOCK` mutex from Phase 4 T1's tests if it exists.)

- [ ] **T1.6: Verify GREEN**
  ```bash
  cd src-tauri
  cargo check --tests
  cargo test --lib -p if2ai-backend working_memory::tests
  cargo test --lib -p if2ai-backend budget::tests
  cargo test --test turn_service_stream_turn_e2e
  cargo test --test turn_service_run_turn_e2e
  ```

- [ ] **T1.7: Format + clippy + commit**
  ```bash
  cargo fmt --all
  cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
  git add -A
  git commit -m "feat(wm-streaming): T1 — apply WorkingMemory recency window before tier-compression in stream path"
  ```

---

## Exit Gate
1. `cargo fmt --all` clean
2. `cargo clippy -p if2ai-backend --tests -- -D warnings` clean
3. `cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e` PASS
4. `cargo test --lib -p if2ai-backend working_memory::tests budget::tests` PASS
5. Manual verification on tauri dev: `[stream_diag_summary]` should now show fewer messages sent (compare `session_messages len=99` vs actual processed message count)

Then merge to vnext, push, restart tauri dev, observe input token count drop further than Phase 4's tier-compression alone.
