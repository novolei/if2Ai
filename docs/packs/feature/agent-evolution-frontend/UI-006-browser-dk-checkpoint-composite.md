# UI-006: Browser + DK + Checkpoint + ContentSimplified 复合面板

## Status
- State: active

## Goal
新建 `EvolutionMiscPanel` 组件，整合剩余 4 个 slice：① `BrowserHealthBar` 消费 `s.browser_health`（strategy / session_ok 状态行）；② `DomainKnowledgePanel` 消费 `s.domain_knowledge`（entry 列表：key / excerpt）；③ `CheckpointBar` 消费 `s.checkpoint_updated`（最新 checkpoint ts + 内容摘要）；④ `ContentSimplifiedHistogram` 消费 `s.content_simplified`（kept_chars / dropped_chars 迷你柱状图）。4 个子区域垂直排列，各自独立空状态占位。

## Spec (verifiable)
- `browser_health` 空时 BrowserHealthBar 显示"No browser events" → `node:test::ui_006::browser_health_empty_state`
- `domain_knowledge` 含 2 条时 DomainKnowledgePanel 渲染 2 行 → `node:test::ui_006::dk_panel_populated`
- `checkpoint_updated` 含最新条目时 CheckpointBar 展示 ts + 摘要 → `node:test::ui_006::checkpoint_bar_shows_latest`
- `content_simplified` 含 5 条时 ContentSimplifiedHistogram 渲染 5 个柱 → `node:test::ui_006::histogram_renders_bars`
- 4 个 selector 均精确订阅各自 slice，无全量 state 订阅 → `node:test::ui_006::all_selectors_precise`

## Files (scope — write list)
- `src/components/chat/EvolutionMiscPanel.tsx`     (new — 4 子区域复合组件)
- `src/components/chat/EvolutionMiscPanel.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`             (useEvolutionEventSelector)
- `src/transport/runtime-event-payloads.ts`        (BrowserHealthPayload / DomainKnowledgePayload / CheckpointUpdatedPayload / ContentSimplifiedPayload)

## Contract (review must check)
- 4 个 selector 各自独立（不共用一个全量 selector）
- 不调用任何 IPC 命令（I3）
- 不写入 localStorage / sqlite（I4）
- histogram 使用 CSS / 内联 style，不引入 charting 库（I7）
- `npm run lint` PASS（I7）

## Out of Scope
- ❌ 不实现 domain knowledge 的手动编辑或删除
- ❌ 不实现 checkpoint 的恢复操作（仅展示）
- ❌ 不将 EvolutionMiscPanel 挂载到 EvolutionDevDrawer Tab（由 UI-001 完成）
- ❌ 不引入 recharts / d3 等依赖

## Depends on
- UI-001（EvolutionDevDrawer Tab 插槽 done）
- FEAT-INT-001（4 个 Payload interface + store done）

## Verify
- `./scripts/pack run UI-006`
- `node --test src/components/chat/EvolutionMiscPanel.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
