# MIG-019 Pending Permission Recovery

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [execution-mode-policy-routing](../../staff-remediation/gap-modules/execution-mode-policy-routing/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

把权限请求从“后端同步等前端一次回复”升级为“可持久化、可恢复、可重连”的 pending permission 工作流。

## Depends On

- `MIG-016`
- `MIG-017`

## Unlocks

- `MIG-020`
- `MIG-021`

## Why Now

1. 当前 permission prompt 在刷新、重连、未来多 viewer 场景下都偏脆弱。
2. pending permission 是 session continuity 的关键节点，不能只存在于一次性 mpsc 阻塞中。
3. benchmark 的 control protocol 已经证明 permission 应该是 session lifecycle 的一部分。

## Allowed Files

- `src-tauri/src/modules/application/permission_service.rs`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/src/commands/agent/**`
- `src/runtime-projection/**`
- `src/api/**`
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
- [MIG-016 Canonical Run Event Log Foundation](./MIG-016-canonical-run-event-log-foundation.md)

## Required Changes

1. 权限请求时写入 `pending permission` 记录，并发出 canonical event。
2. 前端刷新或重连后，可重新取回当前 session 的 pending permission。
3. 用户完成 allow/deny 后，系统写入 `permission_resolved` 并清除 pending。
4. 至少新增一条后端测试和一条前端测试覆盖恢复路径。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml permission`
- `npm test -- permission`
- `npm run build`
- pending permission 在刷新后仍可恢复展示

## Out Of Scope

- 不重做工具权限分级规则
- 不做多窗口协作 UI
- 不做 approval analytics

## Execution Notes

- 先让 pending 可恢复，再考虑高级审批策略。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
