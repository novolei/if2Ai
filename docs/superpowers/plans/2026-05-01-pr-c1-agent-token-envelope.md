# PR C-1: agent-token → runtime_event envelope cut-over Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `agent-token` 频道完全收敛到 `runtime_event` envelope 频道，引入后端 `runtime_event::dispatch` helper 与前端 `envelope-router`（C-2/C-3 复用）。

**Architecture:** 后端 `AgentStreamEmitter::emit_payload` 调用 `runtime_event::dispatch(... event_type=Conversation/Tool, family=event_type, payload=StreamTokenPayload ...)`；前端 bridge 单条 `listen('runtime_event')` + router 把 `Conversation/Tool` family 解包 `payload as StreamTokenPayload` 喂给现有 `translateAgentTokenPayload`；删除 `listenToAgentTokenStream` 在 bridge 中的调用。

**Tech Stack:** Rust (Tauri 2 / tokio), TypeScript / React, Vitest, cargo test

**Spec:** `docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md`

**Merge order:** **C-1 first**（C-2 / C-3 depend on `runtime_event.rs` helper + envelope router introduced here）.

---

## File Structure

**Create:**
- `src-tauri/src/modules/runtime/runtime_event.rs` — canonical dispatch helper.
- `src/runtime-projection/envelope-router.ts` — family-based router for `runtime_event` listener.
- `src/runtime-projection/__tests__/envelope-router.test.ts` — router unit tests.

**Modify:**
- `src-tauri/src/modules/runtime/mod.rs` — `pub mod runtime_event;`.
- `src-tauri/src/modules/runtime/stream_emitter.rs` — `emit_payload` 改走 envelope；保留 `to_envelope()`.
- `src-tauri/src/modules/application/turn_service/stream_task_run_log.rs` — 移除冗余 `append_with_correlation`（envelope path 已落 run log）。
- `src/runtime-projection/runtime-projection-bridge.ts` — 注入 envelope listener + 移除 `listenToAgentTokenStream`。
- `src/lib/tauri.ts` — 标记 `listenToAgentTokenStream` 为 `@deprecated`（不删除以免破坏现有 `streaming.ts`）。

**Test:**
- `src-tauri/src/modules/runtime/runtime_event.rs` — 内联 `#[cfg(test)] mod tests`.
- `src-tauri/src/modules/runtime/stream_emitter.rs` — 新增 `emit_payload_dispatches_envelope` 测试。
- `src/runtime-projection/__tests__/envelope-router.test.ts` — 路由 + unwrap 行为。

---

## Task 1: 后端 `runtime_event::dispatch` helper

**Files:**
- Create: `src-tauri/src/modules/runtime/runtime_event.rs`
- Modify: `src-tauri/src/modules/runtime/mod.rs`

- [ ] **Step 1: Write failing test for dispatch round-trip**

Create `src-tauri/src/modules/runtime/runtime_event.rs` with the test shell first (no impl):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};

    #[derive(serde::Serialize)]
    struct ChatPayload<'a> {
        event_type: &'a str,
        text: &'a str,
    }

    #[test]
    fn dispatch_returns_envelope_with_event_type_and_family() {
        std::env::set_var(
            crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
            "1",
        );
        let env = dispatch(
            None,
            RuntimeEventType::Conversation,
            "text_delta",
            CorrelationIds {
                run_id: Some("run-1".into()),
                stream_id: Some("s1".into()),
                ..CorrelationIds::default()
            },
            &ChatPayload { event_type: "text_delta", text: "hi" },
            None,
        )
        .expect("dispatch ok with kill-switch");
        assert_eq!(env.event_type, RuntimeEventType::Conversation);
        assert_eq!(env.payload_family.0, "text_delta");
        assert_eq!(env.correlation.run_id.as_deref(), Some("run-1"));
        std::env::remove_var(
            crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test -p if2ai dispatch_returns_envelope --lib`
Expected: FAIL — `dispatch` not defined; module not registered.

- [ ] **Step 3: Implement helper**

In `src-tauri/src/modules/runtime/runtime_event.rs` above the test module, add:

```rust
//! Canonical helper for emitting any runtime event onto the
//! `runtime_event` Tauri channel with run-log mirroring.
//!
//! Thin wrapper over `evolution_emitter::emit_evolution_event` so
//! non-evolution emit sites (chat-runtime: agent-token / permission /
//! memory) can adopt the canonical pipeline without leaking the
//! "evolution" name. See ARCHITECTURE.md §3.4 / §6.1.

use serde::Serialize;
use tauri::AppHandle;

use crate::modules::runtime::contracts::common::{
    CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
};
use crate::modules::runtime::evolution_emitter::{emit_evolution_event, EmitError};
use crate::modules::runtime::event_log::RunEventLogger;

/// Emit one canonical runtime envelope and (best-effort) append it to
/// the run log via `append_sync_from_envelope`.
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

In `src-tauri/src/modules/runtime/mod.rs`, add:

```rust
pub mod runtime_event;
```

(Place alphabetically with other `pub mod` lines.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test -p if2ai dispatch_returns_envelope --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/modules/runtime/runtime_event.rs src-tauri/src/modules/runtime/mod.rs
git commit -m "feat(runtime): add canonical runtime_event::dispatch helper

Thin wrapper over emit_evolution_event so chat-runtime emit sites
(agent-token/permission/memory) can adopt the envelope+run-log
discipline without depending on the 'evolution' name. Spec:
docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md"
```

---

## Task 2: 改造 `AgentStreamEmitter::emit_payload` 走 envelope

**Files:**
- Modify: `src-tauri/src/modules/runtime/stream_emitter.rs`

Background: `StreamTokenPayload::to_envelope()` 已存在；`emit_payload` 当前调 `self.window.emit(AGENT_TOKEN_EVENT, payload)`。我们改为：构造 envelope，emit 到 `RUNTIME_EVENT_CHANNEL`，写 run-log。`AgentStreamEmitter` 当前不持有 `RunEventLogger`，所以本任务只切换 broadcast 频道；run-log 继续由调用方 (`stream_task_run_log::append_stream_event`) 负责（已 PR-B 落）。后续 Task 5 评估能否合并。

- [ ] **Step 1: Write failing test for envelope dispatch**

Add to `#[cfg(test)] mod tests` in `src-tauri/src/modules/runtime/stream_emitter.rs`:

```rust
#[test]
fn emit_payload_serializes_to_envelope_via_to_envelope() {
    let mut p = StreamTokenPayload::skeleton("s7", "text_delta");
    p.text = Some("hello".into());
    p.correlation = Some(CorrelationIds {
        run_id: Some("run-9".into()),
        ..CorrelationIds::default()
    });
    let env = p.to_envelope().expect("text_delta should map to Conversation");
    assert_eq!(env.event_type, RuntimeEventType::Conversation);
    assert_eq!(env.payload_family.0, "text_delta");
    assert_eq!(env.correlation.run_id.as_deref(), Some("run-9"));
    let inner: StreamTokenPayload =
        serde_json::from_value(env.payload).expect("envelope payload is StreamTokenPayload");
    assert_eq!(inner.text.as_deref(), Some("hello"));
    assert_eq!(inner.stream_id, "s7");
}
```

(`RuntimeEventType` / `CorrelationIds` 已在 file 顶端 `use`。)

- [ ] **Step 2: Run test to verify it passes immediately**

Run: `cd src-tauri && cargo test -p if2ai emit_payload_serializes --lib`
Expected: PASS — this asserts existing behavior of `to_envelope()`. **If FAIL, stop and report.**

This is a **regression-anchor** test: it locks the unwrap contract that the frontend router relies on.

- [ ] **Step 3: Replace `emit_payload` body**

In `src-tauri/src/modules/runtime/stream_emitter.rs`, find:

```rust
    pub fn emit_payload(&self, payload: StreamTokenPayload) {
        self.emit_event(AGENT_TOKEN_EVENT, payload);
    }
```

Replace with:

```rust
    pub fn emit_payload(&self, payload: StreamTokenPayload) {
        let Some(envelope) = payload.to_envelope() else {
            tracing::warn!(
                event_type = %payload.event_type,
                "[stream_emitter] StreamTokenPayload event_type unmapped — dropping"
            );
            return;
        };
        if let Err(e) = self.window.emit(
            crate::modules::runtime::evolution_emitter::RUNTIME_EVENT_CHANNEL,
            &envelope,
        ) {
            tracing::trace!(
                channel = crate::modules::runtime::evolution_emitter::RUNTIME_EVENT_CHANNEL,
                error = %e,
                "[stream_emitter] runtime_event emit failed (non-fatal)"
            );
        }
    }
```

- [ ] **Step 4: Run full stream_emitter tests**

Run: `cd src-tauri && cargo test -p if2ai stream_emitter --lib`
Expected: ALL PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/modules/runtime/stream_emitter.rs
git commit -m "feat(stream_emitter): emit AgentStreamEmitter payloads as envelopes

emit_payload now routes through to_envelope() and broadcasts on
RUNTIME_EVENT_CHANNEL instead of legacy 'agent-token'. Run log is
still appended by stream_task_run_log::append_stream_event."
```

---

## Task 3: 前端 envelope router

**Files:**
- Create: `src/runtime-projection/envelope-router.ts`
- Create: `src/runtime-projection/__tests__/envelope-router.test.ts`

- [ ] **Step 1: Write failing test**

Create `src/runtime-projection/__tests__/envelope-router.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest'

import type { RuntimeEventEnvelope } from '@/transport/contracts'
import { makeEnvelopeRouter } from '../envelope-router'

function envelope(eventType: string, family: string): RuntimeEventEnvelope {
  return {
    schemaVersion: { major: 1, minor: 0, patch: 0 } as never,
    eventType: eventType as never,
    payloadFamily: family,
    emittedAt: '2026-05-01T00:00:00Z',
    correlation: {},
    payload: {},
  } as RuntimeEventEnvelope
}

describe('makeEnvelopeRouter', () => {
  it('dispatches to the matching family handler', () => {
    const a = vi.fn()
    const b = vi.fn()
    const router = makeEnvelopeRouter([
      { eventType: 'conversation', family: 'text_delta', handle: a },
      { eventType: 'permission', handle: b },
    ])
    router(envelope('conversation', 'text_delta'))
    router(envelope('permission', 'prompt_opened'))
    expect(a).toHaveBeenCalledTimes(1)
    expect(b).toHaveBeenCalledTimes(1)
  })

  it('skips unmatched envelopes silently', () => {
    const a = vi.fn()
    const router = makeEnvelopeRouter([
      { eventType: 'conversation', family: 'text_delta', handle: a },
    ])
    router(envelope('conversation', 'thinking_delta'))
    router(envelope('memory', 'lifecycle'))
    expect(a).not.toHaveBeenCalled()
  })

  it('matches on family list', () => {
    const a = vi.fn()
    const router = makeEnvelopeRouter([
      {
        eventType: 'conversation',
        family: ['text_delta', 'thinking_delta'],
        handle: a,
      },
    ])
    router(envelope('conversation', 'text_delta'))
    router(envelope('conversation', 'thinking_delta'))
    expect(a).toHaveBeenCalledTimes(2)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/runtime-projection/__tests__/envelope-router.test.ts`
Expected: FAIL — `envelope-router` module not found.

- [ ] **Step 3: Create router implementation**

Create `src/runtime-projection/envelope-router.ts`:

```ts
// Family-based router for the canonical `runtime_event` Tauri
// channel. Used by `runtime-projection-bridge` to dispatch one
// envelope to the right translator. See:
// docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md

import type { RuntimeEventEnvelope } from '@/transport/contracts'

export type EnvelopeRoute = (envelope: RuntimeEventEnvelope) => void

export interface FamilyHandler {
  /** Match against `envelope.eventType` (snake_case from the Rust enum). */
  eventType: string
  /**
   * Optional `payload_family` narrowing. When omitted, the handler
   * matches any family for the given `eventType`.
   */
  family?: string | string[]
  handle: EnvelopeRoute
}

export function makeEnvelopeRouter(handlers: FamilyHandler[]): EnvelopeRoute {
  return (envelope) => {
    for (const h of handlers) {
      if (h.eventType !== envelope.eventType) continue
      if (h.family) {
        const list = Array.isArray(h.family) ? h.family : [h.family]
        if (!list.includes(envelope.payloadFamily as string)) continue
      }
      h.handle(envelope)
      return
    }
  }
}
```

If `RuntimeEventEnvelope` is not yet exported from `@/transport/contracts`, locate the existing import path used by `runtime-event-translator.ts` (likely already exported); if missing, add `export type { RuntimeEventEnvelope } from './runtime-event-payloads'` or equivalent — confirm the canonical type before adding a new one.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/runtime-projection/__tests__/envelope-router.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/runtime-projection/envelope-router.ts src/runtime-projection/__tests__/envelope-router.test.ts
git commit -m "feat(runtime-projection): add envelope router for runtime_event channel

Family-based dispatcher used by runtime-projection-bridge to route
one runtime_event envelope to the matching translator. Reused by
PR C-2 (permission) and C-3 (memory)."
```

---

## Task 4: bridge cut-over — 移除 `listenToAgentTokenStream`，单线 listen runtime_event

**Files:**
- Modify: `src/runtime-projection/runtime-projection-bridge.ts`

- [ ] **Step 1: Inspect current bridge wiring**

Read `src/runtime-projection/runtime-projection-bridge.ts` lines 51-185 to confirm: import list, `track(listenToAgentTokenStream(...))` block (≈ lines 124-131), order with other listeners.

- [ ] **Step 2: Add envelope listener + remove agent-token listener**

In `src/runtime-projection/runtime-projection-bridge.ts`:

1. Add imports:

```ts
import { listen } from '@tauri-apps/api/event'
import { RUNTIME_EVENT_CHANNEL } from '@/transport/contracts'
import type { RuntimeEventEnvelope, StreamTokenPayload } from '@/transport/contracts'
import { makeEnvelopeRouter, type FamilyHandler } from './envelope-router'
```

(Verify `RUNTIME_EVENT_CHANNEL` is exported from `@/transport/contracts`; if not, add `export const RUNTIME_EVENT_CHANNEL = 'runtime_event'`.)

2. Remove from imports: `listenToAgentTokenStream`.

3. Replace the `track(listenToAgentTokenStream(...), 'agent-token')` block (≈ lines 124-131) with the runtime_event listener block:

```ts
  const familyHandlers: FamilyHandler[] = [
    {
      eventType: 'conversation',
      handle: (envelope) => {
        const payload = envelope.payload as StreamTokenPayload
        const event = translateAgentTokenPayload(payload)
        if (event) store.dispatch(event)
      },
    },
    {
      eventType: 'tool',
      handle: (envelope) => {
        const payload = envelope.payload as StreamTokenPayload
        const event = translateAgentTokenPayload(payload)
        if (event) store.dispatch(event)
      },
    },
  ]
  const router = makeEnvelopeRouter(familyHandlers)
  track(
    listen<RuntimeEventEnvelope>(RUNTIME_EVENT_CHANNEL, (event) => {
      router(event.payload)
    }),
    'runtime_event',
  )
```

(Tauri `listen` returns `Promise<UnlistenFn>`; the existing `track` helper accepts that shape.)

- [ ] **Step 3: TypeScript check + lint**

Run: `pnpm typecheck` (or `pnpm tsc --noEmit`) and `pnpm lint --fix src/runtime-projection/runtime-projection-bridge.ts`
Expected: ZERO errors. Fix any contract drift before proceeding.

- [ ] **Step 4: Run frontend tests**

Run: `pnpm vitest run src/runtime-projection/`
Expected: ALL PASS. Pay special attention to any bridge-level integration tests; if a test specifically asserts `listenToAgentTokenStream` was called, update it to assert against `listen('runtime_event', ...)` instead.

- [ ] **Step 5: Commit**

```bash
git add src/runtime-projection/runtime-projection-bridge.ts
git commit -m "feat(bridge): cut-over agent-token to runtime_event envelope

Bridge now subscribes to runtime_event once and routes Conversation/
Tool families through the existing translateAgentTokenPayload. The
legacy listenToAgentTokenStream call is removed from the bridge
(streaming.ts continues to expose it for components that haven't
migrated yet)."
```

---

## Task 5: 验证端到端 + 文档锚点

**Files:**
- Modify: `ARCHITECTURE.md` §6.1 — 增补一行说明 chat-runtime 单频道生效。

- [ ] **Step 1: Full backend test**

Run: `cd src-tauri && cargo test -p if2ai`
Expected: ALL PASS.

- [ ] **Step 2: Full frontend test**

Run: `pnpm vitest run` (or per-suite if monorepo)
Expected: ALL PASS.

- [ ] **Step 3: Manual smoke test (text)**

Document expected manual verification (do not execute in CI):

```
1. pnpm tauri dev
2. Open chat, send "hello", observe text streamed token-by-token in UI.
3. Open devtools, listen for `runtime_event` events: should see
   `eventType: "conversation"` with `payloadFamily: "text_delta"`.
4. Confirm session run log under app-data dir contains JSONL entries
   with `event_type: "conversation"` for the turn.
```

- [ ] **Step 4: Update ARCHITECTURE.md**

In `ARCHITECTURE.md` §6.1, locate the paragraph about `runtime_event` + Evolution 落盘 (added in PR-B). Append:

```markdown
- 2026-05-01：`agent-token` 频道由 `AgentStreamEmitter::emit_payload` 收敛到
  `runtime_event`（PR C-1）；前端 `runtime-projection-bridge.ts` 改为单条
  `listen('runtime_event')` + family router。`runtime/runtime_event.rs::dispatch`
  作为非-evolution 路径的 canonical helper。
```

- [ ] **Step 5: Commit + open PR**

```bash
git add ARCHITECTURE.md
git commit -m "docs(arch): record agent-token cut-over to runtime_event (PR C-1)"
```

Push branch and open PR titled `[C-1] runtime: agent-token → runtime_event envelope cut-over`.

---

## Rollback

`git revert` of the 5 commits restores both backend `agent-token` emit and frontend `listenToAgentTokenStream` listener. The `runtime_event::dispatch` helper and `envelope-router.ts` survive (no callers); they can stay.

## Verification Summary

- `cargo test -p if2ai` ALL PASS
- `pnpm vitest run` ALL PASS
- `pnpm typecheck` ZERO errors
- Manual smoke: text/thinking deltas render; tool calls render; `runtime_event` envelopes visible in devtools.
