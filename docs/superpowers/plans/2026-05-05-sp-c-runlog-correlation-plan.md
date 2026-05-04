# SP-C Plan — Streaming Run-Log Correlation 修复（FIX-7）

**Spec**: `2026-05-05-sp-c-runlog-correlation-spec.md` (APPROVED 2026-05-05)
**Worktree**: `~/Documents/IfAI/if2Ai-sp-c/`
**Branch**: `sp-c/runlog-correlation-unify`
**Fork point**: `vnext@2b8eda9` (post-SP-F1)
**Execution model**: subagent-driven-development (one task per fresh subagent, two-stage review)

---

## Strategy

Two surgical commits + final verification. Test-first ordering because the
new test passes immediately against the current (correct) `stream_task_run_log`
helper — landing it in **C1** locks the contract before **C2** rewires the
callers, so if C2 ever needs to be reverted the contract test stays valid
to catch a future re-introduction of the broken pattern.

```
C1 (Task 1)  add stream_task_run_log unit contract test         →  green immediately, locks "writes preserve correlation"
C2 (Task 2)  swap 7 callsites + delete legacy duplicate         →  the actual fix
Final        full cargo test + 5 rg gates + push branch + open PR
```

No `git revert` needed mid-plan. If C2 fails review, only C2 is amended/dropped.

## Pre-flight checks (do once, before Task 1)

Run inside `~/Documents/IfAI/if2Ai-sp-c/`:

```bash
git status -s                               # expect: empty
git rev-parse HEAD                          # expect: 2b8eda9...
git branch --show-current                   # expect: sp-c/runlog-correlation-unify
git worktree list                           # expect: main + this worktree
```

If any check fails, **stop** and re-evaluate.

---

## Task 1 — Add unit contract test to `stream_task_run_log.rs` (commit C1)

### Files

- **Modify**: `src-tauri/src/modules/application/turn_service/stream_task_run_log.rs`
  (currently 138 lines)

### Steps

1. Append a `#[cfg(test)] mod tests` block at the end of the file with **two**
   `#[tokio::test]` async tests:

   - **`append_stream_event_preserves_payload_correlation`**
     - Build a `RunEventLogger` rooted at a unique temp dir
       (`std::env::temp_dir().join(format!("sp-c-corr-{}", uuid_or_nanos))`)
       via `RunEventLogger::for_base_dir(&root, "sess-c", "run-c")`.
     - Build a `StreamTokenPayload` with:
       - `event_type: "delta".into()`
       - `correlation: Some(CorrelationIds { stream_id: Some("stream-1".into()), team_id: Some("team-a".into()), member_id: Some("mem-1".into()), parent_run_id: Some("parent-7".into()), delegation_id: Some("deleg-9".into()), ..Default::default() })`
       - other fields filled with defaults / minimal valid values per
         `StreamTokenPayload`'s actual shape (look at the struct in
         `runtime::stream_emitter` to fill required fields).
     - Call `append_stream_event(&logger, &payload).await`.
     - Read `logger.file_path().unwrap()` to a `String`.
     - Take the first non-empty line, parse as `RunLogEntry`
       (`serde_json::from_str`).
     - Assert:
       - `entry.team_id.as_deref() == Some("team-a")`
       - `entry.member_id.as_deref() == Some("mem-1")`
       - `entry.parent_run_id.as_deref() == Some("parent-7")`
       - `entry.delegation_id.as_deref() == Some("deleg-9")`
       - `entry.correlation_id.as_deref() == Some("stream-1")`
         (this is what `merge_correlation_from_ids` does with `stream_id`;
         double-check the field name by reading
         `event_log.rs::merge_correlation_from_ids` to confirm — the
         existing test `append_with_correlation_persists_team_fields_to_jsonl`
         uses `entry.correlation_id.as_deref() == Some("stream-99")`, so
         this is correct.)
       - `entry.event_type == "delta"` (no normalization for plain `delta`).
     - Cleanup: best-effort `std::fs::remove_dir_all(&root).ok()`.

   - **`append_stream_event_normalizes_tool_call_update_running_to_tool_call_running`**
     - Same logger setup.
     - `StreamTokenPayload` with `event_type: "tool_call_update".into()` and
       `tool_status: Some("running".into())` (the other relevant fields can
       use defaults; `correlation: None` is fine here — this test isolates
       the normalization branch).
     - Call `append_stream_event`, read jsonl line, parse `RunLogEntry`.
     - Assert `entry.event_type == "tool_call_running"`.
     - Cleanup as above.

2. **Imports** at the top of the new `mod tests` block:
   - `use super::*;` (to get the local `append_stream_event`)
   - `use crate::modules::runtime::contracts::common::CorrelationIds;`
   - `use crate::modules::runtime::event_log::{RunEventLogger, RunLogEntry};`
   - `use crate::modules::runtime::stream_emitter::StreamTokenPayload;`
   - Adjust paths if the actual modules differ — read
     `runtime/event_log.rs` (lines 432-457 already show the pattern) and
     mirror it.

3. The `unique_temp_root` helper used by `event_log.rs` tests is
   `pub(crate)` or `pub`-in-its-own-mod-tests; if not accessible from this
   sibling module's tests, **inline the equivalent** (4 lines, see
   `event_log.rs` tests) rather than expanding the helper's visibility.

### Why this commit is safe

The current `stream_task_run_log::append_stream_event` (lines 49-67) already
calls `append_with_correlation`. Both new tests pass against `vnext@2b8eda9`
unchanged. **C1 adds tests only — no production code changes — so it cannot
break anything.** The contract is locked first.

### Verification

```bash
cd ~/Documents/IfAI/if2Ai-sp-c
cargo test --lib turn_service::stream_task_run_log -- --nocapture
# Expect: 2 passed (both new tests)

cargo test --lib
# Expect: full suite still green
```

### Commit message (Task 1)

```
test(turn_service): add stream_task_run_log contract tests

Lock the invariant that append_stream_event in stream_task_run_log
preserves payload.correlation (Agents Teams team_id / member_id /
parent_run_id / delegation_id / stream_id → correlation_id) and
normalizes event types correctly.

Lands BEFORE the call-site migration (next commit) so the contract
gate is in place — any future helper that bypasses
RunEventLogger::append_with_correlation will fail these tests.

Two tests:
- append_stream_event_preserves_payload_correlation: full overlay
  via merge_correlation_from_ids.
- append_stream_event_normalizes_tool_call_update_running_to_tool_call_running:
  exercises the event_type normalization branch.

Refs: SP-C / FIX-7. No production code change.
```

### Hard rules for the implementer (Task 1)

- **Do NOT** modify any production code in this commit. Test-only.
- If `StreamTokenPayload` requires non-trivial fields, fill them with the
  smallest plausible values (e.g. empty strings, `0`, `Vec::new()`) — do not
  invent business logic.
- If `unique_temp_root` is private, inline a 4-line equivalent. Do not
  widen visibility on existing helpers.
- Use a unique temp-dir suffix (`std::time::SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()` or similar) so parallel `cargo test` runs don't collide.
- Selective `git add` of the modified file only — do **not** `git add -A`
  (worktree should be clean per pre-flight, but defensive).

---

## Task 2 — Swap 7 callsites + delete legacy duplicate (commit C2)

### Files

- `src-tauri/src/modules/application/turn_service/stream_iteration.rs`
  (modify `use` block at line ~60; 3 call sites at 208, 482, 580 unchanged
  by name — they're already calling unqualified `append_stream_event`)
- `src-tauri/src/modules/application/turn_service/stream_event_loop.rs`
  (modify single-line `use` at line 36; 6 call sites at 241, 275, 363,
  397, 533, 565 unchanged)
- `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs`
  (4 inline qualified calls at 362, 410, 523, 732 — rewrite each path)
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
  (delete `pub(super) async fn append_stream_event` at lines 234-250;
  17 lines including any preceding `///` doc lines if present)

### Steps

1. **`stream_iteration.rs`**: Locate the `use super::stream_task::{...};`
   block (currently at lines 59-63):
   ```rust
   use super::stream_task::{
       append_stream_event, apply_memory_recall_success_finalization_guard,
       should_retry_announced_tool_intent_no_tool, should_retry_tool_required_no_tool,
       tool_batch_signature, REPEATED_TOOL_BATCH_LIMIT,
   };
   ```
   Remove `append_stream_event` from the brace list and add a separate
   import line directly below:
   ```rust
   use super::stream_task::{
       apply_memory_recall_success_finalization_guard,
       should_retry_announced_tool_intent_no_tool, should_retry_tool_required_no_tool,
       tool_batch_signature, REPEATED_TOOL_BATCH_LIMIT,
   };
   use super::stream_task_run_log::append_stream_event;
   ```
   The 3 call sites (`append_stream_event(refs.run_event_logger, &payload).await`
   at lines 208, 482, 580) require **no further changes** — they already use
   the unqualified name resolved by the new import.

2. **`stream_event_loop.rs`**: Replace the single line at 36:
   ```rust
   use super::stream_task::append_stream_event;
   ```
   with:
   ```rust
   use super::stream_task_run_log::append_stream_event;
   ```
   The 6 call sites are unchanged.

3. **`stream_tool_execution.rs`**: 4 inline replacements (no `use` to add —
   each call is already fully qualified):
   ```diff
   - super::stream_task::append_stream_event(...)
   + super::stream_task_run_log::append_stream_event(...)
   ```
   At lines 362, 410, 523, 732. Use `StrReplace` per line if any `...`
   text differs; otherwise `replace_all = true` is acceptable IF a
   pre-replacement grep confirms exactly 4 hits in this file and zero
   collisions.

4. **`stream_task.rs`**: Delete the entire `pub(super) async fn append_stream_event`
   function at lines 234-250 (verify line range with a fresh `Read`
   before deleting — earlier in-file edits could shift lines). Include
   the function signature, body, and any directly preceding `///` doc
   comments. Do NOT touch `append_remembered_permission_events` or the
   guard predicates that follow.

### Verification (run all of these — every one must pass)

```bash
cd ~/Documents/IfAI/if2Ai-sp-c

# 1. Tests
cargo test --lib turn_service
# Expect: full turn_service test suite green, including the 2 tests added in Task 1

cargo test --lib
# Expect: full lib suite green

# 2. Compile
cargo check
# Expect: clean (warnings OK, errors not)

# 3. Anti-regression greps (exact expected counts):

rg -n 'stream_task::append_stream_event' src-tauri/src/
# Expect: 0 hits (zero callers of the deleted function)

rg -n 'pub\(super\) async fn append_stream_event' src-tauri/src/
# Expect: 1 hit — only stream_task_run_log.rs:49

rg -n 'stream_task_run_log::append_stream_event' src-tauri/src/
# Expect: ≥ 5 hits — break down:
#   stream_iteration.rs:    1 (the use-import)
#   stream_event_loop.rs:   1 (the use-import)
#   stream_tool_execution.rs: 4 (inline qualified calls)
#   = 6 total minimum

rg -n 'fn append_stream_event' src-tauri/src/
# Expect: exactly 1 hit — stream_task_run_log.rs:49
```

If any verification step fails, **do not commit**. Fix and re-verify.

### Commit message (Task 2)

```
fix(turn_service): route streaming events through correlation-aware run-log helper (FIX-7)

Commit 595c48f introduced stream_task_run_log::append_stream_event,
which calls RunEventLogger::append_with_correlation so that
payload.correlation (team_id / member_id / parent_run_id /
delegation_id / stream_id) is promoted to RunLogEntry top-level
columns. The 7 production call sites were never switched, so 100%
of streaming events have been persisted to run.jsonl with these
fields = None — silently breaking Agents Teams replay/audit/
correlation queries on stored logs.

This commit:
- Switches all 7 call sites in stream_iteration.rs (3),
  stream_event_loop.rs (6 via single-import change), and
  stream_tool_execution.rs (4 inline qualified calls) to use
  stream_task_run_log::append_stream_event.
- Deletes the now-orphan duplicate from stream_task.rs (17 lines).

Forward effect: every streaming event written from this commit
forward carries the full CorrelationIds overlay if the in-memory
StreamTokenPayload had one. Historical run.jsonl files are not
backfilled (out of scope).

Behaviour for events without payload.correlation is unchanged
(merge_correlation_from_ids no-ops on None). The contract is
locked by the unit tests added in the previous commit.

Refs: SP-C / FIX-7. ARCHITECTURE.md §9.3 (Agents Teams correlation).
```

### Hard rules for the implementer (Task 2)

- **Re-read each file before editing** — line numbers in this plan are
  from `vnext@2b8eda9`; if Task 1 added lines anywhere (it should only
  have touched `stream_task_run_log.rs`), other files are unaffected,
  but verify.
- **Do not** rename, reorder, or otherwise touch `append_remembered_permission_events`
  or any guard predicate in `stream_task.rs`. Surgical deletion only.
- **Do not** move `apply_memory_recall_success_finalization_guard`,
  `should_retry_*`, `tool_batch_signature`, or `REPEATED_TOOL_BATCH_LIMIT`
  into `stream_task_run_log.rs` opportunistically. SP-D will deal with
  god-file decomposition; SP-C stays narrow.
- **Selective `git add`** of the 4 modified files only.
- If any `rg` count differs from the expected, **stop, report, do not
  commit** — investigate whether Task 1's test edits shifted something
  unexpectedly or there's an unexpected fixture caller.

---

## Final verification (after both commits land)

```bash
cd ~/Documents/IfAI/if2Ai-sp-c

git log --oneline -5
# Expect (top-down):
#   <C2 sha> fix(turn_service): route streaming events through correlation-aware run-log helper (FIX-7)
#   <C1 sha> test(turn_service): add stream_task_run_log contract tests
#   2b8eda9  docs: populate CLAUDE.md with Pack workflow and vNext architecture guide
#   ffbbf13  docs(plan): SP-F1 followups — 7 minor items captured for SP-F2 / SP-A
#   b6bf7d1  chore(stores): stop re-exporting appendMessage/updateMessage from chat-store (FIX-17)

git diff vnext --stat
# Expect:
#   ~ 4 source files changed in src-tauri/src/modules/application/turn_service/
#   ~ +30 ish, -25 ish (test +40, callsite swap +7/-7, helper delete -17, use-block tweak ±2)
#   + 2 docs files (this plan + the spec)

cargo test --lib                                  # full pass
cargo check                                       # clean
rg -n 'stream_task::append_stream_event' src-tauri/src/  # 0
```

Push and open PR (after final verification + integrated review pass):

```bash
git push -u origin sp-c/runlog-correlation-unify
gh pr create --base vnext --title "fix(turn_service): route streaming events through correlation-aware run-log helper (SP-C / FIX-7)" --body "$(cat <<'EOF'
## Summary

Commit 595c48f introduced a corrected `append_stream_event` helper that
preserves Agents Teams correlation fields (team_id / member_id /
parent_run_id / delegation_id / stream_id) when persisting streaming
events to `run.jsonl`, but **forgot to switch the 7 production call
sites**. As a result, 100% of streaming events on `vnext` today are
persisted with those fields = `None`, silently breaking Agents Teams
replay/audit on stored logs.

This PR:
- Adds 2 unit-level contract tests in `stream_task_run_log.rs` that lock
  "streaming run-log writes preserve `payload.correlation`" + verify
  event_type normalization still works (commit 1).
- Switches all 7 call sites to the corrected helper and deletes the
  orphan duplicate from `stream_task.rs` (commit 2).

## Spec & Plan

- Spec: `docs/superpowers/plans/2026-05-05-sp-c-runlog-correlation-spec.md`
- Plan: `docs/superpowers/plans/2026-05-05-sp-c-runlog-correlation-plan.md`

## Test plan

- [x] `cargo test --lib turn_service::stream_task_run_log` (2 new tests pass)
- [x] `cargo test --lib` (full suite green)
- [x] `cargo check` (clean)
- [x] `rg 'stream_task::append_stream_event' src-tauri/src/` returns 0
- [x] `rg 'fn append_stream_event' src-tauri/src/` returns exactly 1 (the canonical helper)

## Out of scope

- FIX-9 (`runtime → application` reverse dependency) — deferred to SP-C2.
- SP-D god-file decomposition — `stream_task.rs` is now ~979 lines (down
  from 996) but still over the guideline; tracked separately.
- Historical `run.jsonl` backfill — forward-only fix.
EOF
)"
```

## Review pipeline (per subagent-driven-development)

After Task 1 commit: spawn a `code-reviewer` subagent to review the test
quality (correct field names, parallel-safe temp dir, appropriate assertions,
no production drift).

After Task 2 commit: spawn a `code-reviewer` subagent to review surgical
correctness (only intended sites changed, no incidental edits, all 5 grep
gates pass with stated counts).

After both: a final integrated review across the 2-commit sequence before
push.

If any reviewer flags blocking issues, fixup commit before proceeding.

## Rollback

Single PR, two cleanly separable commits. `git revert <C2-sha>` restores
the broken-but-running pre-state while keeping the contract test (which
will then start failing — useful, makes the regression loud). Full
`git revert` of both commits returns to `2b8eda9` exactly.
