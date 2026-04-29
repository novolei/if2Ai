# UI-002: DaemonHealthDashboard（daemon_health ring buffer 表格展示）

## Status
- State: active

## Goal
新建 `DaemonHealthDashboard` 纯展示组件，消费 `useEvolutionEventSelector(s => s.daemon_health)` 订阅 ring buffer；以表格形式展示每条 `DaemonHealthPayload`（check_name / state / recovery_action / ts）；空状态显示占位文本"No daemon health events"。

## Spec (verifiable)
- `daemon_health` 为空时渲染"No daemon health events"占位 → `node:test::ui_002::empty_state_shows_placeholder`
- `daemon_health` 含 2 条记录时渲染 2 行 → `node:test::ui_002::populated_state_renders_rows`
- 每行含 check_name / state / recovery_action 三个字段 → `node:test::ui_002::row_contains_required_fields`
- `useEvolutionEventSelector` 以 `s => s.daemon_health` 正确订阅 → `node:test::ui_002::selector_subscribes_daemon_health`
- 组件无 IPC import → `node:test::ui_002::no_ipc_import`

## Files (scope — write list)
- `src/components/chat/DaemonHealthDashboard.tsx`     (new — 表格组件)
- `src/components/chat/DaemonHealthDashboard.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`                (useEvolutionEventSelector)
- `src/transport/runtime-event-payloads.ts`           (DaemonHealthPayload interface)

## Contract (review must check)
- selector 必须精确为 `s => s.daemon_health`（不订阅全量 state）
- 不调用任何 IPC 命令（I3）
- 不写入 localStorage / sqlite（I4）
- 不引入新 npm 依赖（使用现有 ds/ 表格组件）
- `npm run lint` PASS（I7）

## Out of Scope
- ❌ 不实现 daemon health 的手动 probe 触发按钮
- ❌ 不改 ring buffer cap（已在 store 固定为 50）
- ❌ 不实现行展开 / 详情 drawer
- ❌ 不在此 Pack 内挂载到 EvolutionDevDrawer（由 UI-001 完成）

## Depends on
- UI-001（EvolutionDevDrawer Tab 插槽 done）
- FEAT-INT-001（DaemonHealthPayload + store done）

## Verify
- `./scripts/pack run UI-002`
- `node --test src/components/chat/DaemonHealthDashboard.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
