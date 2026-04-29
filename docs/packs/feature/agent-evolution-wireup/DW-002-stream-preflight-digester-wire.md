# DW-002: Stream Preflight Digester Wire（stream_task.rs → digest_messages_for_preflight）

## Status
- State: active

## Goal
在 `stream_task.rs:542` 的 `build_iteration_request` 调用点之前，调用 `digest_messages_for_preflight(messages, llm_arc)` 把结果赋给 `PreflightContext.digested_messages: Some(…)`（现行值永远 `None`）。压缩成功时广播 `CompressionEvent`；失败时静默回退 `None` 并记 `tracing::warn`。

## Spec (verifiable)
- `session_messages` token 数 >500 时 `PreflightContext.digested_messages` 为 `Some(…)` → `tests::preflight_hooks::digester_populates_digested_messages_on_long_context`
- digester 返回 `Err` 时 `digested_messages` 回退 `None`，主流程继续 → `tests::preflight_hooks::digester_error_falls_back_to_none`
- 压缩成功后广播 `CompressionEvent` envelope（family="message_digest"）→ `tests::preflight_hooks::digester_success_emits_compression_event`
- `IF2AI_DISABLE_DIGESTER=1` 时跳过调用，`digested_messages` 保持 `None` → `tests::preflight_hooks::env_flag_skips_digester`
- 短消息列表（≤500 tokens）不触发 digester → `tests::preflight_hooks::short_context_skips_digester`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/stream_task.rs`   (modify — 注入 digest call)
- `src-tauri/tests/preflight_hooks.rs`                              (modify — 补 5 个 digester tests)

## Reads (read-only)
- `src-tauri/src/modules/application/turn_service/preflight_hooks.rs` (digest_messages_for_preflight)
- `src-tauri/src/modules/application/turn_service/stream_preflight.rs` (PreflightContext 字段)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`                 (emit_evolution_event — WU-001)

## Contract (review must check)
- 不改 `build_iteration_request` 对外签名（I1）
- 不改 `PreflightContext` struct 定义（字段已由 FEAT-TE-002 加入，只赋值）
- 失败时不 panic；`tracing::warn` 后继续（I7 clippy pass）
- `IF2AI_DISABLE_DIGESTER=1` 检查必须在 digester 调用前

## Out of Scope
- ❌ 不修改 MessageDigester / compress 内部算法（TE-002 只读）
- ❌ 不改 mini-index 注入（已由 WU-004 覆盖）
- ❌ 不改 checkpoint 注入逻辑（已由 WU-004 覆盖）
- ❌ 不实现 UI 组件

## Depends on
- WU-001（emit_evolution_event）
- WU-004（PreflightContext.digested_messages 字段已由 WU-004 wire，此 Pack 仅补 stream_task 调用点）

## Verify
- `./scripts/pack run DW-002`
- `cargo test --test preflight_hooks`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
