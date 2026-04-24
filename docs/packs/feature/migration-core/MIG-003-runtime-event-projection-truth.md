# MIG-003 Runtime Event Projection Truth

## Status

- State: `active`
- Task Ref: T-002 (docs/vnext_new/task.md §2)
- Spec Ref: §7.1 MIG-003 (docs/vnext_new/spec.md)
- Design Ref: §3.2 Step 1 (docs/vnext_new/design.md)
- Depends On: T-001 (GAP-002 done)
- Last Updated: `2026-04-24`

---

## Goal

确保 runtime-projection-bridge 是唯一 runtime event ingestion 入口；translator 扩展支持 T-001 新增的 correlation 字段；前端不再新增直接消费 raw agent-token 的代码路径。

## Spec (verifiable)

- runtime-projection-bridge 确认为唯一 runtime event ingestion 入口 → 测试 `bridge_is_only_ingestion_entry`
- translator 支持 correlation 字段传递 → 测试 `translator_preserves_correlation_from_payload`
- 前端不新增直接消费 raw agent-token 的代码路径 → `npm run build` 通过 + grep 无新增 `listen.*agent-token`
- stream_run_bound event 可从 payload correlation.runId 派生 → 测试 `run_bound_event_uses_correlation_run_id`

## Files (scope — write list)

- `src/runtime-projection/runtime-event-translator.ts` (modify — 扩展 translator 支持 correlation)
- `src/runtime-projection/runtime-projection-bridge.ts` (modify — 确认唯一入口 + run_bound 从 correlation 派生)
- `src/runtime-projection/types.ts` (modify — event variant 扩展 correlation)
- `src/runtime-projection/runtime-projection-store.ts` (modify — 如需)

## Reads (read-only inputs)

- docs/vnext_new/spec.md §7.1 MIG-003
- docs/vnext_new/design.md §3.2 Step 1
- docs/design-docs/if2ai-vnext-session-runtime-blueprint.md
- src/lib/tauri.ts — listenToStream / listenToAgentTokenStream
- src/api/conversations.ts — ChatStreamHandle

## Contract (review must check)

### vNext 通用约束
- [ ] 不新增直接消费 raw Tauri event 的 UI surface（No New Raw Consumer）
- [ ] 无 unwrap() / expect() / todo!() 在非测试代码
- [ ] 跨模块用 crate::modules::*

### 本 Pack 特有约束
- runtime-projection-bridge 是唯一将 backend wire payload 转为 CanonicalRuntimeEvent 的地方
- translator 中 runId 优先从 payload.correlation.runId 读取，fallback 到 payload.stream_id
- ChatStreamHandle.subscribe 只用于 transport side-effect（TTS/Todo/resume），不派生展示内容
- 不引入新的二次真相

## Out of Scope
- ❌ 不做 Chat Truth Cutover（属于 T-003 MIG-017）
- ❌ 不做 projection single truth 完整收口（属于 T-005 GAP-003）
- ❌ 不改后端 Rust 代码
- ❌ 不顺手重构 chat-ui.tsx

## Verify
- `npm run build`
- `npm test -- runtime-projection`
- vNext 专项门：
  - [ ] V2 Single Truth: `grep -r "listen.*agent-token" src/modules/ | grep -v node_modules | grep -v ".test."`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
