# GFR-017: Split `tauri.ts` invoke wrappers per feature

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-016` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 017 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/lib/tauri.ts` 各 feature 段落
-   - 顺序：browser → session → project → skills → settings → harness → memory（每步独立 sub-pack）
-   - 每段 thin invoke wrapper（含 `installSkillFromDistribution` 中的 sha256/解码 业务逻辑）

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 按 feature 切到 `src/transport/{browser,session,project,skills,settings,harness,memory}.ts`
- `installSkillFromDistribution` 中的非 IPC 业务逻辑提到 `src/modules/skills/install/`
- `tauri.ts` 对应段直接删，留 deprecated re-export 段
- 调用方 import 路径 **本 GFR 内不动**（一次只切 wrapper 文件位置）

---

## Files (scope)

- src/lib/tauri.ts

---

## Out of Scope

- ❌ 不改 invoke 命令名字符串（I3）
- ❌ 不在同一 PR 切多个 feature；按 feature 拆 sub-PR
- ❌ 不删 `tauri.ts` 顶部的 `isTauri` 判断（保留至 GFR-018）

---

## Activation Trigger

1. `GFR-016` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
