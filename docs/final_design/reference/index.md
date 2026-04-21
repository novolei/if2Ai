# Reference 参考资料

> If2Ai 项目结构与编码规范的完整参考 —— 开发者的工具箱

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [project-structure.md](./project-structure.md) | 🏗️ 完整项目目录说明 |
| [coding-style.md](./coding-style.md) | 📏 编码规范与 lint 合约 |

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **Bounded Context** | 模块化边界，一个目录一个职责 |
| **God-file** | 超 800 行的大文件，需重构拆分（god-file watchlist） |
| **lint_architecture.py** | 架构级 lint 工具，检查模块边界和文件大小 |
| **Pack 流水线** | 5 步结构化开发流程 |
| **CHARTER** | Pack 流水线章程，定义所有开发规则 |
| **REGISTRY** | Pack 注册表，跟踪所有活跃和已完成的 Pack |
| **降级链** | 记忆系统提供商的回退顺序 |

## 🏗️ 源码位置

| 组件 | 路径 |
|------|------|
| 架构 lint | `scripts/lint_architecture.py` |
| Pack 工具 | `scripts/pack` |
| 编码规范 | `docs/references/coding-style-and-lint-contract.md` |
| Pack 章程 | `docs/packs/CHARTER.md` |
| Pack 注册表 | `docs/packs/REGISTRY.md` |
| 后端模块 | `src-tauri/src/modules/`（22 个） |
| 前端模块 | `src/modules/`（7 个） |
| 前端组件 | `src/components/` |

## 🔗 相关资源

- [Guide 模块](../guide/) — 快速开始与配置
- [Agent 模块](../agent/) — 代理系统设计
- [Memory 模块](../memory/) — 记忆系统设计
- [Skills 模块](../skills/) — 技能系统设计
- [AGENTS.md](../../../AGENTS.md) — 项目导航地图
