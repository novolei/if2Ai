# 桌面应用快速开始

> 从零到运行：If2Ai 桌面应用安装、配置与构建完整指南

## 📍 系统要求

### 主要支持：macOS

| 项目 | 最低要求 | 推荐配置 |
|------|----------|----------|
| 操作系统 | macOS 11.0 (Big Sur) | macOS 14.0+ (Sonoma) |
| 芯片 | Intel / Apple Silicon | Apple Silicon (M1+) |
| 内存 | 8 GB | 16 GB+ |
| 磁盘 | 2 GB 可用空间 | 5 GB+ |

### 其他平台（规划中）

- **Windows**：架构已预留支持，待 CI 构建脚本就绪
- **Linux**：Tauri 2 原生支持，待测试覆盖

> 源码参考：`src-tauri/tauri.conf.json` 中 `macOS.minimumSystemVersion: "11.0"`

## 🏗️ 安装步骤

### 1. 安装 Rust 工具链

```bash
# 安装 rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证版本（需要 1.77+）
rustc --version
cargo --version
```

### 2. 安装 Node.js

```bash
# 推荐使用 nvm 管理 Node 版本
nvm install 20
nvm use 20

# 验证版本（需要 Node 20+）
node --version
npm --version
```

### 3. 安装 Tauri CLI 及项目依赖

```bash
# 克隆项目
git clone <repo-url> && cd if2Ai

# 安装前端依赖
npm install

# 安装 Tauri CLI（已包含在 devDependencies 中）
# 也可全局安装
npm install -g @tauri-apps/cli
```

> 源码参考：`package.json` 中 `@tauri-apps/cli: "latest"`, `@tauri-apps/api: "latest"`

## 🔄 开发环境搭建

### 启动开发服务器

```bash
# 方式一：Tauri 开发模式（推荐，同时启动前端 + 后端）
npm run tauri:dev

# 方式二：仅启动前端（Vite 热更新）
npm run dev
```

开发模式下：
- 前端 Vite 开发服务器运行于 `http://localhost:9527`
- Rust 后端自动编译并挂载
- 前端热更新（HMR）即时生效，Rust 修改需等待重编译

> 源码参考：`vite.config.ts` 中 `server.port: 9527`，`tauri.conf.json` 中 `devUrl`

### 编译期常量

Vite 构建时注入以下常量（单一信源 = `package.json`）：

```typescript
// vite.config.ts → define
__APP_VERSION__  // 来自 package.json version
__APP_NAME__     // 来自 package.json name
```

### 路径别名

```typescript
// tsconfig.json + vite.config.ts
import { Something } from '@/components/...'  // @ → ./src
```

## 📝 首次运行与配置

### 1. API Key 配置

首次启动会进入新手引导流程，引导配置：

| 环境变量 | 用途 | 获取方式 |
|----------|------|----------|
| `ANTHROPIC_API_KEY` | Anthropic Claude 主提供商 | [console.anthropic.com](https://console.anthropic.com/) |
| `ANTHROPIC_AUTH_TOKEN` | Bearer Token 认证（OAuth） | OAuth 流程获取 |
| `OPENAI_API_KEY` | OpenAI 兼容提供商 | [platform.openai.com](https://platform.openai.com/) |
| `XAI_API_KEY` | xAI 提供商 | [console.x.ai](https://console.x.ai/) |

### 2. 数据目录

If2Ai 默认数据目录为 `~/.if2ai/`：

```
~/.if2ai/
├── log/                  # 后端日志（按日轮转）
├── sessions/             # 会话数据
├── projects/             # 项目数据
├── memory/               # 记忆数据库
│   ├── memory.db         # SQLite 主库
│   ├── vector_db/        # LanceDB 向量库
│   ├── jobs.db           # 后台任务队列
│   ├── pinned.md         # 置顶记忆
│   └── summaries/        # 会话摘要
├── trajectories/         # 学习轨迹（ShareGPT JSONL）
├── browser-cold-state.json  # 浏览器冷状态
└── models/tts/           # TTS 模型文件
```

### 3. 可选环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `IF2AI_SESSIONS_DIR` | `~/.if2ai/sessions` | 自定义会话目录 |
| `IF2AI_PROJECTS_DIR` | `~/.if2ai/projects` | 自定义项目目录 |
| `IF2AI_HARNESS_ENABLED` | `0` | 启用测试 Harness |
| `IF2AI_HRR_ENABLED` | `false` | 启用 HRR 混合记忆 |
| `RUST_LOG` | `info` | Rust 日志级别 |

## 🚀 构建发布

### macOS 构建

```bash
# 开发构建
npm run build

# 正式发布构建（含版本号同步）
npm run release:macos
```

`release:macos.sh` 脚本同步版本号到：
- `package.json`（单一信源）
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

### 构建产物

```
target/release/bundle/
├── dmg/              # macOS 安装镜像
└── macos/            # .app 应用包
```

### 关键构建配置

```jsonc
// tauri.conf.json
{
  "productName": "If2Ai",
  "version": "0.4.0",
  "identifier": "dev.if2ai.desktop",
  "bundle": {
    "active": true,
    "resources": [
      "resources/bundled-skills",    // 内置技能包
      "resources/voices/*.wav",       // 语音资源
      "resources/voices/*.mp3"
    ],
    "macOS": {
      "minimumSystemVersion": "11.0",
      "infoPlist": "Info.plist"
    }
  }
}
```

## 🔗 相关资源

- [Tauri 2 官方文档](https://v2.tauri.app/)
- [Rust 安装指南](https://www.rust-lang.org/tools/install)
- [架构深度解析](./02-architecture.md)
- [功能概览](./03-features.md)
