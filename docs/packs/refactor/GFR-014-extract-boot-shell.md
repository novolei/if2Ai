# GFR-014: Extract splash + onboarding + activation gate from `App.tsx`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-013` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 014 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/App.tsx` (≈splash + onboarding + activation overlay 切换部分)
-   - `showSplash` / `showOnboarding` 状态 + 3s 超时 fallback
-   - `cross:onboarding-reset` 跨窗口监听
-   - activation gate overlay 嵌入
-   - 复用 `src/boot/BootShell.tsx`（85 行已存在）

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 扩 `src/modules/boot-shell/` 或 `src/boot/`：BootShell 接管 splash + onboarding + activation 三态
- 状态来源切到 `src/runtime-projection` store
- `App.tsx` 中相关代码替换为 `<BootShell>{children}</BootShell>`

---

## Files (scope)

- src/App.tsx
- src/boot/BootShell.tsx

---

## Out of Scope

- ❌ 不改 3s onboarding 超时值
- ❌ 不改 splash 关闭 → projects 装载的时序耦合
- ❌ 不改 `cross:onboarding-reset` 事件名（I3）

---

## Activation Trigger

1. `GFR-013` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
