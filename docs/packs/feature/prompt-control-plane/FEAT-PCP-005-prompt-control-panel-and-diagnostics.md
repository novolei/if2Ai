# FEAT-PCP-005: Prompt Control Panel And Diagnostics

## Status

- State: `done`
- Owner: `@executor`
- Depends On: `FEAT-PCP-001`, `FEAT-PCP-002`, `FEAT-PCP-003`, `FEAT-PCP-004`
- Last Updated: `2026-04-22`
- Completed Commit: `d902a06`

## Goal
为用户提供结构化 Prompt Control Panel，并把 activated/suppressed prompt entries 投影到 diagnostics UI。

## Spec
- settings 可保存 default scenario profile / prompt diagnostics toggle → 测试 `modules::runtime::config::tests::config_reads_prompt_control_defaults`
- 前端可读取 prompt diagnostics summary → 测试 `transport::tests::prompt_diagnostics_contract_round_trips`
- control panel 只暴露结构化配置，不暴露原始 prompt 编辑 → 测试 `src/modules/settings/...` 对应前端测试

## Files (scope)
- `src-tauri/src/modules/runtime/config/*`
- `src-tauri/src/modules/application/prompt_coordinator.rs`
- `src/transport/contracts.ts`
- `src/modules/settings/**/*`

## Reads
- `docs/design-docs/prompt-control-plane-foundation.md`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`

## Contract
- 不提供 base system prompt 自由编辑
- 不改 IPC 命令名（I3）
- 不引入新 dependency

## Out of Scope
- ❌ 不支持用户自定义任意 markdown prompt 上传
- ❌ 不做 prompt marketplace

## Verify
- ./scripts/pack run FEAT-PCP-005

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
