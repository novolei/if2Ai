# FEAT-SB-002: Browser Runtime Projection

## Status

- State: `draft`
- Owner: `@executor`
- Depends On: `FEAT-SB-001`
- Last Updated: `2026-04-25`

## Goal
Move browser status from a UI-only side channel toward the canonical runtime/tool projection so Smart Browser state can be replayed, reviewed, and shown in one timeline.

## Spec
- Browser lifecycle events convert into typed runtime projection events → 测试 `runtime_projection::tests::projects_smart_browser_events`
- Existing `"browser-status"` listener still hydrates BrowserCard during transition → 测试 `src/stores/browser-slice.test.ts`
- Smart Browser projection includes backend, URL, running, takeover, action, diagnostics counts, and escalation state → 测试 `runtime_projection::tests::preserves_browser_projection_fields`

## Files (scope)
- `src/runtime-projection/browser-events.ts` (new)
- `src/runtime-projection/runtime-event-translator.ts`
- `src/runtime-projection/runtime-event-reducer.ts`
- `src/stores/browser-slice.ts`
- `src/components/browser/BrowserCard.tsx`
- `src-tauri/src/modules/browser/events.rs`
- `src-tauri/src/modules/smart_browser/contract.rs`

## Reads
- `docs/design-docs/smart-browser-architecture.md`
- `src/stores/README.md`
- `src/stores/browser-slice.ts`
- `src/components/browser/BrowserCard.tsx`
- `src-tauri/src/modules/browser/events.rs`

## Contract
- BrowserCard remains visible for existing local browser sessions
- Do not remove `"browser-status"` until a later cutover pack
- Runtime projection is additive and replay-safe
- No raw browser-use MCP state may bypass projection

## Out Of Scope
- ❌ 不接入 browser-use MCP
- ❌ 不重设计完整 cockpit UI
- ❌ 不迁移 unrelated chat projection state

## Verify
- ./scripts/pack run FEAT-SB-002
- cargo test --manifest-path src-tauri/Cargo.toml smart_browser
- npm run build

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
