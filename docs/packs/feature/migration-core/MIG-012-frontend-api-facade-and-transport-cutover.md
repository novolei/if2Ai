# MIG-012 Frontend API Facade And Transport Cutover

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-21`

---

## Goal

为 If2Ai 前端建立 `src/api/*` 领域 facade，让页面与壳层停止直接依赖 `@/lib/tauri` 的业务 helper，形成可切换 transport 的前端接入层。

## Depends On

- `MIG-010`

## Unlocks

- `MIG-013`
- `MIG-014`
- `MIG-015`

## Why Now

1. 当前 `src/lib/tauri.ts` 仍是 mega facade，前端容器层知道太多后端命令细节。
2. 如果不先做 API facade，后续 AppShell/store 重构只会把 `invoke` 复制到更多文件里。
3. benchmark 的桌面前端稳定性，很大程度来自 `api client -> domain api -> store/page` 这条接入组织方式。

## Allowed Files

- `src/lib/tauri.ts`
- `src/api/**`
- `src/transport/**`
- `src/App.tsx`
- `src/**/*.test.*`

## Forbidden Files

- `src-tauri/**`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [Runtime Contracts And Event Projection Design](../../staff-remediation/runtime-contracts-and-event-projection-design.md)

## UClaw / Benchmark References

- [desktop/src/api/client.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/api/client.ts>)
- [desktop/src/api/sessions.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/api/sessions.ts>)
- [desktop/src/api/settings.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/api/settings.ts>)

## Current Evidence

- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1) 仍混合 event listener、DTO、业务 helper。
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:3) 直接 import 大量 Tauri helper。

## Required Changes

1. 新建统一 frontend API facade 层，如 `src/api/client.ts` 与分领域 `src/api/*`。
2. 让 `App.tsx`、shell、store 不再直接 import 大量 `@/lib/tauri` 业务 helper。
3. 把 `src/lib/tauri.ts` 收缩到底层 transport / native bridge 责任。
4. 保留 runtime projection 所需 listener seam，但把业务读写面迁出。

## Cutover

- Canonical frontend access seam: `src/api/*` facade。
- Legacy path to retire: 页面和容器层直接依赖 `@/lib/tauri` 的业务 helper。
- Legacy path retired when新前端入口默认通过 `src/api/*` 读写业务数据。

## Guardrails

- Do not merge if只是新增 `src/api`，但现有壳层继续直接 import 大量 `tauri.ts` helper。
- Do not merge if `src/api/*` 只是简单 re-export，未形成领域边界。
- Rollback plan: 保留 `tauri.ts` 直连，但撤回未落地的 facade 入口。

## Workflow Truth Delta

- `app_boot: partial -> partial`
- `stream_projection: partial -> partial`

## Acceptance

- `npm run build`
- 至少一条 frontend API facade 或 transport adapter 测试通过
- `App.tsx` 直接 import 的 `@/lib/tauri` 业务 helper 数量显著下降

## Out Of Scope

- 不做 AppShell/router 全量重构
- 不做 chat store 引入
- 不做后端 gateway conversations

## Not This Pack

- 即使相关，也不在本 pack 内完成所有页面切换。
- 即使相关，也不在本 pack 内做 runtime projection canonical cutover。

## Execution Notes

- 先立 facade，再拆页面；不要反过来做。
