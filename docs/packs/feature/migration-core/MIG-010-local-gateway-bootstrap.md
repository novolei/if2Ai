# MIG-010 Local Gateway Bootstrap

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-23`

---

## Goal

在 If2Ai 内建立一个最小可用的 local gateway bootstrap seam，让前端未来可以不直接依赖海量 Tauri command，而是先通过稳定的 gateway URL / health / bootstrap surface 接入。

## Depends On

- `MIG-001`

## Unlocks

- `MIG-011`
- `MIG-012`
- `MIG-015`

## Why Now

1. 当前 `src-tauri/src/main.rs` 既是 native host，又是超大业务命令暴露面，这会阻塞后续所有桌面壳层收缩。
2. `cc-haha-main` 最值得迁移的不是 UI，而是 `desktop host -> local server` 这个稳定边界。
3. 不先建立 gateway bootstrap，前端 API facade 只能继续绑死在 `invoke/listen` 海洋上。

## Allowed Files

- `src-tauri/src/main.rs`
- `src-tauri/src/modules/**`
- `src-tauri/src/commands/**`
- `src/lib/tauri.ts`
- `src/transport/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/components/**`
- `src/modules/chat/**`
- `src/modules/settings/**`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)
- [Backend Application Control Plane Refactor Design](../../staff-remediation/backend-application-control-plane-refactor-design.md)

## UClaw / Benchmark References

- [desktop/src-tauri/src/lib.rs](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src-tauri/src/lib.rs>)
- [desktop/src/lib/desktopRuntime.ts](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src/lib/desktopRuntime.ts>)

## Current Evidence

- [src-tauri/src/main.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/main.rs:9) 直接汇总大批业务 command。
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1) 仍承担前端主要 transport facade。
- [desktop/src-tauri/src/lib.rs](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src-tauri/src/lib.rs:42>) 只暴露极少数 host 级 command，如 `get_server_url`。

## Required Changes

1. 建立 If2Ai 的 local gateway bootstrap contract，至少包含 `get_gateway_url` 或等价 bootstrap surface。
2. 建立 gateway health / readiness seam，避免前端在服务未就绪时直接撞业务 command。
3. 把 gateway bootstrap 与 native host 责任从业务 command 面中显式区分出来。
4. 为后续 `src/api/*` facade 预留统一入口，不要求本 pack 完成全部前端切换。

## Cutover

- Canonical bootstrap seam: local gateway URL + health/readiness contract。
- Legacy path to retire: 前端直接把 Tauri command registry 当作唯一服务入口。
- Legacy path retired when frontend bootstrap 可以通过统一 gateway seam 感知 backend ready 状态。

## Guardrails

- Do not merge if只是新增一个 URL command，但没有 readiness / health 语义。
- Do not merge if gateway seam 仍然要求前端自己知道大量 command 级细节。
- Rollback plan: 回退到 direct Tauri path，但不得留下既有 gateway bootstrap 又有无声明 direct boot 的双入口。

## Workflow Truth Delta

- `app_boot: partial -> partial`
- `chat_prompt_dispatch: partial -> partial`

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build`
- 至少一条 gateway bootstrap / readiness 测试通过
- 前端存在可调用的统一 bootstrap seam，而不只是业务 command 列表

## Code Audit 2026-04-23

- Status: done; skip bootstrap foundation.
- Evidence: `gateway_service`, `gateway_bootstrap_test`, frontend `gateway-re-export`, and boot readiness seam exist.
- Remaining Gap: real transport replacement remains future work, not this pack.

## Out Of Scope

- 不做 conversations / streaming API 全量迁移
- 不做前端 AppShell 重构
- 不做 activation lifecycle

## Not This Pack

- 即使相关，也不在本 pack 内做 chat streaming cutover。
- 即使相关，也不在本 pack 内做 session/chat store 引入。

## Execution Notes

- 目标是先把“服务入口”独立出来，而不是立刻重写全部 transport。
