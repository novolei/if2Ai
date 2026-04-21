# GFR-015: Extract top-level surface router + window-bridge from `App.tsx`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-014` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 015 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/App.tsx` 顶层 surface 切换（chat / settings / browser / memory / specialized panels）
- `src/App.tsx` 设置窗口 / browser viewer 窗口管理 + `startWindowDrag`
- active model + TTS / STT 偏好 sync 部分

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 新建 `src/modules/shell-router/ShellRouter.tsx`（内部 router 抽象，不引入 react-router）
- 新建 `src/modules/window-bridge/`：`useSettingsWindow.ts`、`useBrowserViewerWindow.ts`、跨窗口事件名常量注册表
- active model 偏好 sync 迁到 `src/modules/preferences/`（与 GFR-013 共用）
- `App.tsx` 退化为 `<BootShell><ShellRouter /></BootShell>`，目标 ≤ 100 行

---

## Files (scope)

- src/App.tsx

---

## Out of Scope

- ❌ 不引入 react-router 或路由库
- ❌ 不改跨窗口事件名字符串（统一登记为 const，不改值）
- ❌ 不在本 GFR 内 url-deep-link / 浏览器后退

---

## Activation Trigger

1. `GFR-014` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
