# FEAT-PCP-002: Scenario And Task-Focus Catalog

## Status

- State: `done`
- Owner: `@executor`
- Depends On: `FEAT-PCP-001`
- Last Updated: `2026-04-22`
- Completed Commit: `d902a06`

## Goal
建立 `ScenarioPromptCatalog` 与 task-focus overlays，让 `chat / coding / research / planning / review` 不再只是一段内联文案。

## Spec
- 五类 scenario profile 都有独立 catalog entry → 测试 `modules::application::prompt_catalog::tests::scenario_catalog_contains_all_profiles`
- 协调器会根据 `ScenarioProfileHint` 激活对应 entry → 测试 `modules::application::prompt_coordinator::tests::scenario_hint_activates_matching_entry`
- planning/review 可挂 task-focus overlay → 测试 `modules::application::prompt_coordinator::tests::task_focus_overlay_is_conditional`

## Files (scope)
- `src-tauri/src/modules/application/prompt_catalog/scenario.rs` (new)
- `src-tauri/src/modules/application/prompt_coordinator.rs`
- `src-tauri/src/modules/application/prompt_planner/*`

## Reads
- `docs/design-docs/prompt-control-plane-foundation.md`
- `docs/packs/feature/prompt-planner-alignment/MIG-008-coding-mode-prompt-enhancement.md`

## Contract
- 不改 execution_mode canonical contract
- 不把 scenario profile 混回 system prompt
- 不引入新 dependency

## Out of Scope
- ❌ 不实现 tool prompt
- ❌ 不实现 utility prompt

## Verify
- ./scripts/pack run FEAT-PCP-002

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
