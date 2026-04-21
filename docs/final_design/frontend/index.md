# 前端架构（Frontend）

> If2Ai 前端架构文档：React + TypeScript + 运行时投影的完整指南

## 📚 子文档目录

| 文档 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户/设计师 | 组件库、设计系统、主题切换 |
| [02-implementation.md](./02-implementation.md) | 开发者 | 状态架构、组件层次、样式体系 |
| [03-runtime-projection.md](./03-runtime-projection.md) | 开发者 | 运行时投影系统深度解析 |

## 📍 核心概念速查表

| 概念 | 说明 | 源码位置 |
|------|------|----------|
| MainShell | 主界面壳组件 | `src/App.tsx` |
| RuntimeProjection | 运行时投影（纯函数 Reducer） | `src/runtime-projection/` |
| Transport Layer | IPC 桥接层 | `src/transport/contracts.ts` + `src/lib/tauri.ts` |
| Paico Design | 翡翠薄雾设计系统 | `src/styles/globals.css` |
| ShadCN + Radix | 23 个基础组件 | `src/components/ui/` |
| Conversation Slice | 对话状态切片 | `src/stores/conversation-slice.ts` |
| Browser Slice | 浏览器状态切片 | `src/stores/browser-slice.ts` |
| Event Translator | 后端事件翻译器 | `src/runtime-projection/runtime-event-translator.ts` |

## 🔗 相关链接

- [桌面架构](../desktop/02-architecture.md)
- [API 模块](../api/) — 流式事件的来源
- [Paico 设计规范](../../references/if_2_ai_动态视觉系统规范.md)
