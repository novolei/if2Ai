# GFR-018: Migrate `tauri.ts` listeners to `runtime-projection/translator/`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-017` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 018 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/lib/tauri.ts` 所有 `listen*` helper
-   - `listenMemoryEvent` / `listenToStream` / `listenToPermissionRequests` / `listenBrowserStatus` / `listenToChatPrefill`

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 迁到 `src/runtime-projection/translator/`（M2 已有部分）
- 建立事件名 const 注册表，禁止再出现裸字符串
- `tauri.ts` 退化为 ≤ 100 行 deprecated shim
- `isTauri` 判断收敛到 `src/transport/runtime.ts` 单点

---

## Files (scope)

- src/lib/tauri.ts

---

## Out of Scope

- ❌ 不改任何事件名字符串（I3）
- ❌ 不改裸事件 → envelope 转换语义（必须保留双轨）
- ⚠️ 必须 GFR-016 + GFR-017 全 done 之后才能动；listener 迁移即"运行时真相切换"

---

## Activation Trigger

1. `GFR-017` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
