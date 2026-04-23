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

1. `MIG-016` Canonical run event log foundation
2. `MIG-017` Runtime projection chat truth cutover
3. `MIG-018` Session history replay and paging
4. `MIG-019` Pending permission recovery
5. `MIG-020` Session supervisor foundation
6. `MIG-021` Resume contract and run recovery
7. `MIG-022` Tool attempt ledger and timeline contract
8. `MIG-023` Canonical run report from event log

## Reads

- `docs/packs/REGISTRY.md`
- `src-tauri/src/modules/application/turn_service/*`
- `src-tauri/src/modules/runtime/*`
- `src/runtime-projection/*`
- `src/stores/*`
