# AWL-011: Tool Intent Nudge & Completion Evidence Invariant

## Status
- State: done

## Goal
Ensure tool-required work cannot complete from prose alone when the assistant
announces it will inspect, write, run, or verify with tools.

## Spec (verifiable)
- Assistant prose that says it will use tools but emits no tool call is detected
  -> test `assistant_tool_intent_nudge_detects_announced_action`.
- The loop issues one nudge with required tool choice before terminal failure
  -> test `announced_tool_intent_gets_one_nudge_retry`.
- Tool-required work without successful mutating tool evidence remains failed /
  resumable, not completed -> test `tool_required_no_tool_report_failed_with_plan`.
- Failed mutating attempts do not satisfy completion evidence
  -> test `failed_mutating_tool_attempt_does_not_satisfy_continuation_evidence`.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_preflight.rs`
- `src-tauri/src/modules/runtime/stream_outcome.rs`
- `src-tauri/src/modules/runtime/recoverability.rs`

## Done
- Tool intent nudge is part of the canonical TurnService streaming path.
- Completion evidence requires successful mutating tool results for artifact work.
