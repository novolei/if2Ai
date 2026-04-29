# UI-005: SelfEditProposalList + VerificationVerdictList（2-Tab 面板）

## Status
- State: active

## Goal
新建 `SelfEditPanel` 组件，包含两个内部 Tab：① `SelfEditProposalList` 消费 `s.self_edit_proposal`，展示每条 `SelfEditProposalPayload`（proposal_id / description / utility_score / origin_failure）；② `VerificationVerdictList` 消费 `s.verification_decision`，展示每条 `VerificationDecisionPayload`（proposal_id / decision / reason / gate）。两列表均支持空状态占位。

## Spec (verifiable)
- `self_edit_proposal` 为空时 Tab ① 展示"No proposals"占位 → `node:test::ui_005::proposal_list_empty_state`
- `self_edit_proposal` 含 3 条时渲染 3 行，含 proposal_id + description + utility_score → `node:test::ui_005::proposal_list_populated`
- `verification_decision` 含 2 条时 Tab ② 渲染 2 行，含 decision + gate 字段 → `node:test::ui_005::verdict_list_populated`
- Tab ① selector 精确为 `s => s.self_edit_proposal` → `node:test::ui_005::proposal_selector_correct`
- Tab ② selector 精确为 `s => s.verification_decision` → `node:test::ui_005::verdict_selector_correct`

## Files (scope — write list)
- `src/components/chat/SelfEditPanel.tsx`     (new — 2-Tab 容器 + 两子组件)
- `src/components/chat/SelfEditPanel.test.ts` (new — 5 node:test cases)

## Reads (read-only)
- `src/state/evolution-event-store.ts`        (useEvolutionEventSelector)
- `src/transport/runtime-event-payloads.ts`   (SelfEditProposalPayload + VerificationDecisionPayload)

## Contract (review must check)
- 两个 selector 必须分开订阅（不订阅全量 state）
- 不调用任何 IPC 命令（I3）
- 不写入 localStorage / sqlite（I4）
- RunInspectorPanel.tsx 内的 SelfEdit tab 由此组件提供（非内联代码）
- 不引入新 npm 依赖（I7）

## Out of Scope
- ❌ 不实现 proposal 手动接受 / 拒绝操作
- ❌ 不改 verification gate 逻辑（FEAT-AE-002 只读）
- ❌ 不实现 proposal 详情展开 drawer
- ❌ 不在此 Pack 将 SelfEditPanel 挂载到 RunInspectorPanel（仅创建组件）

## Depends on
- UI-001（EvolutionDevDrawer Tab 插槽 done）
- FEAT-INT-001（SelfEditProposalPayload + VerificationDecisionPayload + store done）

## Verify
- `./scripts/pack run UI-005`
- `node --test src/components/chat/SelfEditPanel.test.ts`
- `npm run lint`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
