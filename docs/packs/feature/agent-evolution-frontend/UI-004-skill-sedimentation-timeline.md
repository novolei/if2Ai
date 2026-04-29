# UI-004: SkillSedimentationTimeline（skill_sedimented ring buffer 时间轴）

## Status
- State: active

## Goal
新建 `SkillSedimentationTimeline` 纯展示组件，消费 `useEvolutionEventSelector(s => s.skill_sedimented)`；以时间轴形式展示每条 `SkillSedimentedPayload`（skill_name / description / tool_sequence chips / ts）；空状态显示"No skills sedimented yet"；tool_sequence 以 chip 列表形式内联展示。

## Spec (verifiable)
- `skill_sedimented` 为空时展示占位文本 → `node:test::ui_004::empty_state_shows_placeholder`
- 含 2 条时渲染 2 个时间轴条目 → `node:test::ui_004::populated_state_renders_entries`
- 每条目含 skill_name + description + tool_sequence chips → `node:test::ui_004::entry_contains_required_fields`
- tool_sequence 数组展示为独立 chip（每个工具名一个 chip）→ `node:test::ui_004::tool_sequence_renders_as_chips`
- selector 精确订阅 `s.skill_sedimented` → `node:test::ui_004::selector_subscribes_skill_sedimented`

## Files (scope — write list)
- `src/components/chat/SkillSedimentationTimeline.tsx`     (new — 时间轴组件)
- `src/components/chat/SkillSedimentationTimeline.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`                    (useEvolutionEventSelector)
- `src/transport/runtime-event-payloads.ts`               (SkillSedimentedPayload interface)

## Contract (review must check)
- selector 精确为 `s => s.skill_sedimented`
- 不调用任何 IPC 命令（I3）
- 不写入 localStorage / sqlite（I4）
- chip 数量无上限展示（不截断 tool_sequence，ring buffer 已控制条数）
- 不引入新 npm 依赖（I7）

## Out of Scope
- ❌ 不实现 skill 手动推广 / 删除操作
- ❌ 不实现 skill 详情展开 drawer
- ❌ 不改 sedimentation 内部算法
- ❌ 不在此 Pack 挂载到 EvolutionDevDrawer Tab

## Depends on
- UI-001（EvolutionDevDrawer Tab 插槽 done）
- FEAT-INT-001（SkillSedimentedPayload + store done）

## Verify
- `./scripts/pack run UI-004`
- `node --test src/components/chat/SkillSedimentationTimeline.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
