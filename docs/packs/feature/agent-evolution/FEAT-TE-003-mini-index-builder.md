# FEAT-TE-003: 迷你索引构建器 L1

## Status
- State: pending

## Goal
实现 GenericAgent L1 模式：每次 LLM 调用前从完整会话历史生成 ≤30 行的纯文本
迷你索引（工具调用摘要 + 关键决策 + 当前目标），作为 priority-95 的 PromptBlock
注入 prompt planner，替换原始 working-memory dump，目标 ≤500 tokens。

## Spec (verifiable — 每条配 1 个 test name)
- `build_mini_index` 对 10+ 条历史消息输出 ≤30 行字符串 → `tests::context_compression::mini_index_respects_line_limit`
- 生成的迷你索引 token 数 ≤500（cl100k_base 计量） → `tests::context_compression::mini_index_respects_token_budget`
- 迷你索引作为 `priority = 95` 的 PromptBlock 注入，planner 中可被 diagnostics 追踪 → `tests::prompt_planner::mini_index_block_has_priority_95`
- 空历史（0 条消息）时返回空字符串，不 panic → `tests::context_compression::mini_index_empty_history_no_panic`

## Files (scope — write list)
- src-tauri/src/modules/runtime/context_compression/mini_index.rs               (new)
- src-tauri/src/modules/application/prompt_planner/planner.rs                   (modify)
- src-tauri/src/modules/application/prompt_planner/block.rs                     (modify)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/runtime/context_compression.rs  §ContextTier（来自 FEAT-TE-001）
- src-tauri/src/modules/application/prompt_planner/diagnostics.rs
- src-tauri/src/modules/application/prompt_planner/build_request.rs

## Contract (review must check)
- 不改任何 IPC 命令名或事件字面量（I3）
- 注入的 PromptBlock 使用现有 `PromptBlock` 结构体，不新增字段
- 不引入新 Cargo.toml dependency
- priority 值 95 不与现有 block 冲突（review 须检查现有 priority 分布）
- `cargo clippy -D warnings` 通过（I7）

## Out of Scope
- ❌ 不实现 LLM 辅助摘要（属于 FEAT-TE-002）
- ❌ 不实现工具结果摘要（属于 FEAT-TE-004）
- ❌ 不改 stream_preflight.rs
- ❌ 不修改前端
- ❌ 不顺手重构 planner.rs 其他逻辑

## Verify
- ./scripts/pack run FEAT-TE-003

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
