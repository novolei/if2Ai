# MIG-021 Resume Contract And Run Recovery

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

把当前 `resume_cursor` 升级成完整的 run recovery contract，让系统不仅知道“能不能 resume”，还知道“为什么能、从哪里恢复、风险是什么”。

## Depends On

- `MIG-018`
- `MIG-020`

## Unlocks

- `MIG-023`

## Why Now

1. 当前 resume 更像失败后的字符串提示，还不是产品级恢复契约。
2. session supervisor 立住后，需要明确 recoverable run 的 typed reason。
3. mutating tool / partial success / timeout 的恢复风险不能继续靠 UI 猜。

## Allowed Files

- `src-tauri/src/modules/runtime/**`
- `src-tauri/src/modules/application/turn_service/**`
- `src-tauri/src/modules/application/**`
- `src/runtime-projection/**`
- `src/modules/chat/**`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Forbidden Files

- `src-tauri/src/modules/learning/**`
- `src/modules/settings/**`
- `docs/_legacy/**`

## Source Of Truth

- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-020 Session Supervisor Foundation](./MIG-020-session-supervisor-foundation.md)
- [MIG-018 Session History Replay And Paging](./MIG-018-session-history-replay-and-paging.md)

## Required Changes

1. 为 failed / interrupted run 生成 typed recoverability 信息：`resume_available / resume_cursor / resume_reason / safe_to_retry_mutations`。
2. `stream_complete` / `stream_error` payload 带 recovery contract。
3. 前端显示 resume CTA 与原因说明，而不是只显示 cursor。
4. 至少新增测试覆盖 network timeout 与 mutating tool 的 recoverability 差异。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml resume`
- `npm test -- runtime-projection`
- `npm run build`
- mutating tool 场景不会被误标为无条件安全 resume

## Out Of Scope

- 不做跨设备恢复
- 不自动重放 mutating tool
- 不做 retry policy 自动调优

## Execution Notes

- 先给恢复语义定型，再扩自动恢复。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
