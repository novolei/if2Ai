# FEAT-PCP-001: Prompt Coordinator Foundation

## Status
- State: active

## Goal
建立 `PromptCoordinator` 与 `PromptAssemblyDecision` 基础层，统一决定本 turn 该激活哪些 prompt layers / catalogs / overlays。

## Spec
- 协调器能产出结构化 assembly decision → 测试 `modules::application::prompt_coordinator::tests::coordinator_builds_assembly_decision`
- decision 至少覆盖 identity / scenario / tool_policy / memory / utility / coordinator 六类 lanes → 测试 `modules::application::prompt_coordinator::tests::decision_contains_all_core_lanes`
- `TurnService` 通过协调器而不是直接拼 planner 输入 → 测试 `modules::application::turn_service::tests::prepare_chat_inputs_uses_prompt_coordinator`
- diagnostics 可见 activated/suppressed prompt entries → 测试 `modules::application::prompt_coordinator::tests::decision_records_activation_reasons`

## Files (scope)
- `src-tauri/src/modules/application/prompt_coordinator.rs` (new)
- `src-tauri/src/modules/application/prompt_planner/*`
- `src-tauri/src/modules/application/turn_service/mod.rs`

## Reads
- `docs/design-docs/prompt-control-plane-foundation.md`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`
- `src-tauri/src/modules/identity/*`

## Contract
- 不改 IPC 命令名（I3）
- 不改 sqlite schema（I4）
- 不引入新 dependency

## Out of Scope
- ❌ 不实现 settings UI
- ❌ 不实现 markdown prompt 文件热编辑
- ❌ 不实现具体 tool prompt catalog 内容

## Verify
- ./scripts/pack run FEAT-PCP-001

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
