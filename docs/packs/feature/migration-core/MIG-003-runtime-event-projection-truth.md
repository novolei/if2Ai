# MIG-003 Runtime Event Projection Truth

## Status

- State: `partial`
- Owner: `@executor`
- Gap Module: [runtime-projection-and-shell](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

把 if2Ai 前端从“老 listener + projection bridge 并行运行”迁移到“canonical runtime event -> processor -> projection store -> UI”单一真相路径。

## Why Now

1. 只要 projection 仍是并行层，前端就不会真正受 runtime state 驱动。
2. UClaw 的前端闭环能力主要来自 event ingestion 和 AppStore 投影稳定，而不是单个页面设计。
3. 当前 `App.tsx` 与 `chat-ui.tsx` 过大，说明 runtime 语义尚未被 store 接管。

## Allowed Files

- `src/runtime-projection/**`
- `src/App.tsx`
- `src/components/ui/chat-ui.tsx`
- `src/modules/chat/components/**`
- `src/lib/tauri.ts`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/src/modules/learning/**`
- `src-tauri/src/modules/harness/**`
- `docs/exec-plans/**`

## Source Of Truth

- [Current Architecture](../../../../ARCHITECTURE.md)
- [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)
- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Runtime Projection And Shell Usage](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)
- [Runtime Projection And Shell Implementation](../../staff-remediation/gap-modules/runtime-projection-and-shell/02-implementation.md)

## UClaw References

- [run_state/projection.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/run_state/projection.rs)
- [RuntimeEventProcessor.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/RuntimeEvents/RuntimeEventProcessor.swift)
- [AppStore+EventProcessing.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore+EventProcessing.swift)

## Required Changes

1. 明确唯一 runtime event ingestion 入口。
2. 停止让 chat 页面直接理解底层 event payload。
3. 把老 stream listener 逐步 cutover 到 projection store。
4. 让 run state、approval state、memory state、turn state 通过统一 reducer 投影。

## Acceptance

- `npm run build`
- 关键聊天 UI 不再同时依赖老 listener 与 projection bridge 作为双真相
- 至少一条前端状态投影测试通过

## Code Audit 2026-04-23

- Status: partial.
- Evidence: `runtimeProjectionStore`, bridge, reducer, `chat-run-projection`, and projection/history tests exist; targeted frontend tests passed.
- Remaining Gap: `App.tsx` / chat still use compatibility raw stream listeners and `conversation-slice`; final UI is not yet projection-only.

## Out Of Scope

- 不做 settings 页整理
- 不做 strategy diagnostics
- 不做 activation 商业逻辑

## Execution Notes

- 先缩真相来源，再缩 UI 文件体积。
- 本 Pack 的设计、实现、review、验收必须完整参照 [If2Ai vNext Session Runtime Blueprint](../../../design-docs/if2ai-vnext-session-runtime-blueprint.md)。
