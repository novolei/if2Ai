# MIG-002 Execution Mode Routing And Policy Enforcement

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [execution-mode-policy-routing](../../staff-remediation/gap-modules/execution-mode-policy-routing/01-usage-guide.md)
- Last Updated: `2026-04-21`

---

## Goal

把 if2Ai 的 execution mode 与 step preflight 从 advisory/shadow seam 升级成真正决定请求路由和工具执行去留的 product gate。

## Why Now

1. 当前 classifier、execution pill、prepare step seam 都只是在“说明会怎么做”，没有真实改动主执行路径。
2. 这是用户最容易感知“系统说会判断，但实际上没有变”的地方。
3. UClaw 已经把 boundary、permission、sandbox 放进真实协调器，而不是仅用于 trace。

## Allowed Files

- `src-tauri/src/modules/application/request_intelligence_service.rs`
- `src-tauri/src/modules/control_plane/prepare_step_execution.rs`
- `src-tauri/src/modules/control_plane/**`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/modules/settings/**`
- `src/runtime-projection/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Execution Mode Policy Routing Usage](../../staff-remediation/gap-modules/execution-mode-policy-routing/01-usage-guide.md)
- [Execution Mode Policy Routing Implementation](../../staff-remediation/gap-modules/execution-mode-policy-routing/02-implementation.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)

## UClaw References

- [policy/coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs)
- [runtime/contracts.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/runtime/contracts.rs)

## Required Changes

1. 把 request intelligence 输出接入实际 route choice，而不是仅记录 advisory decision。
2. 把 `prepare_step_execution` 接入真实 preflight 阻断路径。
3. 明确 boundary、permission、sandbox 的统一 outcome contract。
4. 让 command/runtime 层对 denied/requires_approval/granted 有明确行为差异。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条 execution-mode 或 preflight 行为测试通过
- `request_intelligence` 不再只是日志或 preview-only 输出
- `prepare_step_execution` 不再只是 shadow seam

## Out Of Scope

- 不做前端 execution pill 视觉优化
- 不做 activation gate
- 不做 memory policy 扩展

## Execution Notes

- 先把 route semantics 说清，再动前端表现层。
