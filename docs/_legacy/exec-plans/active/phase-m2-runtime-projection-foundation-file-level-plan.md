# Phase M2 Runtime Projection Foundation File-Level Plan

> 将 `M2` 的 `m2.1 + m2.2 + m2.3` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m2.1` Split transport contracts from tauri facade
2. `m2.2` Introduce runtime event translator
3. `m2.3` Introduce runtime event queue and reducer

目标是先把前端的“事实输入面”收口，建立 `transport -> translator -> queue -> reducer` 的单一通道，再进入后续 store 和 shell 重构。

## 2. 当前事实基线

### 2.1 `tauri.ts` 现状

当前 [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1) 同时承担：

1. IPC bridge
2. runtime contract type center
3. memory event type center
4. onboarding / activation / settings DTO center
5. event subscribe helper

这会直接妨碍 `M2` 的核心目标，因为一旦 contract 继续堆在 `tauri.ts`，前端就没有稳定的 transport boundary。

### 2.2 runtime event 现状

当前主聊天路径仍然主要靠 [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1) 直接监听和解释事件，尤其是：

1. stream token payload
2. permission request payload
3. onboarding cross-window reset
4. telemetry drawer data source

也就是说，事件归一化现在还没有一个正式前端入口。

### 2.3 现有前端状态现实

> **2026-04-20 audit fix**：M2 audit 阶段 `src/stores/memory-slice.ts`
> 已删除（零 in-tree consumer），决议落档于
> `src/stores/README.md`。memory 主路径**唯一 SoT** 是
> `src/runtime-projection/runtime-projection-store.ts`。下文所有
> 关于 memory store 迁移的指引，应在 `runtime-projection` 投影层
> 内完成，不再回写 `src/stores/memory-slice.ts`。

当前仓库剩余的 external-store 风格状态实现：

1. [src/stores/browser-slice.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/browser-slice.ts:1)
   —— browser 子系统 UI convenience state，保留
2. [src/stores/conversation-slice.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/conversation-slice.ts:1)
   —— conversation 列表 UI 状态，保留

这意味着 `M2` 不该粗暴“清空 stores 重建”，而应该先建立新的 canonical projection pipeline，再渐进迁移调用点。

## 3. 实施原则

1. 先收 transport boundary，再收 event normalization。
2. translator 必须是单一入口，页面组件不得继续兼容 legacy payload。
3. reducer 只投影状态，不发明业务真相。
4. 这一轮允许 `src/stores/` 与新的 `src/state/` 共存，但新主路径必须开始依赖 `src/state/`。

## 4. 严格执行顺序

1. `F0` preflight inventory
2. `F1` 新建 `src/transport/contracts.ts`
3. `F2` 瘦身 `src/lib/tauri.ts`
4. `F3` 建 `runtime-event-translator`
5. `F4` 建 `runtime-event-queue`
6. `F5` 建 `runtime-event-reducer`
7. `F6` 在主入口接入新 pipeline
8. `F7` compile + manual verification

禁止并行：

1. `tauri.ts` contract 拆分
2. `App.tsx` 事件监听改写
3. `ChatUI` 主路径重构

因为这三项一起动时，最容易让事件 shape 和消费方同时漂移。

## 5. 文件级实施方案

## 5.1 `F0` Preflight Inventory

### 必查文件

- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)
- [src/stores/browser-slice.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/browser-slice.ts:1)
- [src/stores/README.md](/Users/ryanliu/Documents/IfAI/if2Ai/src/stores/README.md:1)
  （memory-slice 已删，README 记录决议；新主路径见
  `src/runtime-projection/runtime-projection-store.ts`）

### 必做动作

1. 列出 `tauri.ts` 中所有 runtime / memory / activation / onboarding / settings DTO 分组。
2. 列出 `App.tsx` 中所有直接解释后端 payload 的位置。
3. 列出已有 store 中哪些字段属于 projection，哪些字段只是 UI convenience state。

## 5.2 `F1` 新建 `src/transport/contracts.ts`

### 新增文件

- `src/transport/contracts.ts`
- `src/transport/index.ts`

### 修改文件

- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)

### 第一批必须迁出的类型

1. `ContextBudgetUsage`
2. `MemoryEventPayload`
3. `MemoryContextItem`
4. `StreamTokenPayload`
5. `PermissionRequestPayload`
6. `PermissionMode`

### 如果 activation DTO 已存在也应一并迁出

至少把 onboarding/activation 主路径会直接消费的 DTO 同步归位，避免后续 `AppBootGate` 仍继续从 `tauri.ts` 读契约。

### 必须落实

1. `contracts.ts` 只承载 typed transport contracts，不放 invoke/listen 实现。
2. `tauri.ts` 改为 import contracts，而不是自己声明 contracts。
3. 所有新 import path 优先指向 `@/transport/contracts`。

### 这一步不要做的事

1. 不要在 `contracts.ts` 混入 store 类型。
2. 不要把 UI 专用 view-model 也塞进 transport contracts。
3. 不要改字段语义或 snake_case/camelCase 协议。

## 5.3 `F2` 瘦身 `src/lib/tauri.ts`

### 修改文件

- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)

### 必须落实

把 `tauri.ts` 收成三类职责：

1. `invoke(...)` facade
2. `listen(...)` facade
3. typed IPC helper

### 最低结构建议

```ts
export * from '@/transport/contracts'

export async function startAgentStream(...) { ... }
export async function listenToStream(...) { ... }
export async function listenToPermissionRequests(...) { ... }
```

### 第一阶段允许保留

1. 现有大量 settings-related invoke helper
2. browser / memory / tts / stt helper

但这些 helper 不得再附带重新定义 transport contract。

## 5.4 `F3` 建 `runtime-event-translator`

### 新增文件

- `src/runtime-projection/runtime-event-translator.ts`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### 最低结构建议

```ts
export type CanonicalRuntimeEvent =
  | { kind: 'stream_text_delta'; ... }
  | { kind: 'stream_thinking_delta'; ... }
  | { kind: 'stream_complete'; ... }
  | { kind: 'stream_error'; ... }
  | { kind: 'permission_request'; ... }
  | { kind: 'memory_event'; ... }

export function translateRuntimeEvent(input: ...): CanonicalRuntimeEvent | null
```

### 第一批必须覆盖的输入源

1. `agent-token`
2. permission request event
3. memory event

### 第一阶段不要做的事

1. 不要在 translator 内直接改 React state。
2. 不要把 reducer 逻辑混进 translator。
3. 不要在页面组件里保留第二套 legacy payload fallback。

## 5.5 `F4` 建 `runtime-event-queue`

### 新增文件

- `src/runtime-projection/runtime-event-queue.ts`

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### 目标

把高频 streaming event 和普通 control event 统一纳入可控通道，避免：

1. 直接在 listener callback 中多点 `setState`
2. 事件消费顺序依赖 React render timing
3. 后续 reducer/store 接入时还要重新梳理一遍事件节流

### 最低结构建议

```ts
export interface RuntimeEventQueue {
  push(event: CanonicalRuntimeEvent): void
  subscribe(fn: (batch: CanonicalRuntimeEvent[]) => void): () => void
}
```

### 推荐实现要求

1. 支持 batch flush
2. 保证事件顺序稳定
3. 为未来 dev inspector 留 hooks

## 5.6 `F5` 建 `runtime-event-reducer`

### 新增文件

- `src/runtime-projection/runtime-event-reducer.ts`
- `src/runtime-projection/types.ts`

### 修改文件

- `src/runtime-projection/runtime-event-translator.ts`
- `src/runtime-projection/runtime-event-queue.ts`

### reducer 第一版必须能输出的 projection

1. run stream text/thinking/status
2. permission prompt projection
3. memory evidence projection
4. browser status projection 的接缝

### 最低结构建议

```ts
export interface RuntimeProjectionSnapshot {
  runs: ...
  approvals: ...
  memory: ...
}

export function reduceRuntimeEvent(
  prev: RuntimeProjectionSnapshot,
  event: CanonicalRuntimeEvent
): RuntimeProjectionSnapshot
```

### 第一阶段不要做的事

1. 不要把 localStorage 偏好写进 reducer。
2. 不要把 project/session 列表 CRUD 混进 reducer。
3. 不要在 reducer 里自己“推断” execution mode。

## 5.7 `F6` 在主入口接入新 pipeline

### 修改文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)

### 必须落实

1. stream listener 先走 translator。
2. translator 输出进入 queue。
3. queue flush 驱动 reducer。
4. `App.tsx` 暂时可以仍持有局部 state，但新增状态更新必须通过 projection pipeline 进入。

### 这一步刻意不做

1. 不要求 `App.tsx` 彻底瘦身。
2. 不要求 `ChatWorkspace` 和 `ChatUI` 立即切 projection store。
3. 不要求 activation/onboarding 立即接进 pipeline。

## 5.8 `F7` Compile + Manual Verification

### 必跑

- `npm run build:web`

### 必做人工检查

1. 会话中连续 streaming 时不丢字、不乱序。
2. permission request 仍正常弹出。
3. Telemetry / memory evidence 相关 UI 不因 contract 拆分而报错。
4. `tauri.ts` 顶部不再继续塞新的 runtime contract type。

## 6. 完成定义

只有当 reviewer 可以明确指出“前端现在已经有一条单一 runtime event 归一化通道”，这一段才算完成。
