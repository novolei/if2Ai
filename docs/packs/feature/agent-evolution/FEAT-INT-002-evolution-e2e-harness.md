# FEAT-INT-002: 端到端 Evolution Harness Suite

## Status
- State: active

## Goal
新增 `src-tauri/tests/evolution_e2e.rs`，用 5 条集成测试覆盖一条完整 evolution 数据流：失败报告 → 聚类 → 自编辑提案（AE-001）→ 验证门（AE-002）→ 升级状态机（AE-003）；并行：技能沉淀（SE-001）→ 去重（SE-002）→ 向量索引（SE-004）。每条断言验证跨 Pack 数据契约不破。零 UI、零真实网络、零持久化 I/O。

## Spec (verifiable — 每条配 1 个 test name)
- `ClusteredFailureSet` → `generate_proposals`(Mock LLM) → `verify_proposals` → `next_stage` 推进 Shadow→Canary1Pct → `tests::evolution_e2e::evolution_e2e_self_edit_pipeline`
- 8 条对话 → `extract_skill_drafts` → `dedup_drafts` → `index_skill`(MockVectorStore) → `search_skills` 命中 → `tests::evolution_e2e::evolution_e2e_skill_pipeline`
- daemon 探针失败累积 → cluster → 生成 proposal → 验证门 PASS → stage 推进 → `tests::evolution_e2e::evolution_e2e_self_healing_to_proposal`
- `history_to_domain_knowledge`(DK-003) → constitution 验证 → 并行 sedimentation+dedup → 两管道共享同一 Embedder 实例 → `tests::evolution_e2e::evolution_e2e_domain_knowledge_to_skill`
- `compress_for_request`(TE-001/002) + `inject_checkpoint`(DK-002) → 压缩后注入 checkpoint → 最终 token 数满足预算约束 → `tests::evolution_e2e::evolution_e2e_checkpoint_inject_into_compressed_request`

## Files (scope — write list)
- `src-tauri/tests/evolution_e2e.rs`  (new — 5 个 e2e 集成测试，预计 ~400 LOC)

## Reads (read-only inputs)
- `src-tauri/tests/evolution_phase0.rs`                          (现有测试辅助函数参考)
- `src-tauri/src/modules/learning/self_edit/proposal.rs`         (generate_proposals API)
- `src-tauri/src/modules/learning/self_edit/verification.rs`     (verify_proposals API)
- `src-tauri/src/modules/learning/self_edit/promotion.rs`        (next_stage API)
- `src-tauri/src/modules/learning/failure_clustering.rs`         (ClusteredFailureSet API)
- `src-tauri/src/modules/skills/sedimentation/mod.rs`            (extract_skill_drafts API)
- `src-tauri/src/modules/skills/sedimentation/dedup.rs`          (dedup_drafts API)
- `src-tauri/src/modules/skills/vector_index.rs`                 (index_skill / search_skills API)
- `src-tauri/src/modules/runtime/daemon/mod.rs`                  (DaemonProbe / health_check API)
- `src-tauri/src/modules/skills/domain_knowledge/contributor.rs` (history_to_domain_knowledge API)
- `src-tauri/src/modules/skills/guard/constitution.rs`           (evaluate_constitution API)
- `src-tauri/src/modules/runtime/working_checkpoint.rs`          (inject_checkpoint API)
- `src-tauri/src/modules/runtime/context_compression/mod.rs`     (compress_for_request API)
- `src-tauri/src/modules/runtime/budget.rs`                      (TierBudgetAllocation — token 预算断言)
- `src-tauri/src/modules/runtime/contracts/common.rs`            (RuntimeEventType 18 变体 — INT-001 交付后)

## Contract (review must check)
- 不改任何 src 代码（仅 tests/ 新建文件）（I6 精神适用）
- `cargo test --test evolution_e2e` 全 5 条 PASS
- `cargo clippy -D warnings` 通过（I7）
- 测试内所有 LLM / VectorStore / CDP 调用均为 in-process mock（零真实 IO）
- 每条测试在 500ms 内完成（无网络、无 sleep）

## Out of Scope
- ❌ 不修改任何 src/ 代码（仅允许 tests/ 新增文件）
- ❌ 不接入真实 LLM / Chromium / sqlite / 文件系统
- ❌ 不改任何已完成 Pack 的公共 API
- ❌ 不接入前端（零 TS / TSX 文件）
- ❌ 不修改 Cargo.toml（dev-dependencies 中 mockall 若已存在直接用；若不存在禁止新增）

## Depends on
FEAT-INT-001（RuntimeEventType 18 变体），FEAT-EVO-000, FEAT-TE-001~004, FEAT-SE-001~004, FEAT-SH-001~003, FEAT-AE-001~003, FEAT-DK-001~003, FEAT-BR-001~003

## Verify
- `./scripts/pack run FEAT-INT-002`
- `cargo test --test evolution_e2e -- --nocapture`

## Done
- Verify 全 PASS
- REGISTRY 状态改为 done
