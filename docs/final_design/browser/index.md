# Browser 浏览器自动化

> If2Ai 的浏览器控制子系统 —— 基于 Chrome CDP 协议的 AI 驱动浏览器自动化。

## 📚 文档目录

| 文件 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | 浏览器控制命令、DOM 快照、会话管理 |
| [02-implementation.md](./02-implementation.md) | 开发者 | CDP 协议集成、页面交互 API、会话隔离 |

## 📍 核心概念速查

| 概念 | 说明 |
|------|------|
| **BrowserSession** | 单个浏览器会话，封装一个 chromiumoxide Browser + Page 对 |
| **BrowserRegistry** | 全局会话注册表，`DashMap<session_id, BrowserSession>` |
| **CDP** | Chrome DevTools Protocol，Chrome 远程调试协议 |
| **AXTree Snapshot** | 无障碍树快照，供 LLM 理解页面结构 |
| **ColdState** | 冷状态持久化，跨应用重启保持 URL |
| **BrowserProfileMode** | 浏览器配置模式：临时 / 持久化 |
| **ActionLog** | 操作日志，供 AI 错误恢复 |
| **Takeover** | 用户接管模式，暂停 AI 操作避免冲突 |

## 🏗️ 源码位置

```
src-tauri/src/modules/browser/
├── mod.rs              # 模块入口 + 公开导出
├── chrome_finder.rs    # Chrome 二进制发现（macOS/Linux/Windows）
├── cold_state.rs       # 冷状态持久化（跨重启 URL 保存）
├── errors.rs           # BrowserError 错误类型
├── events.rs           # Tauri 事件载荷 + 发射辅助
├── profile.rs          # 浏览器配置管理（临时/持久化/列表/删除）
├── registry.rs         # ⭐ 全局会话注册表（DashMap + ColdState）
├── session.rs          # ⭐ 单会话生命周期 + 页面操作
└── snapshot.rs         # ⭐ DOM AXTree 快照脚本
```

## 🔗 相关链接

- [Security 模块](../security/) — 浏览器操作路径验证
- [Skills 模块](../skills/) — 浏览器技能可调用 BrowserSession
- [cc-haha 浏览器差距分析](./02-implementation.md#️-与-cc-haha-差距分析)
