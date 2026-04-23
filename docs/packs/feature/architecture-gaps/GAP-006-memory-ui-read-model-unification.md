# GAP-006: Memory UI Read Model Unification

## Status
- State: draft

## Goal
统一 memory evidence、recent events、MemoryBrowser、MemoryDebugTab 的前端读面，避免 memory 状态由多条 IPC/read model 分别推断。

## Spec
- memory telemetry/evidence 走统一 projection/evidence facade → `memory_ui_uses_single_read_model`
- MemoryBrowser/DebugTab direct IPC 包装进 `src/api/memory.ts` typed facade → `memory_pages_do_not_import_raw_tauri`
- skip/degraded/cache_hit 等 reasoned states 在 UI 中保持不丢失 → `memory_reasoned_states_are_preserved`

## Files
- `src/api/**`
- `src/runtime-projection/**`
- `src/components/memory/**`
- `src/modules/settings/pages/MemoryDebugTab.tsx`
- `src/components/chat/TelemetryDrawer.tsx`
- `src/**/*.test.*`

## Reads
- `ARCHITECTURE.md` §3.3, §6-7
- `src/stores/README.md`
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`

## Contract
- 不改变 memory backend schema。
- 不降级 `cache_hit`、`empty_input`、`upstream_missing`、`llm_degraded` 等可诊断状态。
- 不让 settings page 直接订阅 raw runtime events。

## Verify
- `./scripts/pack run GAP-006`
- `npm test -- memory`
- `npm run build`

