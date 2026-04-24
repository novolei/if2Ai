# GAP-003: Frontend Projection Single Truth

## Status
- State: done
- Updated: 2026-04-25

## T-005 Execution Summary

T-003 (MIG-017) already completed the core cutover:
- `activeMessages` passes only user messages to `projectConversationMessagesFromRuns`
- Assistant/tool/thinking/completion are derived from `runtimeProjectionStore`
- `appendMessage` / `updateMessage` are `@deprecated` in conversation-slice

T-005 formalized the remaining GAP-003 contracts:

1. **`conversation-slice.ts`** — Added explicit GAP-003 contract docs: store is a
   "compatibility / display adapter", NOT a source of transcript truth.
   Permitted uses (user anchoring, non-transcript UI state, metadata) and
   prohibited uses (assistant/tool/thinking/completion messages, raw stream
   transcript mutations) are enumerated.

2. **`session-store.ts`** — Added GAP-003 single truth contract: store holds
   only **pointers** (active session ID cursor), NOT transcript truth.

3. **`chat-run-projection.test.ts`** — Added "replaces legacy assistant/tool
   placeholders with projection-derived content" test (GAP-003), verifying
   that `projectConversationMessagesFromRuns` never leaks non-user messages
   through as-is.

### Verification
- `cargo check --manifest-path src-tauri/Cargo.toml` PASS
- `cargo test --manifest-path src-tauri/Cargo.toml --lib -- service_registry` 2/2 PASS
- `npx vitest run src/runtime-projection/chat-run-projection.test.ts` 9/9 PASS

## Goal
把 chat/runtime UI 的最终展示事实从 `conversation-slice + raw listener + projection` 并行态收敛到 projection-first。

## Spec
- assistant text/thinking/tool/completion 从 projection selector 派生 → `chat_messages_derive_from_projection_runs`
- raw stream listener 不直接 mutate final message transcript → `raw_listener_is_transport_only`
- permission prompt 只读 `runtimeProjectionStore.approvals` → `permission_prompt_uses_projection_only`

## Files
- `src/App.tsx`
- `src/stores/**`
- `src/runtime-projection/**`
- `src/modules/chat/**`
- `src/api/**`
- `src/**/*.test.*`

## Reads
- `ARCHITECTURE.md` §3, §7
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`
- `docs/packs/feature/migration-core/MIG-003-runtime-event-projection-truth.md`
- `docs/packs/feature/migration-core/MIG-017-runtime-projection-chat-truth-cutover.md`

## Contract
- 不让新组件直接订阅 raw Tauri runtime events。
- `sessionStore` 只能保存指针，不保存 transcript truth。
- `conversation-slice` 只能作为 compatibility/display adapter。

## Verify
- `./scripts/pack run GAP-003`
- `npm test -- runtime-projection`
- `npm run build`

