# DW-003: Stream Finalize 并行沉淀钩子（finalize_stream_task 生产接入）

## Status
- State: active

## Goal
在 `stream_finalize.rs::finalize_stream_task` 的 turn 成功分支内，spawn 三个独立 tokio task：① `run_sedimentation_pipeline(history, llm_arc)` → emit `SkillSedimented`；② `extract_turn_checkpoint(text)` 持久化 → emit `CheckpointUpdated`；③ `run_domain_knowledge_contributor(history, llm_arc)` → emit `DomainKnowledge`。三条管道任一失败只记 `tracing::warn`，不阻塞 turn 完成。

## Spec (verifiable)
- turn 成功后 sedimentation pipeline 被调用一次 → `tests::finalize_hooks::sedimentation_called_on_turn_success`
- sedimentation panic 不影响 turn 完成（failure isolation）→ `tests::finalize_hooks::sedimentation_panic_does_not_block`
- 成功沉淀后广播 `SkillSedimented` envelope → `tests::finalize_hooks::sedimented_emits_skill_sedimented_event`
- `extract_turn_checkpoint` 返回 `CheckpointExtraction::Found` 时广播 `CheckpointUpdated` → `tests::finalize_hooks::checkpoint_extracted_emits_event`
- `run_domain_knowledge_contributor` 返回非空列表时广播 `DomainKnowledge` → `tests::finalize_hooks::domain_knowledge_event_emitted`
- `IF2AI_DISABLE_AUTO_SKILL=1` 时 ① ③ 管道跳过，② checkpoint 仍执行 → `tests::finalize_hooks::env_flag_disables_sedimentation_only`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`  (modify — 插入 3 spawn)
- `src-tauri/tests/finalize_hooks.rs`                                  (new — 6 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/application/turn_service/finalize_hooks.rs`   (run_sedimentation_pipeline / extract_turn_checkpoint / run_domain_knowledge_contributor)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`                 (emit_evolution_event — WU-001)
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`  (FinalizeStreamInputs + 成功分支位置)

## Contract (review must check)
- 3 条管道必须 `tokio::spawn` 异步，不阻塞 `finalize_stream_task` 返回（I7）
- 不改 `finalize_stream_task` 对外签名（I1）
- 不改任何 IPC 事件名（I3）
- `IF2AI_DISABLE_AUTO_SKILL=1` 只屏蔽 ① ③；② checkpoint 不受开关影响

## Out of Scope
- ❌ 不修改 SedimentationPipeline / Contributor 内部逻辑（finalize_hooks.rs 只读）
- ❌ 不改 turn 失败 / 取消路径
- ❌ 不实现 UI 展示组件
- ❌ 不接入 TE / AE 类模块

## Depends on
- WU-001（emit_evolution_event）
- WU-003（finalize_hooks.rs helper 函数 done）

## Verify
- `./scripts/pack run DW-003`
- `cargo test --test finalize_hooks`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
