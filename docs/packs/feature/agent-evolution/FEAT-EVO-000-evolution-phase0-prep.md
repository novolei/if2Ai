# FEAT-EVO-000: Evolution Phase 0 准备（根依赖）

## Status
- State: active

## Goal
为全部 Agent Evolution Pack（TE/SE/SH/AE/DK/BR 系列）建立基础切入点：
(a) 在 `skills/mod.rs` 添加 `sedimentation` 子模块占位；
(b) 将 `work_loop.rs` 中的 skill resolution 逻辑抽取为独立子函数 `resolve_skill_plan`（已存在）并加 `pub(super)` 可见性；
(c) 在 `src/transport/contracts.ts` 的 `RuntimeEventType` 联合类型中添加 10 个 evolution 事件占位（空 reducer 兼容）。

## Spec (verifiable — 每条配 1 个 test name)
- `skills/mod.rs` 导出 `pub mod sedimentation` 且编译通过 → `tests::evolution::phase0::sedimentation_module_compiles`
- `work_loop.rs` 的 `resolve_skill_plan` 函数签名为 `pub(super) fn` 且原调用路径不变 → `tests::evolution::phase0::skill_resolution_refactor_no_regression`
- `contracts.ts` 中 `RuntimeEventType` 包含全部 10 个新事件字面量 → `e2e/evolution-phase0.spec.ts step: contracts_has_10_new_event_types`

## Files (scope — write list)
- src-tauri/src/modules/skills/mod.rs                                      (modify)
- src-tauri/src/modules/application/turn_service/work_loop.rs              (modify)
- src/transport/contracts.ts                                               (modify)
- src-tauri/tests/evolution/phase0_tests.rs                                (new)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/skills/manager/mod.rs
- src-tauri/src/modules/application/turn_service/stream_finalize.rs

## Contract (review must check)
- 不改任何现有 IPC 命令名（I3）
- 不改 `RuntimeEventType` 已有 8 个字面量（I3）
- 不引入新 Cargo.toml dependency
- `resolve_skill_plan` 函数体不变，仅补 `pub(super)` 可见性修饰（I6 精神）
- `sedimentation` 模块初始内容为空 stub（`// TODO: FEAT-SE-001`），编译通过即可

## Out of Scope
- ❌ 不实现任何 sedimentation 逻辑（属于 FEAT-SE-001）
- ❌ 不实现 10 个新 RuntimeEventType 的 reducer（属于 FEAT-INT-001）
- ❌ 不修改 stream_preflight.rs 或 budget.rs
- ❌ 不做前端组件变更

## Verify
- ./scripts/pack run FEAT-EVO-000

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
