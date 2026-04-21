# MIG-004 Prompt Planning Traceability

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [chat-prompt-dispatch](../../staff-remediation/gap-modules/chat-prompt-dispatch/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 prompt planner 从“组装 prompt 的 skeleton”升级成“可追踪、可比较、可回放的 prompt plan contract”。

## Why Now

1. 没有 traceable prompt plan，后面的 replay、compare、regression diagnosis 都会失焦。
2. UClaw 的 planner 强在 plan identity 和 diagnostics，而不是单纯 block 拼接。
3. 当前 if2Ai 的 planner 还缺 plan hash、trace id、diagnostic metadata。

## Allowed Files

- `src-tauri/src/modules/application/prompt_planner.rs`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/src/modules/harness/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Chat Prompt Dispatch Implementation](../../staff-remediation/gap-modules/chat-prompt-dispatch/02-implementation.md)

## UClaw References

- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs)

## Required Changes

1. 为 PromptPlan 增加稳定 identity 与 diagnostics。
2. 为 block 和 plan 建立可比较的 hash / trace 元数据。
3. 让 planner 结果可直接进入 harness/run report，而不是二次猜测。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- planner 结构存在稳定 trace metadata
- 至少一条 planner contract 测试通过

## Out Of Scope

- 不做新的 prompt 文案设计
- 不做 execution mode route
- 不做前端投影迁移

## Execution Notes

- 优先保证 contract 稳定性，而不是把 metadata 做得很花。
