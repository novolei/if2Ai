# Guide 用户指南

> If2Ai 从安装到上手的完整指南 —— 新用户的第一站

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [quick-start.md](./quick-start.md) | 🚀 快速开始指南——从零到运行的完整步骤 |
| [env-vars.md](./env-vars.md) | ⚙️ 环境变量与配置说明 |
| [faq.md](./faq.md) | ❓ 常见问题解答 |

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **Tauri dev** | 开发模式命令 `npm run tauri:dev`，启动 Vite (9527) + Rust 后端 |
| **Tauri build** | 生产构建 `npm run build`，输出 macOS .dmg / .app |
| **API Key** | 首次运行时通过引导流程配置，存储在 `~/.if2ai/` |
| **数据目录** | `~/.if2ai/`——日志、记忆、会话、模型、声音样本等 |
| **Vite 端口** | 默认 `9527`，在 `vite.config.ts` 中配置 |
| **版本同步** | `package.json` → `tauri.conf.json` → `Cargo.toml` 通过构建脚本同步 |

## 🏗️ 源码位置

| 组件 | 路径 |
|------|------|
| NPM 脚本 | `package.json` (`scripts` 字段) |
| Vite 配置 | `vite.config.ts` |
| Tauri 配置 | `src-tauri/tauri.conf.json` |
| Rust 入口 | `src-tauri/src/main.rs` |
| 前端入口 | `src/main.tsx` |
| 引导流程 | `src/modules/onboarding/` |
| 环境检查 | `src-tauri/src/modules/system_check/` |
| 快速开始旧版 | `QUICKSTART.md` |

## 🔗 相关资源

- [Reference 模块](../reference/) — 项目结构与编码规范
- [Agent 模块](../agent/) — 代理系统设计
- [Memory 模块](../memory/) — 记忆系统使用指南
- [AGENTS.md](../../../AGENTS.md) — 项目导航地图
