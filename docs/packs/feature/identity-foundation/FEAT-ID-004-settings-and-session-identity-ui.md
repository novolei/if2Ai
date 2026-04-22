# FEAT-ID-004: Settings And Session Identity UI

## Status

- State: `done`
- Owner: `@executor`
- Depends On: `FEAT-ID-001`, `FEAT-ID-003`
- Last Updated: `2026-04-22`
- Completed Commit: `d902a06`

---

## Goal

在产品 UI 中暴露 identity 配置与切换能力，使用户能够：

1. 在 Settings 页面设置默认 Soul / Persona
2. 在当前 Session 中切换 Persona
3. 在高级选项中切换当前 Session 的 Soul

---

## Why Now

1. 用户明确提出要在设置界面手动更改 Soul / Persona。
2. 如果 identity 只存在于后端，不具备产品可见性与可控性。
3. Session-level persona 切换是 identity feature 的核心用户价值之一。

---

## Spec

1. Settings 页面新增 `Agent Identity` 区块，显示当前默认 Soul / Persona  
   → 测试 `settings_ui::tests::identity_section_renders_defaults`

2. Settings 页面支持修改默认 Soul  
   → 测试 `settings_ui::tests::default_soul_can_be_changed`

3. Settings 页面支持按 Soul 过滤 Persona 列表并修改默认 Persona  
   → 测试 `settings_ui::tests::persona_options_are_filtered_by_selected_soul`

4. Chat / Session header 提供当前 session Persona 切换入口  
   → 测试 `chat_ui::tests::session_persona_switcher_renders`

5. 当前 session Persona 切换后，前端状态刷新并与后端 session identity 同步  
   → 测试 `chat_ui::tests::changing_session_persona_updates_session_state`

6. 当前 session Soul 切换放在高级选项，并有明确风险提示  
   → 测试 `chat_ui::tests::session_soul_switch_requires_advanced_confirmation`

7. 当前 identity 在 UI 上可见，但不造成界面噪音  
   → 测试 `chat_ui::tests::current_identity_badge_is_visible`

---

## Files (scope)

- `src/modules/settings/**`
- `src/components/settings/**`
- `src/modules/chat/**`
- `src/components/**` (session header / badge / selector components as needed)
- `src/state/**` or chat/session store files
- `src/api/identity.ts`
- `src/api/sessions.ts`
- `src/lib/tauri.ts`

---

## Reads

- `docs/design-docs/identity-soul-persona-memory-foundation.md`
- `src/api/**`
- `src/modules/chat/types.ts`
- `src/components/settings/SettingsApp.tsx`

---

## Contract

- Persona 是常规切换入口，Soul 是高级入口
- Persona 下拉列表必须按当前 Soul 过滤
- 修改 global defaults 不应 silently 覆盖当前 session identity
- 修改 current session identity 时必须明确“仅影响当前会话后续回复”
- UI 不得直接持有 hard-coded souls/personas；必须从 API 读取

---

## Implementation Notes

1. 建议新增独立组件：
   - `IdentitySettingsSection`
   - `SessionPersonaPicker`
   - `SessionSoulPicker`
   - `IdentityBadge`
2. Soul 切换交互建议使用二次确认，而 Persona 切换可直接提交。
3. 若现有 Settings 页面结构较重，优先保持视觉与交互一致，不做顺手 UI 重构。

---

## Verify

- `pnpm test` / repo 对应前端测试命令
- 与现有前端测试框架兼容的 identity/settings/chat 组件测试
- 必要时补充手动验证步骤写进 pack 执行记录

---

## Acceptance

- 用户可在 Settings 中改默认 Soul / Persona
- 用户可在 Session 中切 Persona
- 用户可在高级选项中切 Soul
- 当前 identity 有稳定 UI 显示

---

## Out Of Scope

- 不做用户自定义 identity 编辑器
- 不做 project-level defaults
- 不做 persona marketplace
