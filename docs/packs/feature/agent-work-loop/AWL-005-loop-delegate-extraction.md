# AWL-005: LoopDelegate Extraction

## Status
- State: active

## Goal
Extract a small `AgentLoopDelegate` boundary around the existing streaming loop so
future loop kinds can evolve without continuing to enlarge `stream_task.rs` and
finalization glue.

## Spec (verifiable)
- Existing streaming success path runs through `StreamingAgentLoopDelegate` and returns `Completed` -> test `tests::loop_delegate_streaming_success_completed`.
- Provider error maps to `FailedWithPlan` with final report inputs -> test `tests::loop_delegate_provider_error_failed_with_plan`.
- Approval block maps to `NeedsApproval` without losing pending operation metadata -> test `tests::loop_delegate_approval_blocked`.
- Budget exhaustion maps to `ExhaustedWithSummary` -> test `tests::loop_delegate_budget_exhausted`.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/agent_loop_delegate.rs` (new)
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream.rs`
- `src-tauri/src/modules/application/turn_service/stream_event_loop.rs`
- `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/runtime/contracts/agent_loop.rs`

## Reads
- `docs/design-docs/agent-work-loop/AWL-005-loop-delegate-extraction.md`
- `docs/design-docs/agent-work-loop/AWL-002-work-loop-router-enforcement.md`

## Contract
- Behavior-preserving extraction first; no new provider semantics.
- Keep event names, IPC commands, and sqlite schema unchanged.
- Keep `TurnService` as the runtime entry point.
- No frontend changes except type compatibility if compilation requires it.

## Out of Scope
- New specialized delegates.
- MCP Workbench.
- Skill marketplace or remote skill install flow.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass.
- `REGISTRY.md` moves this pack to done with date.
