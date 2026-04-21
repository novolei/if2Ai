# MIG-006 Frontend Shell Truth

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [runtime-projection-and-shell](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 if2Ai 前端主壳层改造成由统一 shell truth 驱动，而不是由多个页面、overlay 和局部状态各自解释运行时。

## Why Now

1. 只有在 runtime projection 成为真相之后，shell 层才能真正稳定。
2. UClaw 值得迁移的是 AppStore 方法，不是 SwiftUI 写法本身。
3. 当前 activation overlay、chat shell、session shell 仍然分散。

## Allowed Files

- `src/App.tsx`
- `src/boot/**`
- `src/modules/chat/components/**`
- `src/runtime-projection/**`
- `src/state/**`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Runtime Projection And Shell Implementation](../../staff-remediation/gap-modules/runtime-projection-and-shell/02-implementation.md)

## UClaw References

- [AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift)
- [AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift)

## Required Changes

1. 建立 shell truth store 或等价总线。
2. 让 activation/session/run/composer 状态通过统一壳层投影。
3. 页面消费 projection，不自己解释 transport payload。

## Acceptance

- `npm run build`
- `App.tsx` 职责下降
- 主 shell 状态存在清晰统一入口

## Out Of Scope

- 不做 settings 页面大整理
- 不做商业 license 完整实现

## Execution Notes

- 不要把它做成新的前端 god file。
