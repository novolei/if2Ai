# AWL-002: WorkLoopRouter Enforcement

## Status
- State: active

## Goal
Turn loop routing from descriptive metadata into enforceable runtime policy. Simple
turns should not get tools, plan-first turns should block mutation before approval,
and every route must finish with a final report.

## Spec (verifiable)
- `DirectAnswer` strips tool definitions and emits a direct-answer final report -> test `tests::work_loop_router_direct_answer_hides_tools`.
- `PlanThenConfirm` allows read-only planning but blocks mutating tools before approval -> test `tests::work_loop_router_plan_then_confirm_blocks_mutation`.
- `SpecializedSurface` still emits routing and final-report events -> test `tests::work_loop_router_specialized_surface_reports`.
- Provider failure or budget exhaustion maps to `FailedWithPlan` / `ExhaustedWithSummary` -> test `tests::work_loop_router_terminal_failures_report`.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/stream.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/runtime/contracts/agent_loop.rs`
- `src/runtime-projection/types.ts`
- `src/runtime-projection/runtime-event-translator.ts`

## Reads
- `docs/design-docs/agent-work-loop/AWL-002-work-loop-router-enforcement.md`
- `src-tauri/src/modules/application/request_intelligence_service.rs`
- `src-tauri/src/modules/control_plane/tool_execution_broker.rs`

## Contract
- Keep `TurnService` as the only turn entry point.
- Do not rename existing runtime event names or IPC commands.
- Do not add a second frontend/runtime truth source.
- Remote/community skills remain proposal-only.

## Out of Scope
- `AgentLoopDelegate` extraction; covered by `AWL-005`.
- MCP Workbench UI; covered by `MCP-002`.
- HTTP/SSE/WS MCP transports.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service::work_loop -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass.
- `REGISTRY.md` moves this pack to done with date.
