# CTX-001: Continuation Transcript Truth

## Status
- State: done

## Goal
Make short continuation turns such as "继续" inherit the last unfinished
tool-required task from durable event-log facts instead of guessing from text.

## Spec (verifiable)
- Durable `final_run_report` with `tool_required_no_tool` restores failed /
  resumable context -> test `ctx_001_replays_unfinished_goal_from_event_log`.
- Continuation prompt contribution includes unfinished goal, failure reason, and
  resume cursor when available.
- No new session/runtime truth source is introduced; replay reads
  `runtime::history::read_session_history_event_page`.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/runtime/history.rs` (read-only API use)

## Done
- `prepare_chat_inputs` augments route context from the canonical run event log.
- Continuation context enters provider prompt as a structured contribution.
