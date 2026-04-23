# MIG-014 Session And Chat Store Foundation

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

为 If2Ai 建立最小可用的 `session store + chat store` 基础层，把 session list、active session、streaming state、permission prompt state 从 `App.tsx` 及零散组件状态中收口。

## Depends On

- `MIG-012`
- `MIG-013`

## Unlocks

- `MIG-015`
- `MIG-006`

## Why Now

1. 没有 session/chat store，AppShell 只会变成新的 props drilling 中枢。
2. 当前 streaming、permission、conversation、session loading 状态仍散落在 `App.tsx` 和聊天组件里。
3. benchmark 的桌面稳定性，本质上来自 `api -> store -> page` 的运行组织方式。

## Allowed Files

- `src/stores/**`
- `src/modules/chat/**`
- `src/App.tsx`
- `src/app/**`
- `src/api/**`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/**`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [Frontend Information Architecture UI Redesign](../../staff-remediation/frontend-information-architecture-ui-redesign.md)

## UClaw / Benchmark References

- [desktop/src/stores/sessionStore.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/stores/sessionStore.ts>)
- [desktop/src/stores/chatStore.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/stores/chatStore.ts>)

## Current Evidence

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:204) 仍直接维护 projects、sessions、conversations、sessionLoading、streamAbortHandles 等状态。
- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1) 仍需要承接大量运行期状态输入。
- [src/stores/conversation-slice.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/conversation-slice.ts:1) 目前还不是 canonical session/chat truth。

## Required Changes

1. 建立 session store：负责 session list、active session、session metadata。
2. 建立 chat store：负责 per-session message stream、pending permission、stream state、stop/abort 语义。
3. 让壳层和页面从 store 消费会话态，不再各自维护同类状态。
4. 为后续 conversations/streaming gateway cutover 预留 store action seam。

## Cutover

- Canonical session/chat truth: dedicated session/chat stores。
- Legacy path to retire: `App.tsx` 直接维护核心 session/chat runtime state。
- Legacy path retired when新增聊天页面默认经由 store 读写会话状态。

## Guardrails

- Do not merge if store 只是把 `useState` 平移成另一个全局 god store。
- Do not merge if streaming 与 permission 仍各自绕过 store 直接进 UI。
- Rollback plan: 回退到局部 state，但撤回未完成的 store 挂接点。

## Workflow Truth Delta

- `stream_projection: partial -> partial`
- `app_boot: canonical_ready -> canonical_ready`

## Acceptance

- `npm run build`
- 至少一条 session store 或 chat store 测试通过
- `App.tsx` 不再直接持有主要 per-session chat runtime state

## Code Audit 2026-04-23

- Status: done; skip store foundation.
- Evidence: `session-store`, `conversation-slice`, `chat-store`, and session/chat store tests exist and pass.
- Remaining Gap: final projection-only runtime truth belongs to `MIG-017` / `GAP-003`.

## Out Of Scope

- 不做后端 streaming 协议重构
- 不做 `chat-ui.tsx` 全量拆分
- 不做 settings/domain store 全量引入

## Not This Pack

- 即使相关，也不在本 pack 内做 gateway conversations API 落地。
- 即使相关，也不在本 pack 内做 runtime projection canonical cutover。

## Execution Notes

- 先把会话态收口，再做聊天组件切片，否则会一边拆一边回流。
