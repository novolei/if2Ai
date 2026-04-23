# Migration Core Packs

## Scope

本目录承载 If2Ai 主聊天链、session lifecycle、runtime projection、tool contract、event log、harness report 的迁移路线图。

## Why

目标不是继续在 Tauri command 与页面状态之间堆 glue code，而是把 If2Ai 升级成：

1. `SessionMeta` 轻元数据
2. `Run/Event Log` 运行事实源
3. `Projection` 前端唯一真相
4. `Session Supervisor` 生命周期治理
5. `Canonical Run Report` replay / eval / audit 输入

## Canonical Blueprint

本路线图的总纲是 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。

`MIG-003`、`MIG-007`、`MIG-016` ~ `MIG-023` 的设计、实现、review、验收必须完整参照该蓝图执行。

## Pack Order

Current code-audited sequence after 2026-04-23 scan:

1. `MIG-016` done; skip unless event-log foundation regresses.
2. `MIG-017` partial; continue projection-only chat cutover.
3. `MIG-018` done; skip replay/paging foundation.
4. `MIG-019` done; skip pending-permission recovery foundation.
5. `MIG-020` partial; build first-class SessionSupervisor snapshot owner.
6. `MIG-021` partial; finish typed resume/recovery contract.
7. `MIG-022` partial; finish attempt ledger and retry timeline.
8. `MIG-023` partial; derive canonical run report from event log.

Already completed supporting packs: `MIG-010`, `MIG-011`, `MIG-012`, `MIG-013`, `MIG-014`, `MIG-015`, `MIG-007`.

Still partial supporting packs: `MIG-003`, `MIG-006`, `MIG-008`, `MIG-009`.

## Reads

- `docs/packs/REGISTRY.md`
- `src-tauri/src/modules/application/turn_service/*`
- `src-tauri/src/modules/runtime/*`
- `src/runtime-projection/*`
- `src/stores/*`
