# PR C-2: permission-request → runtime_event envelope cut-over Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `permission-request` 频道收敛到 `runtime_event` envelope；前端 bridge 单线 listen。

**Architecture:** `permission_service` 用 `runtime_event::dispatch(handle, RuntimeEventType::Permission, "prompt_opened", correlation, &payload, Some(logger))` 替换原 `self.window.emit("permission-request", ...)` 与旁路 `event_logger.append_sync("permission_requested", ...)`。前端 bridge 增加 Permission family handler，移除 `listenToPermissionRequests`。

**Tech Stack:** Rust (Tauri 2 / tokio), TypeScript / React, Vitest, cargo test

**Spec:** `docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md`

**Merge order:** **Depends on PR C-1**（uses `runtime/runtime_event.rs::dispatch` and `envelope-router.ts`）。

---

## Pre-flight

确认 main 分支已含 PR C-1 引入的：
- `src-tauri/src/modules/runtime/runtime_event.rs::dispatch`
- `src/runtime-projection/envelope-router.ts::makeEnvelopeRouter`
- `runtime-projection-bridge.ts` 中的 `listen(RUNTIME_EVENT_CHANNEL)` + `familyHandlers` 数组

否则必须先 rebase 或等待。

---

## File Structure

**Modify:**
- `src-tauri/src/modules/application/permission_service.rs` — 替换原始 emit + 旁路 log。
- `src/runtime-projection/runtime-projection-bridge.ts` — `familyHandlers` 增加 Permission 条目，移除 `listenToPermissionRequests`。

**Test:**
- `src-tauri/src/modules/application/permission_service.rs` — 新增 emit dispatcher 测试（用 mock logger 验证 envelope + run-log 落盘）。

---

## Task 1: 后端 — 将 permission-request emit 切到 envelope

**Files:**
- Modify: `src-tauri/src/modules/application/permission_service.rs`

- [ ] **Step 1: Read current emit site**

Read `src-tauri/src/modules/application/permission_service.rs` lines 95-145 to capture: payload field shape, `event_logger` access, surrounding context (request_id, session_id, tool_name, etc.).

- [ ] **Step 2: Write failing test for envelope dispatch**

Add to `permission_service.rs` `#[cfg(test)] mod tests` (create the test module if missing). The test must:
1. Build a `RunEventLogger` over a `tempfile::TempDir`.
2. Call the (new) helper `emit_permission_prompt(...)` (extracted in Step 4) with a fake `AppHandle: None` and the logger.
3. Read back the JSONL file, deserialize the entry, assert `event_type == "permission"` and that the payload contains `request_id`, `tool_name`, `permission_mode`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::event_log::RunEventLogger;
    use tempfile::tempdir;

    #[test]
    fn permission_prompt_dispatch_persists_envelope_to_run_log() {
        let dir = tempdir().expect("tempdir");
        let logger = RunEventLogger::new_for_test(dir.path()).expect("logger");
        let entry = emit_permission_prompt_for_test(
            None,
            "req-1",
            "sess-1",
            "browser_navigate",
            "always",
            "session",
            "Navigate to https://example.com?",
            Some(&logger),
        )
        .expect("dispatch ok");
        assert_eq!(entry.event_type, RuntimeEventType::Permission);
        assert_eq!(entry.payload_family.0, "prompt_opened");
        let on_disk = logger.read_test_jsonl().expect("jsonl");
        assert!(on_disk.contains("\"event_type\":\"permission\""));
        assert!(on_disk.contains("\"request_id\":\"req-1\""));
    }
}
```

> If `RunEventLogger::new_for_test` / `read_test_jsonl` do not yet exist, locate the test helpers used in `event_log.rs::tests` (e.g. `append_sync_from_envelope_persists_full_correlation_to_jsonl`) and reuse the same construction style. Do not invent new public API in this PR — copy the in-test construction inline if needed.

- [ ] **Step 3: Run test to verify it fails**

Run: `cd src-tauri && cargo test -p if2ai-backend permission_prompt_dispatch --lib`
Expected: FAIL — `emit_permission_prompt_for_test` not defined.

- [ ] **Step 4: Replace raw emit with `runtime_event::dispatch`**

In `permission_service.rs`, locate the block:

```rust
let _ = self.window.emit(
    "permission-request",
    serde_json::json!({
        "request_id": request_id,
        "session_id": self.session_id,
        "tool_name": request.tool_name,
        "permission_mode": request.required_mode.as_str(),
        "current_mode": request.current_mode.as_str(),
        "message": message,
    }),
);
if let Some(event_logger) = &self.event_logger {
    let _ = event_logger.append_sync(
        "permission_requested",
        serde_json::json!({ /* ... */ }),
    );
}
```

Replace with:

```rust
let payload = serde_json::json!({
    "request_id": request_id,
    "session_id": self.session_id,
    "tool_name": request.tool_name,
    "permission_mode": request.required_mode.as_str(),
    "current_mode": request.current_mode.as_str(),
    "message": message,
});
let correlation = crate::modules::runtime::contracts::common::CorrelationIds {
    session_id: Some(self.session_id.clone()),
    ..Default::default()
};
if let Err(err) = crate::modules::runtime::runtime_event::dispatch(
    self.app_handle.as_ref(),
    crate::modules::runtime::contracts::common::RuntimeEventType::Permission,
    "prompt_opened",
    correlation,
    &payload,
    self.event_logger.as_deref(),
) {
    tracing::warn!(
        request_id = %request_id,
        tool = %request.tool_name,
        error = %err,
        "[permission] envelope dispatch failed"
    );
}
```

**Verify** that `self.app_handle: Option<AppHandle>` exists on `PermissionService`. If only `self.window: WebviewWindow` is available, use `self.window.app_handle()` (Tauri 2 returns `&AppHandle`). Adjust the `Option<&AppHandle>` parameter accordingly.

Then extract the test-friendly seam:

```rust
#[cfg(test)]
fn emit_permission_prompt_for_test(
    handle: Option<&tauri::AppHandle>,
    request_id: &str,
    session_id: &str,
    tool_name: &str,
    permission_mode: &str,
    current_mode: &str,
    message: &str,
    logger: Option<&crate::modules::runtime::event_log::RunEventLogger>,
) -> Result<crate::modules::runtime::contracts::common::RuntimeEventEnvelope, crate::modules::runtime::evolution_emitter::EmitError> {
    let payload = serde_json::json!({
        "request_id": request_id,
        "session_id": session_id,
        "tool_name": tool_name,
        "permission_mode": permission_mode,
        "current_mode": current_mode,
        "message": message,
    });
    let correlation = crate::modules::runtime::contracts::common::CorrelationIds {
        session_id: Some(session_id.into()),
        ..Default::default()
    };
    crate::modules::runtime::runtime_event::dispatch(
        handle,
        crate::modules::runtime::contracts::common::RuntimeEventType::Permission,
        "prompt_opened",
        correlation,
        &payload,
        logger,
    )
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd src-tauri && cargo test -p if2ai-backend permission_prompt_dispatch --lib`
Expected: PASS.

- [ ] **Step 6: Run full permission_service tests**

Run: `cd src-tauri && cargo test -p if2ai-backend permission --lib`
Expected: ALL PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/modules/application/permission_service.rs
git commit -m "feat(permission): emit permission-prompt as runtime_event envelope

permission_service now dispatches RuntimeEventType::Permission with
family 'prompt_opened' through runtime_event::dispatch, replacing
both the raw 'permission-request' channel emit and the side-band
event_logger.append_sync('permission_requested', ...) write."
```

---

## Task 2: 前端 bridge — 注册 Permission family handler，移除 `listenToPermissionRequests`

**Files:**
- Modify: `src/runtime-projection/runtime-projection-bridge.ts`

- [ ] **Step 1: Read existing bridge layout**

After PR C-1 lands, the bridge contains a `familyHandlers: FamilyHandler[]` array (introduced in C-1 Task 4). Locate it.

- [ ] **Step 2: Add Permission handler entry**

Append to `familyHandlers` (alphabetical or after existing Conversation/Tool entries):

```ts
    {
      eventType: 'permission',
      family: 'prompt_opened',
      handle: (envelope) => {
        const payload = envelope.payload as PermissionRequestPayload
        store.dispatch(translatePermissionRequestPayload(payload))
      },
    },
```

Add `PermissionRequestPayload` to the import from `@/transport/contracts` if missing.

- [ ] **Step 3: Remove `listenToPermissionRequests` block**

Remove the block:

```ts
  track(
    listenToPermissionRequests((payload) => {
      store.dispatch(translatePermissionRequestPayload(payload))
    }),
    'permission-request',
  )
```

Remove `listenToPermissionRequests` from the `@/lib/tauri` import.

- [ ] **Step 4: TypeScript check + lint**

Run: `pnpm typecheck && pnpm lint --fix src/runtime-projection/runtime-projection-bridge.ts`
Expected: ZERO errors.

- [ ] **Step 5: Run frontend tests**

Run: `pnpm vitest run src/runtime-projection/`
Expected: ALL PASS. If a test asserts `listenToPermissionRequests` was called, update it to assert the Permission family handler is invoked given a Permission envelope.

- [ ] **Step 6: Commit**

```bash
git add src/runtime-projection/runtime-projection-bridge.ts
git commit -m "feat(bridge): route Permission envelopes through runtime_event family handler

Removes the dedicated permission-request listener; the bridge now
unwraps RuntimeEventType::Permission envelopes from the canonical
runtime_event channel."
```

---

## Task 3: 验证 + 文档锚点

**Files:**
- Modify: `ARCHITECTURE.md` §6.1.

- [ ] **Step 1: Full backend test**

Run: `cd src-tauri && cargo test -p if2ai-backend`
Expected: ALL PASS.

- [ ] **Step 2: Full frontend test**

Run: `pnpm vitest run`
Expected: ALL PASS.

- [ ] **Step 3: Manual smoke**

Document:

```
1. pnpm tauri dev
2. Trigger any tool requiring permission (e.g. browser_navigate to
   a new origin in non-yolo mode).
3. Confirm permission dialog still appears with the same message.
4. Devtools listen on 'runtime_event' should show
   { eventType: 'permission', payloadFamily: 'prompt_opened', ... }.
5. Run log JSONL contains an entry with `event_type: "permission"`
   for the request.
```

- [ ] **Step 4: Update ARCHITECTURE.md**

Append to §6.1 (after the C-1 line):

```markdown
- 2026-05-01：`permission-request` 频道在 `permission_service` 内收敛到
  `runtime_event` envelope (`event_type=permission`, `family=prompt_opened`)；
  前端 bridge `familyHandlers` 增加 Permission 条目（PR C-2）。
```

- [ ] **Step 5: Commit + open PR**

```bash
git add ARCHITECTURE.md
git commit -m "docs(arch): record permission-request cut-over to runtime_event (PR C-2)"
```

Push and open PR titled `[C-2] runtime: permission-request → runtime_event envelope cut-over`.

---

## Rollback

`git revert` restores raw `permission-request` channel emit and the side-band `event_logger.append_sync` write; bridge restores `listenToPermissionRequests`. No data migration concerns (run log keeps both shapes).

## Verification Summary

- `cargo test -p if2ai-backend` ALL PASS
- `pnpm vitest run` ALL PASS
- Manual: permission dialog flow unchanged; `runtime_event` shows Permission envelope; run log JSONL contains the entry.
