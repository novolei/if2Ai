# GF-03 — Progressively Extract `src/App.tsx` (3,310 LOC) into Focused Sub-Effects / Boot Helpers

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §4 GF-03、§7.4
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)
> **依赖**：无独立硬依赖。**注意配合**：ER-04（`App.tsx:1393-94` legacy 通道注释更新）可在本系列任一 PR 顺手完成；ER-03（rapid session-switch race 原子 swap）应在 PR-2 / PR-6 之前完成或与 PR-6 合并；MIG-017 末次（ChatUI 直读 projection、移除 `messages` prop）由 GF-01 末次承担，本计划**不**做。
> **预计**：2-3 周，6 PR

---

## 1. 总览

`src/App.tsx` 当前 3,310 LOC，是 vNext 前端唯一仍承载多职责的 god-component：boot 协同、session 加载/切换、stream 生命周期、permission overlay、title stage / undo-redo、updater banner、git probe、telemetry shortcut、chat resize/drag、cross-window sync、conversation slice 写入封装、runtime projection wiring、updater 状态机、…。函数体内含 **~38 个 hook**（22 useState/useCallback、12 useEffect、7 useMemo、5 useRef）。单 `sendMessage` ≈ 565 行。

部分早期工作已完成：`runBootSequence` 已抽到 `src/boot/boot-orchestrator.ts`（168 LOC）；App.tsx 仅 driver；bootstrap-store / chat-store / session-store / runtimeProjectionStore 已是真值；`wireRuntimeProjectionListeners` 已是单行 effect。

剩余 god 体量集中在 9 个 cluster：
1. **Bootstrap setter wrappers** (290-621) — 9 个 `Dispatch<SetStateAction<T>>` shim
2. **Session load** (`handleSelectSession` 1629-1852, 224 行 history → Message 转换)
3. **Title stage / auto-rename** (823-1168, ~345 行)
4. **Stream lifecycle** (`sendMessage` 2239-2804, ~565 行)
5. **Permission overlay** (628-640, 2209-2233, 3240-3296)
6. **Runtime/effect wiring** (boot+models+evolution 212-271, projection 1400-1403, updater 1405-1478, compact 1485-1507, prefill 1509-1526, hotkeys 1529-1558, undo-redo 2120-2207, git probe 1218-1252)
7. **Pane / drag** (2813-2928)
8. **Updater banner** (384-389, 1405-1478, 3016-3123)
9. **Misc effects**（git probe / prefill / hotkeys / compact toast / undo-redo / telemetry drawer）

注释 `App.tsx:1393-94` 仍写「`agent-token` / `permission-request` / `memory_event`」，实际已 retire（PR D-1 / D-2 / FIX-14 已确认）；本计划顺带在 PR-1 改成「canonical `runtime_event` envelope」。

## 2. 问题

| # | Cluster | 行数估 | 副作用 |
|---|---------|-------|--------|
| C1 | Boot + models refresh + evolution listener effect | 60 | mixes 3 listeners + boot driver in one useEffect |
| C2 | Setter shims (`setProjects` … `setStreamAbortHandles`) | 330 | 重复模板，污染主体 |
| C3 | Title stage / auto-rename / `syncGeneratedSessionTitle` | 345 | 业务逻辑混在 god-component；4 个 ref + 2 个 state |
| C4 | `handleSelectSession` history → Message 转换 | 224 | 复杂 reducer-like 逻辑应独立 |
| C5 | `sendMessage` stream loop（含 slash/skill/compact 分支） | 565 | 嵌套 closure 难以测试 |
| C6 | Permission overlay JSX + handler | 100 | 已 projection-driven，可纯组件化 |
| C7 | Updater banner state machine + check effect | 175 | 与 chat 主体无耦合 |
| C8 | Pane resize / window drag / focus mode | 115 | UI 局部状态，可独立 |
| C9 | Misc effects | 280 | 互不相关，宜各自抽 hook |

合计 ~2,194 LOC；保留 boot driver + router 装配 + TS prop 桥接 ~500 LOC，恰好命中目标。

## 3. 目标

- **每新文件 ≤ 400 LOC**（hard ceiling；ChatStream 例外目标 ≤ 480 LOC）
- **App.tsx ≤ 500 LOC**，仅做 store 订阅、子 hook 组合、AppShell 装配
- **零行为变化**（projection-first 维持；undo / resume / cursor / auto-rename 行为像素级一致）
- **每 PR 单独可回滚**；npm test 全程绿
- **TS 类型严格**：抽出函数若依赖 store snapshot 需通过参数注入或直接订阅
- **顺带修复 ER-04** 注释（不算独立 PR）

## 4. 非目标

- 不做 **MIG-017 末次切割**（ChatUI 直读 projection、删 `messages` prop）— 归 GF-01 末次（DT-03 合并）。
- 不做 **ER-03**（`runtimeProjectionStore.swapSession()` 原子 swap）— 标记 `PR-6 必须依赖 ER-03 落地`。
- 不重写 stream loop 业务（仅迁移 + 拆函数 + 注入依赖；assistant message id / autoResume 行为不动）。
- 不删除 setter shim 模式。
- 不引入新 lib / 新状态库。

## 5. 设计

### 5.1 PR 总览

| PR | 抽出物 | 路径 | LOC est | App.tsx 来源行 | 主要风险 |
|----|--------|------|---------|---------------|----------|
| PR-1 | runtime / updater / hotkeys / prefill effect 集中 | `src/app-effects/RuntimeProjectionWiring.tsx` + `useUpdaterBanner.ts` + `useGlobalHotkeys.ts` + `useChatPrefill.ts` + `useAutoCompactToast.ts` | 80 + 150 + 60 + 50 + 50 | 1400-1558, 1485-1507, 3022-3123 | low |
| PR-2 | Session load / convert history | `src/session/loadConversationHistory.ts` | 280 | 1629-1852 | medium |
| PR-3 | Setter shim 集中 | `src/stores/use-store-setter-shims.ts` | 380 | 290-621 | low |
| PR-4 | Title stage / auto-rename | `src/session/titleStage.ts` + `useSessionTitleStage.ts` | 360 | 823-1168 | medium |
| PR-5 | Permission overlay | `src/permission/PermissionOverlayHost.tsx` + `usePermissionOverlay.ts` | 180 | 628-640, 2209-2233, 3240-3296 | low |
| PR-6 | Stream lifecycle | `src/session/sendChatTurn.ts` + `src/session/SessionEffects.tsx` | 460 + 120 | 2102-2811 | high — 与 ER-03 协同 |

合计抽出 ≈ 2,170 LOC；App.tsx 余下 boot driver + router 组合 ≈ 480 LOC ✅。

> 原始改善建议含 `src/boot/runBootSequence.ts`，但 `boot-orchestrator.ts` 已存在且 168 LOC，**不再单独 PR**；只在 PR-1 顺带把 boot useEffect 内的 models-changed / evolution listener 一起搬到 RuntimeProjectionWiring。

### 5.2 详细设计

#### PR-1 — `RuntimeProjectionWiring.tsx` + 4 hooks（≤ 400 LOC each）

**`src/app-effects/RuntimeProjectionWiring.tsx`** (≤ 80 LOC, 纯 effect 容器组件)
- 责任：mount 时 `wireRuntimeProjectionListeners()`、`runtime_event` evolution listener、`if2ai://models-changed` listener、`listenChatCompactCompleted` toast。
- 来源：App.tsx **L212-271**（boot effect 中的 models + evolution 部分）+ **L1400-1403** + **L1485-1507**。

**`src/app-effects/useUpdaterBanner.ts`** (≤ 150 LOC)
- 返回：`{ appUpdaterState, latestUpdaterVersion, updaterBannerVisible, dismiss, runUpdater }`
- 来源：App.tsx **L384-389** + **L1405-1478** + **L3016-3123**。

**`src/app-effects/useGlobalHotkeys.ts`** (≤ 60 LOC)
- Cmd+, → openSettingsWindow；Cmd+Shift+D → telemetry drawer toggle。
- 来源：**L1529-1558** + state **L1543**.

**`src/app-effects/useChatPrefill.ts`** (≤ 50 LOC)
- listen prefill → setActiveSection("chat") + setInput。
- 来源：**L1509-1526**.

**`src/app-effects/useAutoCompactToast.ts`** (≤ 50 LOC)
- 已含在 RuntimeProjectionWiring 内的 alt：可独立成 hook 视长度而定；preferred 独立。

> ER-04 顺手：此 PR 改 App.tsx **L1393-94** 注释为「canonical `runtime_event` envelope routed via the projection bridge」。

#### PR-2 — `src/session/loadConversationHistory.ts` (≈ 280 LOC)

```ts
export type LoadedSessionHistory = {
  conversation: Conversation;
  recoveredTodos: TodoItem[];
  titleState: SessionTitleState;
};
export async function loadConversationHistory(args: {
  sessionId: string;
  projectId: string;
  sessionMeta: SessionMeta | undefined;
  projectWorkdir: string | undefined;
}): Promise<LoadedSessionHistory>;
```
- 内部封装 `getSession` + `getSessionHistoryPage` + `replayRunLogEntriesToMessages` + `upsertToolMessage` + `extractTodosFromToolResult`。
- App.tsx 的 `handleSelectSession` 改为调用此函数 → 三个 setter 写入 + 错误降级。
- 来源：**L1629-1852**.
- 配套：抽 `extractTodosFromToolResult`（**L677-698**）、`extractMemoryStoreFields`（**L713-760**）到同目录 `messageExtraction.ts`（≤ 80 LOC）。

#### PR-3 — `src/stores/use-store-setter-shims.ts` (≈ 380 LOC, hook composition)

返回单一 object：
```ts
const {
  projects, setProjects,
  projectSessions, setProjectSessions,
  currentProject, setCurrentProject,
  activeProjectId, setActiveProjectId,
  activeSessionId, setActiveSessionId,
  conversations, setConversations,
  sessionLoading, setSessionLoading,
  sessionTodos, setSessionTodos,
  sessionTitleStates, setSessionTitleStates,
  streamAbortHandles, setStreamAbortHandles,
} = useAppStateSetters();
```
- 来源：**L289-380, 397-440, 451-476, 497-561, 593-621**.
- 行为零变化；`useCallback` 依赖列表保持 `[]`（store 持久引用）。
- 单测：每个 wrapper round-trip。

#### PR-4 — `src/session/titleStage.ts` + `useSessionTitleStage.ts` (≈ 360 LOC)

`titleStage.ts`（pure functions, ≤ 200 LOC, 100% testable）：
- `formatSessionTitle`、`normalizeSessionTitleSource`、`isMeaningfulUserMessage`、`getMeaningfulUserMessages`、`getInitialSessionTitleCandidate`、`getCorrectionTitleCandidate`、`getInitialSessionTitleState`、`areTitlesSimilar`、`currentTitleIsResolved`、`buildLoopCompletionStatus`。
- 来源：**L762-942**。
- `MAX_AUTO_RENAME_COUNT`、`PLACEHOLDER_SESSION_TITLE`、`GENERIC_USER_PROMPTS` 移至此模块。

`useSessionTitleStage.ts` (≈ 160 LOC, hook)：
- 返回：`{ maybeAutoRenameSession, syncSessionTitle, syncGeneratedSessionTitle, handleRenameSession }`。
- 内部依赖：注入 setConversations、setProjectSessions、setSessionTitleStates、conversations、projectSessions、sessionTitleStatesRef、pendingAutoTitleSessionIdsRef。
- 来源：**L944-1168, L2008-2020**。
- **L562-567** 的 `sessionTitleStatesRef` sync useEffect 一并迁入 hook 内部。

#### PR-5 — `PermissionOverlayHost.tsx` + `usePermissionOverlay.ts` (≈ 180 LOC)

**`src/permission/usePermissionOverlay.ts`** (≤ 80 LOC)
- 返回 `{ permissionPrompt, decide(decision, scope) }`。
- 内部：`useRuntimeProjectionSelector(s => s.approvals)` → 取 first → 构造 `PermissionRequestPayload`；`decide` = respondPermission + projection dispatch `permission_resolved`。
- 来源：**L628-640, L2209-2233**.

**`src/permission/PermissionOverlayHost.tsx`** (≤ 100 LOC)
- 纯展示 Dialog，prop = hook 返回值。
- 来源：**L3240-3296**.

App.tsx 仅 `<PermissionOverlayHost />` 一行。

#### PR-6 — `src/session/sendChatTurn.ts` + `SessionEffects.tsx` (≈ 460 + 120 LOC)

**`src/session/sendChatTurn.ts`** (≈ 460 LOC, async function)
```ts
export async function sendChatTurn(args: {
  text: string;
  sessionIdOverride?: string;
  isInternalResume?: boolean;
  resumeCursor?: string;
  deps: SendChatTurnDeps;
}): Promise<void>;
```
- 来源：**L2235-2804**.
- 拆 4 个内部 helper：`createAssistantMessage`、`flushAssistantDeltas` cluster、`handleProjectedStreamSideEffect`、slash branch（skill / `/compact` / builtin）。
- `agentVoice` 通过 deps 注入。

**`src/session/SessionEffects.tsx`** (≈ 120 LOC)
- 责任：托管 `sessionLoadingRef` / `autoResumeAttemptsRef` / `attemptedAutoResumeCursorsRef` / `sessionLoadingRef sync useEffect`、把 `sendChatTurn` / `stopAgentStream` / `handleResumeFromCursor` / undo / redo 暴露为 hook：`useSessionRuntime()`。
- 把 undo/redo（**L2120-2207**）、`refreshConversationUndoStatus`、`conversationUndoStatus` state 迁入。
- 来源：**L641-643 (refs), L1350-1352 (sync), L2102-2207, L2806-2811**.

**ER-03 协同**：PR-6 的合并 PR 描述必须包含「需 ER-03 已落地」前置。

## 6. PR 拆分（推荐顺序）

```
PR-1  RuntimeProjectionWiring + 4 effect hooks + ER-04 注释  (low risk, 准备地基)
   ↓
PR-2  loadConversationHistory   (history 转换隔离 + 单测)
   ↓
PR-3  use-store-setter-shims    (与 PR-2 / PR-4 解耦后再拆 shim)
   ↓
PR-4  titleStage + useSessionTitleStage   (依赖 PR-3 setter shape)
   ↓
PR-5  PermissionOverlayHost     (独立，可与 PR-4 并行)
   ↓
PR-6  sendChatTurn + SessionEffects   (压轴；依赖 PR-2 / PR-3 / PR-4 完成；建议 ER-03 已落地)
```

## 7. TDD 策略

设施：Vitest + jsdom（已是 npm test 默认 loader），无需 UI snapshot。

| PR | 测试 |
|----|------|
| PR-1 | `useUpdaterBanner.test.ts` — boot status fetch + 6h throttle + dismiss 持久化；`useGlobalHotkeys.test.ts`；`useChatPrefill.test.ts` |
| PR-2 | `loadConversationHistory.test.ts` — (a) replay-only path (b) fullSession-only path (c) replay 优先于 fullSession 当含 assistant/tool (d) tool_use + tool_result merge (e) TodoWrite 提取 (f) 异常降级 |
| PR-3 | `use-store-setter-shims.test.tsx` — 9 个 setter 各 round-trip |
| PR-4 | `titleStage.test.ts` — 全部 pure 函数 boundary；`useSessionTitleStage.test.tsx` — `maybeAutoRenameSession` 不在 placeholder 时不变；`syncSessionTitle` 失败回滚 |
| PR-5 | `usePermissionOverlay.test.tsx` — 选 first approval；`decide` dispatch projection event；`PermissionOverlayHost.test.tsx` — 4 个按钮分别触发对应 (decision, scope) |
| PR-6 | `sendChatTurn.test.ts` — (a) builtin slash (b) skill slash (c) `/compact` (d) normal turn (e) `stream_complete` cleanup (f) `stream_error` + auto resume (g) `startChatTurn` throw → error message + sessionLoading=false |

## 8. 文件清单（per PR）

### PR-1
- `src/app-effects/{RuntimeProjectionWiring.tsx, useUpdaterBanner.ts, useGlobalHotkeys.ts, useChatPrefill.ts, useAutoCompactToast.ts}` (新增)
- `src/app-effects/__tests__/*.test.ts(x)` (新增)
- `src/App.tsx` (删除对应 effect，加 `<RuntimeProjectionWiring />`、`useUpdaterBanner()` 等；ER-04 注释更新)

### PR-2
- `src/session/loadConversationHistory.ts` (新增)
- `src/session/messageExtraction.ts` (新增)
- `src/session/__tests__/loadConversationHistory.test.ts` (新增)
- `src/App.tsx` (`handleSelectSession` 简化为 setter 编排)

### PR-3
- `src/stores/use-store-setter-shims.ts` (新增)
- `src/stores/__tests__/use-store-setter-shims.test.tsx` (新增)
- `src/App.tsx` (replace 9 个 useCallback)

### PR-4
- `src/session/titleStage.ts` (新增)
- `src/session/useSessionTitleStage.ts` (新增)
- `src/session/__tests__/{titleStage,useSessionTitleStage}.test.{ts,tsx}` (新增)
- `src/App.tsx` (删除 title cluster)

### PR-5
- `src/permission/usePermissionOverlay.ts` (新增)
- `src/permission/PermissionOverlayHost.tsx` (新增)
- `src/permission/__tests__/*.test.tsx` (新增)
- `src/App.tsx` (`overlays` 渲染收敛)

### PR-6
- `src/session/sendChatTurn.ts` (新增)
- `src/session/handleSlashCommand.ts` (新增, 视长度)
- `src/session/SessionEffects.tsx` (新增；含 useSessionRuntime)
- `src/session/__tests__/sendChatTurn.test.ts` (新增)
- `src/App.tsx` (剩余 ≤ 500 LOC)

## 9. 验证

每 PR 必须通过：

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml         # noop（前端 PR）
npm test
npm run build:web
npm run check:version
```

每 PR 在描述中报告 `wc -l src/App.tsx` before / after + 新文件 LOC + ≤ 400 LOC 上限是否触线。

PR-6 合并前手工 smoke：
1. start chat turn → 触发权限 → 响应 → turn 完成 → reload → history 回来。
2. partial_success → auto resume 一次（max 2 次）。
3. `/help`、`/skills`、`/compact`、skill slash 各跑一遍。
4. 撤销 / 重做（含 streaming 中确认弹框）。
5. 快速切换 session 5 次（验证 ER-03 已生效，无错配 projection 事件）。

## 10. 风险与缓解

| 风险 | 缓解 |
|------|------|
| **快速切换 session race** | 标记为 ER-03 依赖；PR-6 描述要求 ER-03 已合并；smoke 第 5 步专项验证 |
| `sendMessage` closure 重构破坏 assistant message id 序 | PR-6 用 deps-injected pattern 保留所有 closure 变量；新增 7 个 case 单测 |
| Title rollback 与 ref 状态分离后失同步 | PR-4 把 `pendingAutoTitleSessionIdsRef` 收入 hook 内部；测试覆盖回滚路径 |
| 抽出 hook 后 `useEffect` 依赖列表漂移 | 全部新 hook 用 `[]` + 显式 ref / store snapshot 模式；ESLint exhaustive-deps 不放 disable |
| PR 之间 rebase 冲突 | 顺序串行 PR-1 → 2 → 3 → 4 → 5 → 6；PR-5 可与 PR-4 并行 |
| TS prop drilling 改动可能击穿 ChatWorkspace prop 类型 | 本计划不动 ChatWorkspace prop；setter 类型与现状一致 |
| `/compact` 仍 import `@/lib/tauri` | 不在本计划范围；归 RD-01 |
| `import("@/modules/git/api")` 动态加载位移 | 抽 `useCurrentBranch.ts` 时保留 dynamic import 形 |

## 11. PR 描述模板

```markdown
## 改善 ID
GF-03 (PR-X — <topic>) — docs/IMPROVEMENTS-2026-05-05.md §4

## 变更摘要
<1-3 句>

## Before / After
- src/App.tsx LOC: <before> → <after>  (target after PR-6: ≤ 500)
- 新增文件: <list with LOC>
- 行为变化: 无 / <specific>

## 验证
- [ ] npm test
- [ ] npm run build:web
- [ ] App.tsx LOC 截图（or `wc -l`）
- [ ] 新文件 ≤ 400 LOC
- [ ] 手工 smoke（PR-6 必填）

## 关联
- Plan: docs/superpowers/plans/2026-05-05-gf03-app-tsx-extraction.md
- 依赖: <PR-N>
- 顺带: ER-04 (PR-1) / ER-03 (PR-6 prerequisite)
```
