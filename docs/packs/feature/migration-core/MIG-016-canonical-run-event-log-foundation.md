# MIG-016 Canonical Run Event Log Foundation

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

为每次 chat run 建立 append-only canonical event log 基础层，先旁路记录运行事实，不替换现有 session JSON 持久化。

## Depends On

- `MIG-001`
- `MIG-015`

## Unlocks

- `MIG-017`
- `MIG-018`
- `MIG-023`

## Why Now

1. 没有 run event log，projection、paging、resume、harness report 都缺统一事实源。
2. 当前 `session.json` 同时承担 metadata / history / resume source，后续复杂度会继续堆高。
3. benchmark 的强项本质是 session event log 可恢复，而不是单个 streaming API。

## Allowed Files

- `src-tauri/src/modules/application/turn_service/**`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/src/commands/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/learning/**`
- `docs/_legacy/**`

## Source Of Truth

- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-001 Canonical Chat Execution Spine](./MIG-001-canonical-chat-execution-spine.md)
- [MIG-015 Gateway Conversations And Streaming Surface](./MIG-015-gateway-conversations-and-streaming-surface.md)

## Benchmark References

- [src/server/services/sessionService.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/src/server/services/sessionService.ts>)
- [src/assistant/sessionHistory.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/src/assistant/sessionHistory.ts>)

## Required Changes

1. 为 `run_agent_turn` / `start_agent_stream` 生成稳定 `run_id`。
2. 在 run 生命周期关键节点写入 canonical event：`run_started`、text/thinking/tool、permission、complete/error。
3. event log 使用 append-only 写入，带 `event_id / session_id / run_id / seq / occurred_at / payload`。
4. event log 写失败不得阻断主链，但必须可观测。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml event_log`
- `cargo test --manifest-path src-tauri/Cargo.toml turn_service`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- 至少一条测试验证同一 run 的 `seq` 单调递增
- 至少一条测试验证 `stream_complete` 或 `stream_error` 落盘

## Out Of Scope

- 不做前端 projection cutover
- 不替换现有 session JSON 读取路径
- 不做 history paging API

## Execution Notes

- 先把事实源立住，再让 projection 和 report 改读它。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
