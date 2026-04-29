# UI-001: EvolutionDevDrawer 容器（Tab 切换 6 子面板）

## Status
- State: active

## Goal
新建 `EvolutionDevDrawer` 顶层组件，含 6 个 Tab（DaemonHealth / Compression / Skills / SelfEdit / Browser+DK / Checkpoint）；在 `TelemetryDrawer.tsx` 内挂载，默认隐藏（`IF2AI_DEV_EVOLUTION_UI=true` 或 settings devMode 开关控制）。组件本身无 IPC 调用，仅作 Tab 容器。

## Spec (verifiable)
- `EvolutionDevDrawer` 默认不渲染（devMode=false 时返回 null）→ `node:test::ui_001::drawer_hidden_by_default`
- devMode=true 时渲染并展示 6 个 Tab → `node:test::ui_001::drawer_shows_six_tabs`
- `TelemetryDrawer.tsx` 导入并挂载 `EvolutionDevDrawer` → `node:test::ui_001::drawer_mounted_in_telemetry_drawer`
- Tab 切换时 activeTab 状态正确更新 → `node:test::ui_001::tab_switch_updates_state`
- 组件无 IPC import（纯展示）→ `node:test::ui_001::no_ipc_import_in_drawer`

## Files (scope — write list)
- `src/components/chat/EvolutionDevDrawer.tsx`     (new — 容器 + Tab 切换)
- `src/components/chat/TelemetryDrawer.tsx`        (modify — 挂载 EvolutionDevDrawer)
- `src/components/chat/EvolutionDevDrawer.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`             (useEvolutionEventSelector hook)
- `src/state/index.ts`                             (devMode 开关来源)
- `src/components/chat/TelemetryDrawer.tsx`        (现有 Drawer 结构)

## Contract (review must check)
- 不调用任何 IPC 命令（I3）
- 不写入 localStorage / sqlite（I4）
- 不引入新 npm 依赖（使用现有 ds/ 组件）
- `IF2AI_DEV_EVOLUTION_UI` env 或 devMode settings 开关控制可见性
- `npm run lint` PASS（I7）

## Out of Scope
- ❌ 不实现 6 个子面板的真实内容（由 UI-002~006 完成）
- ❌ 不改 TelemetryDrawer 现有非 evolution 内容
- ❌ 不做 Tab 的动画或过渡效果
- ❌ 不接入后端任何新 IPC 命令

## Depends on
- FEAT-INT-001（evolutionEventStore + useEvolutionEventSelector done）

## Verify
- `./scripts/pack run UI-001`
- `node --test src/components/chat/EvolutionDevDrawer.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
