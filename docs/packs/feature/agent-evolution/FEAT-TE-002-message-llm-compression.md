# FEAT-TE-002: 消息级 LLM 压缩

## Status
- State: pending

## Goal
对会话历史中处于 `RecentHistory` tier、token 数超过阈值（默认 500）的消息，
使用 `UtilityLlm` 生成单段摘要替换原文，并将 `CompressedMessage` 枚举写入
`context_compression/digester.rs`；集成到 `stream_preflight` 的 tier 压缩流程中。

## Spec (verifiable — 每条配 1 个 test name)
- `MessageDigester::digest` 对长消息返回 `CompressedMessage::Summarized`，token 数 < 原始 50% → `tests::context_compression::digester_compresses_long_message`
- 短消息（≤500 tokens）直接返回 `CompressedMessage::Full`，不触发 LLM 调用 → `tests::context_compression::digester_skips_short_message`
- `stream_preflight` 集成后，压缩后的摘要替换原始消息进入请求体 → `tests::context_compression::preflight_uses_digested_messages`
- LLM 调用失败时降级保留原始消息，不丢失上下文 → `tests::context_compression::digester_fallback_on_llm_error`

## Files (scope — write list)
- src-tauri/src/modules/runtime/context_compression/digester.rs                 (new)
- src-tauri/src/modules/runtime/context_compression.rs                          (modify → 重构为 mod 目录)
- src-tauri/src/modules/application/turn_service/stream_preflight.rs            (modify)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/api/providers/openai_compat.rs  §UtilityLlm trait
- src-tauri/src/modules/runtime/context_compression.rs  §CompressedMessage 定义（来自 FEAT-TE-001）

## Contract (review must check)
- 不改任何 IPC 命令名或事件字面量（I3）
- LLM 摘要调用走现有 `UtilityLlm` trait，不新增 provider 配置
- 不引入新 Cargo.toml dependency
- `cargo clippy -D warnings` 通过（I7）

## Out of Scope
- ❌ 不实现迷你索引（属于 FEAT-TE-003）
- ❌ 不实现工具结果摘要（属于 FEAT-TE-004）
- ❌ 不改前端
- ❌ 不顺手重构 compact.rs 或 budget.rs

## Verify
- ./scripts/pack run FEAT-TE-002

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
