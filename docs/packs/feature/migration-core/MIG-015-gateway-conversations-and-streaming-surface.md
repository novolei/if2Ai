# MIG-015 Gateway Conversations And Streaming Surface

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-22`
- Completed: `2026-04-22` (App.tsx 主聊天链已完成 cutover，使用 startChatTurn API)

---

## Goal

把 If2Ai 的 conversations / streaming / permission response 主路径接到 local gateway surface 上，让前端面对稳定的会话 API 与流式事件，而不是直接围绕 Tauri command 拼主聊天链路。

## Depends On

- `MIG-001`
- `MIG-010`
- `MIG-012`
- `MIG-014`

## Unlocks

- `MIG-003`
- `MIG-007`
- `MIG-008`

## Why Now

1. 只有真正把 conversations 与 streaming surface 稳定下来，桌面壳层重构才会落到产品主路径。
2. 当前 `run_agent_turn` / `start_agent_stream` / `respond_permission` 仍通过 direct Tauri seam 进入前端主流程。
3. benchmark 最强的一段正是 session lifecycle 与 stream/permission 由服务层集中治理。

## Allowed Files

- `src-tauri/src/main.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/modules/application/**`
- `src-tauri/src/modules/control_plane/**`
- `src/lib/tauri.ts`
- `src/api/**`
- `src/stores/**`
- `src/modules/chat/**`
- `src-tauri/tests/**`
- `src/**/*.test.*`

## Forbidden Files

- `src/modules/settings/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)
- [MIG-001 Canonical Chat Execution Spine](./MIG-001-canonical-chat-execution-spine.md)

## UClaw / Benchmark References

- [src/server/services/conversationService.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/src/server/services/conversationService.ts>)
- [src/server/__tests__/conversation-service.test.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/src/server/__tests__/conversation-service.test.ts>)
- [desktop/src/lib/desktopRuntime.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/lib/desktopRuntime.ts>)

## Current Evidence

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1) 仍是 direct Tauri chat command 主入口。
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:202) 仍直接暴露 `runAgentTurn`、`startAgentStream` 等 helper。
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:3) 当前聊天主链依赖 direct command helper。

## Required Changes

1. 为 conversations / streaming / permission response 建立统一 gateway surface。
2. 让前端 session/chat store 通过 `src/api/*` 与该 surface 交互。
3. 收口聊天主路径上的 direct Tauri command 使用，避免前端继续了解 runtime 细节。
4. 让 streaming、stop、permission response、history load 至少有一条 canonical service path。

## Cutover

- Canonical conversation surface: local gateway conversations/streaming API。
- Legacy path to retire: `App.tsx` / chat 页面直接围绕 Tauri agent commands 拼主聊天链。
- Legacy path retired when主聊天路径默认经由 gateway surface 而不是 direct command helper。

## Guardrails

- Do not merge if gateway surface 与 direct Tauri command 在主聊天链上长期并存为双真相。
- Do not merge if store 只是换一个调用点，实际 runtime semantics 仍散落在 UI 中。
- Rollback plan: 回退到 direct Tauri path，但撤回未完成的 gateway chat path，避免半切换。

## Workflow Truth Delta

- `chat_prompt_dispatch: partial -> canonical_ready`
- `permission_approval: partial -> canonical_ready`
- `stream_projection: partial -> partial`

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build`
- 至少一条 conversations / streaming / permission surface 测试通过
- 前端主聊天链不再默认直接调用 `@/lib/tauri` 的 agent command helper

## Out Of Scope

- 不做 runtime projection 全量 cutover
- 不做 harness replay/eval
- 不做 activation remote lifecycle

## Not This Pack

- 即使相关，也不在本 pack 内做所有页面的 transport 切换。
- 即使相关，也不在本 pack 内做 learning/self-evolution 接入。

## Execution Notes

- 先让一条主聊天链完全跑通，再扩展到其它会话能力。
