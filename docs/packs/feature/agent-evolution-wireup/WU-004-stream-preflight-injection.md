# WU-004: Stream Preflight 注入钩子（TE-002/003 + DK-002 Wire-up）

## Status
- State: active

## Goal
在 `stream_task.rs::build_iteration_request` 调用前，依序执行：① `WorkingCheckpointStore::load` 注入 checkpoint（`inject_checkpoint`）；② `MessageDigester::digest` 压缩消息列表并将结果传入 `PreflightContext.digested_messages: Some(…)`；③ 在 `planner.rs::build_prompt_plan` 内调用 `render_mini_index_block(build_mini_index(&messages))` 将迷你索引推入 PromptBlock（priority 95）。三步均有独立失败降级：失败则静默回退原始行为，并发 `CompressionEvent` / `CheckpointUpdated` evolution event。

## Spec (verifiable)
- `digested_messages` 在消息 >500 tokens 时为 `Some(…)` 不为 `None` → `tests::preflight_hooks::digester_populates_digested_messages`
- digester 失败时 `digested_messages` 回退 `None`，主流程不中断 → `tests::preflight_hooks::digester_failure_falls_back`
- checkpoint 注入后 `CompressionEvent`（family="checkpoint_injected"）被广播 → `tests::preflight_hooks::checkpoint_inject_emits_event`
- `render_mini_index_block` 返回 `Some` 时被推入 `blocks` → `tests::preflight_hooks::mini_index_block_pushed_to_plan`
- `IF2AI_DISABLE_DIGESTER=1` 时 MessageDigester 不被调用 → `tests::preflight_hooks::env_flag_disables_digester`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/stream_task.rs`       (modify — 注入 ①② 步骤)
- `src-tauri/src/modules/application/prompt_planner/planner.rs`         (modify — 调用 render_mini_index_block)
- `src-tauri/tests/preflight_hooks.rs`                                  (new — 5 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/runtime/context_compression/digester.rs`       (MessageDigester::digest)
- `src-tauri/src/modules/runtime/context_compression/mini_index.rs`     (build_mini_index)
- `src-tauri/src/modules/runtime/working_checkpoint.rs`                 (inject_checkpoint / load)
- `src-tauri/src/modules/application/turn_service/stream_preflight.rs`  (PreflightContext 结构)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`                  (emit_evolution_event — WU-001)

## Contract (review must check)
- `PreflightContext.digested_messages` 字段必须已存在（FEAT-TE-002 done 时已加）
- 不改 `build_iteration_request` 对外签名（I1）
- `render_mini_index_block` 返回 `None` 时不 push block（不引入空 block）
- `IF2AI_DISABLE_DIGESTER=1` 与 `IF2AI_DISABLE_MINI_INDEX=1` 分开控制 ②③

## Out of Scope
- ❌ 不修改 MessageDigester / MiniIndex 内部算法
- ❌ 不改 TE-001 TierBudget 预算分配逻辑
- ❌ 不影响 turn 失败 / resume 路径的 build_iteration_request 调用（仅首次迭代路径）
- ❌ 不实现 UI 展示

## Depends on
- WU-001（emit_evolution_event）
- FEAT-TE-002/003、FEAT-DK-002（实现 done）

## Verify
- `./scripts/pack run WU-004`
- `cargo test --test preflight_hooks`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
