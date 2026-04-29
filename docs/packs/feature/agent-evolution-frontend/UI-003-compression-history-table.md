# UI-003: CompressionHistoryTable（compression_event 时间轴展示）

## Status
- State: active

## Goal
新建 `CompressionHistoryTable` 纯展示组件，消费 `useEvolutionEventSelector(s => s.compression_event)`；以时间轴列表形式展示每条 `CompressionEventPayload`（ts / kept_tokens / dropped_tokens / passthrough / tier）；空状态显示"No compression events"；同时在 `ContextBar.tsx` 内嵌入最新一条压缩摘要（kept/dropped 数字行内展示）。

## Spec (verifiable)
- `compression_event` 为空时展示占位文本 → `node:test::ui_003::empty_state_shows_placeholder`
- 含 3 条时渲染 3 行，按时间戳倒序 → `node:test::ui_003::populated_state_renders_rows_desc`
- 每行含 kept_tokens + dropped_tokens + tier 字段 → `node:test::ui_003::row_contains_required_fields`
- `ContextBar.tsx` 嵌入最新一条压缩摘要（取 ring buffer 最后一项）→ `node:test::ui_003::context_bar_shows_latest_compression`
- selector 精确订阅 `s.compression_event` → `node:test::ui_003::selector_subscribes_compression_event`

## Files (scope — write list)
- `src/components/chat/CompressionHistoryTable.tsx`     (new — 时间轴列表)
- `src/components/chat/ContextBar.tsx`                  (modify — 嵌入最新摘要行)
- `src/components/chat/CompressionHistoryTable.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`                  (useEvolutionEventSelector)
- `src/transport/runtime-event-payloads.ts`             (CompressionEventPayload interface)
- `src/components/chat/ContextBar.tsx`                  (现有 Token 预算 UI 结构)

## Contract (review must check)
- selector 精确为 `s => s.compression_event`
- ContextBar 修改仅追加摘要行，不移动现有 Token 预算显示元素（I1 类比）
- 不调用任何 IPC 命令（I3）
- 不引入新 npm 依赖（I7）

## Out of Scope
- ❌ 不实现压缩触发按钮或手动重压缩
- ❌ 不修改 TierBudget 展示逻辑（TE-001）
- ❌ 不改 ring buffer cap
- ❌ 不在此 Pack 挂载到 EvolutionDevDrawer Tab

## Depends on
- UI-001（EvolutionDevDrawer Tab 插槽 done）
- FEAT-INT-001（CompressionEventPayload + store done）

## Verify
- `./scripts/pack run UI-003`
- `node --test src/components/chat/CompressionHistoryTable.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
