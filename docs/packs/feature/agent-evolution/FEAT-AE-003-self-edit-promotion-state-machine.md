# FEAT-AE-003: 分级升级状态机

## Status
- State: active

## Goal
在 `learning/self_edit/promotion.rs` 实现 4 阶段状态机（Shadow → Canary1Pct → Canary10Pct → Production）。
升级条件：每级 sample_size ≥ 50 且 failure_rate < 5%；任何级 failure_rate ≥ 10% 立刻 demote 到上一级。
纯 in-memory 逻辑，不接入 strategy_registry / strategy_rollout 实际执行。

## Spec (verifiable — 每条配 1 个 test name)
- Shadow + sample ≥ 50 + failure_rate < 5% → Promote(Canary1Pct) → `tests::self_edit::promotion_shadow_healthy_promotes`
- Canary1Pct + failure_rate ≥ 10% → Demote(Shadow) → `tests::self_edit::promotion_canary1_high_failure_demotes`
- Production + sample ≥ 50 + failure_rate < 5% → Hold（已最高级）→ `tests::self_edit::promotion_production_healthy_holds`
- 任意级 sample_size < 50 → Hold（样本不足）→ `tests::self_edit::promotion_insufficient_sample_holds`
- Shadow + failure_rate ≥ 10% → Hold（最低级，不再 demote）→ `tests::self_edit::promotion_shadow_demote_stays_shadow`

## Files (scope — write list)
- src-tauri/src/modules/learning/self_edit/promotion.rs   (new — PromotionStage / StageTransition / next_stage)
- src-tauri/src/modules/learning/self_edit/mod.rs         (modify — re-export PromotionStage / StageTransition / next_stage)
- src-tauri/tests/self_edit.rs                            (modify — 添加 5 个状态机测试)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/learning/strategy_registry.rs     (RolloutState 定义参考，不复用)
- src-tauri/src/modules/learning/promotion_gate.rs        (PromotionDecision 参考)
- src-tauri/src/modules/learning/self_edit/mod.rs         (SelfEditProposal 上下文)

## Contract (review must check)
- 不改 IPC 命令名/事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- 不修改 `learning/strategy_registry.rs` `RolloutState` enum 定义
- 不修改 `learning/strategy_rollout.rs` 任何 pub API
- 不接入 trust_tracker / strategy_registry_service
- 纯 pure functions，无副作用，不落盘

## Out of Scope
- ❌ 不接入 strategy_rollout 实际流量切分逻辑
- ❌ 不修改 RolloutState / ActivationAudit / RollbackAudit
- ❌ 不接入 trust_tracker
- ❌ 不接入 prompt_planner / runtime/daemon/
- ❌ 不修改 contracts.ts

## Verify
- ./scripts/pack run FEAT-AE-003

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
