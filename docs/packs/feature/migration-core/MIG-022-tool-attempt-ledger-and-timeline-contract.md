# MIG-022 Tool Attempt Ledger And Timeline Contract

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [execution-mode-policy-routing](../../staff-remediation/gap-modules/execution-mode-policy-routing/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

为每个工具调用建立 attempt ledger 和统一 timeline contract，让工具失败、重试、policy decision、duration、settlement 都可被稳定追踪与展示。

## Depends On

- `MIG-016`
- `MIG-020`
- `MIG-021`

## Unlocks

- `MIG-023`

## Why Now

1. 没有 attempt ledger，重试、失败与最终 settle 只能混在单条 tool card 中。
2. 不断流体验需要回答“卡在哪次尝试、为什么重试、还能否继续”。
3. 后续 harness report 与 tool timeline UI 都需要 typed attempt facts。

## Allowed Files

- `src-tauri/src/modules/application/tool_executor.rs`
- `src-tauri/src/modules/control_plane/**`
- `src-tauri/src/modules/application/turn_service/**`
- `src-tauri/src/modules/runtime/**`
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
- [MIG-007 Worker Tool Execution Contract](./MIG-007-worker-tool-execution-contract.md)
- [MIG-021 Resume Contract And Run Recovery](./MIG-021-resume-contract-and-run-recovery.md)

## Required Changes

1. 每个 tool call 生成稳定 `tool_call_id`，每次尝试生成 `attempt_id / attempt_no`。
2. 统一工具生命周期：`queued / authorizing / running / retrying / completed / failed / cancelled / blocked`。
3. canonical event 与 stream payload 都带 attempt 相关字段。
4. 前端 timeline 能准确显示多次 retry，而不是把多次尝试混成一条。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml tool_execution`
- `npm test -- tool-timeline`
- `npm run build`
- 至少一条测试验证多次 retry 的 timeline 顺序稳定

## Out Of Scope

- 不批量改所有工具 UI 样式
- 不改 tool registry 发现逻辑
- 不做 analytics dashboard

## Execution Notes

- 先把 attempt contract 做实，再让 UI 吃它。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
