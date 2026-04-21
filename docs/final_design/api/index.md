# LLM 提供商系统（API）

> If2Ai 的 LLM 提供商抽象层：统一管理多模型接入与流式通信

## 📚 子文档目录

| 文档 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | 提供商配置、模型选择、流式输出 |
| [02-implementation.md](./02-implementation.md) | 开发者 | 架构实现、SSE 解析、认证、差距分析 |

## 📍 核心概念速查表

| 概念 | 说明 | 源码位置 |
|------|------|----------|
| Provider trait | 统一提供商接口 | `api/providers/mod.rs` |
| ClawApiClient | Anthropic Claude 提供商 | `api/providers/claw_provider.rs` |
| OpenAiCompatClient | OpenAI 兼容提供商 | `api/providers/openai_compat.rs` |
| ProviderManager | 提供商注册与路由 | `api/providers/manager.rs` |
| SseParser | SSE 流式解析器 | `api/sse.rs` |
| AuthSource | 认证方式枚举 | `api/providers/claw_provider.rs` |
| ProviderKind | 提供商类型枚举 | `api/providers/mod.rs` |
| MessageRequest | 统一请求模型 | `api/types.rs` |
| StreamEvent | 统一流式事件 | `api/types.rs` |

## 🔗 相关链接

- [桌面架构](../desktop/02-architecture.md) — Provider 在 AppState 中的位置
- [学习系统](../learning/) — Learning 使用 API 进行反思生成
- [前端模块](../frontend/) — 前端流式渲染使用 API 事件
