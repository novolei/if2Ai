# If2Ai - AI Agent Desktop Application

一个基于 Tauri + Rust + Svelte 的跨平台 AI 智能体桌面应用，旨在为 [hermes-agent](https://github.com/) 提供现代的桌面应用版本。

## 📋 项目概述

**If2Ai** 是一个完整的 AI 智能体系统的桌面实现，提供以下功能：

- 🤖 **LLM 集成** - 支持 OpenAI、Claude 等多个 LLM 提供商
- 🛠️ **Agent 框架** - 灵活的 Agent 管理和执行框架
- 🗃️ **向量数据库** - 集成 Pinecone、Weaviate 等向量数据库
- 🖥️ **跨平台** - Windows、macOS、Linux 完整支持
- ⚡ **高性能** - Rust 后端提供卓越性能

## 🛠️ 技术栈

| 层级 | 技术 | 描述 |
|------|------|------|
| **桌面框架** | Tauri 2.0 | 轻量级跨平台应用框架 |
| **后端** | Rust | 性能优化的异步后端 |
| **前端** | Svelte + Vite | 反应式 UI 框架 |
| **IPC** | Tauri Commands | 类型安全的前后端通信 |

## 📁 项目结构

```
if2Ai/
├── src/                          # 前端代码 (Svelte)
│   ├── components/               # UI 组件
│   ├── lib/                      # 工具库
│   ├── styles/                   # 样式文件
│   ├── App.svelte                # 主应用组件
│   └── main.ts                   # 入口点
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── commands/             # Tauri commands 处理器
│   │   ├── modules/              # 核心业务模块
│   │   ├── main.rs               # Rust 入口点
│   │   └── build.rs              # 构建脚本
│   └── Cargo.toml                # Rust 依赖配置
├── index.html                    # HTML 入口
├── package.json                  # NPM 依赖配置
├── tauri.conf.json               # Tauri 配置
├── vite.config.ts                # Vite 配置
└── tsconfig.json                 # TypeScript 配置
```

## 🚀 快速开始

### 前置条件

- Node.js 18+ 和 npm
- Rust 1.60+
- Tauri 环境（macOS: Xcode, Windows: MSVC, Linux: 构建工具）

### 安装

```bash
# 1. 克隆仓库
git clone <repository-url>
cd if2Ai

# 2. 安装依赖
npm install

# 3. 安装 Tauri CLI
npm install --save-dev @tauri-apps/cli
```

### 开发

```bash
# 启动开发服务器
npm run tauri dev
```

### 构建

```bash
# 构建生产版本
npm run build
```

## 📦 可用命令

- `npm run tauri dev` - 启动开发服务器
- `npm run build` - 构建应用
- `npm run preview` - 预览生产构建
- `npm run test` - 运行测试
- `npm run test:ui` - 运行 UI 测试

## 🔧 核心模块计划

### Phase 1 - 基础设施 (当前)
- [x] 项目框架搭建
- [ ] 前后端通信机制
- [ ] 基础 UI 框架

### Phase 2 - 核心功能
- [ ] LLM 集成模块
- [ ] Agent 管理系统
- [ ] 对话界面

### Phase 3 - 扩展功能
- [ ] 向量数据库集成
- [ ] 插件系统
- [ ] 集群支持

## 🤝 贡献指南

## 📄 许可证

## 💡 相关资源

- [Tauri 官方文档](https://tauri.app/)
- [Svelte 文档](https://svelte.dev/)
- [Rust 官方文档](https://www.rust-lang.org/learn)

---

**当前版本**: 0.1.0 | **更新日期**: 2026-04-11
