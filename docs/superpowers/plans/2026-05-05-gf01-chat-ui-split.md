# GF-01 — Split `chat-ui.tsx` (5,043 LOC) into Focused Child Modules

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §4 GF-01 + §1 DT-03
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)
> **预计**：3 周，9 PR（含末次 DT-03 cutover）
> **依赖**：无强阻塞；末次 PR 软依赖 ER-02（raw `invoke('model_list_available')` / `listen('if2ai://models-changed')` 收到 `useAvailableModels()`）。若 ER-02 未先行，本计划在 PR-08 内联完成同样迁移。

---

## 1. 总览

`src/components/ui/chat-ui.tsx` 当前 5,043 行，承载整张 Chat 视图：transcript 渲染（含 virtual list）+ composer（含 slash / @-file 补全 + 拖拽 chip）+ Pickers（model / branch / permission mode）+ Chips（memory / turn-cost / routing）+ Tool/Memory/Diff 卡片 + Thinking blocks + Final/Recovery/Error 卡 + Project files rail（含 file preview iframe）+ Project preview 面板 + Context bar + Todo panel + 模型 invoke 旁路 + density/font 切换。

实测顶层声明显示**清晰的纯渲染子组件**已经存在（`ChatTranscript`、`ComposerDock`、`ProjectFilesRail`、`ChatMessage`、`ToolCallMessage`、`MarkdownContent`、`SkillsSlashReport`、`CodeBlock`、`ThinkingBlock`、`FinalRunReportCard`、`RecoveryCard`、`ErrorCard`、`MemoryStoreToolCard`、`SlashCommandSuggestions`、`AtFileSuggestions`），它们与 Shell 之间通过纯 props 通信，**抽出几乎全部是机械搬运**。

最后一公里是 DT-03：今日 `messages` 仍由 `App.tsx` 调 `projectConversationMessagesFromRuns(s.runs)` 后 prop-drill 注入；末次 PR 让 ChatUI Shell（≤ 200 LOC orchestrator）通过 `useRuntimeProjectionSelector` 自己读 projection，并**移除 `messages` prop**。

## 2. 问题

1. **God file**：5,043 行违反 §Architecture Boundaries「单文件 ≤ 500 LOC 软上限」。每次 chat 渲染调整都需理解全文；review diff 噪音大；并行开发冲突频繁。
2. **DT-03 漂移**：ChatUI 通过 prop drill 接收 messages，违反 P3「Projection-first UI」；阻碍 multi-window 与 transcript 真值收敛。
3. **ER-02 残留**（chat-ui:298、chat-ui:327）：raw `invoke('model_list_available')` + `listen('if2ai://models-changed')`，违反 Hard Rule。
4. **隐式依赖**：density / font / drop-item / atOverlay / slashOverlay 等 state 在 5K 行内交织，难以单测。
5. **测试盲区**：当前 chat-ui 测试仅覆盖整体 smoke；按子组件拆分后可针对每块加 render-equivalent 测试。

## 3. 目标

- 每个新 child module ≤ 500 LOC（多数 ≤ 300）。
- ChatUI Shell（`chat-ui/ChatUI.tsx`）≤ 200 LOC，纯 orchestrator。
- **末次 PR（PR-09）** 完成 DT-03：移除 `messages: Message[]` prop；改用 `useRuntimeProjectionSelector(s => projectConversationMessagesFromRuns(s.runs))`。
- 每个抽出 PR **render-equivalent**（行为零变化）。
- ER-02 在 PR-08 完成。

## 4. 非目标

- 不重写 chat 数据流（仅 DT-03 末次切换数据来源）。
- 不动 `<MemoryWriteCard>` / `<WriteToolDiffCard>` / `<VirtualMessageList>` / `<TodoPanel>` / `<ProjectPreviewPanel>` / `<ContextBar>` / `<MemoryChip>` / `<TurnCostChip>` / `<RoutingChip>` / `<ModelPicker>` / `<BranchPicker>` / `<SlashResultCard>`（**已是独立模块**，本计划只**移动**对它们的引用）。
- 不重构 toolcall 内部 markdown 解析与 ASCII 图归一化（保留为 helper utils）。
- 不引入 zustand 之外的新状态库。
- 不动后端。

## 5. 设计

> 所有新文件落在 `src/components/chat/chat-ui/`。子目录 `pickers/` `chips/` `cards/` `panels/` `sidebars/` `composer/` `transcript/` `hooks/` `utils/`。
> 每个 child 仅消费 props（DT-03 切换前），不直读 store；hooks 集中在 Shell 抽出的 `chat-ui/hooks/`。

### 5.1 Transcript cluster

| 文件 | 责任 | 来源行 | LOC | Props/Hook 表面 |
|---|---|---|---|---|
| `chat-ui/transcript/ChatTranscript.tsx` | virtual list / 直渲染切换；EmptyState；primaryThinkingMessageIds 计算 | 1915-2039 + 4509-4523 | ~140 | messages, bottomPadding, sessionTitle, projectLabel, defaultWorkdir, bottomRef, scrollRef, onScroll, onCopyMessage, onResumeFromCursor, copiedMessageId, densityMode, fontMode, isLeftPaneCollapsed, contentRightInset, contentMaxWidth |
| `chat-ui/transcript/ChatMessage.tsx` | 单条消息壳 | 3067-3287 | ~225 | message, onCopyMessage, onResumeFromCursor, isCopied, defaultWorkdir, isPrimaryThinkingMessage, densityMode, fontMode |
| `chat-ui/transcript/MarkdownContent.tsx` | ReactMarkdown + remark/rehype + ASCII normalize | 3286-3426 | ~140 | content, fontMode |
| `chat-ui/cards/ThinkingBlock.tsx` | 折叠思考；ThinkingSummaryNode | 4903-4968 | ~70 | thinking, thinkingTime, defaultOpen? |
| `chat-ui/cards/SkillsSlashReport.tsx` | `/skills` 命令报告 | 3445-3664 + parser 3582-3640 | ~330 | content |
| `chat-ui/cards/CodeBlock.tsx` | 代码高亮 + copy 按钮 | 3665-3923 + MessageCopyButton 3924-3953 | ~280 | node, inline, className, children |
| `chat-ui/cards/ToolCallCard.tsx` | tool call 渲染（含 WriteToolDiffCard 包裹） | 2611-2823 | ~210 | message, onCopyMessage, defaultWorkdir, fontMode |
| `chat-ui/cards/MemoryStoreToolCard.tsx` | 已有壳 → 抽 | 2552-2610 | ~60 | message |
| `chat-ui/cards/FinalRunReportCard.tsx` | AWL-004 final report 卡 | 4544-4629 | ~85 | report, onResumeFromCursor |
| `chat-ui/cards/RecoveryCard.tsx` | 恢复提示卡 | 4630-4692 | ~63 | message, onResumeFromCursor |
| `chat-ui/cards/ErrorCard.tsx` | 错误展示卡 | 4693-4902 | ~210 | message |
| `chat-ui/cards/LoadingIndicator.tsx` | wave dots loading | 4525-4543 | ~20 | — |

### 5.2 Composer cluster

| 文件 | 责任 | 来源行 | LOC |
|---|---|---|---|
| `chat-ui/composer/ComposerDock.tsx` | 主 composer 容器 | 2041-2549 | ~360 |
| `chat-ui/composer/SlashCommandSuggestions.tsx` | slash 弹层 | 2824-2905 | ~82 |
| `chat-ui/composer/AtFileSuggestions.tsx` | @-file 弹层 + 文件/目录浏览 | 2906-3066 | ~160 |
| `chat-ui/pickers/PermissionModePicker.tsx` | 权限模式 dropdown | 2437-2447 + items 4982-4998 | ~80 |
| `chat-ui/pickers/StrengthPicker.tsx`（可选） | strength dropdown | inline | ~60 |

> `ModelPicker` / `BranchPicker` 已存在（`@/components/chat/`），不动；本计划只在 `pickers/index.ts` 做 barrel re-export 集中。

### 5.3 Sidebars + Panels cluster

| 文件 | 责任 | 来源行 | LOC |
|---|---|---|---|
| `chat-ui/sidebars/ProjectFilesRail.tsx` | 项目文件 rail 主体 | 1427-1659 | ~233 |
| `chat-ui/sidebars/ProjectRailGroup.tsx` | rail group 渲染 | 1660-1679 | ~20 |
| `chat-ui/sidebars/ProjectRailFilePreview.tsx` | rail 内 file preview（含 iframe） | 1680-1772 | ~93 |
| `chat-ui/sidebars/ProjectRailTreeNode.tsx` | rail tree node | 1773-1890 | ~120 |
| `chat-ui/sidebars/utils.ts` | inferPreviewLanguage / sortRailDirectoryEntries / icon helpers | 1891-1914 | ~30 |
| `chat-ui/panels/TodoPanelMount.tsx` | TodoPanel 包装 + collapsed state hook | 1243-1258 + state | ~50 |
| `chat-ui/panels/ProjectPreviewMount.tsx` | ProjectPreviewPanel 包装 + 全部 preview state machine | 367-372、1027-1194、1327-1351 | ~280 |
| `chat-ui/panels/ContextBarMount.tsx` | ContextBar 挂载 | 1259-1276 | ~30 |

### 5.4 Cards / Tool helpers cluster (utils)

| 文件 | 责任 | 来源行 | LOC |
|---|---|---|---|
| `chat-ui/utils/toolCallDisplay.ts` | normalizeToolStatus / glyphs / build* / pickToolHeadline / buildToolDetailLines / buildToolCommandResultLine / normalizeToolLabel / renderInlineToolSummary / pickToolString / summarizeToolResult / shortenMiddle / firstNonEmptyLine / tryParseJson | 3974-4508 | ~480（**逼近上限，必要时拆 builders.ts vs glyphs.ts 两文件**） |
| `chat-ui/utils/diagnosticCopy.ts` | buildDiagnosticCopyText | 4159-4164 | ~10 |
| `chat-ui/utils/markdownNormalize.ts` | normalizeAsciiDiagramBlocks / transformAsciiRelationshipBlock / looks*-helpers / normalizeCodeForDisplay / normalizeNarrativeTextForDisplay / normalizeKeywordLineForDisplay / hashString | 3756-3923 | ~250 |
| `chat-ui/utils/text.ts` | formatShortTime / formatDuration / truncateText / redactSensitiveText / summarizeThinkingText | 3954-3973、5028-5050 | ~80 |
| `chat-ui/utils/items.ts` | modelItems / strengthItems / permissionModeItems / labelFor 三个 | 4970-4998 | ~40 |
| `chat-ui/utils/icons.tsx` | FontSansIcon / FontSerifIcon / DensityCompactIcon / DensityComfortableIcon | 193-256 | ~70 |
| `chat-ui/utils/MenuItemButton.tsx` | dropdown item 共用 button | 5006-5026 | ~25 |

### 5.5 Hooks cluster (Shell state extraction)

| 文件 | 责任 | LOC |
|---|---|---|
| `chat-ui/hooks/useDensityFontMode.ts` | density / font 持久化 | ~60 |
| `chat-ui/hooks/useChatScroll.ts` | bottomRef / scrollRef / scrollRaf / isAtBottom / scroll handler | ~120 |
| `chat-ui/hooks/useProjectRailState.ts` | rail path / breadcrumbs / expanded / loading / sort / preview error / width | ~200 |
| `chat-ui/hooks/useProjectPreviewState.ts` | tabs / drafts / save state / dirty / autosave timer | ~220 |
| `chat-ui/hooks/useComposerOverlays.ts` | slash / @ overlay state + timers | ~100 |
| `chat-ui/hooks/useComposerDropItems.ts` | drop items + handlers | ~80 |

### 5.6 Shell

| 文件 | 责任 | LOC |
|---|---|---|
| `chat-ui/ChatUI.tsx` | orchestrator：组合 hooks + 渲染 transcript / panels / composer / sidebars；DT-03 后内部读 projection | ≤ 200 |
| `src/components/ui/chat-ui.tsx` | 1-line re-export shim：`export { ChatUI } from '@/components/chat/chat-ui/ChatUI'`（保 callsite 兼容；后续 PR 删 shim） | ~5 |

> **BrowserCard / ArtifactCard**：源文件未发现独立的 browser iframe 卡（仅 1754 行 file-preview iframe，已归入 `ProjectRailFilePreview`），且无独立 artifact 卡。本计划**不创建**这两个文件。

## 6. PR 序列（极重要）

> 每 PR 之间都 rebase；每 PR **render-equivalent**；每 PR 跑 `npm test` + `npm run build:web` + 人工 chat smoke。
> 顺序原则：先抽**叶子卡**（无回写依赖），再抽**结构性子组件**（Transcript / ComposerDock），最后抽**state-heavy 面板** + **DT-03 cutover**。

| # | PR | 抽出内容 | Shell LOC 估 | 关键风险 |
|---|---|---|---|---|
| **PR-01** | `gf01-leaf-cards` | `ThinkingBlock` + `LoadingIndicator` + `EmptyState` + `MemoryStoreToolCard` + `FinalRunReportCard` + `RecoveryCard` + `ErrorCard` + `MenuItemButton` + `utils/icons.tsx` + `utils/text.ts` + `utils/items.ts` | ~4,300 | 极低；纯渲染 |
| **PR-02** | `gf01-tool-utils` | `utils/toolCallDisplay.ts`（≤ 480；若超 500 则 split → builders + glyphs）+ `utils/diagnosticCopy.ts` + `utils/markdownNormalize.ts` | ~3,650 | 中：函数多；TDD 用 snapshot 对照 |
| **PR-03** | `gf01-markdown-codeblock` | `MarkdownContent` + `CodeBlock` + `MessageCopyButton` + `SkillsSlashReport` | ~3,000 | 中：rehype/remark plugin 顺序敏感 |
| **PR-04** | `gf01-toolcall-card` | `ToolCallCard` + `MemoryStoreToolCard` 引用更新 | ~2,650 | 中：依赖 PR-02 utils |
| **PR-05** | `gf01-transcript` | `ChatTranscript` + `ChatMessage`（含 chip 槽） | ~2,150 | 高：transcript 是中心；render snapshot test 强守护 |
| **PR-06** | `gf01-composer` | `ComposerDock` + `SlashCommandSuggestions` + `AtFileSuggestions` + `PermissionModePicker` | ~1,400 | 高：键盘事件复杂；keydown e2e |
| **PR-07** | `gf01-sidebars-panels` | `ProjectFilesRail` + `ProjectRailGroup` + `ProjectRailFilePreview` + `ProjectRailTreeNode` + `sidebars/utils.ts` + `panels/TodoPanelMount` + `panels/ProjectPreviewMount` + `panels/ContextBarMount` | ~600 | 高：含 file preview iframe + 拖拽宽度 |
| **PR-08** | `gf01-hooks-er02` | 抽 6 个 hooks；**ER-02**：在 `src/api/models.ts` 暴露 `useAvailableModels()` + `subscribeModelsChanged()`，删除 chat-ui 内 raw invoke + listen | ~250 | 中 |
| **PR-09** | `gf01-dt03-projection-cutover` | **DT-03**：`ChatUI.tsx` 内部 `useRuntimeProjectionSelector(...)` + 移除 `messages: Message[]` prop；同步 `App.tsx` callsite；删 shim；新增 **projection-first invariant test** | ≤ 200 | **最高**：唯一改数据流的 PR |

> 9 个 PR；W1：PR-01~02；W2：PR-03~05；W3：PR-06~07；W4：PR-08~09。

## 7. TDD 策略

### 7.1 框架
Vitest + @testing-library/react + jsdom（与项目当前一致）。

### 7.2 既有 chat-ui 测试
PR-01 起每 PR **首先**跑现有 `npm test` 确保零回归。**禁止** snapshot update 除非 DT-03 explain 数据来源切换。

### 7.3 每抽一个组件 → 加一条最小 render test

```ts
// chat-ui/cards/ThinkingBlock.test.tsx
import { render, screen } from '@testing-library/react'
import { ThinkingBlock } from './ThinkingBlock'

it('renders collapsed by default and expands on click', () => {
  render(<ThinkingBlock thinking="A\nB" thinkingTime={1234} />)
  expect(screen.getByText(/已完成思考/)).toBeInTheDocument()
})
```

为 `MarkdownContent` / `CodeBlock` / `SkillsSlashReport` / `ToolCallCard` 各加一条带 fixture 的 snapshot test。

### 7.4 PR-09 DT-03 invariant tests（**新增**）
新文件 `src/components/chat/chat-ui/ChatUI.dt03.test.tsx`：

1. **projection-first invariant**：mount `<ChatUI />` 时 store 注入两条 user msg + 一条 assistant run；断言渲染消息数 = `projectConversationMessagesFromRuns` 输出数。
2. **no `messages` prop**：TS 编译期断言 `ChatUIProps` 不再含 `messages` 字段（`expectTypeOf`）。
3. **store update re-renders**：dispatch 一条 token append；断言 `screen.getByText(...)` 出现。
4. **session swap**：调 `runtimeProjectionStore.swapSession?.('s2')`（若 ER-03 已上线）→ 断言旧 session 消息消失。
5. **regression**：重跑 PR-05 引入的 ChatTranscript snapshot；diff 应为零。

### 7.5 ER-02 测试
`src/api/models.useAvailableModels.test.ts`：mock invoke + window event；断言返回 items + subscribe/unsubscribe 路径正确。

## 8. 文件清单 per PR

> 仅列**新增 / 删除 / 重定向 import**；既有公共组件不动。

**PR-01**（新增 11 文件）：`chat-ui/cards/{ThinkingBlock,LoadingIndicator,EmptyState,MemoryStoreToolCard,FinalRunReportCard,RecoveryCard,ErrorCard}.tsx`、`chat-ui/utils/{icons.tsx,text.ts,items.ts,MenuItemButton.tsx}`。

**PR-02**（新增 3 文件）：`chat-ui/utils/{toolCallDisplay,diagnosticCopy,markdownNormalize}.ts`。

**PR-03**（新增 4 文件）：`chat-ui/transcript/MarkdownContent.tsx`、`chat-ui/cards/{CodeBlock,MessageCopyButton,SkillsSlashReport}.tsx`。

**PR-04**（新增 1 + test）：`chat-ui/cards/ToolCallCard.tsx`。

**PR-05**（新增 2 + tests）：`chat-ui/transcript/{ChatTranscript,ChatMessage}.tsx`。

**PR-06**（新增 4 文件）：`chat-ui/composer/{ComposerDock,SlashCommandSuggestions,AtFileSuggestions}.tsx`、`chat-ui/pickers/PermissionModePicker.tsx` + `pickers/index.ts`。

**PR-07**（新增 8 文件）：`chat-ui/sidebars/{ProjectFilesRail,ProjectRailGroup,ProjectRailFilePreview,ProjectRailTreeNode}.tsx` + `sidebars/utils.ts`；`chat-ui/panels/{TodoPanelMount,ProjectPreviewMount,ContextBarMount}.tsx`。

**PR-08**（新增 6 hooks + 改 `src/api/models.ts`）：`chat-ui/hooks/{useDensityFontMode,useChatScroll,useProjectRailState,useProjectPreviewState,useComposerOverlays,useComposerDropItems}.ts`。`src/api/models.ts` 加 `useAvailableModels()` + `subscribeModelsChanged()`。

**PR-09**（新增 `chat-ui/ChatUI.tsx` + 改 `src/components/ui/chat-ui.tsx` 为 1 行 re-export shim）：
- `chat-ui/ChatUI.tsx` 内部 `useRuntimeProjectionSelector(...)` 取 messages
- `App.tsx`：删除 `projectConversationMessagesFromRuns` 调用与 `messages` prop
- 新增 `chat-ui/ChatUI.dt03.test.tsx`

## 9. 验证

每个 PR：

```bash
npm test
npm run build:web
# 视改动决定是否 cargo 类
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

**手工 smoke checklist**（每 PR）：

1. 新建 session → 发短消息 → assistant 流式回复正常
2. 触发工具调用 → 工具卡渲染、状态轮转、复制按钮工作
3. 触发权限 → permission overlay 弹出 → 响应后 turn 继续
4. 打开 file rail → 折叠/展开 → 点开文本文件 → preview 面板出现 → 编辑 → 保存（autosave 900ms）
5. Slash 命令 `/skills` → SkillsSlashReport 渲染
6. @-file 弹层 → 选中插入 → 发送
7. 拖拽文件到 composer → chip 出现 → 移除 chip
8. 切换 model picker / permission picker / branch picker
9. 长 transcript（>50 条）→ VirtualMessageList 启用，滚动平滑
10. **PR-09 专项**：reload 应用 → ChatUI 立即显示 projection 历史；触发新 turn → 流式更新；同时打开第二窗口（若支持）→ 两窗同步

## 10. 风险与缓解

| 风险 | 严重度 | 缓解 |
|---|---|---|
| **回归 chat 渲染**（最高风险） | 高 | 每 PR render-equivalent；每 PR 跑既有快照测试 + 新增 child render test |
| `useMemo` 边界 / `React.memo` 比较器丢失 → 重渲染抖动 | 中 | 抽出后保留原 `(prev, next) => …` 比较器；用 React DevTools Profiler 验证 commit 数不增 |
| `MarkdownContent` cache 跨模块共享语义丢失 | 中 | 在 `chat-ui/transcript/MarkdownContent.tsx` 内部保留 module-level Map；测试覆盖 cache key |
| ToolCallCard 依赖太多 helper → import 链膨胀 | 中 | PR-02 先抽 utils，PR-04 再抽卡 |
| ER-02 hook 与 chat-ui 抽出竞争同一区段 | 中 | 合到 PR-08；本计划文档化「ER-02 在 PR-08 内联完成」 |
| DT-03 后 prop drift（如 sessionId 路径丢失） | 高 | PR-09 增加专项 invariant test；commit message 标 `[BREAKING]` |
| `App.tsx` 仍持 `projectConversationMessagesFromRuns` 调用未移除 → DT-03 不彻底 | 高 | PR-09 必须删除该调用 |
| 大 PR 难 review | 中 | 每 PR 严格控制在 ≤ 800 LOC diff |
| 与 GF-03（App.tsx 拆分）的 rebase 冲突 | 中 | 约定 GF-03 不改 ChatUI callsite，仅 GF-01 PR-09 改 |

## 11. PR 模板

```markdown
## 改善 ID
GF-01 (PR <#> — <slug>) [+ DT-03 / ER-02 if PR-08/09]
docs/IMPROVEMENTS-2026-05-05.md §4 GF-01 / §1 DT-03 / §3 ER-02

## 变更摘要
<1-3 句>：本 PR 抽出 <files>，从 chat-ui.tsx 移出 <N> 行；行为零变化（render-equivalent）。

## Before / After
- chat-ui.tsx LOC: <before> → <after>
- 新文件: <list>
- 行为变化: 无 / DT-03 数据来源切换 (PR-09)

## 验证
- [ ] npm test（含新增 child render test）
- [ ] npm run build:web
- [ ] 人工 smoke checklist 1-9（PR-09 含 10）

## 关联
- Plan: docs/superpowers/plans/2026-05-05-gf01-chat-ui-split.md
- 总览: docs/superpowers/plans/2026-05-05-improvements-wave1-overview.md
- (PR-08) ER-02 follow-up: src/api/models.ts useAvailableModels / subscribeModelsChanged
- (PR-09) DT-03 cutover: removes `messages` prop; ChatUI now reads runtimeProjectionStore directly
```
