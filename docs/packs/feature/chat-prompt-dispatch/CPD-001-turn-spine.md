# CPD-001 Turn Spine

## Status

- State: `active`
- Owner: `@executor`
- Gap Module: [chat-prompt-dispatch](../../staff-remediation/gap-modules/chat-prompt-dispatch/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 If2Ai 的 chat prompt dispatch 主链从 `commands/agent.rs` 主导的内联编排，收口为由 `TurnService` 主导的更明确 turn spine。

---

## Why Now

这是当前最值得先收的 P0 gap，因为：

1. `chat_prompt_dispatch` 在 workflow truth 中仍是 `partial`。
2. 只要主 chat spine 还没收口，execution mode、policy、memory、harness 都更像外挂层。
3. 用户感知“什么都没实现”的核心来源，就是发消息后的主行为还没有被新结构真正接管。

参考：

- [if2ai-workflow-truth.md](../../staff-remediation/if2ai-workflow-truth.md)
- [chat-prompt-dispatch/02-implementation.md](../../staff-remediation/gap-modules/chat-prompt-dispatch/02-implementation.md)

---

## Allowed Files

- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/modules/application/prompt_planner.rs`
- `src-tauri/src/modules/application/provider_service.rs`
- `src-tauri/src/modules/application/memory_injection_service.rs`
- `src-tauri/src/modules/application/memory_coordinator.rs`
- `src-tauri/tests/`

---

## Forbidden Files

- `src/modules/settings/**`
- `src/runtime-projection/**`
- `src-tauri/src/modules/learning/**`
- `src-tauri/src/modules/harness/**`
- `docs/exec-plans/**`
- `docs/design-docs/**`

---

## Source Of Truth

- [Chat Prompt Dispatch Gap — Usage](../../staff-remediation/gap-modules/chat-prompt-dispatch/01-usage-guide.md)
- [Chat Prompt Dispatch Gap — Implementation](../../staff-remediation/gap-modules/chat-prompt-dispatch/02-implementation.md)
- [If2Ai Workflow Truth Registry](../../staff-remediation/if2ai-workflow-truth.md)

---

## UClaw References

- [CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md)
- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs)
- [memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs)

---

## Required Changes

1. 收口 `run_agent_turn` 与 `start_agent_stream` 的 shared preflight orchestration，进一步减少它们直接处理 provider / prompt / memory 的内联逻辑。
2. 让 `TurnService` 更明确承担 chat turn 的准备主线，减少 command 层直接理解 prompt block 和 memory artifact 的责任。
3. 为后续真实 execution kernel 留出清晰的 `prepare -> execute -> finalize` 结构，而不是继续把关键主线散落在 command 层。
4. 增加至少一条针对 turn spine 的回归测试，证明主路径仍可工作。

---

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条新增或更新的测试通过：
  - `cargo test --manifest-path src-tauri/Cargo.toml`
- `agent.rs` 中 provider / prompt / memory 准备相关主线职责进一步下降。
- `TurnService` 文档与代码都更明确地体现 chat turn spine 角色。

---

## Out Of Scope

- 不做 execution mode routing 真分流
- 不做 runtime projection 前端迁移
- 不做 harness compare / gate 增量改造
- 不做 learning / strategy lifecycle
- 不做 activation lifecycle

---

## Execution Notes

1. 先复述 Goal、Allowed Files、Acceptance、Out Of Scope 再开始修改。
2. 不要读取其他规划文档，除非本 pack 已经明确引用。
3. 这次重点是“收主线”，不是“顺手做更多模块化”。

