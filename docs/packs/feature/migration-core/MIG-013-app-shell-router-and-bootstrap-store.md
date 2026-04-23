# MIG-013 App Shell Router And Bootstrap Store

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [runtime-projection-and-shell](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)
- Last Updated: `2026-04-23`

---

## Goal

把 If2Ai 前端主入口从单一 `App.tsx` 大壳层，拆成 `AppShell + ContentRouter + bootstrap store` 的稳定结构，让 boot / ready / error / main-shell 责任不再揉在一个组件里。

## Depends On

- `MIG-012`

## Unlocks

- `MIG-014`
- `MIG-009`

## Why Now

1. 当前 `App.tsx` 同时承担 boot、onboarding、project/session 初始化、UI 容器与多类局部状态。
2. 只有先把壳层容器化，后续 chat/session/settings store 才有清晰挂载点。
3. benchmark 的 AppShell 价值在于稳定的容器职责，而不是视觉样式本身。

## Allowed Files

- `src/App.tsx`
- `src/app/**`
- `src/boot/**`
- `src/modules/app-shell/**`
- `src/state/**`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/**`
- `src/components/ui/chat-ui.tsx`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [Frontend Information Architecture UI Redesign](../../staff-remediation/frontend-information-architecture-ui-redesign.md)
- [Runtime Projection And Shell Usage](../../staff-remediation/gap-modules/runtime-projection-and-shell/01-usage-guide.md)

## UClaw / Benchmark References

- [desktop/src/App.tsx](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/App.tsx>)
- [desktop/src/components/layout/AppShell.tsx](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/components/layout/AppShell.tsx>)
- [desktop/src/components/layout/ContentRouter.tsx](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/components/layout/ContentRouter.tsx>)

## Current Evidence

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:89) 仍是超大主入口。
- [src/boot/BootShell.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/boot/BootShell.tsx:1) 目前还是 minimal container。
- [src/shell/MainShell.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/shell/MainShell.tsx:1) 目前仍是 transition shell，而非完整 router owner。

## Required Changes

1. 建立明确的 `AppShell` / `ContentRouter` / bootstrap store 分层。
2. 把 boot readiness、startup error、surface selection 从 `App.tsx` 中收出去。
3. 让 `App.tsx` 降为壳层挂载点，而不是业务状态总中心。
4. 为后续 session/chat/settings store 留出稳定挂载位置。

## Cutover

- Canonical shell container: `AppShell + ContentRouter + bootstrap store`。
- Legacy path to retire: `App.tsx` 单点承载 boot + main surface + state glue。
- Legacy path retired when主壳层状态只经由 shell container 组织。

## Guardrails

- Do not merge if只是把 `App.tsx` 逻辑搬到另一个更大的壳层文件。
- Do not merge if bootstrap store 只是新的全局杂物箱。
- Rollback plan: 回退到现有 `App.tsx`，但不得留下双重 boot router。

## Workflow Truth Delta

- `app_boot: partial -> canonical_ready`

## Acceptance

- `npm run build`
- 至少一条 shell / router / bootstrap store 测试通过
- `App.tsx` 职责显著下降，主壳层存在明确统一入口

## Code Audit 2026-04-23

- Status: done; skip AppShell/router/bootstrap foundation.
- Evidence: `AppShell`, `ContentRouter`, `BootShell`, `bootstrap-store`, and boot/router tests exist and pass.
- Remaining Gap: full shell truth unification belongs to `MIG-006`.

## Out Of Scope

- 不做 chat transcript/composer 细拆
- 不做 activation remote lifecycle
- 不做 backend gateway conversations

## Not This Pack

- 即使相关，也不在本 pack 内引入完整 session/chat store。
- 即使相关，也不在本 pack 内做 `chat-ui.tsx` 全量拆分。

## Execution Notes

- 先拆容器，再拆聊天；不要试图一步把所有页面一起重构。
