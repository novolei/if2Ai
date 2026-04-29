# FEAT-AE-001: 失败分析 → 自编辑提案生成器

## Status
- State: active

## Goal
在 `learning/self_edit/proposal.rs` 实现提案生成器：消费 `ClusteredFailureSet` + 历史 `Vec<InputMessage>`，
调用 `UtilityLlm` 为每个高频 cluster（occurrences ≥ 3）生成 `Vec<SelfEditProposal>`。
draft-only，不落盘，不入 strategy_registry。

## Spec (verifiable — 每条配 1 个 test name)
- 空 cluster 输入 → 返回空 vec → `tests::self_edit::proposal_empty_cluster_returns_empty`
- 高频 cluster（occurrences ≥ 3）→ 至少 1 个提案 → `tests::self_edit::proposal_high_freq_cluster_generates`
- LLM 调用失败（mock 返回 Err）→ 优雅降级返回空 vec → `tests::self_edit::proposal_llm_error_graceful_degradation`
- 提案 id 唯一非空 + after 字段非空 → `tests::self_edit::proposal_fields_non_empty_and_unique_ids`

## Files (scope — write list)
- src-tauri/src/modules/learning/self_edit/mod.rs         (new — 子模块入口，pub re-export ProposalKind / SelfEditProposal)
- src-tauri/src/modules/learning/self_edit/proposal.rs    (new — generate_proposals async fn)
- src-tauri/src/modules/learning/mod.rs                   (modify — 添加 pub mod self_edit;)
- src-tauri/tests/self_edit.rs                            (new — 集成测试，4 个)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/learning/failure_clustering.rs
- src-tauri/src/modules/learning/mod.rs
- src-tauri/src/modules/api/mod.rs                        (InputMessage / InputContentBlock)

## Contract (review must check)
- 不改 IPC 命令名/事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency（uuid 已在 Cargo.toml 中）
- `cargo clippy -D warnings` 通过 (I7)
- 不修改 `learning/strategy_registry.rs` / `strategy_rollout.rs` 任何 pub API
- `self_edit/` 全部为 draft-only：不落盘、不调 strategy_registry_service

## Out of Scope
- ❌ 不实现验证门（FEAT-AE-002）
- ❌ 不实现分级升级（FEAT-AE-003）
- ❌ 不接入 prompt_planner / runtime/daemon/
- ❌ 不修改 contracts.ts（除 wiring tax 新增字面量）
- ❌ 不落盘、不写任何持久化代码

## Verify
- ./scripts/pack run FEAT-AE-001

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
