# Steward-Alignment Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Land 3 closeout items deferred from Phase 2: relocate `agent_loop` to `runtime` (fix layering inversion); remove dead nudge fields from `AgenticLoopConfig` (honest API surface); wire `force_text_after_truncations` end-to-end (close a real safety-valve gap).

**Worktree:** `.worktrees/steward-align-phase3` (branch `feature/steward-alignment-phase3` from `vnext` `d142658`)

**Predecessors:** Phase 1 (`9f65b0d`) and Phase 2 (`d142658`).

---

## Cross-Session Conventions

- Commit prefix: `feat(steward-align-p3): T<N>-<tag> — <one-line>`
- Cargo time budget per task: `cargo check --tests` + targeted `cargo test --test <name>` only
- `cargo clippy -p if2ai-backend --tests -- -D warnings` (crate-only)
- Each task = one independent commit; rollback-able

---

# T1 — L1-α: Relocate `agentic_loop` + `loop_config` into `runtime/agent_loop/`

**Why:** `runtime/run_delegate.rs` imports from `application::turn_service::agentic_loop` — runtime → application inversion (reviewer flagged). Cleanest fix: move the loop algorithm + config into `runtime` where both delegates can import without inversion.

**Files:**
- Move: `src-tauri/src/modules/application/turn_service/agentic_loop.rs` → `src-tauri/src/modules/runtime/agent_loop/loop_runner.rs`
- Move: `src-tauri/src/modules/application/turn_service/loop_config.rs` → `src-tauri/src/modules/runtime/agent_loop/config.rs`
- Create: `src-tauri/src/modules/runtime/agent_loop/mod.rs` (declares + re-exports)
- Modify: `src-tauri/src/modules/application/turn_service/mod.rs` (remove `pub mod agentic_loop` + `pub mod loop_config`; add re-export shims for backward-compat)
- Modify: `src-tauri/src/modules/runtime/mod.rs` (declare `pub mod agent_loop;`)
- Update imports across consumers (find via `rg "application::turn_service::agentic_loop|application::turn_service::loop_config|application::turn_service::AgenticLoopConfig"`)

**Steps:**
- [ ] **T1.1**: Create `runtime/agent_loop/` directory with `mod.rs` re-exporting both modules
- [ ] **T1.2**: `git mv` both files into the new location
- [ ] **T1.3**: Update `runtime/mod.rs` to declare `pub mod agent_loop`
- [ ] **T1.4**: Add backward-compat re-export shim in `turn_service/mod.rs`:
  ```rust
  pub use crate::modules::runtime::agent_loop as agentic_loop;
  pub use crate::modules::runtime::agent_loop::AgenticLoopConfig;
  ```
- [ ] **T1.5**: Update existing imports across the codebase to point at new path (search `rg`, replace one-by-one). Backward-compat shim covers any missed sites.
- [ ] **T1.6**: Verify `cargo check --tests`, run all 5 standard tests (conversation::tests + 4 e2e/agentic), `cargo clippy`
- [ ] **T1.7**: Commit `feat(steward-align-p3): T1 — relocate agentic_loop + loop_config into runtime/agent_loop/ (L1-α)`

---

# T2 — N1-α: Remove Inert Nudge Fields from `AgenticLoopConfig`

**Why:** `enable_tool_intent_nudge` and `max_tool_intent_nudges` are dead in production. Both `StreamDelegate` and `RunDelegate` short-circuit empty-tool-calls to `RespondResult::Text`, making `run_agentic_loop`'s nudge path unreachable. if2Ai's real nudge logic lives delegate-side (`stream_iteration::handle_no_tool_calls`). Honest API surface > pretend safety valve.

**Files:**
- Modify: `runtime/agent_loop/config.rs` (post-T1) — remove 2 fields + their defaults
- Modify: `runtime/agent_loop/loop_runner.rs` (post-T1) — remove the nudge logic block from `run_agentic_loop`
- Modify: any tests / doc that referenced the removed fields
- Modify: `runtime/agent_loop/config.rs` doc-comment — update wiring-status block accordingly

**Steps:**
- [ ] **T2.1**: Audit every reference: `rg -n "enable_tool_intent_nudge|max_tool_intent_nudges|nudge_count" src-tauri/`
- [ ] **T2.2**: Delete the 2 fields from `AgenticLoopConfig` struct + `Default` impl
- [ ] **T2.3**: Delete the `nudge_count` local + nudge logic block in `run_agentic_loop` body (specifically the `if calls.is_empty() && config.enable_tool_intent_nudge && nudge_count < config.max_tool_intent_nudges { ... }` branch). The post-T2 loop body returns `LoopOutcome::Failure` when delegates somehow return empty `ToolCalls` (the `debug_assert!` in StreamDelegate already enforces this won't happen).
- [ ] **T2.4**: Update `defaults_match_steward_baseline` test in `config.rs` — drop the now-removed asserts
- [ ] **T2.5**: Update unit tests `tool_intent_nudge_injects_message_when_calls_empty` + `nudge_capped_at_max_then_loop_proceeds` in `agentic_loop_unit.rs`. Either remove (since the nudge no longer exists) or rewrite to assert the new "empty ToolCalls returns LoopOutcome::Failure" behavior (recommend the latter — preserves test coverage of the loop's response to empty calls).
- [ ] **T2.6**: Update `loop_config.rs` module-level doc-comment: remove the "inert in production" note for the now-removed fields. Update `stream_task.rs` field doc-comment too.
- [ ] **T2.7**: Verify `cargo check --tests`, run targeted tests (agentic_loop_unit, conversation::tests, both e2e), `cargo clippy`
- [ ] **T2.8**: Commit `feat(steward-align-p3): T2 — remove inert nudge fields from AgenticLoopConfig (N1-α)`

---

# T3 — N2-β: Wire `force_text_after_truncations` End-to-End

**Why:** This is a real safety-valve gap. Streams that hit `length` truncation today have no automatic recovery — they keep retrying with the same prompt + tools until `max_iterations` exits. With T2's plumbing in place (`state.last_finish_reason` via `MessageDelta::stop_reason`), wiring `ctx.force_text` provides automatic "drop tools after N consecutive truncations" recovery.

**Files:**
- Modify: `runtime/agent_loop/loop_runner.rs` (post-T1) — keep `truncation_count` + `ctx.force_text` (already there from T5a)
- Modify: `application/turn_service/stream_delegate.rs` — `call_llm` reads `ctx.force_text`; if true, build the next preflight request with `disable_tools_once`
- Modify: `application/turn_service/stream_iteration.rs` — `iteration_preflight` accepts `disable_tools: bool`; passes it to `build_iteration_request`; build skips tool defs when set
- Modify: `runtime/run_delegate.rs` — `call_llm` reads `ctx.force_text`; build sync request without tool defs when set
- Modify: `runtime/conversation.rs` (or wherever the request builder lives for sync path) — accept optional disable_tools flag

**Steps:**
- [ ] **T3.1**: Audit current `iteration_preflight` signature for any existing `disable_tools` plumbing; `rg -n "disable_tools|tool_choice" src-tauri/src/modules/application/turn_service/`
- [ ] **T3.2**: Add `disable_tools: bool` parameter (default false) to `iteration_preflight`'s shared-refs construction or as a separate arg. Thread it through to `build_iteration_request` which currently includes tool defs unconditionally.
- [ ] **T3.3**: In `StreamDelegate::call_llm`, pass `ctx.force_text` to `iteration_preflight`. Remember: `before_llm_call` runs `iteration_preflight`, NOT `call_llm`. So actually move the `ctx.force_text` read into `before_llm_call` and pass to `iteration_preflight`.
- [ ] **T3.4**: For sync path: in `RunDelegate::call_llm`, if `ctx.force_text` is true, build the `ApiRequest` without tool definitions (or with `tool_choice = "none"` if the API surface uses that pattern). `RunDelegate::call_llm` builds the request itself, so this is a single in-method change.
- [ ] **T3.5**: Write a stream-side regression test that asserts: with `force_text_after_truncations: 1` and a mock provider that returns `finish_reason: "length"` on iteration 1, then `finish_reason: "stop"` on iteration 2 with empty tools, the second iteration's request omits tool definitions. (If the test is too heavy, skip and rely on a unit test on the preflight builder.)
- [ ] **T3.6**: Update `loop_config.rs` doc-comment: change `force_text_after_truncations` from "inert" to "wired"
- [ ] **T3.7**: Verify `cargo check --tests`, run targeted tests, `cargo clippy`
- [ ] **T3.8**: Commit `feat(steward-align-p3): T3 — wire force_text_after_truncations end-to-end into both delegates (N2-β)`

---

# Phase 3 Final Exit Gate

After T3:
1. `cargo fmt --all` clean
2. `cargo clippy -p if2ai-backend --tests -- -D warnings` clean
3. Both e2e + 9 conversation::tests + agentic_loop_unit + loop_config_force_text PASS
4. `rg "application::turn_service::agentic_loop"` returns only the backward-compat shim (or zero if shim removed)
5. `rg "enable_tool_intent_nudge|max_tool_intent_nudges"` returns zero matches
6. `loop_config.rs` doc shows: `max_iterations` wired, `force_text_after_truncations` wired (post-T3)

Then: dispatch final code-reviewer, run `finishing-a-development-branch` → merge to `vnext`, push.
