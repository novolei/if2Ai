# CLAW-CLI Baseline 框架设计报告

> 深度扫描时间：2026-04-12
> 源码路径：`/Users/ryanliu/Documents/IfAI/if2Ai/rust/crates/claw-cli/`
> 依赖 Crate：`runtime/`、`api/`、`tools/`、`plugins/`

---

## 目录结构

```
claw-cli/src/
├── main.rs      (176KB) — CLI 主入口，REPL 循环，LiveCli 应用构建
├── app.rs       (12KB)  — 简化版 CliApp（非 REPL 模式）
├── args.rs      (2.7KB) — 命令行参数定义（clap）
├── init.rs      (15KB)  — 仓库初始化逻辑（CLAW.md / .claw.json 生成）
├── render.rs    (25KB)  — 终端渲染器（Markdown / ANSI / 流式输出）
└── input.rs     (37KB)  — LineEditor（Vim 风格多模式编辑器）
```

---

## 一、CLI 入口与动作分发 (`main.rs`)

### 1.1 `main()` 极简入口

```rust
fn main() {
    if let Err(error) = run() {
        eprintln!("{}", render_cli_error(&error.to_string()));
        std::process::exit(1);
    }
}
```

### 1.2 `run()` — CLI 动作分发中心

```rust
fn run() -> Result<(), Box<dyn std::error::Error>> {
    match parse_args(&env::args().skip(1).collect::<Vec<_>>())? {
        CliAction::Init             => run_init()?,
        CliAction::Repl { .. }      => run_repl(...)?
        CliAction::Prompt { .. }    => LiveCli::new()?.run_turn_with_output(...)?
        CliAction::ResumeSession { .. } => resume_session(...)?,
        CliAction::Login            => run_login()?,
        CliAction::Logout           => run_logout()?,
        CliAction::DumpManifests    => dump_manifests(),
        CliAction::BootstrapPlan    => print_bootstrap_plan(),
        CliAction::Agents { .. }    => LiveCli::print_agents(...)?,
        CliAction::Skills { .. }    => LiveCli::print_skills(...)?,
        CliAction::PrintSystemPrompt { .. } => print_system_prompt(...),
        CliAction::Version          => print_version(),
        CliAction::Help             => print_help(),
    }
    Ok(())
}
```

### 1.3 `CliAction` 枚举

```rust
enum CliAction {
    Init,
    Repl { model: String, allowed_tools: Option<AllowedToolSet>, permission_mode: PermissionMode },
    Prompt { prompt: String, model: String, output_format: CliOutputFormat,
              allowed_tools: Option<AllowedToolSet>, permission_mode: PermissionMode },
    ResumeSession { session_path: PathBuf, commands: Vec<String> },
    Login, Logout,
    DumpManifests,
    BootstrapPlan,
    Agents { args: Option<String> },
    Skills { args: Option<String> },
    PrintSystemPrompt { cwd: PathBuf, date: String },
    Version,
    Help,
}
```

### 1.4 模型别名解析

| 用户输入 | 解析为 |
|---------|--------|
| `opus` | `claude-opus-4-6` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5-20251213` |
| `grok-mini` | `grok-3-mini` |

默认模型：`claude-opus-4-6`

---

## 二、LiveCli — 核心应用结构

### 2.1 `LiveCli` 定义 (main.rs:1067)

```rust
struct LiveCli {
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    system_prompt: Vec<String>,
    runtime: ConversationRuntime<DefaultRuntimeClient, CliToolExecutor>,
    session: SessionHandle,
}
```

### 2.2 `LiveCli::new()` 构造函数 (main.rs:1077)

```rust
fn new(model, enable_tools, allowed_tools, permission_mode) -> Result<Self> {
    let system_prompt = build_system_prompt()?;
    let session = create_managed_session_handle()?;
    let runtime = build_runtime(
        Session::new(), model.clone(), system_prompt.clone(),
        enable_tools, true, allowed_tools.clone(), permission_mode, None,
    )?;
    cli.persist_session()?;  // 初始化后立即持久化
    Ok(cli)
}
```

### 2.3 `build_runtime()` — Runtime 构建工厂 (main.rs:2973)

```rust
fn build_runtime(session, model, system_prompt, enable_tools, emit_output,
                 allowed_tools, permission_mode, progress_reporter)
    -> Result<ConversationRuntime<DefaultRuntimeClient, CliToolExecutor>>
{
    let (feature_config, tool_registry) = build_runtime_plugin_state()?;
    Ok(ConversationRuntime::new_with_features(
        session,
        DefaultRuntimeClient::new(
            model, enable_tools, emit_output,
            allowed_tools.clone(), tool_registry.clone(), progress_reporter
        )?,
        CliToolExecutor::new(allowed_tools.clone(), emit_output, tool_registry.clone()),
        permission_policy(permission_mode, &tool_registry),
        system_prompt,
        feature_config,
    ))
}
```

**依赖注入链：**
```
build_runtime_plugin_state()
  → ConfigLoader::default_for(cwd)
  → RuntimeConfig::load()
  → PluginManager::new()
  → GlobalToolRegistry::with_plugin_tools()
  → (feature_config, tool_registry)
```

---

## 三、DefaultRuntimeClient — LLM API 客户端 (main.rs:3048)

```rust
struct DefaultRuntimeClient {
    runtime: tokio::runtime::Runtime,    // Tokio 异步运行时
    client: ClawApiClient,                 // HTTP API 客户端（Anthropic）
    model: String,
    enable_tools: bool,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
    progress_reporter: Option<InternalPromptProgressReporter>,
}
```

实现 `ApiClient` trait：
```rust
impl ApiClient for DefaultRuntimeClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // 构建 MessageRequest → client.stream_message() → 解析 SSE → 返回 AssistantEvent
    }
}
```

**API 调用链：**
```
stream()
  → MessageRequest { system_prompt, messages }
  → client.stream_message()
  → HTTP POST /v1/messages (Anthropic)
  → SSE 响应流
  → AssistantEvent::{TextDelta, ToolUse, Usage, MessageStop}
```

---

## 四、CliToolExecutor — 工具执行器 (main.rs:3858)

```rust
struct CliToolExecutor {
    renderer: TerminalRenderer,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
}
```

实现 `ToolExecutor` trait：
```rust
impl ToolExecutor for CliToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        // 调用 GlobalToolRegistry.execute_tool()
    }
}
```

---

## 五、REPL 交互循环 (main.rs:1013)

```rust
fn run_repl(model, allowed_tools, permission_mode) -> Result<()> {
    let mut cli = LiveCli::new(model, true, allowed_tools, permission_mode)?;
    let mut editor = input::LineEditor::new("> ", slash_command_completion_candidates());
    println!("{}", cli.startup_banner());

    loop {
        match editor.read_line()? {
            input::ReadOutcome::Submit(input) => {
                let trimmed = input.trim();
                if trimmed.is_empty() { continue; }
                if matches!(trimmed, "/exit" | "/quit") {
                    cli.persist_session()?;
                    break;
                }
                if let Some(command) = SlashCommand::parse(trimmed) {
                    if cli.handle_repl_command(command)? {
                        cli.persist_session()?;
                    }
                    continue;
                }
                editor.push_history(&input);
                cli.run_turn(&input)?;
            }
            input::ReadOutcome::Cancel => {},
            input::ReadOutcome::Exit => { cli.persist_session()?; break; },
        }
    }
    Ok(())
}
```

---

## 六、Agent 完整 Loop 流程

```
用户输入 (LineEditor)
    │
    ▼
cli.run_turn(&input)        [main.rs:1169]
    │
    ▼
runtime.run_turn()          [runtime/conversation.rs]
    │
    ├─ session.messages.push(user_message)
    │
    ▼ LOOP
    ├─ ApiRequest { system_prompt, messages }
    ├─ client.stream() → AssistantEvent 流
    │       TextDelta → 累积文本
    │       ToolUse   → 收集 pending_tool_uses
    │       Usage     → 记录 token
    │       MessageStop → 结束
    │
    ├─ build_assistant_message() → ConversationMessage
    │
    ├─ if pending_tool_uses.is_empty():
    │       └─ break (turn 结束)
    │
    ├─ else for each tool_use:
    │       ├─ permission_policy.authorize()  [permissions.rs]
    │       │       ├─ Allow   → 继续
    │       │       └─ Deny    → 生成 error tool_result
    │       │
    │       ├─ PreToolUse Hook
    │       │
    │       ├─ tool_executor.execute(tool_name, input)
    │       │       └─ tool_registry.execute_tool()
    │       │               └─ match name:
    │       │                   "bash"       → runtime::bash::execute_bash()
    │       │                   "read_file" → runtime::file_ops::read_file()
    │       │                   ...
    │       │
    │       ├─ PostToolUse Hook
    │       │
    │       └─ tool_result 入 session.messages
    │       └─ LOOP 继续
    │
    ▼
TurnSummary { assistant_messages, tool_results, iterations, usage }
    │
    ▼
render.rs TerminalRenderer 渲染输出（Markdown / 流式）
    │
    ▼
persist_session() — 会话持久化到 ~/.claw/sessions/
```

---

## 七、配置加载层级 (config.rs)

| Source | Path |
|--------|------|
| User (legacy) | `~/.claw.json` |
| User | `~/.claw/settings.json` |
| Project | `{cwd}/.claw.json` |
| Project | `{cwd}/.claw/settings.json` |
| Local | `{cwd}/.claw/settings.local.json` |

**Deep merge**：所有存在的配置文件按优先级递归合并（BTreeMap）。

---

## 八、Slash 命令系统

`main.rs` 中的 `SlashCommand` 定义（仅本地命令）：

```rust
enum SlashCommand {
    Help,
    Status,
    Compact,
    Unknown(String),
}
```

解析：`SlashCommand::parse(trimmed)` 以 `/` 前缀匹配。

---

## 九、关键文件索引

| 文件 | 职责 |
|------|------|
| `main.rs` | CLI 入口、LiveCli、REPL 循环、build_runtime 工厂 |
| `app.rs` | 简化版 CliApp（非 REPL） |
| `args.rs` | clap 参数定义 |
| `init.rs` | 仓库初始化（CLAW.md 生成） |
| `render.rs` | Markdown/ANSI 渲染器、流式 Spinner |
| `input.rs` | Vim 风格 LineEditor |
| `runtime/conversation.rs` | Agent Loop 状态机 |
| `runtime/session.rs` | Session / ConversationMessage / ContentBlock |
| `runtime/permissions.rs` | PermissionPolicy / PermissionMode |
| `runtime/bash.rs` | Bash 命令执行 |
| `runtime/sandbox.rs` | Linux namespace 沙箱 |
| `runtime/file_ops.rs` | 文件读写/glob/grep |
| `runtime/config.rs` | ConfigLoader / RuntimeConfig |
| `api/client.rs` | ProviderClient 路由层 |
| `api/providers/claw_provider.rs` | Anthropic API 客户端 |
| `api/providers/openai_compat.rs` | OpenAI/xAI 兼容客户端 |
| `tools/src/lib.rs` | GlobalToolRegistry / MVP 工具规格 |
| `plugins/src/lib.rs` | 插件系统 |
