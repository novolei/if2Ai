# MIG-001 Canonical Chat Execution Spine

## Status

- State: `active`
- Owner: `@executor`
- Gap Module: [chat-prompt-dispatch](../../staff-remediation/gap-modules/chat-prompt-dispatch/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 if2Ai 的 chat turn 主链从 `commands/agent.rs` 主导的 command-heavy 内联编排，收口为一个真正拥有 `prepare -> execute -> finalize` 生命周期的 canonical orchestrator。

## Why Now

1. 这是全部迁移的根前提，不先收这条主链，后面 execution mode、memory、projection 都只能挂在旁路上。
2. 用户“什么都没实现”的核心体感，来自主聊天路径仍然没有被新架构真正接管。
3. UClaw 最重要的先进设计不是零散 feature，而是主 turn spine 已经收成单一真相。

## Allowed Files

- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/modules/application/provider_service.rs`
- `src-tauri/src/modules/application/prompt_planner.rs`
- `src-tauri/src/modules/application/memory_coordinator.rs`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src/runtime-projection/**`
- `src-tauri/src/modules/harness/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Chat Prompt Dispatch Usage](../../staff-remediation/gap-modules/chat-prompt-dispatch/01-usage-guide.md)
- [Chat Prompt Dispatch Implementation](../../staff-remediation/gap-modules/chat-prompt-dispatch/02-implementation.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)

## UClaw References

- [engine/facade.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/engine/facade.rs)
- [runtime/contracts.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/runtime/contracts.rs)

## Required Changes

1. 提炼一个唯一 chat turn orchestration 入口，明确拥有 turn lifecycle，而不是只做 preflight。
2. 让 `run_agent_turn` 与 `start_agent_stream` 通过同一 orchestration 主线进入 runtime。
3. 把 provider resolution、prompt prepare、memory prepare、execute handoff、finalize hook 放进同一结构。
4. 明确 command 层只保留 IPC adapter 责任，不再承担实际编排真相。
5. 增加针对 canonical turn spine 的回归测试。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条相关测试通过：
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `TurnService` 或新 orchestrator 不再在 doc comment 中声明“不拥有 runtime construction / tool loop / stream emission”这一类关键主链责任。
- `agent.rs` 中主编排责任显著下降，变成 adapter + bridge，而不是内联 orchestration brain。

## Out Of Scope

- 不做 execution mode 真分流
- 不做前端 runtime projection cutover
- 不做 memory persistence 闭环
- 不做 harness compare/gate 扩展

## Execution Notes

- 先复述 Goal、Allowed Files、Acceptance、Out Of Scope 再开工。
- 只读本 pack 明确引用的真相文档。
