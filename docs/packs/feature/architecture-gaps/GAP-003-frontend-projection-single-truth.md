# GAP-003: Frontend Projection Single Truth

## Status
- State: draft

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

