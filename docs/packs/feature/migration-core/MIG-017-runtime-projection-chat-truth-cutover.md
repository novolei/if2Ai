# MIG-017 Runtime Projection Chat Truth Cutover

## Status

- State: `partial`
- Owner: `@executor`
- Gap Module: [runtime-projection-and-shell](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

让聊天主 UI 从“raw stream listener + conversation slice”并行双真相，迁移到“canonical runtime projection -> UI”单一真相路径。

## Depends On

- `MIG-003`
- `MIG-014`
- `MIG-015`
- `MIG-016`

## Unlocks

- `MIG-018`
- `MIG-020`

## Why Now

1. 只要 chat 主路径还直接理解 transport payload，projection 就仍是附属层。
2. event log 落地后，前端必须尽快切到 replayable projection，不然会形成第三套真相。
3. benchmark 的 session 完整性来自稳定 projection，而不是页面自己拼状态。

## Allowed Files

- `src/runtime-projection/**`
- `src/stores/**`
- `src/modules/chat/**`
- `src/api/**`
- `src/App.tsx`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/**`
- `src/modules/settings/**`
- `docs/_legacy/**`

## Source Of Truth

- [Current Architecture](../../../../ARCHITECTURE.md)
- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [MIG-003 Runtime Event Projection Truth](./MIG-003-runtime-event-projection-truth.md)
- [MIG-014 Session And Chat Store Foundation](./MIG-014-session-and-chat-store-foundation.md)

## Required Changes

1. chat 主 UI 渲染 assistant text / thinking / tool cards / completion 只读 projection store。
2. raw stream listener 不再直接塑造最终 UI message，只负责把 event 送入 projection。
3. 刷新后可用 projection snapshot 恢复当前 run 文本、thinking、tool 状态。
4. 至少新增一条 reducer / projection 测试覆盖主聊天路径。

## Acceptance

- `npm test -- runtime-projection`
- `npm test -- chat-store`
- `npm run build`
- 聊天主路径不再长期并行依赖老 listener 与 projection 作为双真相

## Code Audit 2026-04-23

- Status: partial.
- Evidence: `chat-run-projection` and runtime projection tests pass; UI already overlays run projection onto chat messages.
- Remaining Gap: final chat UI still derives from `Conversation + RunProjection`; raw listener and `conversation-slice` are not fully retired.

## Out Of Scope

- 不做 settings / shell 全量重构
- 不做 activation lifecycle
- 不做 history paging

## Execution Notes

- 先 cut 真相来源，再压缩组件体积。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
