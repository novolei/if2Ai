# AWL-012: Todo Ledger Enforcement & Tool-Step Evidence

## Status
- State: done

## Goal
Make `TodoWrite` a runtime ledger instead of a cosmetic progress list. For
tool-required work, an unfinished todo ledger must guide the next tool-backed
step and prevent the run from being marked complete without matching tool
evidence.

## Spec (verifiable)
- `TodoWrite` does not count as artifact mutation evidence
  -> test `todo_write_is_not_artifact_mutation_evidence`.
- Tool-required turns with an unfinished todo ledger receive a prompt
  contribution that names the active/pending ledger state
  -> covered by `todo_ledger_incomplete_forces_terminal_status` and the
  `todo_ledger` prompt path.
- After a real non-`TodoWrite` mutating tool succeeds, the loop nudges the
  model to reconcile `TodoWrite` before continuing.
- A tool-required run with unfinished active/pending todos cannot close as
  completed; it finalizes as `todo_ledger_incomplete`
  -> test `todo_ledger_incomplete_forces_terminal_status`.
- Specific failure statuses such as invalid tool args or approval blocks are
  not overwritten by the todo guard
  -> test `todo_ledger_does_not_override_specific_failure_status`.
- `todo_ledger_incomplete` is failed/resumable and appears in
  `FinalRunReport`
  -> tests `todo_ledger_incomplete_is_failed_and_resumable`,
  `classify_todo_ledger_incomplete`,
  `todo_ledger_incomplete_report_failed_with_plan`.

## Files (scope)
- `src-tauri/src/modules/tools/builtin/todo_write.rs`
- `src-tauri/src/modules/application/tool_heuristics.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/todo_ledger.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/runtime/stream_outcome.rs`
- `src-tauri/src/modules/runtime/recoverability.rs`

## Done
- Todo ledger state is read through the canonical `TodoWrite` store path.
- `TodoWrite` no longer satisfies artifact completion evidence by itself.
- Unfinished active/pending todos block completed finalization on natural model
  stops while preserving more specific failure/approval statuses.
- Final reports include todo-ledger failure diagnostics and remain resumable.
