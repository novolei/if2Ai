# FEAT-TE-004: 工具结果摘要化

## Status
- State: pending

## Goal
将 `stream_tool_execution.rs` 中现有的 `summarize_tool_result_for_model`（粗字符截断）
升级为：token 数 ≤500 时原文保留；token 数 >500 时使用 `UtilityLlm` 生成 LLM 辅助摘要；
LLM 失败时降级到字符截断。减少单工具结果对上下文预算的冲击。

## Spec (verifiable — 每条配 1 个 test name)
- 短结果（≤500 tokens）原文返回，不触发 LLM 调用 → `tests::stream_tool_execution::summarize_short_result_passthrough`
- 长结果（>500 tokens）返回的摘要 token 数 ≤500 → `tests::stream_tool_execution::summarize_long_result_respects_token_limit`
- LLM 摘要调用失败时降级字符截断，不 panic，不丢失工具调用记录 → `tests::stream_tool_execution::summarize_fallback_on_llm_error`
- 摘要结果中保留工具名称和成功/失败标记（供 trajectory 追踪） → `tests::stream_tool_execution::summarize_preserves_tool_metadata`

## Files (scope — write list)
- src-tauri/src/modules/application/turn_service/stream_tool_execution.rs       (modify)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/api/providers/openai_compat.rs  §UtilityLlm trait
- src-tauri/src/modules/runtime/context_compression.rs  §estimate_tokens（来自 FEAT-TE-001）
- src-tauri/src/modules/application/turn_service/stream_finalize.rs  §ToolInvocationRecord

## Contract (review must check)
- 不改任何 IPC 命令名或事件字面量（I3）
- 不改工具调用结果的 persist 路径（ledger 仍写入原始结果，仅 LLM 请求侧用摘要）（I4）
- 不引入新 Cargo.toml dependency
- 摘要仅用于 LLM 请求组装，不修改 event log / timeline 记录
- `cargo clippy -D warnings` 通过（I7）

## Out of Scope
- ❌ 不实现消息级压缩（属于 FEAT-TE-002）
- ❌ 不实现迷你索引（属于 FEAT-TE-003）
- ❌ 不改 budget.rs 或 context_compression.rs
- ❌ 不改前端
- ❌ 不顺手重构 stream_task.rs

## Verify
- ./scripts/pack run FEAT-TE-004

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
