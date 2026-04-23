# MIG-007 Worker Tool Execution Contract

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [execution-mode-policy-routing](../../staff-remediation/gap-modules/execution-mode-policy-routing/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

把 tool/worker 执行从历史遗留的分散语义，迁移到统一 contract: capability、permission、sandbox、approval、event emission。

## Why Now

1. 没有统一 worker/tool contract，execution mode 与 preflight gate 会在下游再次失真。
2. UClaw 的 worker 体系价值在于执行语义统一，不在于名称。
3. if2Ai 已经有 adoption design，但还没有真接进主链。

## Allowed Files

- `src-tauri/src/modules/control_plane/**`
- `src-tauri/src/modules/tools/**`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [Current Architecture](../../../../ARCHITECTURE.md)
- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [If2Ai Worker Adoption Design](../../staff-remediation/if2ai-worker-adoption-design.md)

## UClaw References

- `/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/workers`
- [runtime/contracts.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/runtime/contracts.rs)

## Required Changes

1. 为工具和 worker 建立统一执行 contract。
2. 明确 capability 与风险分类。
3. 明确 approval / sandbox / event emission 行为。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条 worker/tool contract 测试通过
- 至少一个旧工具路径被迁入统一 contract

## Code Audit 2026-04-23

- Status: done; skip for worker/tool contract foundation.
- Evidence: `ToolExecutionBroker` and `prepare_step_execution` exist; `ToolRegistryExecutor` routes tool dispatch through the broker and enforced preflight.
- Remaining Gap: retry/attempt timeline is not part of this pack; continue with `MIG-022`.

## Out Of Scope

- 不做前端 UI 改造
- 不做 harness 总线重构

## Execution Notes

- 先保证 contract 清晰，再批量迁工具。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
