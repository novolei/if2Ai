# Phase M2 State Boot And Shell File-Level Plan

> 将 `M2` 的 `m2.4 + m2.5 + m2.6 + m2.7` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m2.4` Create feature stores for session/run/memory/approval
2. `m2.5` Introduce execution-mode store
3. `m2.6` Introduce activation gate boot layer
4. `m2.7` Refactor `App.tsx` into boot shell and main shell

目标是把 `App.tsx` 当前混合的启动流、工作台流、事件流拆成可治理的前端骨架。

## 2. 当前事实基线

### 2.1 `App.tsx` 现状

当前 [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1) 同时承担：

1. splash + boot
2. onboarding route decision
3. project/session 初始加载
4. stream listener 与会话更新
5. permission prompt
6. telemetry drawer open state
7. 工作台主 JSX 装配

这已经明显超出了 root component 的合理责任范围。

### 2.2 activation 现状

当前 activation 在前端仍然主要是 onboarding 的第 6 步语义：

1. [src/modules/onboarding/OnboardingApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/onboarding/OnboardingApp.tsx:1)
2. [src/modules/onboarding/hooks/useOnboarding.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/onboarding/hooks/useOnboarding.ts:1)

这还不是 blueprint 要求的：

`startup -> onboarding -> activation gate -> main shell`

### 2.3 execution mode 现状

当前前端还没有正式的：

1. `execution-mode-store`
2. explainability projection surface
3. manual override UI control boundary

这意味着 runtime classifier 即使在后端成型，前端也没有对应的 canonical projection 容器。

## 3. 实施原则

1. 先建 store，再拆 shell。
2. activation gate 先做硬门禁 overlay，不要先追求漂亮视觉。
3. onboarding 流可继续存在，但不再独占 boot truth。
4. 这一轮允许 `App.tsx` 仍持有部分过渡状态，但新增主路径状态必须进 store。

## 4. 严格执行顺序

1. `S0` preflight inventory
2. `S1` 建 `src/state/` 骨架
3. `S2` 建 session/run/memory/approval stores
4. `S3` 建 execution-mode store
5. `S4` 建 activation store + gate state
6. `S5` 建 `AppBootGate.tsx`
7. `S6` 建 boot shell / main shell
8. `S7` 回写 `App.tsx`
9. `S8` compile + manual verification

禁止并行：

1. activation gate UI
2. onboarding hook 改造
3. App shell JSX 大搬家

因为这三项一起动时，最容易让启动路径和主工作台路径同时失稳。

## 5. 文件级实施方案

## 5.1 `S0` Preflight Inventory

### 必查文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [src/modules/onboarding/OnboardingApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/onboarding/OnboardingApp.tsx:1)
- [src/modules/onboarding/hooks/useOnboarding.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/onboarding/hooks/useOnboarding.ts:1)
- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
- [src/stores/browser-slice.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/browser-slice.ts:1)

### 必做动作

1. 标出 `App.tsx` 里所有 boot-only state。
2. 标出 `App.tsx` 里所有 main-shell-only state。
3. 标出 onboarding hook 当前直接调用的 activation command。

## 5.2 `S1` 建 `src/state/` 骨架

### 新增文件

- `src/state/index.ts`
- `src/state/internal/store-utils.ts`

### 修改文件

- 无强制修改旧业务文件，但需要在后续 store 文件统一 import 此处的工具。

### 目标

为新 projection stores 建一个明确入口，同时避免直接复制 `src/stores/*` 的每个实现细节。

### 明确决策

1. `src/stores/` 保留用于 legacy/专项 store。
2. `src/state/` 用于 M2 之后的 canonical runtime projection stores。
3. executor 不得在这轮强行重命名所有旧 stores。

## 5.3 `S2` 建 session/run/memory/approval stores

### 新增文件

- `src/state/session-store.ts`
- `src/state/run-store.ts`
- `src/state/memory-store.ts`
- `src/state/approval-store.ts`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- 视接入情况可改 [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)

### 建议 store 分工

`session-store.ts`

1. active project/session ids
2. session metadata projection
3. recent session projection

`run-store.ts`

1. active stream/run status
2. message timeline projection
3. loading / stop / recovering flags

`memory-store.ts`

1. current turn memory evidence
2. memory summary / chip / audit projection

`approval-store.ts`

1. pending permission request
2. last decision projection
3. approval dialog open/close state

### 这一步不要做的事

1. 不要把 project CRUD action 全部塞进 store。
2. 不要把 typography/density 这种本地 UI 偏好塞进 runtime stores。
3. 不要让 store 重新解释后端 risk/complexity。

## 5.4 `S3` 建 execution-mode store

### 新增文件

- `src/state/execution-mode-store.ts`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### 第一版必须承载的字段

1. `executionMode`
2. `riskLevel`
3. `complexityLevel`
4. `reasonCodes`
5. `matchedRules`
6. `manualOverride`
7. `lastUpdatedAt`

### 明确约束

1. `manualOverride` 是 UI control plane 状态，不是 classifier 真相。
2. reason codes/matched rules 必须来自后端 emitted data 或 canonical translator 输出。
3. 这一轮如果后端字段尚未齐全，可以先设计 typed empty state，但不得前端自己造 classifier。

## 5.5 `S4` 建 activation store + gate state

### 新增文件

- `src/state/activation-store.ts`
- `src/boot/activation-gate-state.ts`

### 修改文件

- [src/modules/onboarding/hooks/useOnboarding.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/onboarding/hooks/useOnboarding.ts:1)
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### activation store 第一版必须承载

1. activation status
2. license snapshot
3. revoked / expired / deactivated flags
4. last checked at
5. onboarding-ready-but-not-activated state

### 关键要求

当前 onboarding hook 中的：

1. `activation_validate`
2. `activation_start`
3. `activation_complete`

仍可保留以服务旧流程，但 `AppBootGate` 的状态判定不得继续直接绑死在 onboarding 的 `Ready/Onboarding` 二元语义上。

## 5.6 `S5` 建 `AppBootGate.tsx`

### 新增文件

- `src/boot/AppBootGate.tsx`
- `src/boot/BootSplash.tsx`
- `src/boot/BootRouteDecision.ts`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### `AppBootGate` 最低职责

1. splash minimum duration
2. onboarding state fetch
3. activation snapshot fetch
4. boot route decision

### 第一版 route decision 至少要区分

1. `show_splash`
2. `show_onboarding`
3. `show_activation_gate`
4. `show_main_shell`

### 这一步不要做的事

1. 不要在 `AppBootGate` 里做项目/会话全量加载。
2. 不要把 ChatWorkspace 渲染逻辑塞回 gate。
3. 不要把 settings / telemetry 状态塞进 gate。

## 5.7 `S6` 建 boot shell / main shell

### 新增文件

- `src/modules/app-shell/BootShell.tsx`
- `src/modules/app-shell/MainShell.tsx`
- `src/modules/app-shell/AppShellBoundary.tsx`

### 修改文件

- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### shell 分工要求

`BootShell.tsx`

1. 承载 splash / onboarding / activation gate 容器位
2. 不承载聊天工作台状态

`MainShell.tsx`

1. 承载 `GlobalNavbar + SectionWorkspace + ChatWorkspace + drawers/dialogs`
2. 通过 store/projection 输入主数据

### 特别注意

`MainShell` 可以先接 props，再逐步转 store；第一轮重点是把 `App.tsx` 的 JSX 责任切开，而不是一口气全部换成 hooks。

## 5.8 `S7` 回写 `App.tsx`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### 必须落实

`App.tsx` 最终只保留：

1. root-level provider wiring
2. boot gate orchestration
3. main shell assembly

### 这轮完成后不应继续留在 `App.tsx` 的东西

1. splash/onboarding/project loading 的混合实现
2. activation route decision 的 inline 逻辑
3. 大块主工作台 JSX

### 允许暂时保留

1. 若干 legacy event listener glue
2. 过渡期 props drilling

前提是这些 glue 已经明显被 boot shell / main shell 包住。

## 5.9 `S8` Compile + Manual Verification

### 必跑

- `npm run build:web`

### 必做人工检查

1. 首次启动仍可正确进入 onboarding。
2. 已完成 onboarding 的用户不会反复掉回 onboarding。
3. activation 未通过时会进入 gate，而不是直接进主聊天页。
4. `App.tsx` 肉眼可见明显瘦身。

## 6. 完成定义

只有当 reviewer 可以明确说出“现在已经有 boot layer，且 `App.tsx` 不再自己偷偷兼任 boot orchestrator 和 main workspace controller”时，这一段才算完成。
