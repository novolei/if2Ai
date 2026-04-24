# MIG-020 Session Supervisor Foundation

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-24`

---

## Goal

引入 session supervisor 基础层，统一管理 active run、disconnect grace period、recoverable state、retry budget、pending permission 等会话生命周期状态。

## Depends On

- `MIG-016`
- `MIG-019`

## Unlocks

- `MIG-021`
- `MIG-022`

## Why Now

1. 当前 active run / stop / retry / permission 状态分散在 turn_service、store、UI 之间。
2. 没有 supervisor，就很难建立稳定的 session continuity product surface。
3. benchmark 的 server layer 强项，本质就是 session supervisor。

## Allowed Files

- `src-tauri/src/modules/application/**`
- `src-tauri/src/commands/session.rs`
- `src/api/**`
- `src/stores/session-store.ts`
- `src/runtime-projection/**`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Forbidden Files

- `src-tauri/src/modules/learning/**`
- `src/modules/settings/**`
- `docs/_legacy/**`

## Source Of Truth

- [Current Architecture](../../../../ARCHITECTURE.md)
- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-019 Pending Permission Recovery](./MIG-019-pending-permission-recovery.md)
- [MIG-015 Gateway Conversations And Streaming Surface](./MIG-015-gateway-conversations-and-streaming-surface.md)

## Required Changes

1. 为每个 session 建立统一 supervisor snapshot：`active_run_id / run_status / recoverable / pending_permission_count / last_error_kind / disconnect_grace_until`。
2. start / stop / error / complete / permission pending 时同步更新 supervisor 状态。
3. 前端能读取 supervisor snapshot，而不是从零散状态拼装。
4. 至少新增测试验证 session 状态迁移。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml supervisor`
- `npm test -- session-store`
- `npm run build`
- 前端可区分 active / blocked / recoverable failed 三种 session 状态

## T-006 Execution Summary (2026-04-24)

- Status: done (T-006).
- Evidence: supervisor.rs with SupervisorSnapshot + 9 lifecycle methods + persistence + 5 tests + get_supervisor_snapshot API.
- Remaining: T-007 (supervisor projection frontend).

## Out Of Scope

- 不完成 resume 行为本身
- 不做多 viewer 协调
- 不重写 chat UI

## Execution Notes

- supervisor 先做状态真相，不急着做复杂调度。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
