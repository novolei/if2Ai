# 环境变量与配置说明

> If2Ai 运行时配置、数据目录与特性开关的完整参考

## 📂 数据目录结构

If2Ai 的所有运行时数据存储在 `~/.if2ai/` 目录下：

```
~/.if2ai/
├── log/                  # 日志
│   └── backend.log       # 每日滚动日志文件
├── memory/               # 记忆系统
│   ├── if2ai_memory.db   # SQLite 数据库（记忆 + 会话）
│   ├── lancedb/          # LanceDB 向量索引
│   └── summaries/        # 编译记忆摘要
├── sessions/             # 会话持久化
│   └── *.json            # 各会话 JSON 文件
├── projects/             # 项目元数据
├── trajectories/         # 学习轨迹数据
├── models/               # 模型文件
│   └── tts/              # TTS ONNX 模型
│       ├── decoder.onnx
│       └── ...
└── tts/                  # 语音样本
    └── voices/           # 用户声音样本（.wav / .mp3）
```

> 💡 数据目录由 Rust `dirs` crate 解析，macOS 上默认为 `~/Library/Application Support/dev.if2ai.desktop/` 或 `~/.if2ai/`。具体路径取决于 `src-tauri/src/modules/config/` 中的配置逻辑。

## ⚙️ 运行时配置文件

### 主配置文件

路径：`~/.if2ai/memory_config.json`

此文件由 If2Ai 引导流程和设置面板自动管理，通常无需手动编辑。

### 配置结构概览

```json
{
  "version": 1,
  "providers": {
    "openai": {
      "api_key": "sk-...",
      "base_url": "https://api.openai.com/v1",
      "models": ["gpt-4o", "gpt-4o-mini"]
    },
    "anthropic": {
      "api_key": "sk-ant-...",
      "base_url": "https://api.anthropic.com",
      "models": ["claude-sonnet-4-20250514"]
    }
  },
  "model_selection": {
    "default": "gpt-4o",
    "reasoning": "claude-sonnet-4-20250514",
    "compression": "gemini-2.0-flash"
  },
  "channel_routing": {
    "default_channel": "code"
  }
}
```

源码参考：
- 配置类型定义：`src-tauri/src/modules/config/`
- 配置服务：`src-tauri/src/modules/config/`（`ConfigService`、`AppConfig`、`ProviderConfig`、`ModelSelection`）

## 🔧 特性开关

| 环境变量 | 类型 | 默认值 | 说明 |
|----------|------|--------|------|
| `IF2AI_HRR_ENABLED` | bool | `false` | 启用 HRR 全息表示记忆（实验性） |
| `IF2AI_LOG_LEVEL` | string | `info` | 日志级别：`trace` / `debug` / `info` / `warn` / `error` |
| `IF2AI_BROWSER_PATH` | string | 自动检测 | Chrome/Chromium 可执行文件路径 |
| `IF2AI_DEV_PORT` | number | `9527` | Vite 开发服务器端口（覆盖 `vite.config.ts`） |

> ⚠️ 特性开关通过环境变量设置，在启动 If2Ai 前导出。例如：
> ```bash
> export IF2AI_HRR_ENABLED=true
> npm run tauri:dev
> ```

## 📝 日志配置

### 日志路径

```
~/.if2ai/log/backend.log
```

### 日志特性

| 特性 | 说明 |
|------|------|
| **滚动策略** | 每日滚动，自动创建新文件 |
| **日志级别** | 默认 `info`，可通过 `IF2AI_LOG_LEVEL` 调整 |
| **实现** | `tracing` + `tracing-subscriber` + `tracing-appender` |
| **格式** | 时间戳 + 级别 + 模块 + 消息 |

源码参考：`src-tauri/src/main.rs`（日志初始化）

### 日志级别速查

| 级别 | 用途 |
|------|------|
| `trace` | 最详细，函数级调用追踪 |
| `debug` | 调试信息，模块级状态 |
| `info` | 正常运行信息（默认） |
| `warn` | 警告，可恢复的问题 |
| `error` | 错误，需要关注的问题 |

## 🤖 模型配置

### LLM 提供商

| 提供商 | 配置字段 | 说明 |
|--------|----------|------|
| **OpenAI** | `providers.openai` | GPT-4o、GPT-4o-mini、o1 系列 |
| **Anthropic** | `providers.anthropic` | Claude Sonnet、Haiku 系列 |
| **OpenRouter** | `providers.openrouter` | 统一网关，多提供商模型 |
| **Gemini** | `model_selection.compression` | 用于上下文压缩（非对话） |

源码参考：
- 提供商管理：`src-tauri/src/modules/provider/`
- API 路由：`src-tauri/src/modules/api/`
- 通道配置：`src-tauri/src/modules/channel/`

### TTS 模型

| 模型 | 路径 | 说明 |
|------|------|------|
| **MOSS-TTS-Nano** | `~/.if2ai/models/tts/` | ONNX 格式，本地推理 |
| 声音样本 | `~/.if2ai/tts/voices/` | `.wav` / `.mp3` 参考音频 |

源码参考：`src-tauri/src/modules/tts/`

### STT 模型

| 模型 | 说明 |
|------|------|
| **SenseVoice** | ONNX 格式，OpenFlow ASR 引擎 |

源码参考：`src-tauri/src/modules/stt/`

## ⚠️ 差距分析：与 cc-haha 配置对比

| 维度 | cc-haha | If2Ai | 差距 |
|------|---------|-------|------|
| **配置模板** | `.env.example`（53 行，含多提供商示例） | 无 | ⚠️ 缺少 `.env.example` 配置模板 |
| **环境变量** | 完善的环境变量文档 | 仅 4 个特性开关 | ⚠️ 文档不足 |
| **多提供商示例** | 详细的提供商配置示例 | 仅引导流程 | ⚠️ 缺少手动配置指南 |
| **GUI 配置** | 无 GUI | 设置面板 + 引导流程 | ✅ If2Ai 更优 |
| **数据目录** | 文档化 | 部分文档化 | ⚠️ 需要补充 |

### 增强建议

1. **添加 `.env.example`** — 包含所有可配置环境变量和提供商示例
2. **完善配置文档** — 补充手动编辑配置文件的场景说明
3. **配置验证** — 在设置面板中添加配置有效性检测

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **AppConfig** | 应用总配置结构，包含提供商、模型选择、通道路由 |
| **ProviderConfig** | 单个提供商配置：API Key、Base URL、模型列表 |
| **ModelSelection** | 模型选择策略：default / reasoning / compression |
| **ChannelRouting** | 通道路由：根据请求类型自动选择提供商 |
| **ConfigService** | 配置管理服务：加载、保存、验证配置 |
| **IF2AI_HRR_ENABLED** | HRR 全息记忆特性开关（实验性） |
| **滚动日志** | 每日自动滚动，`tracing-appender` 实现 |

## 🔗 相关资源

- [快速开始指南](./quick-start.md) — 安装与启动步骤
- [常见问题解答](./faq.md) — 配置相关问题
- [Memory 模块](../memory/) — 记忆系统详细配置
- [Voice 模块](../voice/) — 语音模型配置
- [配置源码](../../../src-tauri/src/modules/config/) — `ConfigService` 实现
