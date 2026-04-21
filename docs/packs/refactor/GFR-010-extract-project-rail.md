# GFR-010: Extract `ProjectFilesRail` + merge legacy `ProjectRail.tsx`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-009` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 010 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/components/ui/chat-ui.tsx` (≈1301–1788) `ProjectFilesRail` + 子组件 + autosave 节流 + 树节点渲染
- `src/components/ProjectRail.tsx` (1001 LOC) 整体
- 相关 hook：`useProjectFiles` 等

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 新建 `src/modules/project-rail/`：`ProjectRail.tsx`（合并版）、`tree/`、`preview/`、`hooks/`、`index.ts`
- **合并策略**：以 `chat-ui.tsx` 中的 `ProjectFilesRail` 为主版本；`components/ProjectRail.tsx` 中独有逻辑迁入；重复 UI 删除前列出 diff
- 原 `components/ProjectRail.tsx` 留 deprecated re-export shim

---

## Files (scope)

- src/components/ui/chat-ui.tsx
- src/components/ProjectRail.tsx

---

## Out of Scope

- ❌ 不改 `projectRailWidthV1` 等 localStorage key 名（I4）
- ❌ 不改文件树排序规则
- ⚠️ 合并 2 个 god-file 唯一 GFR；执行前必须列 diff 表

---

## Activation Trigger

1. `GFR-009` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
