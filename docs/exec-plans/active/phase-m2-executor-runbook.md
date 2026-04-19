# Phase M2 Executor Runbook

> 将 `phase-m2-frontend-runtime-projection.yaml` 细化为 executor 可直接执行的前端投影层手册。
>
> 最后更新: 2026-04-20

## 1. 目的

`M2` 的任务是让 React 前端从“隐性编排者”收口为“后端真相的投影层”。  
不是换目录名，也不是单纯做 store 抽取。

## 2. 本阶段必须产出的结果

1. `tauri.ts` 回归 thin bridge。
2. event translator / queue / reducer / feature stores 出现。
3. `App.tsx` 拆成 boot shell + main shell。
4. activation gate 与 execution-mode explainability 进入正式 UI。

## 3. 读取顺序

1. [phase-m2-frontend-runtime-projection.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-frontend-runtime-projection.yaml:1)
2. [runtime-contracts-and-event-projection-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/runtime-contracts-and-event-projection-design.md:1)
3. [frontend-information-architecture-ui-redesign.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/frontend-information-architecture-ui-redesign.md:1)
4. [activation-gate-and-license-lifecycle-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/activation-gate-and-license-lifecycle-design.md:1)
5. [request-intelligence-and-execution-mode-routing-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/request-intelligence-and-execution-mode-routing-design.md:1)
6. [phase-m2-runtime-projection-foundation-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-runtime-projection-foundation-file-level-plan.md:1)
7. [phase-m2-state-boot-shell-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-state-boot-shell-file-level-plan.md:1)
8. [phase-m2-chat-and-visible-surfaces-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-chat-and-visible-surfaces-file-level-plan.md:1)

## 4. 落地决策

1. 类型入口统一到 `src/transport/contracts.ts`
2. 事件归一化统一到 `src/runtime-projection/`
3. feature stores 统一到 `src/state/`
4. boot flow 统一到 `src/boot/`
5. `App.tsx` 只做根装配，不再承载启动编排和主工作台编排

## 5. 严格执行顺序

1. `m2.0` preflight frontend inventory
2. `m2.1` contracts split
3. `m2.2` translator
4. `m2.3` queue + reducer
5. `m2.4` feature stores
6. `m2.5` execution-mode store
7. `m2.6` activation gate boot layer
8. `m2.7` App shell refactor
9. `m2.8` chat main path refactor
10. `m2.9` user-visible projection

## 6. Slice 详细执行说明

## 6.1 `m2.0` Preflight Frontend Inventory

### 必查文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- [src/modules/chat/components/HomeScreen.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/HomeScreen.tsx:1)

### 必做动作

1. 盘出直接处理 runtime event 的位置。
2. 盘出启动流、onboarding、activation 相关 UI 当前入口。
3. 盘出 execution mode 目前有没有前端自行判断的逻辑。

## 6.2 `m2.1` Contracts Split

### 输出文件

- `src/transport/contracts.ts`
- `src/lib/tauri.ts`

### 必须落实

1. runtime / activation / execution-mode / memory 类型迁出 `tauri.ts`
2. `tauri.ts` 只保留 IPC/transport facade

## 6.3 `m2.2` Translator

### 输出文件

- `src/runtime-projection/runtime-event-translator.ts`

### 必须落实

1. legacy payload -> canonical envelope 有单一入口。
2. 页面组件不再自己兼容历史 payload。

## 6.4 `m2.3` Queue And Reducer

### 输出文件

- `src/runtime-projection/runtime-event-queue.ts`
- `src/runtime-projection/runtime-event-reducer.ts`

### 必须落实

1. 高频流事件与普通事件进入统一 reducer 通道。
2. reducer 只做投影状态更新，不重新定义业务真相。

## 6.5 `m2.4` Feature Stores

### 输出文件

- `src/state/session-store.ts`
- `src/state/run-store.ts`
- `src/state/memory-store.ts`
- `src/state/approval-store.ts`

### 必须落实

1. state 分工按投影语义切分。
2. store 不重算 runtime 语义。

## 6.6 `m2.5` Execution-Mode Store

### 输出文件

- `src/state/execution-mode-store.ts`

### 必须落实

1. projection `execution_mode / risk / complexity / reason codes / matched rules`
2. 支持 explainability surface
3. manual override 只做 UI 控制面，不改 classifier 真相

## 6.7 `m2.6` Activation Gate Boot Layer

### 输出文件

- `src/boot/AppBootGate.tsx`
- `src/boot/activation-gate-state.ts`
- `src/state/activation-store.ts`

### 必须落实

1. `startup -> onboarding -> activation gate -> main shell`
2. activation gate 是硬门禁 overlay
3. revoke / expire / deactivate 能回流 gate

## 6.8 `m2.7` App Shell Refactor

### 输出文件

- `src/App.tsx`
- `src/modules/app-shell/`

### 必须落实

1. `App.tsx` 只做根装配
2. boot shell / main shell 职责分离

## 6.9 `m2.8` Chat Main Path Refactor

### 输出文件

- `src/components/ui/chat-ui.tsx`
- `src/modules/chat/`

### 必须落实

1. chat 主路径改读 projection stores
2. debug / telemetry / observability 面板二级展开

## 6.10 `m2.9` User-Visible Projection

### 必须落实

1. Chat/Inspector/Developer Settings 显示 execution mode 和 explainability
2. Settings/Account 显示 activation/license 状态和反激活入口

## 7. 交付物检查清单

- [ ] `src/transport/contracts.ts` 已建立
- [ ] `src/runtime-projection/` 已建立
- [ ] `src/state/` 已建立核心 stores
- [ ] `src/boot/` 已建立
- [ ] `App.tsx` 已明显降责
- [ ] `chat-ui.tsx` 已明显降责
- [ ] activation gate 进入正式 boot flow
- [ ] execution mode explainability 可见
- [ ] `npm run build:web` 已通过
- [ ] 已按 3 份文件级任务单完成 `m2.1 ~ m2.9`

## 8. 推荐提交顺序

1. `m2.1 + m2.2 + m2.3`
2. `m2.4 + m2.5`
3. `m2.6 + m2.7 + m2.8 + m2.9`

对应文件级任务单：

1. [phase-m2-runtime-projection-foundation-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-runtime-projection-foundation-file-level-plan.md:1)
2. [phase-m2-state-boot-shell-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-state-boot-shell-file-level-plan.md:1)
3. [phase-m2-chat-and-visible-surfaces-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-chat-and-visible-surfaces-file-level-plan.md:1)

## 9. 完成标志

只有当 reviewer 能明确指出“前端现在只负责投影和控制，而不是偷偷重算后端语义”时，`M2` 才算完成。
