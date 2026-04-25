# If2Ai

If2Ai 是一个基于 Tauri 2 + Rust + React 的 AI Agent 桌面应用。它把聊天、工具调用、项目上下文、记忆、Persona、浏览器自动化、语音、Git 工作流和应用更新收口到一个本地优先的桌面工作台里，目标是成为老少皆宜、可恢复、可审计、可长期演进的个人 AI 协作系统。

当前版本：`0.4.4`

## 当前能力

- **Agent Chat Workspace**：项目 / 会话 / 运行中任务一体化的聊天工作台，支持流式输出、工具时间线、任务恢复、停止、权限模式和模型切换。
- **Provider & Model Routing**：支持 OpenAI-compatible provider、Ollama、本地/云端模型配置、模型能力探测、thinking 支持标记和主聊天模型设置。
- **Identity / Persona**：内置 Soul + Persona identity registry，支持全局默认和 session override，prompt planner 会把最终身份注入到系统 prompt。
- **Prompt Control Plane**：scenario、task focus、工具提示、diagnostics 和 prompt lane 统一管理，便于定位 prompt 组成问题。
- **Memory System**：工作记忆、向量召回、编译记忆、promotion / conflict policy、session/project/global scope 和 Memory Browser。
- **Runtime Projection**：聊天 UI 以 canonical runtime event / run projection 为事实源，支持历史 replay、pending permission recovery 和 resume cursor。
- **Tools & Browser**：文件、Shell、浏览器/CDP、Web fetch、Git worktree、skills hub 等工具通过 Rust backend 统一调度。
- **Voice**：TTS / STT 配置、语音 profile、消息朗读和 open-flow STT 路径。
- **Smart Browser / Jiaochang**：正在推进 Smart Browser contract 与 Jiaochang pixel agent board 能力，相关 pack 已在 `docs/packs/feature/` 中登记。
- **App Updater**：已接入 Tauri updater 基础配置、release manifest、签名 artifact、客户端状态机规划和 release workflow。

## 架构主线

当前 codebase 正处在 vNext session runtime 的迁移期。架构目标不是“UI 调一个后端命令再把流式文本贴到页面上”，而是：

```text
Session = durable event log + runtime supervisor + frontend projection
```

这意味着：

- `SessionMeta` 只保存轻量产品元数据，例如标题、项目、置顶、identity override、最近运行状态。
- `Run/Event Log` 是运行事实源，append-only，可 replay、可分页、可审计。
- `Projection` 是 UI 读模型，聊天、工具、权限、记忆、恢复状态都应从 projection 派生。
- `Session Supervisor` 负责生命周期治理，包括 active run、cancel、resume、pending permission、retry 和 reconnect。

当前已经落地的骨架包括：

- 前端 `AppShell / ContentRouter / src/api/* / runtimeProjectionStore`
- 后端 `TurnService / control_plane / runtime/event_log / runtime/history / runtime/pending_permission`
- session history replay、pending permission recovery、runtime projection chat truth cutover 的基础能力

仍在迁移中的风险点：

- `session.json`、run event log、raw stream payload、conversation slice、runtime projection 仍有部分重叠职责。
- `src/App.tsx`、`src/components/ui/chat-ui.tsx`、`src-tauri/src/modules/application/turn_service/stream_task.rs` 仍是重点拆分对象。
- 新功能应优先接入 typed API facade 和 projection read model，避免制造新的第二事实源。

更完整的系统说明见 [ARCHITECTURE.md](./ARCHITECTURE.md) 和 [vNext Session Runtime Blueprint](./docs/design-docs/if2ai-vnext-session-runtime-blueprint.md)。

## 技术栈

| 层级 | 技术 | 说明 |
| --- | --- | --- |
| Desktop | Tauri 2 | macOS / Windows / Linux 桌面壳，当前主 binary 为 `if2ai-backend` |
| Backend | Rust + Tokio | Agent runtime、provider、tools、memory、updater、browser、session lifecycle |
| Frontend | React 19 + TypeScript + Vite | App shell、chat workspace、settings、memory、skills、browser surface |
| UI | Tailwind CSS 4 + Radix UI + Lucide | 统一桌面 UI 组件与交互 |
| Storage | JSON + SQLite + LanceDB | Session/project 配置、usage、memory/vector store |
| Browser Automation | chromiumoxide / CDP | 本地浏览器会话、snapshot、Smart Browser adapter |
| Release | Tauri updater + GitHub Releases | signed updater artifact、`latest.json`、If2Ai release manifest |

## 目录结构

```text
if2Ai/
├── src/                         # React + TypeScript 前端
│   ├── api/                     # typed frontend API facades
│   ├── components/              # shared UI / chat / memory components
│   ├── modules/                 # app-shell, chat, settings, skills, jiaochang, browser...
│   ├── runtime-projection/      # canonical stream/run projection
│   ├── stores/                  # frontend state slices
│   └── App.tsx                  # current app composition entry
├── src-tauri/                   # Rust backend / Tauri host
│   ├── src/commands/            # Tauri command boundary
│   ├── src/modules/             # bounded backend contexts
│   ├── tauri.conf.json          # Tauri app / bundle / updater config
│   └── Cargo.toml               # Rust dependencies and binary config
├── docs/
│   ├── packs/                   # Pack workflow source of truth
│   ├── design-docs/             # architecture references
│   ├── product-specs/           # product specs
│   └── qa/                      # audit and remediation docs
├── scripts/                     # pack tooling, release, manifest validation, version checks
├── .github/workflows/           # release CI
└── package.json                 # frontend scripts and dependencies
```

## 快速开始

### 环境要求

- Node.js 20+（Node 18 可能可用，但当前依赖更适合 Node 20+）
- npm
- Rust stable
- macOS 需要 Xcode Command Line Tools
- Tauri 2 运行环境

### 安装依赖

```bash
npm install
```

### 启动开发版

```bash
npm run tauri:dev
```

等价命令：

```bash
npx tauri dev
```

开发 Web 前端会运行在 `http://localhost:9527`，Tauri 主窗口加载同一个 Vite dev server。

### 构建

```bash
npm run build:web
npm run build
```

`npm run build` 会触发 Tauri build，并通过 `beforeBuildCommand` 先执行前端构建。

## 常用命令

| 命令 | 用途 |
| --- | --- |
| `npm run dev` | 只启动 Vite dev server |
| `npm run tauri:dev` | 启动桌面开发版 |
| `npm run build:web` | 构建前端产物 |
| `npm run build` | 构建 Tauri 桌面应用 |
| `npm test` | 运行 Node test runner 覆盖的 TypeScript 测试 |
| `npm run check:version` | 校验 `package.json` / `Cargo.toml` / `tauri.conf.json` 版本一致 |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Rust 后端快速检查 |
| `cargo test --manifest-path src-tauri/Cargo.toml <filter> --lib` | Rust 定向测试 |
| `./scripts/pack verify <PACK-ID>` | Pack 验证入口 |
| `npm run release:manifest:validate -- <manifest> stable` | 校验 release manifest |
| `npm run release:one-click` | 本地一键 release 辅助脚本 |

## 配置和数据

常见运行时文件在用户目录下：

- `~/.if2ai/models.json`：provider / model 配置
- `~/.if2ai/prompt/control-plane.json`：prompt control / identity 默认配置
- `~/.if2ai/prompt/identity-pack.json`：自定义 Soul / Persona pack
- `~/.if2ai/updater-preferences.json`：updater 偏好
- `~/.if2ai/runs` / session 相关目录：运行日志、会话事实和恢复数据

具体路径以当前 Rust config / storage 模块为准。

## 开发流程

项目使用 Pack 流水线作为唯一开发流程入口：

```text
PACK -> BUILD -> VERIFY -> REVIEW -> COMMIT
```

关键文档：

- [AGENTS.md](./AGENTS.md)：项目导航和 hard rules
- [CLAUDE.md](./CLAUDE.md)：Pack 执行规则
- [docs/packs/CHARTER.md](./docs/packs/CHARTER.md)：Pack 流水线规范
- [docs/packs/REGISTRY.md](./docs/packs/REGISTRY.md)：当前 active / done pack 注册表

当前重点路线：

- Migration Core：canonical chat execution spine、runtime event log、projection truth、resume/recovery
- Identity Foundation：Soul / Persona / session identity / memory tagging
- Prompt Control Plane：prompt lanes、diagnostics、tool prompt catalog
- App Updater：transport、signed release artifact、client state machine
- Smart Browser：browser contract、runtime projection、browser-use backend、cloud escalation
- Jiaochang：像素化 agent board 与 runtime cockpit

## 框架边界

### Frontend

- `src/modules/app-shell/`：桌面应用壳、主导航、workspace routing。
- `src/modules/chat/`：聊天工作台入口和 chat-specific UI。
- `src/runtime-projection/`：运行事件到 UI 读模型的投影层，是 vNext UI truth 的核心。
- `src/api/`：前端 typed API facade。页面组件应优先依赖这里，而不是直接调用 raw `invoke`。
- `src/stores/`：选择指针、bootstrap 和兼容状态。长期目标是减少 transcript/runtime fact 在 store 里的重复保存。

### Backend

- `src-tauri/src/commands/`：Tauri IPC 适配层，应保持薄边界。
- `src-tauri/src/modules/application/`：应用服务层，包括 `TurnService`、prompt/memory/provider/tool orchestration。
- `src-tauri/src/modules/control_plane/`：执行前策略、上下文解析、权限准备和审计。
- `src-tauri/src/modules/runtime/`：event log、history、resume、pending permission、stream outcome 等运行时事实。
- `src-tauri/src/modules/identity/`：Soul / Persona / ResolvedIdentity / customization pack。
- `src-tauri/src/modules/memory/`：记忆存储、召回、编译、promotion 和 policy。
- `src-tauri/src/modules/provider/`：provider registry、known models、capability probing、resilience。
- `src-tauri/src/modules/tools/`：工具 registry、toolset、builtin tools 和执行输出。
- `src-tauri/src/modules/smart_browser/`：Smart Browser contract / policy / backend adapter 的新边界。

### Smart Browser Direction

Smart Browser 的设计目标是把本地 Rust CDP 浏览器、browser-use MCP backend 和未来 cloud escalation 统一到一个 If2Ai browser truth 下，而不是让 browser-use 变成第二个浏览器状态源。

参考：[Smart Browser Architecture](./docs/design-docs/smart-browser-architecture.md)

## App Updater

Updater 已经进入 Pack 化实现路径：

- [APP-UPDATER-001](./docs/packs/feature/app-updater/APP-UPDATER-001-updater-transport-release-manifest-settings-ci.md)：transport、manifest、settings state、CI gate 基础闭环
- [APP-UPDATER-002](./docs/packs/feature/app-updater/APP-UPDATER-002-release-ci-signed-artifact.md)：signed updater artifact、`latest.json`、GitHub release workflow
- [APP-UPDATER-003](./docs/packs/feature/app-updater/APP-UPDATER-003-client-state-machine-ux.md)：客户端状态机、设置页 UX、事件驱动状态

Tauri updater endpoint 当前配置在 [src-tauri/tauri.conf.json](./src-tauri/tauri.conf.json)，If2Ai release manifest 默认 URL 在 `src-tauri/src/modules/updater/mod.rs` 中定义，可通过 `IF2AI_UPDATE_MANIFEST_URL` 覆盖。

## Release

Release 相关入口：

- `.github/workflows/release.yml`
- `scripts/release-macos.sh`
- `scripts/publish-github-release.mjs`
- `scripts/release-one-click.mjs`
- `scripts/validate-release-manifest.mjs`

生产 release 必须产出签名 updater artifact 和可验证 manifest；stable channel 不接受用 checksum 冒充 signature。

## 注意事项

- 当前前端不是 Svelte，而是 React + TypeScript。
- 当前向量/记忆栈主要在本地 Rust backend 内，不是 Pinecone/Weaviate 集成。
- `src/App.tsx` 和 `src/components/ui/chat-ui.tsx` 仍然是重点拆分对象；新功能应优先落在 `src/modules/*` 和 typed API facade 中。
- 不要直接绕过 `src/api/*` facade 调用 raw Tauri command，除非 Pack 明确允许。
- 后端跨模块引用遵循 `crate::modules::*` 边界。

## License

当前仓库未声明正式许可证。如需对外发布，请先补齐 `LICENSE` 和 release/legal 文案。
