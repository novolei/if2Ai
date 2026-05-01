# PR C-3: memory_event + memory_after_turn → runtime_event envelope cut-over Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `memory_event` 与 `memory_after_turn` 两条频道收敛到 `runtime_event` envelope；前端 bridge 单线 listen。

**Architecture:** `memory/audit.rs::emit_to_frontend` 与 `application/stream_emitter_service.rs::after_turn` 改用 `runtime_event::dispatch(... RuntimeEventType::Memory, family ∈ {"lifecycle", "after_turn"} ...)`。前端 bridge 增加 Memory family handler 同时处理两个 family，移除 `listenMemoryEvent` 与 `listenMemoryAfterTurn`。

**Tech Stack:** Rust (Tauri 2 / tokio), TypeScript / React, Vitest, cargo test

**Spec:** `docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md`

**Merge order:** **Depends on PR C-1**（uses `runtime/runtime_event.rs::dispatch` and `envelope-router.ts`）。Independent of PR C-2 — can land in either order after C-1.

---

## Pre-flight

确认 main 已含 PR C-1 的 `runtime_event::dispatch` + `envelope-router` + bridge 的 `familyHandlers` 数组。

---

## File Structure

**Modify:**
- `src-tauri/src/modules/memory/audit.rs` — `emit_to_frontend` 改走 envelope。
- `src-tauri/src/modules/application/stream_emitter_service.rs` — `after_turn` 段（lines 84-100）改走 envelope。
- `src/runtime-projection/runtime-projection-bridge.ts` — `familyHandlers` 增加 Memory 两条 family，移除 `listenMemoryEvent` + `listenMemoryAfterTurn`。

**Test:**
- `src-tauri/src/modules/memory/audit.rs` — 新增 emit dispatcher 测试。
- `src-tauri/src/modules/application/stream_emitter_service.rs` — 新增 after_turn envelope 测试。

---

## Task 1: `memory/audit.rs::emit_to_frontend` → envelope

**Files:**
- Modify: `src-tauri/src/modules/memory/audit.rs`

- [ ] **Step 1: Inspect current shape**

Read `src-tauri/src/modules/memory/audit.rs` lines 60-110 to capture: `MemoryEventPayload<'a>` field set, `APP_HANDLE` static usage.

- [ ] **Step 2: Write failing test for envelope serialization**

Add to `audit.rs::tests` module:

```rust
#[test]
fn memory_event_dispatch_emits_envelope_with_lifecycle_family() {
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    std::env::set_var(
        crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
        "1",
    );
    let payload = MemoryEventPayload {
        event: "captured",
        session_id: Some("sess-1"),
        category: Some("preference"),
        recall_category: None,
        result_count: None,
        from_category: None,
        to_category: None,
        extra: None,
        timestamp: "2026-05-01T00:00:00Z".into(),
    };
    let env = crate::modules::runtime::runtime_event::dispatch(
        None,
        RuntimeEventType::Memory,
        "lifecycle",
        CorrelationIds {
            session_id: payload.session_id.map(str::to_owned),
            ..Default::default()
        },
        &payload,
        None,
    )
    .expect("dispatch ok with kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::Memory);
    assert_eq!(env.payload_family.0, "lifecycle");
    std::env::remove_var(
        crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
    );
}
```

- [ ] **Step 3: Run test to verify it fails (then passes — anchor test)**

Run: `cd src-tauri && cargo test -p if2ai-backend memory_event_dispatch --lib`
Expected: PASS once `runtime_event::dispatch` is on `main` (after C-1). This test anchors envelope-shape regression and does NOT depend on us replacing `emit_to_frontend` yet — it only validates that `MemoryEventPayload` serializes round-trip.

- [ ] **Step 4: Replace `emit_to_frontend` body**

Find:

```rust
fn emit_to_frontend(payload: MemoryEventPayload<'_>) {
    let Some(handle) = APP_HANDLE.get() else {
        return;
    };
    if let Err(e) = handle.emit("memory_event", &payload) {
        tracing::warn!(
            "[MemoryAuditEmitter] failed to emit memory_event {event}: {err}",
            event = payload.event,
            err = e
        );
    }
}
```

Replace with:

```rust
fn emit_to_frontend(payload: MemoryEventPayload<'_>) {
    let Some(handle) = APP_HANDLE.get() else {
        return;
    };
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    let correlation = CorrelationIds {
        session_id: payload.session_id.map(str::to_owned),
        ..Default::default()
    };
    if let Err(err) = crate::modules::runtime::runtime_event::dispatch(
        Some(handle),
        RuntimeEventType::Memory,
        "lifecycle",
        correlation,
        &payload,
        None, // memory/audit has no RunEventLogger handle today; envelope is broadcast-only
    ) {
        tracing::warn!(
            event = payload.event,
            error = %err,
            "[MemoryAuditEmitter] runtime_event dispatch failed"
        );
    }
}
```

> Note: `audit.rs` does not currently access `RunEventLogger`; passing `None` is a deliberate scope limitation (broadcast parity only). Run-log integration for memory lifecycle events is a separate slice.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd src-tauri && cargo test -p if2ai-backend memory --lib`
Expected: ALL PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/modules/memory/audit.rs
git commit -m "feat(memory): emit memory lifecycle as runtime_event envelope

emit_to_frontend now dispatches RuntimeEventType::Memory with family
'lifecycle' through runtime_event::dispatch, replacing the legacy
'memory_event' channel."
```

---

## Task 2: `stream_emitter_service.rs::after_turn` → envelope

**Files:**
- Modify: `src-tauri/src/modules/application/stream_emitter_service.rs`

- [ ] **Step 1: Read current shape**

Read `src-tauri/src/modules/application/stream_emitter_service.rs` lines 60-130 to capture: `output` shape, `caller`, `session_id`, `project_id`, `harness_bus` interaction.

- [ ] **Step 2: Write failing test for after_turn envelope**

Add to `stream_emitter_service.rs::tests` (or create the module):

```rust
#[test]
fn memory_after_turn_dispatch_emits_envelope_with_after_turn_family() {
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    std::env::set_var(
        crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
        "1",
    );
    let payload = serde_json::json!({
        "traceVersion": MEMORY_AFTER_TURN_TRACE_VERSION,
        "caller": "test",
        "policyVersion": "v1",
        "decidedAt": "2026-05-01T00:00:00Z",
        "decisions": [],
        "quality": {},
        "conflicts": [],
    });
    let env = crate::modules::runtime::runtime_event::dispatch(
        None,
        RuntimeEventType::Memory,
        "after_turn",
        CorrelationIds {
            session_id: Some("sess-1".into()),
            ..Default::default()
        },
        &payload,
        None,
    )
    .expect("dispatch ok");
    assert_eq!(env.event_type, RuntimeEventType::Memory);
    assert_eq!(env.payload_family.0, "after_turn");
    std::env::remove_var(
        crate::modules::runtime::evolution_emitter::DISABLE_EMIT_ENV,
    );
}
```

- [ ] **Step 3: Run test (anchor — should pass once C-1 is on main)**

Run: `cd src-tauri && cargo test -p if2ai-backend memory_after_turn_dispatch --lib`
Expected: PASS.

- [ ] **Step 4: Replace raw emit**

Find:

```rust
if let Err(err) = app_handle.emit(MEMORY_AFTER_TURN_EVENT, payload) {
    tracing::trace!(
        event = MEMORY_AFTER_TURN_EVENT,
        error = %err,
        "[after_turn] memory_after_turn emit failed (non-fatal)"
    );
}
```

Replace with:

```rust
use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
let correlation = CorrelationIds {
    session_id: Some(session_id.to_string()),
    project_id: project_id.map(|p| p.to_string()),
    ..Default::default()
};
if let Err(err) = crate::modules::runtime::runtime_event::dispatch(
    Some(app_handle),
    RuntimeEventType::Memory,
    "after_turn",
    correlation,
    &payload,
    None, // run-log integration is harness-side; broadcast-only here
) {
    tracing::trace!(
        error = %err,
        "[after_turn] runtime_event dispatch failed (non-fatal)"
    );
}
```

> The harness `EventBus` emit (`AgentEvent::MemoryAfterTurn`) below this block is **kept untouched** — it is a separate truth source per ARCH §6.1 and outside the cut-over scope.

- [ ] **Step 5: Run test**

Run: `cd src-tauri && cargo test -p if2ai-backend stream_emitter_service --lib`
Expected: ALL PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/modules/application/stream_emitter_service.rs
git commit -m "feat(memory): emit memory_after_turn as runtime_event envelope

Replaces app_handle.emit('memory_after_turn', ...) with
runtime_event::dispatch(... RuntimeEventType::Memory, family='after_turn').
The harness EventBus emit on AgentEvent::MemoryAfterTurn is preserved
as a separate governance truth source."
```

---

## Task 3: 前端 bridge — Memory family handler 注册 + 移除两个 legacy listener

**Files:**
- Modify: `src/runtime-projection/runtime-projection-bridge.ts`

- [ ] **Step 1: Add Memory handler entries**

Append to `familyHandlers`:

```ts
    {
      eventType: 'memory',
      family: 'lifecycle',
      handle: (envelope) => {
        const payload = envelope.payload as MemoryEventPayload
        store.dispatch(translateMemoryEventPayload(payload))
      },
    },
    {
      eventType: 'memory',
      family: 'after_turn',
      handle: (envelope) => {
        const payload = envelope.payload as MemoryAfterTurnPayload
        // (1) batch envelope
        store.dispatch(translateMemoryAfterTurn(payload))
        // (2) per-decision fan-out (preserve M3-C closeout behavior)
        for (const decision of payload.decisions) {
          const candidateId = `${payload.policyVersion}::${decision.decidedAt}`
          store.dispatch(translateMemoryWriteDecision(candidateId, decision))
        }
      },
    },
```

Ensure `MemoryEventPayload` and `MemoryAfterTurnPayload` are imported from `@/transport/contracts`; `translateMemoryWriteDecision` is already imported.

- [ ] **Step 2: Remove `listenMemoryEvent` + `listenMemoryAfterTurn` blocks**

Remove the two `track(listenMemoryEvent(...), 'memory_event')` and `track(listenMemoryAfterTurn(...), 'memory_after_turn')` blocks; remove the imports from `@/lib/tauri`.

- [ ] **Step 3: TypeScript + lint**

Run: `pnpm typecheck && pnpm lint --fix src/runtime-projection/runtime-projection-bridge.ts`
Expected: ZERO errors.

- [ ] **Step 4: Run frontend tests**

Run: `pnpm vitest run src/runtime-projection/`
Expected: ALL PASS. Tests asserting `listenMemoryEvent` / `listenMemoryAfterTurn` calls must be updated to use Memory envelope dispatches.

- [ ] **Step 5: Commit**

```bash
git add src/runtime-projection/runtime-projection-bridge.ts
git commit -m "feat(bridge): route Memory envelopes through runtime_event family handlers

Replaces listenMemoryEvent + listenMemoryAfterTurn with two Memory
family handlers (lifecycle / after_turn). Per-decision fan-out for
the rolling write-decision ring is preserved."
```

---

## Task 4: 验证 + 文档锚点

**Files:**
- Modify: `ARCHITECTURE.md` §6.1.

- [ ] **Step 1: Full backend test**

Run: `cd src-tauri && cargo test -p if2ai-backend`
Expected: ALL PASS.

- [ ] **Step 2: Full frontend test**

Run: `pnpm vitest run`
Expected: ALL PASS.

- [ ] **Step 3: Manual smoke**

```
1. pnpm tauri dev, send a turn that triggers memory capture
   (e.g. say "remember that I like dark mode").
2. Observe TelemetryDrawer / memory UI continues to update.
3. Devtools 'runtime_event' should show
   { eventType: 'memory', payloadFamily: 'lifecycle', ... } during
   the turn and { ..., 'after_turn', ... } at turn end (even for
   empty batches — M3-C invariant).
```

- [ ] **Step 4: Update ARCHITECTURE.md**

Append to §6.1:

```markdown
- 2026-05-01：`memory_event` / `memory_after_turn` 收敛到 `runtime_event`
  envelope (`event_type=memory`, family ∈ {`lifecycle`, `after_turn`})；
  harness EventBus 上的 `AgentEvent::MemoryAfterTurn` 保留为治理侧独立
  真值源（PR C-3）。chat-runtime 单频道收敛工作 (C-1/C-2/C-3) 完成。
```

- [ ] **Step 5: Commit + open PR**

```bash
git add ARCHITECTURE.md
git commit -m "docs(arch): record memory cut-over to runtime_event (PR C-3) + chat-runtime single-channel done"
```

Push and open PR titled `[C-3] runtime: memory_event/after_turn → runtime_event envelope cut-over`.

---

## Rollback

`git revert` restores both raw `memory_event` and `memory_after_turn` emits, and the bridge restores both legacy listeners. Harness EventBus path was untouched, no extra rollback needed.

## Verification Summary

- `cargo test -p if2ai-backend` ALL PASS
- `pnpm vitest run` ALL PASS
- Manual: memory chip / TelemetryDrawer continues to update mid-turn and at turn-end (incl. empty batches).
