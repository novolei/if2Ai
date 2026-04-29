# FEAT-AE-002: 自编辑验证门（4 维 Gate）

## Status
- State: active

## Goal
在 `learning/self_edit/verification.rs` 实现 4 维验证门，过滤 `Vec<SelfEditProposal>` 中不合格提案：
(1) after 非空且可解析 markdown/yaml；(2) constitution 通过；(3) cosine 去重 ≥ 0.85；(4) 历史 cluster 失败次数 ≥ 3。
每条 gate 失败附 `VerificationVerdict { proposal_id, verdict, failed_gates, reasons }`。

## Spec (verifiable — 每条配 1 个 test name)
- 干净提案（after 合法 + constitution pass + 不重复）→ Pass → `tests::self_edit::verify_clean_proposal_passes`
- `after` 含危险指令（`rm -rf /`）→ constitution gate Fail → `tests::self_edit::verify_malicious_after_fails_constitution`
- 与历史提案 cosine ≥ 0.85 → dedup gate Fail → `tests::self_edit::verify_duplicate_proposal_fails_dedup`
- 空 `after` 字段 → malformed gate Fail → `tests::self_edit::verify_empty_after_fails_malformed`

## Files (scope — write list)
- src-tauri/src/modules/learning/self_edit/verification.rs (new — verify_proposals fn + VerificationVerdict)
- src-tauri/src/modules/learning/self_edit/mod.rs          (modify — re-export VerificationVerdict / verify_proposals)
- src-tauri/tests/self_edit.rs                             (modify — 添加 4 个验证门测试)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/skills/guard/mod.rs                (evaluate_constitution 签名)
- src-tauri/src/modules/skills/sedimentation/dedup.rs      (Embedder trait + cosine_similarity)
- src-tauri/src/modules/learning/self_edit/proposal.rs     (SelfEditProposal 定义)

## Contract (review must check)
- 不改 IPC 命令名/事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- 不修改 `skills/guard/mod.rs` 已有 pub API（只读取 evaluate_constitution）
- 不修改 `learning/strategy_registry.rs` / `strategy_rollout.rs` 任何 pub API
- 所有验证均 in-memory，不落盘

## Out of Scope
- ❌ 不实现分级 rollout（FEAT-AE-003）
- ❌ 不修改 constitution 规则本身（仅调用）
- ❌ 不接入 strategy_registry / strategy_registry_service
- ❌ 不修改 prompt_planner / runtime/daemon/
- ❌ 不修改 contracts.ts

## Verify
- ./scripts/pack run FEAT-AE-002

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
