# MIG-008 Harness Replay And Eval On Canonical Run Report

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [harness-eval-and-replay](../../staff-remediation/gap-modules/harness-eval-and-replay/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 harness 的 replay/eval 建立在 canonical run report 上，而不是继续围绕大量 seam/skeleton truth 做前置治理。

## Why Now

1. harness 本身不是问题，问题是它现在领先于产品主链太多。
2. 只有当 turn spine、prompt trace、memory lifecycle 稳住后，replay/eval 才有可靠输入。
3. 需要避免继续扩大“治理很强、产品闭环很弱”的结构失衡。

## Allowed Files

- `src-tauri/src/modules/harness/**`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`
- `docs/staff-remediation/if2ai-workflow-truth.md`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Harness Eval And Replay Usage](../../staff-remediation/gap-modules/harness-eval-and-replay/01-usage-guide.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)

## UClaw References

- [engine/facade.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/engine/facade.rs)
- [CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md)

## Required Changes

1. 定义 canonical run report 最小输入面。
2. 让 replay/eval 从该 report 读取，而不是从分散 trace 拼装。
3. 回写 workflow truth 的 `harness_replay` / `harness_eval` 状态。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条 replay 或 eval 相关测试通过
- workflow truth 中相关项不再是 `not_established`

## Out Of Scope

- 不做 learning candidate compare
- 不做 activation
- 不做前端治理页

## Execution Notes

- 这件事必须后置于主链稳定后。
