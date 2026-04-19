# If2Ai Runtime Contracts And Event Projection Design

> 建立 canonical runtime contract，并让前端成为后端真相的投影层。
>
> 最后更新: 2026-04-20

## 1. 目标

本设计解决三个问题：

1. `src/lib/tauri.ts` 目前兼具 transport、类型全集、领域语义，边界不清。
2. 前端仍会直接理解原始 stream / tool / memory payload。
3. execution mode、classifier evidence、memory evidence 尚未成为统一事件合同的一部分。

## 2. 目标结构

后端：

- `runtime/contracts/*.rs`
- `runtime/stream_emitter.rs`
- `commands/*` 只输出 canonical envelope 或 legacy-compatible envelope

前端：

- `src/transport/contracts.ts`
- `src/runtime-projection/runtime-event-translator.ts`
- `src/runtime-projection/runtime-event-queue.ts`
- `src/runtime-projection/runtime-event-reducer.ts`
- feature stores 只消费 normalized envelope

## 3. Canonical Envelope

### 3.1 Top-level fields

- `event_id`
- `schema_version`
- `event_type`
- `run_id`
- `session_id`
- `turn_id`
- `sequence`
- `timestamp_ms`
- `payload`

### 3.2 Required payload families

1. conversation events
2. tool events
3. permission events
4. memory events
5. activation events
6. execution-mode events
7. diagnostics / eval events

## 4. Execution Mode Contract

字段：

- `execution_mode`
- `risk_level`
- `complexity_level`
- `complexity_score`
- `reason_codes`
- `route_hint`
- `requires_plan`
- `classifier_policy_version`
- `classifier_matched_rule_ids`
- `classifier_slot_summary`
- `classifier_ambiguous_escalated`
- `classifier_escalation_source`

## 5. Memory Projection Contract

字段：

- `memory_kind`
- `memory_scope`
- `decision`
- `reason_code`
- `evidence_excerpt`
- `why_captured`
- `stored_at`
- `recall_score`

## 6. Activation Projection Contract

字段：

- `activation_status`
- `license_id`
- `refresh_after_sec`
- `last_trusted_server_time`
- `revoked`
- `expired`
- `deactivated`

## 7. Frontend Projection Rules

1. 前端不重新计算 execution mode。
2. 前端不重新计算 memory write decision。
3. 前端不自行推断 revoke / activation 状态。
4. legacy payload 只能在 translator 层兼容。

## 8. Store 分工

- `run-store`
- `session-store`
- `memory-store`
- `approval-store`
- `execution-mode-store`
- `activation-store`
- `harness-store`

## 9. 执行切片

### Slice C1.1

- 建立 Rust contracts 目录
- 定义 execution mode / activation / memory contract

### Slice C1.2

- TS `contracts.ts`
- translator 兼容旧 payload

### Slice C1.3

- event queue + reducer
- 替换 `App.tsx` 中直接处理的事件逻辑

## 10. 验收

1. `tauri.ts` 不再是事实契约中心。
2. UI 可稳定显示 execution mode、memory evidence、activation state。
3. 断线恢复 / legacy payload 不影响主投影语义。
