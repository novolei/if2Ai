# MIG-023 Canonical Run Report From Event Log

## Status

- State: `partial`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

让 harness / grading / replay 不再依赖分散证据，而是基于 canonical event log 生成统一 run report。

## Depends On

- `MIG-016`
- `MIG-018`
- `MIG-021`
- `MIG-022`

## Unlocks

- `MIG-008`

## Why Now

1. 没有 canonical run report，harness 仍要拼装零散 traces。
2. event log 一旦落地，就应该成为 replay / eval / audit 的统一输入。
3. benchmark 的 session 完整性优势，本质也是 reportable run truth。

## Allowed Files

- `src-tauri/src/modules/harness/**`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/src/modules/application/stream_emitter_service.rs`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/settings/**`
- `docs/_legacy/**`

## Source Of Truth

- [Current Architecture](../../../../ARCHITECTURE.md)
- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-008 Harness Replay And Eval On Canonical Run Report](./MIG-008-harness-replay-and-eval-on-canonical-run-report.md)
- [MIG-016 Canonical Run Event Log Foundation](./MIG-016-canonical-run-event-log-foundation.md)

## Required Changes

1. 基于 event log 构建 canonical `run report`。
2. report 至少包含 run summary、stream outcome、tool summary、permission summary、memory after turn、recoverability summary。
3. harness 优先读取 report，而不是直接拼零散 traces。
4. 至少新增一条测试验证 event log -> run report 的稳定转换。

## Acceptance

- `cargo test --manifest-path src-tauri/Cargo.toml harness`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- 至少一条测试验证 report 独立于旧 trace 拼装路径仍可生成

## Code Audit 2026-04-23

- Status: partial.
- Evidence: harness `run_report`, `trace_aggregator`, `suite_report`, compare and graders exist.
- Remaining Gap: canonical run report is not yet generated directly from event log; harness still has an event-bus/trace truth path.

## Out Of Scope

- 不做前端 report UI
- 不删除旧 harness 路径
- 不做商业化 telemetry

## Execution Notes

- 先让 report 能独立生成，再逐步让 grader 改读它。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
