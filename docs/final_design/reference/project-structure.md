# 项目结构说明

> If2Ai 完整项目目录与模块清单 —— 理解代码组织的第一步

## 🏗️ 顶层目录结构

```
if2Ai/
├── src/                        # React 19 + TypeScript 前端
│   ├── App.tsx                 # 主应用组件（god-file，待重构）
│   ├── main.tsx                # 前端入口
│   ├── components/             # UI 组件库
│   ├── modules/                # 前端业务模块
│   ├── stores/                 # 状态管理
│   ├── transport/              # Tauri IPC 通信层
│   ├── runtime-projection/     # 运行时状态投射
│   ├── boot/                   # 启动引导
│   ├── shell/                  # Shell 路由
│   ├── lib/                    # 工具库
│   ├── assets/                 # 静态资源
│   └── styles/                 # 全局样式
├── src-tauri/                  # Rust 后端（Tauri 2）
│   ├── src/
│   │   ├── main.rs             # Rust 入口
│   │   ├── commands/           # Tauri IPC 命令
│   │   └── modules/            # 核心业务模块（22 个）
│   ├── Cargo.toml              # Rust 依赖配置
│   ├── tauri.conf.json         # Tauri 应用配置
│   ├── build.rs                # 构建脚本
│   ├── capabilities/           # Tauri 权限声明
│   ├── icons/                  # 应用图标
│   └── resources/              # 打包资源
│       ├── bundled-skills/     # 内置技能
│       └── voices/             # 默认声音样本
├── docs/                       # 文档体系
│   ├── final_design/           # 模块化设计文档（本目录）
│   ├── packs/                  # Pack 开发流水线
│   │   ├── CHARTER.md          #   流水线规则
│   │   ├── REGISTRY.md         #   Pack 一览
│   │   ├── refactor/           #   GFR-XXX 重构 Pack
│   │   ├── feature/            #   FEAT-XXX 功能 Pack
│   │   └── snapshots/          #   重构基准快照
│   ├── design-docs/            # 参考设计文档
│   ├── product-specs/          # 产品规格
│   ├── references/             # 参考资料
│   ├── baseline/               # 基线分析
│   ├── bs_gap/                 # 差距分析
│   ├── staff-remediation/      # 整改方案
│   └── _legacy/                # 已冷藏文档
├── scripts/                    # 自动化脚本
│   ├── pack                    # Pack 工具（snapshot/verify/review/scan/init）
│   ├── lint_architecture.py    # 架构 lint 工具
│   ├── pack_helper.py          # Pack 工具辅助
│   └── release-macos.sh        # macOS 发布脚本
├── harness/                    # 测试框架
│   ├── suites/                 #   测试套件（YAML）
│   ├── evaluators/             #   评估器
│   ├── runners/                #   运行器
│   ├── fixtures/               #   测试固件
│   ├── gate.py                 #   门控检查
│   └── runner.py               #   套件运行器
├── rust/                       # 独立 Rust crate 工作空间
│   └── crates/                 #   独立 crate（9 个）
├── dist/                       # 前端构建输出
├── target/                     # Rust 编译输出
├── package.json                # Node.js 配置
├── vite.config.ts              # Vite 构建配置
├── tsconfig.json               # TypeScript 配置
├── Cargo.toml                  # Rust workspace 根配置
└── Cargo.lock                  # Rust 依赖锁定
```

## 🦀 后端模块清单

`src-tauri/src/modules/` 下的 22 个 bounded-context 模块：

| 模块 | 说明 | 源码路径 |
|------|------|----------|
| **api** | API 客户端与请求管理 | `modules/api/` |
| **application** | 应用层服务编排 | `modules/application/` |
| **browser** | Chrome CDP 浏览器自动化 | `modules/browser/` |
| **channel** | 通道路由与提供商选择 | `modules/channel/` |
| **commands** | Tauri IPC 命令桥 | `modules/commands/` |
| **config** | 配置管理服务 | `modules/config/` |
| **control_plane** | 控制面（全局状态管理） | `modules/control_plane/` |
| **harness** | 测试框架后端支持 | `modules/harness/` |
| **learning** | 学习引擎与轨迹追踪 | `modules/learning/` |
| **memory** | 记忆系统（向量+SQLite+HRR） | `modules/memory/` |
| **onboarding** | 引导流程后端逻辑 | `modules/onboarding/` |
| **projects** | 多项目管理 | `modules/projects/` |
| **provider** | LLM 提供商管理 | `modules/provider/` |
| **runtime** | 代理运行时与对话循环 | `modules/runtime/` |
| **scheduler** | 定时任务调度 | `modules/scheduler/` |
| **security** | 安全扫描与威胁检测 | `modules/security/` |
| **session** | 会话持久化（JSON） | `modules/session/` |
| **skills** | 技能发现/安装/执行/扫描 | `modules/skills/` |
| **stt** | 语音识别（SenseVoice） | `modules/stt/` |
| **system_check** | 系统环境检测 | `modules/system_check/` |
| **tools** | 工具注册与执行 | `modules/tools/` |
| **tts** | 语音合成（MOSS-TTS-Nano） | `modules/tts/` |

模块入口统一在 `src-tauri/src/modules/mod.rs` 中注册。

## 🖥️ 前端目录结构

### 业务模块 (`src/modules/`)

| 模块 | 说明 | 源码路径 |
|------|------|----------|
| **app-shell** | 应用外壳 | `modules/app-shell/` |
| **browser-viewer** | 浏览器视图 | `modules/browser-viewer/` |
| **chat** | 聊天界面 | `modules/chat/` |
| **execution-mode** | 执行模式选择 | `modules/execution-mode/` |
| **onboarding** | 引导流程 | `modules/onboarding/` |
| **settings** | 设置面板 | `modules/settings/` |
| **skills** | 技能面板 | `modules/skills/` |

### 组件库 (`src/components/`)

| 组件 | 说明 |
|------|------|
| `AgentOrb.tsx` | 代理状态指示器 |
| `GlobalSearch.tsx` | 全局搜索 |
| `ProjectRail.tsx` | 项目导航栏 |
| `WelcomeScreen.tsx` | 欢迎页面 |
| `chat/` | 聊天相关组件 |
| `memory/` | 记忆管理组件 |
| `browser/` | 浏览器相关组件 |
| `settings/` | 设置相关组件 |
| `ds/` | 设计系统组件 |
| `ui/` | shadcn/ui 基础组件 |
| `loading/` | 加载状态组件 |
| `theme/` | 主题切换组件 |

### 其他前端目录

| 目录 | 说明 |
|------|------|
| `src/stores/` | 状态管理（React hooks） |
| `src/transport/` | Tauri IPC 通信封装 |
| `src/runtime-projection/` | 后端状态投射到前端 |
| `src/boot/` | 应用启动逻辑 |
| `src/shell/` | Shell 路由层 |
| `src/lib/` | 工具库（tauri 封装等） |
| `src/styles/` | 全局样式 |

## 💾 数据存储目录

```
~/.if2ai/
├── log/                  # 日志（每日滚动）
├── memory/               # 记忆（SQLite + LanceDB + 摘要）
├── sessions/             # 会话 JSON 文件
├── projects/             # 项目元数据
├── trajectories/         # 学习轨迹
├── models/tts/           # TTS ONNX 模型
└── tts/voices/           # 声音样本
```

详细说明：[环境变量与配置](../guide/env-vars.md)

## 📦 Pack 流水线目录

```
docs/packs/
├── CHARTER.md             # 流水线规则（章程）
├── REGISTRY.md            # Pack 一览表
├── refactor/              # GFR-XXX 重构 Pack（41 个文件）
├── feature/               # FEAT-XXX 功能 Pack
└── snapshots/             # 重构基准快照
```

开发流程：

```mermaid
graph LR
    A[① PACK<br/>人写 Pack] --> B[② BUILD<br/>Agent 实现]
    B --> C[③ VERIFY<br/>./scripts/pack verify]
    C --> D[④ REVIEW<br/>./scripts/pack review]
    D --> E[⑤ COMMIT<br/>1 PR / 1 commit]
```

详细说明：[docs/packs/CHARTER.md](../../packs/CHARTER.md)

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **Bounded Context** | 模块化边界，一个目录一个职责 |
| **modules/** | 后端 `src-tauri/src/modules/`，前端 `src/modules/` |
| **commands/** | Tauri IPC 命令层，前端调用后端的桥梁 |
| **transport/** | 前端 IPC 封装，类型安全的命令调用 |
| **runtime-projection/** | 后端状态投射到前端的中间层 |
| **Pack** | 最小可审查变更单元 |
| **GFR** | God-File Refactor，大文件重构 Pack |
| **FEAT** | Feature，新功能 Pack |

## 🔗 相关资源

- [编码规范](./coding-style.md) — 开发规范与 lint 合约
- [快速开始](../guide/quick-start.md) — 安装与启动
- [环境变量与配置](../guide/env-vars.md) — 数据目录详解
- [AGENTS.md](../../../AGENTS.md) — 项目导航地图
- [CHARTER.md](../../packs/CHARTER.md) — Pack 流水线规则
