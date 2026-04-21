# MIG-009 Activation License Lifecycle

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [activation-license-lifecycle](../../staff-remediation/gap-modules/activation-license-lifecycle/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 activation/license/deactivation 从当前 placeholder overlay + local snapshot，升级成真实可执行的生命周期系统。

## Why Now

1. 这是必要商业能力，但不应排在 runtime spine 前面。
2. 当前 overlay 已经提供了 mount point，但后端远程生命周期还没有真正存在。
3. 必须避免在半成品主链上优先堆商业 gate。

## Allowed Files

- `src/boot/**`
- `src/runtime-projection/**`
- `src-tauri/src/modules/activation/**`
- `src-tauri/src/commands/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Activation License Lifecycle Usage](../../staff-remediation/gap-modules/activation-license-lifecycle/01-usage-guide.md)
- [Activation License Lifecycle Implementation](../../staff-remediation/gap-modules/activation-license-lifecycle/02-implementation.md)

## UClaw References

- `UClawApp` shell lifecycle patterns

## Required Changes

1. 落地 activation request / redeem / refresh / revoke check / deactivate 的后端 contract。
2. 让 activation projection 反映真实远端状态，而不是仅本地 onboarding 状态。
3. 让 overlay 从 placeholder 升级成真实 lifecycle surface。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build`
- activation 状态来源不再只是 local snapshot

## Out Of Scope

- 不做 chat runtime 主链重构
- 不做 harness replay/eval

## Execution Notes

- 严格后置，必须在主链 P0/P1 基本稳定后推进。
