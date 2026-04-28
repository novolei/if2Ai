# AWL-010: Continuation Context & Tool Evidence Invariants

## Status
- State: done

## Goal
Make "继续 / continue" inherit the prior unfinished concrete work request and
prevent artifact-building turns from completing without real mutating tool
evidence.

## Spec (verifiable)
- Chinese web/game creation routes to `AutoPlanExecute`, not short direct answer -> test `chinese_web_game_creation_routes_to_auto_plan_execute`.
- Tool-required creation requests keep tools exposed and cannot become `DirectAnswer` -> test `creation_request_requires_tools_not_direct_answer`.
- "继续" inherits a prior unfinished tool-required request as `AutonomousWork` with `tool_required_work_intent` -> test `continue_inherits_tool_required_work_from_previous_request`.
- "继续" still recovers the earlier goal after a prior assistant emitted fake function-call text with no real tool evidence -> test `continue_recovers_earlier_tool_required_goal_when_prior_continue_faked_tool_text`.
- Tool-required no-tool runs retry once, then end as failed/resumable `tool_required_no_tool` -> tests `tool_required_no_tool_gets_one_retry_before_terminal_failure`, `tool_required_no_tool_is_failed_and_resumable`.
- Run Inspector displays canonical loop reasons and tool-definition policy -> test `run inspector exposes canonical work loop reason codes and tool policy`.
- Developer telemetry owns Run Inspector and execution decision display; chat transcript/top chrome stay compact -> tests `run inspector is embedded in developer telemetry instead of floating over chat`, `execution mode pill lives in developer telemetry instead of the app title chrome`.

## Files (scope)
- `docs/design-docs/agent-work-loop/AWL-010-continuation-context-tool-evidence-invariants.md`
- `docs/packs/feature/agent-work-loop/AWL-010-continuation-context-tool-evidence-invariants.md`
- `docs/packs/REGISTRY.md`
- `src-tauri/src/modules/control_plane/ingress_classifier.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/runtime/stream_outcome.rs`
- `src-tauri/src/modules/runtime/recoverability.rs`
- `src/components/chat/RunInspectorPanel.tsx`
- `src/components/chat/TelemetryDrawer.tsx`
- `src/components/chat/RunInspectorPanel.final-run-report.test.ts`
- `src/components/ui/chat-ui.tsx`
- `src/modules/execution-mode/ExecutionModePill.tsx`
- `src/modules/chat/components/ChatWorkspace.tsx`
- `src/shell/MainShell.tsx`
- `src/App.tsx`
- `src/main.tsx`
- `src/boot/browser-runtime-guard.test.ts`
- `src/runtime-projection/final-run-report.test.ts`

## Reads
- `docs/packs/feature/agent-work-loop/AWL-002-work-loop-router-enforcement.md`
- `docs/design-docs/agent-work-loop/AWL-002-work-loop-router-enforcement.md`
- `docs/design-docs/agent-work-loop/AWL-010-continuation-context-tool-evidence-invariants.md`

## Contract
- Keep `TurnService` as the only runtime entry point.
- Do not add sqlite schema, IPC command, or provider API changes.
- Do not execute fake textual `<function_calls>`; only real tool events count.
- Do not weaken AWL-002 DirectAnswer tool hiding for ordinary direct answers.

## Out of Scope
- Provider-specific fake tool-call parsing; covered by `PROV-001`.
- MCP Workbench or skill marketplace changes.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service::work_loop -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service::stream_task::tests::tool_required_no_tool_gets_one_retry_before_terminal_failure -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml control_plane::ingress_classifier -- --nocapture && cargo test --manifest-path src-tauri/Cargo.toml runtime::stream_outcome -- --nocapture && cargo test --manifest-path src-tauri/Cargo.toml runtime::recoverability -- --nocapture`
- `npm run test -- browser-runtime-guard final-run-report`
- `npm run build:web`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## Done
- Verify commands pass and live validation follows the design-doc runbook.
