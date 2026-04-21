# 工具模块（Tools）

> If2Ai 的工具执行框架——45+ 内置工具、DashMap 注册中心与安全调度

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 面向用户的使用指南 |
| [02-implementation.md](./02-implementation.md) | 面向开发者的实现原理 |

## 📍 完整工具清单

### 文件 I/O（5 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `read_file` | 读取文件内容 | 高风险 |
| `file_write` | 写入文件 | 高风险 |
| `file_edit` | 编辑文件（搜索替换） | 高风险 |
| `glob_search` | Glob 模式文件搜索 | 高风险 |
| `content_search` | 文件内容搜索 | 高风险 |

### 终端（3 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `bash` | 执行 Bash 命令 | 高风险 |
| `REPL` | 交互式 REPL | 高风险 |
| `PowerShell` | 执行 PowerShell 命令 | 高风险 |

### 网络（4 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `web_fetch` | 抓取网页内容 | 标准 |
| `web_search` | Web 搜索 | 标准 |
| `web_research` | 深度网络研究 | 标准 |
| `http_request` | HTTP 请求 | 标准 |

### 记忆（5 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `memory_store` | 存储记忆 | 高风险 |
| `memory_recall` | 召回记忆 | 标准 |
| `memory_forget` | 删除记忆 | 高风险 |
| `memory_purge` | 批量清除记忆 | 高风险 |
| `memory_export` | 导出记忆 | 标准 |

### 技能（5 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `skill` | 执行技能 | 标准 |
| `skill_find` | 查找技能 | 标准 |
| `skill_manage` | 管理技能 | 标准 |
| `skill_search` | 搜索技能 | 标准 |
| `skill_view` | 查看技能详情 | 标准 |

### 调度（5 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `cron_add` | 添加定时任务 | 高风险 |
| `cron_list` | 列出定时任务 | 标准 |
| `cron_remove` | 删除定时任务 | 高风险 |
| `cron_run` | 手动运行任务 | 高风险 |
| `cron_runs` | 查看运行记录 | 标准 |

### 数据与实用（8 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `json_parse` | JSON 解析 | 标准 |
| `calculator` | 计算器 | 标准 |
| `todo_write` | 待办管理 | 标准 |
| `sleep` | 延迟等待 | 标准 |
| `structured_output` | 结构化输出 | 标准 |
| `tool_search` | 工具搜索 | 标准 |
| `pin_memory` | 钉选记忆 | 标准 |
| `unpin_memory` | 取消钉选 | 标准 |

### 浏览器（1 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `browser_tool` | 浏览器自动化操作 | 标准 |

### 其他（3 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `agent` | 子代理调用 | 高风险 |
| `notebook_edit` | Notebook 编辑 | 标准 |
| `send_user_message` | 发送用户消息 | 标准 |

### 辅助（3 个）

| 工具 | 说明 | 权限等级 |
|------|------|----------|
| `config` | 配置管理 | 标准 |
| `grep_search` | 正则搜索 | 高风险 |
| `web_search_config` | 搜索配置 | 标准 |

## 🏗️ 核心概念速查表

| 概念 | 说明 |
|------|------|
| **ToolRegistry** | DashMap 并发注册中心，工具注册/查询/调度 |
| **ToolEntry** | 工具条目，封装元数据 + 处理器 + 限制 |
| **ToolContext** | 工具执行上下文，携带 session/project/workdir/permissions |
| **ToolOutput** | 多模态工具输出（文本 + 图片） |
| **ToolSet** | 工具集分类（files/terminal/web/memory/scheduler） |
| **ToolExecutionBroker** | 工具执行中介，隔离会话上下文 |
| **ToolHandler** | 工具处理器闭包 `AsyncFn(Value, SharedToolContext) -> Result<String>` |
| **ToolHandlerMultimodal** | 多模态处理器 `AsyncFn(Value, SharedToolContext) -> Result<ToolOutput>` |

## 🏗️ 源码位置

| 组件 | 路径 |
|------|------|
| 模块入口 | `src-tauri/src/modules/tools/mod.rs` |
| 注册中心 | `src-tauri/src/modules/tools/registry.rs` |
| 工具上下文 | `src-tauri/src/modules/tools/context.rs` |
| 工具输出 | `src-tauri/src/modules/tools/output.rs` |
| 工具集 | `src-tauri/src/modules/tools/toolset.rs` |
| 内置工具 | `src-tauri/src/modules/tools/builtin/` |
| 执行中介 | `src-tauri/src/modules/control_plane/tool_execution_broker.rs` |

## 🔗 相关资源

- [工具系统设计文档](../../design-docs/tool-system.md)
- [工具激活](../../design-docs/tool-activation.md)
- [编码规范](../../references/coding-style-and-lint-contract.md)
