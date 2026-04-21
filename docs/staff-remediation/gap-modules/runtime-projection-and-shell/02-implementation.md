# Runtime Projection And Shell Gap — 实现清单

## If2Ai 当前证据

- `App.tsx` 仍承担大量启动与主路径责任：
  - [App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- runtime projection bridge 明确写着和现有 chat listener 并行运行：
  - [runtime-projection-bridge.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-bridge.ts:1)

## 具体 Gap

1. projection store 还不是唯一真相
2. boot shell 只是容器，不是完整 boot state machine
3. chat UI 仍保留大量直接理解原始事件的逻辑

## UClaw 对标

- AppStore + event routing 是主路径：
  - [AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:6)
- MODULE_MAP 对前端壳层状态有诚实标注：
  - [MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:1)

## 建议整改

1. 让 ChatWorkspace / chat-ui 优先消费 projection stores
2. 下线并行老 listener
3. 把 App.tsx 收敛为装配根，而不是业务入口

