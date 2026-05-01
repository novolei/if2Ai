# Runtime Event Channel Cut-over Design (PR C-1 / C-2 / C-3)

**Date:** 2026-05-01
**Owners:** truth-loop / runtime contracts
**Status:** Approved → ready for plans

---

## 1. 动机

ARCHITECTURE.md §3.4 / §6.1 规定「chat-runtime 事实必须经 `runtime_event` envelope 频道，并由 `RunEventLogger` 落 run-log」。当前 `runtime_event` 频道只走 `evolution_emitter::emit_evolution_event`（10 个 evolution family），但仍有 4 个 chat-runtime 事实在旁路：

| 频道 | 后端位置 | 落 run-log？ |
|---|---|---|
| `agent-token` | `runtime/stream_emitter.rs::AgentStreamEmitter::emit_payload` | 仅 `append_with_correlation`（payload 不含 envelope） |
| `permission-request` | `application/permission_service.rs:109` | 旁路 `event_logger.append_sync("permission_requested", ...)` |
| `memory_event` | `memory/audit.rs::emit_to_frontend` | ❌ 无 |
| `memory_after_turn` | `application/stream_emitter_service.rs:94` | ❌ 无（仅 harness EventBus） |

3 个 PR 把上述 4 个频道一次性切换到 `runtime_event` envelope 频道。

## 2. 范围 / 非范围

**In scope**
- 后端：上述 4 个 emit 点全部改走 `runtime_event` envelope；同步落 run-log（envelope-only path）。
- 前端：`runtime-projection-bridge.ts` 改为单条 `listen('runtime_event')` + 按 `event_type` × `payload_family` 分派到现有 translator。

**Out of scope**
- harness EventBus (`bus.emit(AgentEvent::*)`) — MIG-023 治理范畴，独立计划。
- 系统/操作类频道（`browser-status`、`activation_*`、`compact_completed`、`updater`、`onboarding/*`、`jiaochang/*`、stt 等）— 非 chat 事实。
- `to_envelope()` 已存在；不重新设计。

## 3. 设计

### 3.1 后端共享 helper（C-1 引入）

新建 `src-tauri/src/modules/runtime/runtime_event.rs`：

```rust
//! Canonical helper for emitting any runtime event onto the
//! `runtime_event` Tauri channel with run-log mirroring.
//!
//! Wraps `evolution_emitter::emit_evolution_event` so callers outside
//! the evolution families (agent-token / permission / memory) get the
//! same envelope + log discipline without depending on the
//! "evolution" name.

use serde::Serialize;
use tauri::AppHandle;

use crate::modules::runtime::contracts::common::{
    CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
};
use crate::modules::runtime::evolution_emitter::{emit_evolution_event, EmitError};
use crate::modules::runtime::event_log::RunEventLogger;

/// Emit a canonical runtime envelope to the frontend and (when a
/// logger is provided) append it to the canonical run log via
/// `append_sync_from_envelope`.
///
/// This is a thin wrapper over `emit_evolution_event` so non-evolution
/// emit sites (chat-runtime: agent-token / permission / memory) can
/// adopt the canonical pipeline without leaking the "evolution" name.
pub fn dispatch<T: Serialize>(
    handle: Option<&AppHandle>,
    event_type: RuntimeEventType,
    family: impl Into<String>,
    correlation: CorrelationIds,
    payload: &T,
    logger: Option<&RunEventLogger>,
) -> Result<RuntimeEventEnvelope, EmitError> {
    emit_evolution_event(handle, event_type, family, correlation, payload, logger)
}
```

并在 `runtime/mod.rs` 中 `pub mod runtime_event;`。

`payload_family` 字符串约定（与 `stream_emitter::map_event_type_to_runtime` 对齐）：

| event_type | family（snake_case 字符串） |
|---|---|
| `Conversation` | `text_delta` / `thinking_delta` / `thinking_start` / `tool_call_update` / `final_text_override` / `stream_complete` / `stream_error` / `final_run_report` / `run_started` |
| `Tool` | `tool_call_update` |
| `Permission` | `prompt_opened` / `decision_recorded` |
| `Memory` | `lifecycle` / `after_turn` / `write_decision` / `skill_resolution_snapshot` |

### 3.2 前端 family 路由

`src/runtime-projection/runtime-event-bridge-router.ts`（C-1 新建）暴露：

```ts
export type EnvelopeRoute = (envelope: RuntimeEventEnvelope) => void

export interface FamilyHandler {
  /** Match the envelope.event_type (snake_case from Rust enum). */
  eventType: string
  /** Optional payload_family narrowing. When omitted, matches all. */
  family?: string | string[]
  handle: EnvelopeRoute
}

export function makeEnvelopeRouter(handlers: FamilyHandler[]): EnvelopeRoute {
  return (envelope) => {
    for (const h of handlers) {
      if (h.eventType !== envelope.eventType) continue
      const fam = envelope.payloadFamily
      if (h.family) {
        const list = Array.isArray(h.family) ? h.family : [h.family]
        if (!list.includes(fam)) continue
      }
      h.handle(envelope)
      return
    }
  }
}
```

`runtime-projection-bridge.ts` 在 C-1 内：
- 新增 `listen(RUNTIME_EVENT_CHANNEL, router)`（router 由各 PR 注入 family handler）。
- 移除该 PR 对应的 legacy `listenTo*` 调用。

### 3.3 Per-PR 拆分

| PR | 后端文件 | 前端 handler | 移除 listener |
|---|---|---|---|
| **C-1** | `runtime/runtime_event.rs`（新）+ `runtime/stream_emitter.rs::emit_payload` 改造 | Conversation / Tool family handler（`translateAgentTokenPayload` 复用） | `listenToAgentTokenStream` |
| **C-2** | `application/permission_service.rs` | Permission family handler（`translatePermissionRequestPayload` 复用） | `listenToPermissionRequests` |
| **C-3** | `memory/audit.rs` + `application/stream_emitter_service.rs` | Memory family handler（`translateMemoryEventPayload` + `translateMemoryAfterTurn`） | `listenMemoryEvent` + `listenMemoryAfterTurn` |

每条 PR 在 `runtime-projection-bridge.ts` 内独立追加 `FamilyHandler` 条目，文件级冲突仅为机械合并。

### 3.4 兼容期与回滚

- 选用 **B（cut-over）**：单 PR 内同时切后端 emit 点 + 前端 listener。原频道（`agent-token` 等）从该 PR 起停止 emit；任何外部插件（如不存在）将停止收到事件。
- `IF2AI_DISABLE_EVOLUTION_EMIT=1` 现有 kill-switch 同时关闭新路径，保留紧急熔断能力。
- 回滚单元 = 单条 PR revert；C-2 / C-3 不依赖彼此运行时状态，可独立回滚。

### 3.5 合并顺序

C-1 → C-2 → C-3。C-1 引入 `runtime_event.rs` helper 与 bridge router 骨架；C-2 / C-3 复用。

## 4. 测试策略（strict TDD）

每条 PR 必须包含：

1. **Round-trip envelope 单测**：构造 emit 输入 → 调用 `dispatch` → 反序列化 envelope → 断言 `event_type / payload_family / correlation / payload` 全部正确。
2. **Run-log 落盘单测**：对临时 `RunEventLogger` 调用 `dispatch`，再读 jsonl 文件，断言含 `eventType` 与 correlation。
3. **前端 router 单测**：构造 envelope → 路由 → mock translator 被调用一次。
4. **回归**：`cargo test -p if2ai` 全绿；`pnpm test` 受影响 suite 全绿。

## 5. 真值锚点

- ARCHITECTURE.md §3.4「chat-runtime 单事实流」、§6.1「runtime_event + Evolution 落盘」、§9.3「Runtime Contract」
- 当前提交：`evolution_emitter::emit_evolution_event` 行为已含 envelope + `append_sync_from_envelope`（PR-B 已落）
- 上下文：上一轮 Q 已扫出「`runtime_event` 同名旁路 = 0 处」结论；本设计目标是把 4 条同义不同名频道折叠到同一频道。

## 6. 风险

| 风险 | 缓解 |
|---|---|
| 前端单条 listener 性能瓶颈 | router 是同步 switch；envelope 频率 ≤ 旧 4 频道之和；O(family-count) 路由与原各 listener 等开销 |
| 前端 translator 期望 raw `StreamTokenPayload`，envelope 包了一层 | router 在调用 translator 前 unwrap `envelope.payload as StreamTokenPayload`；已在 router 单测覆盖 |
| 跨 worktree 改 `runtime-projection-bridge.ts` 冲突 | C-1 引入 router 后，C-2 / C-3 仅 append `FamilyHandler` 条目；机械合并 |
| 回滚 C-2 / C-3 后前端旧 listener 已删 | 回滚 commit 同时还原 `listenTo*` 调用；说明在 plan 的 "Rollback" 段 |

## 7. 后续

C-1 / C-2 / C-3 全部 land 后：
- 删除 `RUNTIME_EVENT_CHANNEL` 之外的 frontend `listenTo*` 内部接口（`src/lib/tauri.ts`）。
- ARCHITECTURE.md §6.1 增补「chat-runtime 单频道生效日期」一句。
