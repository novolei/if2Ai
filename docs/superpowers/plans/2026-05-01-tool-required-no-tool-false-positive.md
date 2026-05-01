# Plan: Fix `tool_required_no_tool` false-positive on direct_answer turns

**Date**: 2026-05-01  
**Scope**: bug fix in `turn_service::stream_iteration` terminal-status derivation.

## §5 Root cause (systematic-debugging Phase 1–3, completed)

Latest run `52abe145…/dbd0a4be…jsonl` (5/1 14:21):

- `executionMode = direct_execute`, `workLoopDecision.loopKind = direct_answer`,
  `requires_tool_execution_evidence = false`, `tool_choice = none`, `tool_count = 0`.
- Assistant produced pure prose explaining token cost (no tool intent).
- Yet `stream_complete.degraded_reason = tool_required_no_tool`,
  `task_outcome = failed`, `resume_available = true`.

In [`stream_iteration.rs`](../../../src-tauri/src/modules/application/turn_service/stream_iteration.rs)
line 717–726, a match arm sets `terminal_status = "tool_required_no_tool"` whenever
`assistant_claims_tool_execution_without_tool(text)` returns true, **independently
of `work_loop_decision`**. That helper (in `work_loop.rs:1339`) is a fuzzy keyword
OR-of-OR over Chinese/English tokens (`工具`/`bash`/… × `写入`/`write`/…). The
prose contained the substrings `工具` + `_write` (lower-cased contains `write`),
so it false-fires on innocent direct_answer prose.

The arm is also redundant: when the work loop genuinely demands tool evidence,
the prior arm at line 708 already covers it via
`requires_tool_execution_evidence(work_loop)`. The keyword-claim branch only
adds value for the **textual-markup** signal (`<function_calls>`, `<invoke>`,
`tool_use` literal), which is already separately captured into
`state.provider_textual_tool_markup_seen` earlier in `handle_no_tool_calls`.

## Fix (Phase 4)

1. Extract pure helper `no_tool_terminal_status(...)` from the inline `match` so
   it is unit-testable.
2. Replace the keyword-claim arm with a markup-only arm:
   `provider_textual_tool_markup_seen && !has_successful_mutating_tool` →
   `Some("provider_textual_tool_call_markup")`. Keyword-fuzzy logic no longer
   gates terminal status.
3. The existing `requires_tool_execution_evidence` arm continues to terminal-fail
   genuine tool-required turns that produced no mutating tool.

## Tasks

- [x] §5 root cause documented above.
- [x] Extract `no_tool_terminal_status` helper.
- [x] Add unit tests (5 cases, including direct-answer + `工具`/`_write` regression).
- [x] Replace inline match with helper at the call site.
- [x] `cargo test -p if2ai-backend --lib no_tool_terminal_status` — 5 passed / 0 failed.

## §8 TDD note

Following systematic-debugging Phase 4: write failing test first, then minimal
fix. The first regression test (direct_answer + prose) MUST fail on current
master before the helper change is applied.

## N/A

- **Brainstorming N/A** — Skill `brainstorming` (`SKILL.md` `description`) is
  for *creative work / new features / behavior modification*; this is a defect
  with a single explicit root cause and a minimal in-place fix, not a design
  exploration.
