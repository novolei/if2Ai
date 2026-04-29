# WU-003: Stream Finalize 沉淀钩子（SE-001/002/004 + DK-002/003 Wire-up）

## Status
- State: active

## Goal
在 `stream_finalize.rs` turn 成功收尾点，并行执行三条沉淀管道：① `SedimentationPipeline::run` → `dedup_drafts` → `VectorStore::index_skill`；② `extract_checkpoint` 持久化 WorkingCheckpoint；③ `extract_domain_knowledge_candidates` 自动贡献域知识。三条管道各独立 spawn（`tokio::spawn`），任一失败只记日志不阻塞主流程，并各自广播对应 evolution event。

## Spec (verifiable)
- turn 成功后 sedimentation pipeline 被调用一次 → `tests::finalize_hooks::sedimentation_called_on_success`
- sedimentation panic 不影响 turn 结束（failure isolation）→ `tests::finalize_hooks::sedimentation_panic_does_not_propagate`
- 成功沉淀后广播 `SkillSedimented` envelope → `tests::finalize_hooks::sedimented_emits_skill_sedimented_event`
- `extract_checkpoint` 成功后广播 `CheckpointUpdated` → `tests::finalize_hooks::checkpoint_extracted_emits_event`
- `IF2AI_DISABLE_AUTO_SKILL=1` 时 sedimentation + domain knowledge 管道全跳过 → `tests::finalize_hooks::env_flag_disables_sedimentation`
- `extract_domain_knowledge_candidates` 广播 `DomainKnowledge` → `tests::finalize_hooks::domain_knowledge_event_emitted`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`  (modify — 插入 3 个并行钩子)
- `src-tauri/tests/finalize_hooks.rs`                                   (new — 6 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/skills/sedimentation/mod.rs`           (SedimentationPipeline::run)
- `src-tauri/src/modules/skills/sedimentation/dedup.rs`         (dedup_drafts)
- `src-tauri/src/modules/skills/vector_index.rs`                (VectorStore::index_skill)
- `src-tauri/src/modules/runtime/working_checkpoint.rs`         (extract_checkpoint)
- `src-tauri/src/modules/skills/domain_knowledge/contributor.rs`(extract_domain_knowledge_candidates)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`          (emit_evolution_event — WU-001)

## Contract (review must check)
- 3 条管道必须 `tokio::spawn` 异步，不阻塞 turn finalize 返回（I7 clippy pass）
- 不改 stream_finalize 对外的函数签名（I1）
- 不改任何 IPC 事件名（I3）
- `IF2AI_DISABLE_AUTO_SKILL=1` 可屏蔽 ① ③ 管道（② checkpoint 不受此开关影响）

## Out of Scope
- ❌ 不修改 SedimentationPipeline / VectorStore / Contributor 内部逻辑
- ❌ 不改 turn 失败/取消路径（仅 success 分支插入）
- ❌ 不实现 UI 展示组件
- ❌ 不接入 TE / AE 类模块

## Depends on
- WU-001（emit_evolution_event）
- FEAT-SE-001/002/004、FEAT-DK-002/003（实现 done）

## Verify
- `./scripts/pack run WU-003`
- `cargo test --test finalize_hooks`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
