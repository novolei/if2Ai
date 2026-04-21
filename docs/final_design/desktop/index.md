# 桌面应用（Desktop）

> If2Ai 桌面应用模块：基于 Tauri 2 + Rust + React 的智能体桌面应用完整指南

## 📚 子文档目录

| 文档 | 面向 | 内容 |
|------|------|------|
| [01-quick-start.md](./01-quick-start.md) | 用户 | 安装、配置、构建发布 |
| [02-architecture.md](./02-architecture.md) | 开发者 | Tauri 架构、启动流程、IPC 通信 |
| [03-features.md](./03-features.md) | 用户/开发者 | 功能列表、设计系统、差距分析 |

## 📍 核心概念速查表

| 概念 | 说明 | 源码位置 |
|------|------|----------|
| Tauri 2 | Rust 后端 + Web 前端桌面框架 | `src-tauri/` |
| AppState | 全局状态容器，所有管理器的中心注入点 | `src-tauri/src/commands/` |
| main.rs | 启动入口（1,217 行），编排全生命周期 | `src-tauri/src/main.rs` |
| IPC | `invoke` 前端调后端 + `listen` 后端推前端 | `@tauri-apps/api` |
| 多窗口 | 主窗口 + 设置窗口 + 浏览器查看器 | `src-tauri/tauri.conf.json` |
| Paico | If2Ai 设计系统（翡翠薄雾色调） | `src/styles/globals.css` |

## 🔗 相关链接

- [Tauri 2 官方文档](https://v2.tauri.app/)
- [If2Ai 项目架构](../../ARCHITECTURE.md)
- [API 模块文档](../api/)
- [前端模块文档](../frontend/)
