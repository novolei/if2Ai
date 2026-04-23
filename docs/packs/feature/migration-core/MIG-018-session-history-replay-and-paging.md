# MIG-018 Session History Replay And Paging

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

基于 canonical run event log 建立 session history replay 与分页能力，让历史恢复不再依赖一次性读取完整 session message array。

## Depends On

- `MIG-016`
- `MIG-017`

## Unlocks

- `MIG-021`
- `MIG-023`

## Why Now

1. 没有分页与 replay，长会话恢复会继续绑定全量 session JSON。
2. projection 一旦成为前端真相，就需要可重复构建的 history input。
3. benchmark 在 history paging / lazy restore / lineage rebuild 上明显更成熟。

## Allowed Files

- `src-tauri/src/modules/runtime/**`
- `src-tauri/src/commands/session.rs`
- `src-tauri/src/commands/gateway.rs`
- `src/api/**`
- `src/runtime-projection/**`
- `src/stores/**`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Forbidden Files

- `src-tauri/src/modules/learning/**`
- `src/modules/settings/**`
- `docs/_legacy/**`

## Source Of Truth

- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-016 Canonical Run Event Log Foundation](./MIG-016-canonical-run-event-log-foundation.md)
- [MIG-017 Runtime Projection Chat Truth Cutover](./MIG-017-runtime-projection-chat-truth-cutover.md)

## Required Changes

1. 新增按 session 查询 event page 的 API，支持 `limit` + cursor。
2. 提供 replay 路径，把 event log 重建为 conversation history projection。
3. replay 至少覆盖 user / assistant / thinking / tool_use / tool_result / completion。
4. 当 event log 不存在时，允许兼容回退到现有 `get_session` 路径。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml history`
- `npm test -- history-replay`
- `npm run build`
- 至少一条测试验证 paging 前后页无重复无丢失
- 至少一条测试验证 replay 同一批 events 输出稳定

## Out Of Scope

- 不删除 `get_session`
- 不做多端同步
- 不做 tool attempt ledger UI

## Execution Notes

- 先让 replay 正确，再讨论 snapshot checkpoint 加速。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
