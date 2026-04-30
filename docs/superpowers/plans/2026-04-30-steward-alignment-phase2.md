# Steward-Alignment Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Land all Phase 1 deferred items: complete StreamDelegate adoption (5b chain), complete RunDelegate adoption (5c chain), reconcile `max_iterations` defaults, and clean up DW-001 documentation drift.

**Architecture:** 11 commits across two adoption chains plus two doc/wiring fixes. Each commit builds and tests independently. The 5b chain refactors the streaming path incrementally (helpers → finish_reason → delegate). The 5c chain async-ifies the sync path then adopts the delegate.

**Tech Stack:** Same as Phase 1 — Rust 2021, Tokio async, async-trait, tauri 2.

**Worktree:** `.worktrees/steward-align-phase2` (branch `feature/steward-alignment-phase2`, branched from `vnext` at `9f65b0d`)

**Predecessor:** `docs/superpowers/plans/2026-04-30-steward-alignment.md` Phase 1, merged in `9f65b0d`. Spec: `docs/superpowers/specs/2026-04-30-steward-alignment-design.md`. Phase 1 audit findings (5b/5c structural blockers) reproduced inline below where each task lands.

---

## Cross-Session Conventions

- Commit prefix: `feat(steward-align-p2): T<N>-<tag> — <one-line>`
- Cargo time budget per task: `cargo check --tests` + targeted `cargo test --test <name>` only — NOT full `cargo test`
- Use `cargo clippy -p if2ai-backend --tests -- -D warnings` (crate-only, not workspace)
- TDD discipline: failing test first → confirm RED → implement → confirm GREEN → commit
- Each task is independently buildable and rollback-able
- Heavy lifts (T8 async migration) get explicit escalation permission — escalate with structured split if too large

---

# T1 — DW-001 Gap Report §3.2 Stale-Row Cleanup (5 min, doc only)

**Files:**
- Modify: `docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md`

The Phase 1 DW-001 recon found that §3.2 still lists DW-001 as "landed-stub" with stale line citations (~57), even though §13 marks it ✅ done after iteration-7. Reconcile.

- [ ] **Step T1.1: Open and locate**

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai/.worktrees/steward-align-phase2
rg -n "DW-001" docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md
```

Find §3.2's DW-001 row and §13's DW-001 entry.

- [ ] **Step T1.2: Update §3.2 row to "done"**

In the §3.2 table, change the DW-001 row's status from `landed-stub` (or whatever stale label is there) to `done`. Update the line-citation column to point to the iter-7 commits (`50808e1`, `af3a890`, `9e36e72`) instead of the old `~57` reference. Keep the row's other columns (description, owner) unchanged.

- [ ] **Step T1.3: Add a 1-line note above §3.2**

If §3.2 has a header note like "Status as of YYYY-MM-DD", update the date and add a brief: "DW-001 marked done — see §13 for iter-7 wiring details."

- [ ] **Step T1.4: Verify cross-section consistency**

```bash
rg -n "DW-001" docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md
```

§3.2 row, §13 entry, and any other DW-001 mentions must all read consistently as "done" — no contradictions.

- [ ] **Step T1.5: Commit**

```bash
git add docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md
git commit -m "docs(steward-align-p2): T1 — reconcile DW-001 §3.2 row with §13 done-state"
```

---

# T2 — 5b-4: Plumb `finish_reason` from SSE Loop

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/stream_event_loop.rs`
- Modify: every `ApiClient` impl (Anthropic, OpenAI, others — `rg "impl ApiClient"`)
- Test: in-module test in `stream_event_loop.rs`

**Why this first:** Phase 1's audit identified `finish_reason` as the prerequisite for `force_text` and `is_length_truncation` to actually fire on the streaming path. Independently useful regardless of whether full StreamDelegate adoption follows.

- [ ] **Step T2.1: Audit current `StreamEventLoopResult`**

```bash
cd src-tauri
rg -n "struct StreamEventLoopResult" src/modules/application/turn_service/stream_event_loop.rs
sed -n '60,100p' src/modules/application/turn_service/stream_event_loop.rs
```

Confirm current fields. Find where `MessageStop` events are handled inside the SSE loop — that's where the provider's stop reason / finish reason originates.

- [ ] **Step T2.2: Add `finish_reason` field to `StreamEventLoopResult`**

```rust
pub struct StreamEventLoopResult {
    // ... existing fields ...
    /// Provider-supplied finish reason from the final SSE event.
    /// `Some("length")` / `Some("max_tokens")` indicates truncation;
    /// `None` for streams that ended without a finish_reason event
    /// (cancelled, errored, or never reached MessageStop).
    pub finish_reason: Option<String>,
}
```

Update the `Default` impl (or constructor) to init `finish_reason: None`.

- [ ] **Step T2.3: Source it from the SSE event handler**

Inside `run_stream_event_loop`, where `AssistantEvent::MessageStop` is handled, capture the provider's stop reason:

For Anthropic (`message_delta` event):
```rust
AssistantEvent::MessageStop { stop_reason, .. } => {
    result.finish_reason = stop_reason;  // Option<String>
    // ... existing handling ...
}
```

For OpenAI (final chunk's `finish_reason` field):
```rust
AssistantEvent::ChunkFinish { finish_reason } => {
    result.finish_reason = Some(finish_reason);
    // ... existing handling ...
}
```

If the existing `AssistantEvent` enum doesn't carry `stop_reason` / `finish_reason` on the relevant variants, add them as `Option<String>` fields. This is a backward-compatible addition (existing constructors that don't set them default to `None`).

- [ ] **Step T2.4: Update `ApiClient` implementations**

```bash
rg -n "MessageStop|ChunkFinish|finish_reason|stop_reason" src/modules/api/ src/modules/provider/
```

For each `ApiClient` impl (likely `AnthropicProvider`, `OpenAIProvider`, mock impls in tests), make sure their stream parser populates the new field on the relevant `AssistantEvent` variant.

- [ ] **Step T2.5: Write in-module test**

In `stream_event_loop.rs` `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn finish_reason_round_trips_from_message_stop() {
    let mut events = Vec::new();
    events.push(AssistantEvent::MessageStop {
        stop_reason: Some("length".into()),
    });
    let result = run_stream_event_loop(&mut events.into_iter(), /* ... */).await;
    assert_eq!(result.finish_reason.as_deref(), Some("length"));
}

#[tokio::test]
async fn finish_reason_none_when_stream_lacks_message_stop() {
    let events: Vec<AssistantEvent> = vec![];  // empty stream
    let result = run_stream_event_loop(&mut events.into_iter(), /* ... */).await;
    assert!(result.finish_reason.is_none());
}
```

(Adapt to the actual function signature.)

- [ ] **Step T2.6: Verify GREEN**

```bash
cd src-tauri
cargo check --tests
cargo test --lib -p if2ai-backend stream_event_loop::tests
cargo test --test turn_service_stream_turn_e2e
```

E2E must still pass — adding a `finish_reason: Option<String>` field is backward-compatible.

- [ ] **Step T2.7: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T2 — surface provider finish_reason from stream_event_loop"
```

---

# T3 — 5b-2: Extract `iteration_preflight` + `iteration_finalize_no_tools` Helpers

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`

Per Phase 1's audit, the iteration body inside `run_stream_task_body` is ~700 LOC. Extract steps 1-8 (cancellation poll → iter cap → setup → digester preflight → build_iteration_request → diagnostics → cost guard → stream start) into one helper, and the empty-`pending_tool_uses` branch (step 11) into another.

- [ ] **Step T3.1: Read the loop body**

```bash
cd src-tauri
sed -n '483,700p' src/modules/application/turn_service/stream_task.rs
```

(Adjust line ranges to current file — they shifted after Phase 1's `StreamLoopState` extraction.)

Identify the exact boundaries of the two phases.

- [ ] **Step T3.2: Define `PreflightOutcome` enum**

In `stream_task.rs` (or a new sibling `stream_iteration.rs` if cleaner), add:

```rust
enum PreflightOutcome {
    /// Continue to the inner SSE loop with the prepared stream.
    Continue { stream: ProviderStream },
    /// Terminate the iteration loop (cost guard rejected, stream-start failed, etc.).
    /// state.terminal_status / state.last_stream_error_reason already populated.
    BreakTerminal,
    /// Sleep + retry without consuming a real iteration slot.
    RetryAfterSleep { sleep: std::time::Duration },
}
```

- [ ] **Step T3.3: Extract `iteration_preflight` helper**

```rust
async fn iteration_preflight(
    state: &mut StreamLoopState,
    inputs: &StreamTaskInputs,
    cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
    // ... other shared refs ...
) -> PreflightOutcome {
    // Move steps 1-8 from run_stream_task_body's loop body verbatim.
    // Replace the original `break;` / `continue;` / `let stream = ...;` end-states
    // with returns of the appropriate PreflightOutcome variant.
}
```

In `run_stream_task_body`'s loop:
```rust
loop {
    let stream = match iteration_preflight(&mut state, inputs, &mut cancel_rx, ...).await {
        PreflightOutcome::Continue { stream } => stream,
        PreflightOutcome::BreakTerminal => break,
        PreflightOutcome::RetryAfterSleep { sleep } => {
            tokio::time::sleep(sleep).await;
            continue;
        }
    };
    // ... rest of iteration body unchanged ...
}
```

- [ ] **Step T3.4: Define `NoToolOutcome` enum**

```rust
enum NoToolOutcome {
    /// Loop should break (terminal_status already set on state).
    Break,
    /// Loop should continue iterating (retry counter bumped, session_messages mutated).
    Continue,
}
```

- [ ] **Step T3.5: Extract `handle_no_tool_calls` helper**

```rust
async fn handle_no_tool_calls(
    state: &mut StreamLoopState,
    inputs: &StreamTaskInputs,
    // ... shared refs ...
) -> NoToolOutcome {
    // Move step 11 verbatim — textual extraction, tool_required_no_tool retry,
    // tool_intent_nudge retry, terminal-status computation.
}
```

In the loop:
```rust
if state.pending_tool_uses.is_empty() {
    match handle_no_tool_calls(&mut state, inputs, ...).await {
        NoToolOutcome::Break => break,
        NoToolOutcome::Continue => continue,
    }
}
```

- [ ] **Step T3.6: Verify GREEN (no behavior change)**

```bash
cargo check --tests
cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e
```

This is a pure refactor — both e2e tests must pass without modification.

- [ ] **Step T3.7: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T3 — split per-iteration phases into named helpers (5b-2)"
```

---

# T4 — 5b-3: Extract `iteration_run_stream` + `iteration_execute_tools` Helpers

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`

Continue the per-phase extraction. Pull steps 9-10 (capture provider request_id + run inner SSE loop) into one helper, and steps 13-14 (repeated-batch detection + tool batch execution) into another.

- [ ] **Step T4.1: Define `RunStreamOutcome` enum**

```rust
enum RunStreamOutcome {
    /// Inner SSE loop completed; state.pending_tool_uses + state.accumulated_text populated.
    Completed,
    /// Inner SSE loop returned `retry_outer_after_timeout`; sleep + continue.
    RetryAfterSleep { sleep: std::time::Duration },
}
```

- [ ] **Step T4.2: Extract `iteration_run_stream` helper**

```rust
async fn iteration_run_stream(
    state: &mut StreamLoopState,
    stream: ProviderStream,
    cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
    inputs: &StreamTaskInputs,
) -> RunStreamOutcome {
    // Move steps 9-10: state.provider_request_id = stream.request_id();
    // run_stream_event_loop(...) consuming cancel_rx; copy results back to state.
}
```

- [ ] **Step T4.3: Extract `iteration_execute_tools` helper**

```rust
async fn iteration_execute_tools(
    state: &mut StreamLoopState,
    tool_executor: &mut Box<dyn ToolExecutor>,
    inputs: &StreamTaskInputs,
) -> Result<(), AppError> {
    // Move steps 13-14: repeated-batch detection, execute_tool_batch
    // (consumes and restores tool_executor via std::mem::replace pattern).
}
```

- [ ] **Step T4.4: Wire helpers into the loop**

The loop body is now ~80 LOC of straight-line phase calls — preflight → run_stream → no-tools-branch → execute_tools.

- [ ] **Step T4.5: Verify GREEN**

```bash
cargo check --tests
cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e
```

- [ ] **Step T4.6: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T4 — extract run_stream / execute_tools per-iter helpers (5b-3)"
```

---

# T5 — 5b-5: Introduce `StreamDelegate`, Replace Loop Body with `run_agentic_loop`

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`
- Possibly create: `src-tauri/src/modules/application/turn_service/stream_delegate.rs`

With T2 (`finish_reason` plumbed) and T3/T4 (helpers extracted) done, the StreamDelegate adoption is now feasible. ~150 LOC of delegate code + a 6-line loop replacement.

- [ ] **Step T5.1: Implement `StreamDelegate`**

Create the struct holding `Mutex<StreamLoopState>`, `pending_calls: Mutex<HashMap<String, (tool_id, name, input_json)>>`, `pending_results: Mutex<HashMap<...>>`, `cancellation: oneshot::Receiver`, etc. Use `std::mem::take` for moved-then-restored handles (`tool_executor`, `cancel_rx`).

Implement `LoopDelegate`:
- `check_signals` → cancellation poll (return `Stop`; the post-loop translator emits the cancellation payload from `state.terminal_status`)
- `before_llm_call` → call `iteration_preflight`. Map `PreflightOutcome::BreakTerminal` → `Some(LoopOutcome::Failure(reason))`. Map `RetryAfterSleep` → sleep then return `None`.
- `call_llm` → call `iteration_run_stream`. Convert `state.pending_tool_uses` to `RespondResult::ToolCalls { calls: ids, finish_reason: state.last_finish_reason }`. If empty + text accumulated, return `RespondResult::Text(state.accumulated_text)`.
- `execute_tool_calls` → look up 3-tuples from pending_calls, call `iteration_execute_tools`, return ids done.
- `handle_text_response` → `TextAction::Return(LoopOutcome::Response(text))`
- `after_iteration` → no-op (current code has no per-iter hooks)

- [ ] **Step T5.2: Replace loop body**

```rust
let delegate = StreamDelegate { /* init from current locals */ };
let outcome = run_agentic_loop(&delegate, &inputs.loop_config).await;
let state = delegate.state.into_inner();
// Translate LoopOutcome → existing finalize path:
match outcome {
    LoopOutcome::Response(text) => state.accumulated_text = text,
    LoopOutcome::Stopped => {} // terminal_status already set
    LoopOutcome::MaxIterations => state.terminal_status.get_or_insert("max_iterations_reached"),
    LoopOutcome::Failure(_) => {} // state already set
}
// existing outbound hook + AgentLoopDelegateOutput build + finalize_stream_task
```

- [ ] **Step T5.3: Verify GREEN (CRITICAL)**

```bash
cargo check --tests
cargo test --test turn_service_stream_turn_e2e
cargo test --test turn_service_run_turn_e2e
cargo test --test agentic_loop_unit --test loop_config_force_text
```

E2e MUST pass. Common regression sources: tool result ordering, missing event emissions, finalize path getting unexpected LoopOutcome.

- [ ] **Step T5.4: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T5 — StreamDelegate adopts run_agentic_loop (5b-5)"
```

If escalation needed: split T5 further. Don't ship fragile work.

---

# T6 — 5c-2: Extract `RunLoopState` Bag

**Files:**
- Modify: `src-tauri/src/modules/runtime/conversation.rs`

Mirror Phase 1's `StreamLoopState` extraction for the sync path. The sync loop has fewer mutable locals (4 in the function plus `&mut self` on ConversationRuntime), so this is a smaller refactor.

- [ ] **Step T6.1: Audit**

```bash
sed -n '417,604p' src/modules/runtime/conversation.rs
```

Identify per-turn locals (`assistant_messages`, `tool_results`, `iterations`, `prompter`).

- [ ] **Step T6.2: Create `RunLoopState`**

```rust
pub(super) struct RunLoopState {
    pub assistant_messages: Vec<ConversationMessage>,
    pub tool_results: Vec<ToolResultMessage>,
    pub iterations: u32,
}
```

- [ ] **Step T6.3: Replace inline locals**

In `ConversationRuntime::run_turn`, replace the `let mut`s at the function top with `let mut state = RunLoopState::default();` and rewrite loop body to use `state.field`.

(`prompter: Option<&mut dyn PermissionPrompter>` stays as a local — borrow lifetime makes it ineligible for the bag.)

- [ ] **Step T6.4: Verify GREEN**

```bash
cargo test --lib -p if2ai-backend conversation::tests
cargo test --test turn_service_run_turn_e2e
```

The 9 unit tests in `conversation.rs::tests` must all pass.

- [ ] **Step T6.5: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T6 — extract RunLoopState bag from ConversationRuntime::run_turn (5c-2)"
```

---

# T7 — 5c-3: Plumb `finish_reason` in Sync Path

**Files:**
- Modify: `src-tauri/src/modules/runtime/conversation.rs` (build_assistant_message)
- Modify: every `ApiClient::stream` impl that the sync path uses
- Modify: 9 unit-test fixtures in `conversation.rs::tests`

Same pattern as T2 but for the sync `ApiClient::stream` path. Mirrors 5c-3 from the audit's split proposal.

- [ ] **Step T7.1: Add `finish_reason` to `AssistantEvent::MessageStop` (if not done in T2)**

Likely already done by T2 since both paths share `AssistantEvent`. Verify:
```bash
rg -n "MessageStop" src/modules/runtime/ src/modules/api/
```

- [ ] **Step T7.2: Surface in `build_assistant_message`**

In `conversation.rs`, find `build_assistant_message` (~line 638-700). Make it return a struct that includes `finish_reason: Option<String>`, OR populate a field on the assembled message.

- [ ] **Step T7.3: Update 9 unit-test fixtures**

```bash
rg -n "MessageStop|build_assistant_message" src/modules/runtime/conversation.rs
```

For every test that constructs an `AssistantEvent::MessageStop`, add the `stop_reason: None` (or appropriate value) field.

- [ ] **Step T7.4: Verify GREEN**

```bash
cargo test --lib -p if2ai-backend conversation::tests
cargo test --test turn_service_run_turn_e2e
```

- [ ] **Step T7.5: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T7 — surface finish_reason from sync ApiClient::stream (5c-3)"
```

---

# T8 — 5c-4: Async-ify `ApiClient::stream` + `ConversationRuntime::run_turn`

**Files:**
- Modify: `src-tauri/src/modules/api/client.rs` (or wherever `ApiClient` trait lives)
- Modify: every `ApiClient` impl (Anthropic, OpenAI, Mock)
- Modify: `src-tauri/src/modules/runtime/conversation.rs` (run_turn)
- Modify: `src-tauri/src/modules/application/turn_service/run.rs` (caller — already async, just `.await`)
- Modify: 9 unit-test fixtures in `conversation.rs::tests` (migrate to `#[tokio::test]`)

⚠️ **HIGH-RISK TASK.** Per Phase 1 audit, this requires changing a fundamental trait signature and cascading through all impls. Apply audit-first protocol. Explicit escalation permission: if this turns out to be larger than ~3 hours of work, split into T8a (trait signature change + impls) and T8b (run_turn migration + test migration).

- [ ] **Step T8.1: Audit current `ApiClient` trait surface**

```bash
rg -n "trait ApiClient|impl ApiClient" src/modules/
```

List every impl. Confirm the current `stream(&self, request) -> StreamResult` is sync.

- [ ] **Step T8.2: Change trait signature**

```rust
#[async_trait]
pub trait ApiClient: Send + Sync {
    async fn stream(&self, request: ApiRequest) -> Result<ProviderStream, AppError>;
    // ... other methods unchanged ...
}
```

- [ ] **Step T8.3: Migrate impls**

For each impl, change `fn stream` to `async fn stream`. Internal logic typically stays the same; the change is just `&self` → `async &self` and any `block_on` calls unwound to `.await`.

If an impl currently does sync HTTP via `reqwest::blocking`, swap to `reqwest` async client. Verify no callers expect sync semantics elsewhere.

- [ ] **Step T8.4: Async-ify `ConversationRuntime::run_turn`**

```rust
impl ConversationRuntime {
    pub async fn run_turn(&mut self, ...) -> Result<RunReport, AppError> {
        // ... .await on every api_client.stream(...) call
    }
}
```

- [ ] **Step T8.5: Update caller in `run.rs`**

`run.rs::TurnService::run_turn` is already `async`, so just `.await` the call:
```rust
let report = runtime.run_turn(user_message.clone(), prompter).await?;
```

- [ ] **Step T8.6: Migrate 9 unit-test fixtures to `#[tokio::test]`**

```bash
rg -n "#\[test\]" src/modules/runtime/conversation.rs
```

For each (`runs_user_to_tool_to_result_loop_end_to_end_and_tracks_usage`, `records_denied_tool_results_when_prompt_rejects`, `denies_tool_use_when_pre_tool_hook_blocks`, `appends_post_tool_hook_feedback_to_tool_result`, `reconstructs_usage_tracker_from_restored_session`, plus 4 more):
```rust
#[tokio::test]
async fn runs_user_to_tool_to_result_loop_end_to_end_and_tracks_usage() {
    // ... existing body, with .await on run_turn
}
```

- [ ] **Step T8.7: Verify GREEN**

```bash
cargo check --tests
cargo test --lib -p if2ai-backend conversation::tests
cargo test --test turn_service_run_turn_e2e --test turn_service_stream_turn_e2e
```

All 9 unit tests + e2e must pass. If any fail because the test mocked the sync trait, update the mock to async-trait.

- [ ] **Step T8.8: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T8 — async-ify ApiClient::stream + ConversationRuntime::run_turn (5c-4)"
```

If escalation: split into T8a (trait + impls) and T8b (run_turn + tests). Each is independently buildable if the trait change keeps a default `block_on` shim during T8a.

---

# T9 — 5c-5: Extract `process_single_tool_call` Helper

**Files:**
- Modify: `src-tauri/src/modules/runtime/conversation.rs`

Per Phase 1 audit, the per-tool work in the sync loop is:
1. Permission authorize
2. PreToolUse hook → may deny
3. tool_executor.execute()
4. merge_hook_feedback over pre-hook
5. PostToolUse hook → may flip is_error
6. merge_hook_feedback over post-hook
7. Safety: sanitize_tool_output + wrap_for_llm
8. record_tool_outcome telemetry

Extracting this into a helper validates the per-tool surface fits before T10's RunDelegate adoption.

- [ ] **Step T9.1: Define helper signature**

```rust
async fn process_single_tool_call(
    &mut self,
    call: ToolCall,
    state: &mut RunLoopState,
) -> ConversationMessage {
    // All 8 steps above, returning the final tool_result message.
}
```

- [ ] **Step T9.2: Refactor the loop body**

Replace the inline per-tool work with a fan-out:
```rust
for call in tool_uses {
    let result_msg = self.process_single_tool_call(call, &mut state).await;
    state.session_messages.push(result_msg);
}
```

- [ ] **Step T9.3: Verify GREEN**

```bash
cargo test --lib -p if2ai-backend conversation::tests
cargo test --test turn_service_run_turn_e2e
```

- [ ] **Step T9.4: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T9 — extract process_single_tool_call helper (5c-5)"
```

---

# T10 — 5c-6: Introduce `RunDelegate`, Replace Inner Loop with `run_agentic_loop`

**Files:**
- Modify: `src-tauri/src/modules/runtime/conversation.rs`
- Possibly create: `src-tauri/src/modules/runtime/run_delegate.rs`

Final piece. With T6 (RunLoopState bag), T7 (finish_reason), T8 (async), T9 (process_single_tool_call helper) all in place, the RunDelegate adoption is now ~80 LOC.

- [ ] **Step T10.1: Implement `RunDelegate`**

```rust
pub(super) struct RunDelegate<'a> {
    runtime: tokio::sync::Mutex<&'a mut ConversationRuntime>,
    state: tokio::sync::Mutex<RunLoopState>,
    pending_calls: tokio::sync::Mutex<HashMap<String, ToolCall>>,
}

#[async_trait]
impl<'a> LoopDelegate for RunDelegate<'a> {
    async fn check_signals(&self) -> LoopSignal { LoopSignal::Continue }
    async fn before_llm_call(&self, _: &mut LoopContext, _: usize) -> Option<LoopOutcome> { None }
    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String> {
        let mut runtime = self.runtime.lock().await;
        let mut state = self.state.lock().await;
        // Build request (drop tools if ctx.force_text), call api_client.stream().await,
        // run inner loop, capture finish_reason, populate pending_calls.
        // Return RespondResult::Text or RespondResult::ToolCalls { ids, finish_reason }.
    }
    async fn execute_tool_calls(&self, ids: Vec<String>, _: &mut LoopContext) -> Vec<String> {
        // For each id: lookup ToolCall, call self.runtime.process_single_tool_call(call).await,
        // append result message to state.session_messages.
        // Return ids done.
    }
    async fn handle_text_response(&self, text: String, _: &mut LoopContext) -> TextAction {
        TextAction::Return(LoopOutcome::Response(text))
    }
    async fn after_iteration(&self, _: &mut LoopContext, _: usize) {}
}
```

- [ ] **Step T10.2: Replace inner loop in `run_turn`**

```rust
pub async fn run_turn(&mut self, ...) -> Result<RunReport, AppError> {
    // Existing setup ...
    let delegate = RunDelegate {
        runtime: tokio::sync::Mutex::new(self),
        state: tokio::sync::Mutex::new(RunLoopState::default()),
        pending_calls: Default::default(),
    };
    let outcome = run_agentic_loop(&delegate, &config).await;
    // Reclaim state
    let state = delegate.state.into_inner();
    // Build RunReport from outcome + state
    match outcome { /* ... */ }
}
```

The `&mut self` lifetime through `Mutex<&mut Self>` is awkward but workable since `run_turn` is the sole caller — no concurrent access.

- [ ] **Step T10.3: Verify GREEN**

```bash
cargo test --lib -p if2ai-backend conversation::tests
cargo test --test turn_service_run_turn_e2e --test agentic_loop_unit
```

If 9 unit tests pass and e2e passes, adoption is complete.

- [ ] **Step T10.4: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T10 — RunDelegate adopts run_agentic_loop (5c-6)"
```

---

# T11 — `max_iterations` Reconciliation via AppState Injection

**Files:**
- Modify: `src-tauri/src/commands/agent/mod.rs` (or wherever `TurnServiceDeps` is constructed)

After T5 and T10, both production paths now consume `loop_config.max_iterations`. The default `50` (Steward baseline) would 5× the current prod cap of `10` (`agent_max_iterations()` env-var). Reconcile by having AppState inject the env-var-derived value at construction time.

- [ ] **Step T11.1: Locate construction site**

```bash
rg -n "loop_config|AgenticLoopConfig" src/commands/
```

Find where `TurnServiceDeps { ..., loop_config: AgenticLoopConfig::default(), ... }` is built.

- [ ] **Step T11.2: Override `max_iterations` from `agent_max_iterations()`**

```rust
let cap = crate::modules::application::config::agent_max_iterations();
let loop_config = AgenticLoopConfig {
    max_iterations: cap,
    ..AgenticLoopConfig::default()
};
let deps = TurnServiceDeps { /* ..., */ loop_config, /* ... */ };
```

- [ ] **Step T11.3: Verify behavior preserved**

```bash
cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e
```

If any test relied on the default `50`, fix it to set the env var or pass an explicit `AgenticLoopConfig` with `max_iterations: 50`.

- [ ] **Step T11.4: Format + clippy + commit**

```bash
cargo fmt --all
cargo clippy -p if2ai-backend --tests -- -D warnings 2>&1 | tail -20
git add -A
git commit -m "feat(steward-align-p2): T11 — preserve env-var-driven max_iterations cap via AppState injection"
```

---

# Phase 2 Final Exit Gate

After T11:

1. `cargo fmt --all` clean
2. `cargo clippy -p if2ai-backend --tests -- -D warnings` clean
3. `cargo test` (full) PASS — at minimum: agentic_loop_unit, hook_*, tool_attenuation, prompt_cache_hit, skill_*, loop_config_force_text, turn_service_*, conversation::tests
4. `rg "is_length_truncation\|force_text\|run_agentic_loop" src/modules/application/turn_service/stream_task.rs` shows the unified loop in use
5. `rg "run_agentic_loop" src/modules/runtime/conversation.rs` shows the unified loop in use
6. `git log --oneline vnext..HEAD` shows N commits, all `feat(steward-align-p2):` prefixed

Then: dispatch final code-reviewer subagent → use superpowers:finishing-a-development-branch.

---

# Self-Review Notes

- ✅ Spec coverage: every Phase 1 deferred item maps to an explicit task
- ✅ No placeholders, no "TODO later" markers
- ✅ T8 (high-risk async migration) carries explicit escalation permission with split proposal
- ✅ T5 carries the same escalation permission (large StreamDelegate adoption)
- ✅ T11 sequenced after T5+T10 so production paths actually consume the cap
- ✅ T1 (DW-001 doc cleanup) sequenced first as a 5-min warmup
