# FEAT-PCP-004: Utility And Coordinator Prompt Lanes

## Status

- State: `done`
- Owner: `@executor`
- Depends On: `FEAT-PCP-001`
- Last Updated: `2026-04-22`
- Completed Commit: `d902a06`

## Goal
建立 `UtilityPromptCatalog` 与 `CoordinatorPromptCatalog`，让 session-title / tool-summary / away-recap / orchestration overlays 进入统一控制面。

## Spec
- utility prompt entries 可独立查询与渲染 → 测试 `modules::application::prompt_catalog::tests::utility_catalog_resolves_known_entries`
- coordinator overlay 只在复杂 orchestration 场景激活 → 测试 `modules::application::prompt_coordinator::tests::coordinator_overlay_activates_only_for_complex_runs`
- coordinator overlay 不默认进入普通 chat turn → 测试 `modules::application::prompt_coordinator::tests::coordinator_overlay_is_suppressed_for_simple_chat`

## Files (scope)
- `src-tauri/src/modules/application/prompt_catalog/utility.rs` (new)
- `src-tauri/src/modules/application/prompt_catalog/coordinator.rs` (new)
- `src-tauri/src/modules/application/prompt_coordinator.rs`

## Reads
- `docs/design-docs/prompt-control-plane-foundation.md`

## Contract
- 不把 coordinator prompt 常驻到所有 turn
- 不改现有 utility 功能对外契约
- 不引入新 dependency

## Out of Scope
- ❌ 不实现 settings control panel
- ❌ 不实现 prompt markdown marketplace

## Verify
- ./scripts/pack run FEAT-PCP-004

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
