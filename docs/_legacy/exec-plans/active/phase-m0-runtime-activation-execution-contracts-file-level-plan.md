# Phase M0 Runtime Activation Execution Contracts File-Level Plan

> 覆盖 `m0.3 + m0.4 + m0.5`。
>
> 最后更新: 2026-04-20

## 1. 目的

把 `M0` 中最容易漂移的 contract 工作拆成文件级任务，确保 runtime / activation / execution-mode 的第一版 canonical contract 真正落在当前代码树，而不是继续留在蓝图里。

## 2. 覆盖切片

1. `m0.3` runtime contract skeleton
2. `m0.4` activation contract + boot truth
3. `m0.5` execution-mode contract + reason taxonomy

## 3. 新增文件

- `src-tauri/src/modules/runtime/contracts/mod.rs`
- `src-tauri/src/modules/runtime/contracts/common.rs`
- `src-tauri/src/modules/runtime/contracts/activation.rs`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`
- `src-tauri/src/modules/runtime/contracts/memory.rs`
- `src/transport/contracts.ts`

## 4. 修改文件

- `src-tauri/src/modules/runtime/mod.rs`
- [docs/staff-remediation/if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1)
- [docs/staff-remediation/README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)

## 5. 执行顺序

### 5.1 `C1` build runtime contract skeleton

必须先建 Rust contract 入口，再建 TS 对位类型。

Rust 最低结构：

1. `mod.rs` 导出 `common / activation / execution_mode / memory`
2. `common.rs` 定义 `RuntimeEventEnvelope / RuntimeEventType / SchemaVersion`
3. `activation.rs` 定义 `ActivationStatus / ActivationSnapshot`
4. `execution_mode.rs` 定义 `ExecutionMode / RiskLevel / ComplexityLevel / ExecutionModeDecision`
5. `memory.rs` 定义 `MemoryKind / MemoryScope / MemoryDecision / MemoryProjection`

TS 最低对位：

1. `RuntimeEventEnvelope`
2. `ActivationStatus`
3. `ActivationSnapshot`
4. `ExecutionMode`
5. `ExecutionModeDecision`
6. `MemoryProjection`

### 5.2 `C2` wire runtime module exports

必须确认 `src-tauri/src/modules/runtime/mod.rs` 能稳定导出 `contracts`，避免后续 `M1/M2` 再回头修导出链。

### 5.3 `C3` define activation contract v1

在 `activation.rs` 与 [if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1) 中同时固定：

必须覆盖状态：

1. `checking_local`
2. `needs_activation`
3. `requesting_activation`
4. `pending_approval`
5. `redeeming`
6. `activated`
7. `offline_grace`
8. `expired`
9. `revoked`
10. `deactivated`

必须覆盖动作：

1. activation request
2. activation redeem
3. license refresh
4. revoke check
5. deactivate
6. local boot restore

### 5.4 `C4` write boot truth

文档中必须写清：

`startup -> onboarding -> activation gate -> main shell`

每段都要有：

1. 进入条件
2. 退出条件
3. 决策方
4. 失败回流

### 5.5 `C5` define execution-mode contract v1

在 `execution_mode.rs` 和 `src/transport/contracts.ts` 中固定四类 canonical modes：

1. `direct_execute`
2. `auto_plan_execute`
3. `plan_then_confirm`
4. `specialized_surface`

`ExecutionModeDecision` 至少包含：

1. `execution_mode`
2. `risk_level`
3. `complexity_level`
4. `complexity_score`
5. `reason_codes`
6. `route_hint`
7. `requires_plan`
8. `classifier_policy_version`
9. `classifier_matched_rule_ids`
10. `classifier_slot_summary`
11. `classifier_ambiguous_escalated`
12. `classifier_escalation_source`

### 5.6 `C6` update remediation index mapping

回写 [README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)，明确 `M0` 的 contract 落点由本文件承接。

## 6. 必跑验证

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

若已存在前端类型检查链，也建议补跑：

```bash
npm run build:web
```

## 7. 不允许做的事

1. 不在 `src-tauri/src/runtime/` 平行再造一棵新树
2. 不把 `src/lib/tauri.ts` 继续当 contract 真相中心
3. 不在这一段实现完整 activation gate 或 classifier 逻辑
4. 不把 `execution_mode` 和 scenario profile 混为一谈

## 8. 完成定义

只有当 reviewer 能明确指出 runtime / activation / execution-mode 的 canonical type 入口分别在哪里，并确认前端只是投影这些 contract，而不是自己再算一遍时，这一段才算完成。
