# GFR-007: Extract zero-hook markdown helpers from `chat-ui.tsx`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-006` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 007 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/components/ui/chat-ui.tsx` (≈2910–3720, 3724–3954)
-   - `MarkdownContent` + `CodeBlock`
-   - ASCII relationship box → markdown 转换
-   - decorative line strip / hash / narrative & keyword 规范化
-   - `MessageCopyButton` + `formatShortTime` + `copyTextToClipboard` + `extractCodeText`
-   - 模块级 `markdownNormalizeCache = new Map<...>` 单例

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 新建 `src/modules/markdown/`：`MarkdownContent.tsx`、`CodeBlock.tsx`、`normalize.ts`、`copy.ts`、`cache.ts`、`index.ts`
- `chat-ui.tsx` 改为 `import { MarkdownContent, CodeBlock, ... } from '@/modules/markdown'`
- **禁止**把模块级 cache 变成 React state（CHARTER I6 禁改语义）

---

## Files (scope)

- src/components/ui/chat-ui.tsx

---

## Out of Scope

- ❌ 不改 markdown 渲染输出（必须 byte-identical）
- ❌ 不改 `VIRTUAL_LIST_THRESHOLD` 等阈值
- ❌ 不在本 GFR 内动 `ChatMessage` 主体

---

## Activation Trigger

1. `GFR-006` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
