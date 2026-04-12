# 工具系统 (Tools/Runtime) Baseline 设计报告

> 深度扫描时间：2026-04-12
> 源码路径：
> - `/Users/ryanliu/Documents/IfAI/if2Ai/rust/crates/tools/src/lib.rs`
> - `/Users/ryanliu/Documents/IfAI/if2Ai/rust/crates/runtime/src/`
> - `/Users/ryanliu/Documents/IfAI/if2Ai/rust/crates/api/src/`

---

## 一、整体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                     tools/src/lib.rs                                  │
│  GlobalToolRegistry                                                 │
│    ├── mvp_tool_specs()        — 17 个内置工具规格                  │
│    └── with_plugin_tools()     — 插件工具聚合                       │
│  execute_tool() 分派函数                                           │
└────────────────────────────────┬────────────────────────────────────┘
                                 │ tool_registry.execute_tool()
┌────────────────────────────────▼────────────────────────────────────┐
│                   runtime/src/                                       │
│  conversation.rs    — Agent Loop 引擎                                │
│  permissions.rs    — PermissionPolicy / PermissionMode              │
│  sandbox.rs        — Linux namespace 沙箱                           │
│  bash.rs           — execute_bash()                                 │
│  file_ops.rs       — read/write/edit/glob/grep                     │
│  json.rs           — 手写 JSON 解析器                               │
│  mcp_stdio.rs      — McpServerManager (Stdio MCP 生命周期)          │
│  mcp_client.rs     — McpClientTransport 抽象                        │
│  mcp.rs            — 工具名称标准化 / URL 处理                      │
└────────────────────────────────┬────────────────────────────────────┘
                                 │ HTTP / Stdio
┌────────────────────────────────▼────────────────────────────────────┐
│                   api/src/                                           │
│  providers/claw_provider.rs      — Anthropic API                     │
│  providers/openai_compat.rs     — OpenAI / xAI 兼容                 │
│  types.rs                       — 统一类型 (MessageRequest/Response) │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 二、工具注册 (`tools/src/lib.rs`)

### 2.1 `GlobalToolRegistry`

```rust
pub struct GlobalToolRegistry {
    plugin_tools: Vec<PluginTool>,
}

impl GlobalToolRegistry {
    pub fn with_plugin_tools(plugins: Vec<PluginTool>) -> Result<Self, ToolError> {
        // 去重检查：同一工具名不能由多个插件提供
    }
}
```

### 2.2 `mvp_tool_specs()` — 17 个内置工具

| 工具名 | 权限级别 | 对应 runtime 函数 |
|--------|---------|------------------|
| `bash` | `DangerFullAccess` | `runtime::bash::execute_bash()` |
| `read_file` | `ReadOnly` | `runtime::file_ops::read_file()` |
| `write_file` | `WorkspaceWrite` | `runtime::file_ops::write_file()` |
| `edit_file` | `WorkspaceWrite` | `runtime::file_ops::edit_file()` |
| `glob_search` | `ReadOnly` | `runtime::file_ops::glob_search()` |
| `grep_search` | `ReadOnly` | `runtime::file_ops::grep_search()` |
| `WebFetch` | `ReadOnly` | — |
| `WebSearch` | `ReadOnly` | — |
| `TodoWrite` | `WorkspaceWrite` | — |
| `Skill` | `ReadOnly` | — |
| `Agent` | `DangerFullAccess` | — |
| `ToolSearch` | `ReadOnly` | — |
| `NotebookEdit` | `WorkspaceWrite` | — |
| `Sleep` | `ReadOnly` | — |
| `SendUserMessage` / `Brief` | `ReadOnly` | — |
| `Config` | `ReadOnly` | — |
| `StructuredOutput` | `ReadOnly` | — |
| `REPL` | `ReadOnly` | — |
| `PowerShell` | `ReadOnly` | — |

### 2.3 `execute_tool()` 分派表

```rust
pub fn execute_tool(name: &str, input: &Value) -> Result<String, String> {
    match name {
        "bash"          => from_value::<BashCommandInput>(input).and_then(run_bash),
        "read_file"     => from_value::<ReadFileInput>(input).and_then(run_read_file),
        "write_file"    => from_value::<WriteFileInput>(input).and_then(run_write_file),
        "edit_file"     => from_value::<EditFileInput>(input).and_then(run_edit_file),
        "glob_search"   => from_value::<GlobSearchInput>(input).and_then(run_glob_search),
        "grep_search"   => from_value::<GrepSearchInput>(input).and_then(run_grep_search),
        // ... 其余工具
    }
}
```

---

## 三、权限系统 (`runtime/permissions.rs`)

### 3.1 `PermissionMode` 枚举

```rust
pub enum PermissionMode {
    ReadOnly,        // 0 — 只读
    WorkspaceWrite,  // 1 — 可读写 workdir
    DangerFullAccess, // 2 — 无限制（危险）
    Prompt,          // 3 — 写入前需用户确认
    Allow,           // 4 — 无限制
}
```

层级比较（`PartialOrd` 实现）：`ReadOnly < WorkspaceWrite < DangerFullAccess < Prompt < Allow`

### 3.2 `PermissionPolicy`

```rust
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_requirements: BTreeMap<String, PermissionMode>, // 各工具所需权限
}
```

默认工具需求：未注册的工具默认需要 `DangerFullAccess`。

### 3.3 `authorize()` 权限检查算法

```rust
pub fn authorize(&self, tool_name: &str, input: &str,
                 prompter: Option<&mut dyn PermissionPrompter>) -> PermissionOutcome
```

检查流程：
1. `active_mode == Allow` 或 `active_mode >= required_mode` → 直接放行
2. `active_mode == Prompt` 或 `active_mode == WorkspaceWrite && required_mode == DangerFullAccess` → 提示用户
3. 其余情况 → 拒绝

**关键设计**：无 prompter 时（如非交互模式）直接拒绝，不会静默降级。

### 3.4 配置值 → PermissionMode 映射 (config.rs:625)

| 配置值 | 解析为 |
|--------|--------|
| `"default"`, `"plan"`, `"read-only"` | `ReadOnly` |
| `"acceptEdits"`, `"auto"`, `"workspace-write"` | `WorkspaceWrite` |
| `"dontAsk"`, `"danger-full-access"` | `DangerFullAccess` |

---

## 四、沙箱隔离 (`runtime/sandbox.rs`)

### 4.1 `FilesystemIsolationMode`

```rust
pub enum FilesystemIsolationMode {
    Off,
    WorkspaceOnly,  // 默认，仅限 workdir
    AllowList,
}
```

### 4.2 `SandboxConfig`

```rust
pub struct SandboxConfig {
    pub enabled: Option<bool>,
    pub namespace_restrictions: Option<bool>,  // Linux namespace 隔离
    pub network_isolation: Option<bool>,      // 网络隔离
    pub filesystem_mode: Option<FilesystemIsolationMode>,
    pub allowed_mounts: Vec<String>,         // 白名单挂载点
}
```

### 4.3 Linux namespace 沙箱 (`build_linux_sandbox_command`)

**激活条件**：`target_os = linux` 且 `namespace_active || network_active`

```rust
Command::new("unshare")
    .args([
        "--user", "--map-root-user",  // 用户 namespace
        "--mount", "--ipc", "--pid", "--uts", "--fork",  // 资源 namespace
        "--net",  // 网络隔离（可选）
    ])
    .arg("-lc")
    .arg(command)
    .current_dir(cwd)
    .env("HOME", cwd.join(".sandbox-home"))    // 重定向 HOME
    .env("TMPDIR", cwd.join(".sandbox-tmp"))   // 重定向 TMPDIR
```

**非 Linux 平台**：沙箱命令构建返回 `None`，仅设置 `HOME`/`TMPDIR` 环境变量。

### 4.4 容器环境检测

检测信号：
- `/.dockerenv` / `/run/.containerenv` 文件
- 环境变量 `container`/`docker`/`podman`/`kubernetes_service_host`
- `/proc/1/cgroup` 含 `docker`/`containerd`/`kubepods`/`libpod`

---

## 五、Bash 执行 (`runtime/bash.rs`)

### 5.1 `BashCommandInput`

```rust
pub struct BashCommandInput {
    pub command: String,
    pub timeout: Option<u64>,
    pub description: Option<String>,
    pub run_in_background: Option<bool>,
    pub dangerously_disable_sandbox: Option<bool>,
    pub namespace_restrictions: Option<bool>,
    pub isolate_network: Option<bool>,
    pub filesystem_mode: Option<FilesystemIsolationMode>,
    pub allowed_mounts: Option<Vec<String>>,
}
```

### 5.2 执行路径

```
execute_bash(input)
  └─ env::current_dir()  → workdir
  └─ sandbox_status_for_input()  → SandboxStatus
        ├─ run_in_background == true
        │     └─ Command::spawn()  → 后台执行
        └─ else
              └─ execute_bash_async()
                    └─ tokio::process::Command
                          ├─ Linux + namespace active → unshare 命令包装
                          └─ else → sh -lc <command>
```

### 5.3 权限检查位置

**`bash.rs` 本身不执行权限检查**。权限检查发生在 `conversation.rs` 的 `run_turn()` 中：

```rust
let permission_outcome = permission_policy.authorize(&tool_name, &input, prompter);
if !matches!(permission_outcome, PermissionOutcome::Allow) {
    // 生成 error tool_result
}
```

---

## 六、文件操作 (`runtime/file_ops.rs`)

### 6.1 工具函数

| 函数 | 功能 | 返回类型 |
|------|------|---------|
| `read_file(path, offset, limit)` | 读取文件指定行范围 | `ReadFileOutput` |
| `write_file(path, content)` | 创建或更新文件 | `WriteFileOutput` |
| `edit_file(path, old, new, replace_all)` | 替换文件中文本 | `EditFileOutput` |
| `glob_search(pattern, path)` | glob 模式匹配 | `GlobSearchOutput` |
| `grep_search(input)` | 正则搜索文件 | `GrepSearchOutput` |

### 6.2 路径规范化

```rust
fn normalize_path(path: &str) -> io::Result<PathBuf> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join(path)  // 相对路径基于 cwd
    };
    candidate.canonicalize()  // 解析符号链接，获取绝对路径
}
```

**设计要点**：
- `normalize_path` — 文件必须存在，否则返回 `NotFound`
- `normalize_path_allow_missing` — 允许文件不存在，但父目录必须可访问
- `canonicalize()` 防止路径穿越攻击

### 6.3 权限检查

**`file_ops.rs` 本身无权限检查逻辑**，权限检查由上层 (`GlobalToolRegistry` / `conversation.rs`) 负责。

---

## 七、MCP 协议支持 (`runtime/mcp_stdio.rs`)

### 7.1 `McpServerManager` — 多服务器生命周期管理

```rust
pub struct McpServerManager {
    servers: BTreeMap<String, ManagedMcpServer>,    // 已管理服务器
    unsupported_servers: Vec<UnsupportedMcpServer>, // 非 Stdio 服务器
    tool_index: BTreeMap<String, ToolRoute>,       // qualified_name → route
}
```

### 7.2 工具发现流程

```
discover_tools()
  └─ for each Stdio server:
        ├─ ensure_server_ready()  — 启动进程 + initialize()
        └─ loop tools/list (分页 next_cursor)
              └─ 生成 qualified_name: mcp__<server>__<tool>
```

**关键约束**：仅支持 **Stdio 传输**的 MCP 服务器。其他传输（SSE/HTTP/WS）被标记为 unsupported。

### 7.3 进程复用

discovery 和 call 之间**进程保持运行**（不重启），避免重复初始化开销。

### 7.4 JSON-RPC 帧协议 (Content-Length)

```
Content-Length: <payload_bytes>\r\n\r\n<JSON body>
```

---

## 八、API Provider 抽象 (`api/src/providers/`)

### 8.1 `Provider` trait

```rust
pub trait Provider {
    type Stream;
    fn send_message<'a>(&'a self, request: &'a MessageRequest)
        -> ProviderFuture<'a, MessageResponse>;
    fn stream_message<'a>(&'a self, request: &'a MessageRequest)
        -> ProviderFuture<'a, Self::Stream>;
}
```

### 8.2 `ProviderClient` 路由

```rust
pub enum ProviderClient {
    ClawApi(ClawApiClient),    // Anthropic
    Xai(OpenAiCompatClient),   // xAI
    OpenAi(OpenAiCompatClient),// OpenAI
}
```

### 8.3 模型路由表

| 模型别名 | Provider | 认证环境变量 |
|---------|----------|-------------|
| `opus` / `sonnet` / `haiku` + Claude 系列 | ClawApi | `ANTHROPIC_API_KEY` |
| `grok*` 系列 | Xai | `XAI_API_KEY` |
| 其他（fallback） | OpenAI | `OPENAI_API_KEY` |

---

## 九、上下文压缩 (`runtime/compact.rs`)

### 9.1 压缩触发条件

```rust
fn should_compact(session: &Session, config: CompactionConfig) -> bool {
    compactable_count > preserve_recent_messages  // 保留最近 N 条
        && estimated_tokens >= max_estimated_tokens  // 默认 10k tokens
}
```

### 9.2 压缩流程

```
compact_session()
  1. 提取已有摘要（如果存在）
  2. 保留最近 4 条消息
  3. summarize_messages(被移除消息) → XML 摘要
  4. merge_compact_summaries(旧+新摘要)
  5. 用 System 消息替换被压缩的消息
```

---

## 十、System Prompt 构建 (`runtime/prompt.rs`)

`SystemPromptBuilder::build()` 输出的 section 顺序：

1. **Intro**（带/不带 Output Style）
2. **Output Style**（可选）
3. **System**（通用指南）
4. **`__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`**（动态内容分界线）
5. **Environment context**（OS、cwd、date、model）
6. **Project context**（git status/diff、instruction files）
7. **Runtime config**（从 settings.json 加载）
8. **append_sections**（额外自定义 section）

---

## 十一、关键文件索引

| 文件 | 职责 |
|------|------|
| `tools/src/lib.rs` | GlobalToolRegistry、execute_tool 分派、mvp_tool_specs |
| `runtime/permissions.rs` | PermissionPolicy、PermissionMode、authorize |
| `runtime/sandbox.rs` | Linux namespace 沙箱、SandboxConfig |
| `runtime/bash.rs` | execute_bash、BashCommandInput |
| `runtime/file_ops.rs` | read/write/edit/glob/grep |
| `runtime/json.rs` | 手写递归下降 JSON 解析器 |
| `runtime/conversation.rs` | ConversationRuntime、Agent Loop |
| `runtime/compact.rs` | 上下文压缩策略 |
| `runtime/prompt.rs` | SystemPromptBuilder |
| `runtime/mcp_stdio.rs` | McpServerManager、Stdio MCP 协议 |
| `runtime/mcp_client.rs` | McpClientTransport 抽象 |
| `runtime/mcp.rs` | MCP 工具名称标准化 |
| `runtime/session.rs` | Session、ConversationMessage、ContentBlock |
| `runtime/config.rs` | ConfigLoader、RuntimeConfig |
| `api/providers/claw_provider.rs` | Anthropic API 客户端 |
| `api/providers/openai_compat.rs` | OpenAI/xAI 兼容客户端 |
| `api/types.rs` | MessageRequest、MessageResponse、AssistantEvent |
| `plugins/src/lib.rs` | PluginManager、PluginRegistry |
