# SP-C Spec — Streaming Run-Log Correlation 修复（FIX-7）

**Status**: DRAFT — awaiting user approval (hard gate before plan)
**Worktree**: `~/Documents/IfAI/if2Ai-sp-c/`
**Branch**: `sp-c/runlog-correlation-unify`
**Fork point**: `vnext@2b8eda9` (post-SP-F1)
**Scope decision**: R1 (FIX-7 only — FIX-9 deferred to SP-C2)
**Test strategy**: T2 (mechanical fix + 1 unit-level contract test)

---

## 1. Problem

Commit `595c48f` (2026-05-01, "unify run-log envelope path + extend correlation
for Agents Teams") created a corrected `append_stream_event` helper at
`src-tauri/src/modules/application/turn_service/stream_task_run_log.rs:49-67`
that calls `RunEventLogger::append_with_correlation(...)` so
`payload.correlation` (carrying `team_id` / `member_id` / `role_id` /
`parent_run_id` / `delegation_id`, added in the same commit) is promoted to
`RunLogEntry` top-level columns and survives jsonl persistence.

**The commit forgot to switch the call sites.** All 7 production call sites
still use the legacy `super::stream_task::append_stream_event` (defined at
`stream_task.rs:234-250`), which calls the unconditional
`RunEventLogger::append(...)` and **drops `payload.correlation` on the floor**.

Net effect today, on `vnext@2b8eda9`:

- 100% of streaming events (`agent-token` deltas, `tool_call_*` lifecycle,
  `thinking_started`, etc.) are persisted to `run.jsonl` with
  `team_id` / `member_id` / `role_id` / `parent_run_id` / `delegation_id` =
  `None`, regardless of what the in-memory `RuntimeEventEnvelope` contained.
- `RunLogEntry::merge_correlation_from_ids` was wired into the canonical
  envelope path but never reaches the streaming firehose.
- Agents Teams replay, audit, and per-member correlation queries on persisted
  logs are silently broken — the data was never written.

There is no test that catches this. `event_log.rs` has unit coverage for the
underlying `append_with_correlation` API, but no test asserts that
`stream_task_run_log::append_stream_event` actually calls
`append_with_correlation` (vs `append`). That gap is why CI did not flag the
half-finished migration when `595c48f` landed.

## 2. Goal (single sentence)

All streaming-event run-log writes must go through
`stream_task_run_log::append_stream_event` (which calls
`append_with_correlation`), the legacy duplicate in `stream_task.rs` must be
deleted, and a unit-level contract test must lock the invariant
"streaming run-log writes preserve `payload.correlation`."

## 3. Non-goals

- **FIX-9 (`runtime → application` reverse dependency at
  `runtime/conversation.rs:590-606`)** — deferred to SP-C2. That requires a
  design choice (sink `run_agentic_loop` to runtime / lift `Conversation` to
  application / introduce trait inversion / accept it in ARCHITECTURE.md) and
  cannot be resolved as a mechanical change.
- **God-file decomposition (SP-D)** — `stream_task.rs` (996 lines) and
  `stream_finalize.rs` (1503 lines) stay as-is in this PR. SP-C only deletes
  the duplicate `append_stream_event` (~17 lines) from `stream_task.rs`.
- **Frontend changes** — none. `RuntimeEventEnvelope` and the runtime channel
  are unaffected; this only changes how the same envelope is **persisted**.
- **Backfill / data migration** — historical `run.jsonl` files keep their
  null correlation columns. No retroactive repair.

## 4. Scope (in)

### 4.1 Code changes

Three files have inbound calls to swap, plus the legacy helper removal:

| File                                                                      | Lines today                              | Change                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/modules/application/turn_service/stream_iteration.rs`      | 60 (`use`), 208, 482, 580                | Move `append_stream_event` from the `super::stream_task::{...}` block to a separate `use super::stream_task_run_log::append_stream_event;`. Other helpers (`apply_memory_recall_success_finalization_guard`, `should_retry_*`, `tool_batch_signature`, `REPEATED_TOOL_BATCH_LIMIT`) stay in `stream_task`. |
| `src-tauri/src/modules/application/turn_service/stream_event_loop.rs`     | 36 (`use`), 241, 275, 363, 397, 533, 565 | Single-line `use super::stream_task::append_stream_event;` → `use super::stream_task_run_log::append_stream_event;`. 6 call sites unchanged.                                                                                                                                                               |
| `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs` | 362, 410, 523, 732                       | 4 inline call paths `super::stream_task::append_stream_event(...)` → `super::stream_task_run_log::append_stream_event(...)`. No `use` import involved.                                                                                                                                                     |
| `src-tauri/src/modules/application/turn_service/stream_task.rs`           | 234-250                                  | **Delete** the legacy `pub(super) async fn append_stream_event` (17 lines).                                                                                                                                                                                                                                |

### 4.2 Test additions

Add one `#[tokio::test]` to `stream_task_run_log.rs` (new `#[cfg(test)] mod tests`):

- Construct a `RunEventLogger` over a unique temp dir (reuse the
  `unique_temp_root` pattern from `event_log.rs` tests).
- Construct a `StreamTokenPayload` with
  `correlation: Some(CorrelationIds { stream_id, team_id, member_id, parent_run_id, … })`
  and a representative `event_type` (`"delta"`).
- Call `append_stream_event(&logger, &payload).await`.
- Read the resulting `run.jsonl` line, deserialize as `RunLogEntry`.
- Assert top-level `team_id`, `member_id`, `correlation_id`, `parent_run_id`
  match the input correlation (i.e. `merge_correlation_from_ids` ran).
- Assert event-type normalization still works for one mapped variant
  (e.g. `tool_call_update` + `tool_status: "running"` → `tool_call_running`)
  in the same or a sibling test.

This test is the **contract gate**: any future helper that bypasses
`append_with_correlation` will fail it.

## 5. Verification

Run inside the worktree (`cd ~/Documents/IfAI/if2Ai-sp-c/`):

```bash
# 1. New test passes (and so do existing turn_service tests)
cargo test --lib turn_service::stream_task_run_log
cargo test --lib turn_service

# 2. Full lib test pass (catches accidental import or signature breakage)
cargo test --lib

# 3. No leftover references to the deleted helper anywhere in the codebase
rg -n 'stream_task::append_stream_event'           # must be 0 hits
rg -n 'pub\(super\) async fn append_stream_event' src-tauri/src/modules/application/turn_service/stream_task.rs   # must be 0 hits

# 4. Both call paths converge on the corrected helper
rg -n 'stream_task_run_log::append_stream_event'   # must be ≥ 7 hits (4 in stream_tool_execution + 3+1 in stream_iteration/stream_event_loop via use-import sites count differently — be explicit per file)

# 5. cargo check on the full crate (non-test compile)
cargo check
```

The plan document will list expected exact hit counts per `rg` invocation
once written.

## 6. Risk assessment

| Risk                                                                                                                                                                                                                                                        | Likelihood                           | Mitigation                                                                                                                                                                                        |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden caller in test fixtures uses the deleted `stream_task::append_stream_event`                                                                                                                                                                          | Low (grep above is the verification) | `rg` step 3 makes failure loud during verification, before commit                                                                                                                                 |
| `stream_task_run_log::append_stream_event` is `pub(super)`, but the new callers are in sibling modules of the same `turn_service` parent — visibility holds (already proven by current `stream_task::append_stream_event` working from the same call sites) | None                                 | n/a                                                                                                                                                                                               |
| Performance regression from `merge_correlation_from_ids` overlay                                                                                                                                                                                            | Negligible                           | One `if let Some(c) = correlation { entry.merge_correlation_from_ids(c); }` branch per event. The streaming firehose already does serialization and async file I/O on every event; this is noise. |
| Missed event-type variant in test, masking a future regression                                                                                                                                                                                              | Low                                  | The test asserts the specific normalization pair (`tool_call_update + running → tool_call_running`) plus the correlation contract — covers both responsibilities of the helper.                   |
| Conflict with in-flight work in main workspace (`progressive retry escalation`, `after_turn workdir/utility_llm`)                                                                                                                                           | None at SP-C merge time              | Diffs verified orthogonal in brainstorming Q1 — main workspace edits do not touch `append_stream_event` callers. User rebases their wip branch normally after SP-C lands.                         |

## 7. Rollback

Single PR, single logical commit (or two: one for the swap+delete, one for the
test — depending on plan). `git revert` recovers cleanly. No data migration,
no schema change, no on-disk format change.

## 8. Out-of-scope follow-ups (capture for later)

- **SP-C2 (FIX-9)**: Resolve `runtime → application` reverse dependency.
  Needs separate brainstorm (4 design options enumerated in SP-C audit notes).
- **SP-D (god-file decomposition)**: `stream_task.rs` is now 996 → ~979 lines
  after the deletion. Still well over the 500-line guideline; SP-D extraction
  remains separate work.
- **Run-log replay tests for Agents Teams correlation**: A higher-level
  integration test that drives a multi-member team through `stream_task` and
  asserts every persisted entry carries `team_id` would be ideal — but
  expensive (Provider/Tool/Permission mocks). Track separately.

## 9. Approval gate

This spec is approved when the user replies "approved" or "approve" or
equivalent. Until then, no plan and no code changes.
