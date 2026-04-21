# GFR-013: Shrink `chat-ui.tsx` to layout shell only

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-012` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 013 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/components/ui/chat-ui.tsx` 残余主组件 `ChatUI` 体（≈222–1300 + 4287+）
-   - 100 个 hook 调用：state / refs / effects / callbacks
-   - `EmptyState` / `LoadingIndicator` / `RecoveryCard` / `ErrorCard`
-   - 模型 / 字体 / 密度 偏好相关 storage 读写

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 把 stream / memory / permission / recovery 订阅迁到 `runtime-projection` store + selector
- 新建 `src/modules/chat/shell/ChatShell.tsx`（layout 容器）
- `EmptyState` / `LoadingIndicator` / `RecoveryCard` / `ErrorCard` 迁到 `src/modules/system-feedback/`
- 字体 / 密度 / 模型偏好迁到 `src/modules/preferences/`（保留原 localStorage key 名）
- 原 `src/components/ui/chat-ui.tsx` 退化为 `<ChatShell />` 单包装（≤ 100 行）

---

## Files (scope)

- src/components/ui/chat-ui.tsx

---

## Out of Scope

- ❌ 不改 100 个 hook 的执行顺序（必须严格按 React rules of hooks）
- ❌ 不改 `chatDensityModeV2` / `chatFontModeV2` 等 storage key 名
- ❌ 不引入 react-router 或新 state library
- ⚠️ 这是 chat-ui 系列最危险的 GFR；执行前 GFR-007~012 必须全 done

---

## Activation Trigger

1. `GFR-012` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
