# Runtime Projection And Shell Gap — 使用指南

> 解释为什么前端已经有 projection、BootShell、MainShell，但主体验仍没完全改观。

## 核心问题

前端当前最关键的 gap 是：

> projection 层已经存在，但还没有完全取代原来的隐性编排层。

## 关键证据

- [App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- [runtime-projection-bridge.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-bridge.ts:1)
- [AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:6)

## 使用场景

- 分析为什么 projection store 已经有了，chat 还没完全切过去
- 分析为什么 BootShell 出现后，App.tsx 还是很胖

