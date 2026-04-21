# UClaw Migration Pack Index

> 基于 [UClaw Gap Migration Audit](../staff-remediation/uclaw-gap-migration-audit.md) 收敛出的唯一迁移 pack 清单。
>
> 最后更新: 2026-04-21

## 使用方式

1. 先读本索引确认当前应该推进哪个迁移模块。
2. 再只打开对应 pack。
3. 默认不要再回头遍历大量 `exec-plans` 或 phase 文档。

## Migration Packs

1. [MIG-001 Canonical Chat Execution Spine](./migration-core/MIG-001-canonical-chat-execution-spine.md)
2. [MIG-002 Execution Mode Routing And Policy Enforcement](./migration-core/MIG-002-execution-mode-routing-and-policy-enforcement.md)
3. [MIG-003 Runtime Event Projection Truth](./migration-core/MIG-003-runtime-event-projection-truth.md)
4. [MIG-004 Prompt Planning Traceability](./migration-core/MIG-004-prompt-planning-traceability.md)
5. [MIG-005 Real Memory Lifecycle](./migration-core/MIG-005-real-memory-lifecycle.md)
6. [MIG-006 Frontend Shell Truth](./migration-core/MIG-006-frontend-shell-truth.md)
7. [MIG-007 Worker Tool Execution Contract](./migration-core/MIG-007-worker-tool-execution-contract.md)
8. [MIG-008 Harness Replay And Eval On Canonical Run Report](./migration-core/MIG-008-harness-replay-and-eval-on-canonical-run-report.md)
9. [MIG-009 Activation License Lifecycle](./migration-core/MIG-009-activation-license-lifecycle.md)

## 排序原则

- `MIG-001` 到 `MIG-005` 是主链路闭环层，优先级最高。
- `MIG-006` 到 `MIG-008` 是在主链路稳定后接入的产品化与验证层。
- `MIG-009` 是重要但后置的商业/生命周期层。

## 当前建议

- 当前唯一建议 active pack: [MIG-001 Canonical Chat Execution Spine](./migration-core/MIG-001-canonical-chat-execution-spine.md)
