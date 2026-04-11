# If2Ai 模块边界与集成设计

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes Module Architecture  
**核心思想**: 清晰的模块边界，最小化耦合，最大化代码复用

---

## 1. 模块划分原则

### 1.1 Hermes 的模块思想

Hermes 采用**功能分片**策略：

- 每个模块有单一职责
- 模块间通过**注册表模式**通信
- 可选模块通过 `check_fn` gating
- 插件用注册表扩展核心

### 1.2 If2Ai 的 Rust Crate 系统

If2Ai 使用**工作空间 crate** 构建清晰的模块：

```
if2Ai/
├── src-tauri/
│   └── src/
│       ├── main.rs              # 入口程序
│       ├── commands/            # Tauri IPC 命令
│       │   ├── agent.rs         # Agent 相关命令
│       │   ├── tools.rs         # Tool 相关命令
│       │   ├── session.rs       # Session 相关命令
│       │   └── memory.rs        # Memory 相关命令
│       └── modules/             # 业务逻辑模块
│           ├── agent/
│           │   ├── conversation.rs
│           │   ├── prompt.rs
│           │   └── mod.rs
│           ├── provider/
│           │   ├── client.rs
│           │   ├── providers/
│           │   └── mod.rs
│           ├── tools/
│           │   ├── registry.rs
│           │   ├── executor.rs
│           │   └── mod.rs
│           ├── session/
│           │   ├── storage.rs
│           │   ├── manager.rs
│           │   └── mod.rs
│           ├── memory/
│           │   ├── soul.rs
│           │   ├── memory.rs
│           │   ├── user.rs
│           │   └── mod.rs
│           └── plugin/
│               ├── manager.rs
│               ├── loader.rs
│               └── mod.rs
│
└── rust/crates/                 # 可选：单独发布的库
    ├── api/
    ├── runtime/
    ├── tools/
    ├── commands/
    └── plugins/
```

---

## 2. 模块详细设计

### 2.1 Agent Module (agent/)

**职责**: 对话运行循环、步骤调度

**关键文件**:

- `conversation.rs` - ConversationRuntime
- `prompt.rs` - PromptBuilder
- `mod.rs` - 模块公开接口

**公开 API**:

```rust
pub struct ConversationRuntime {
    api: Arc<ProviderManager>,
    prompt_builder: PromptBuilder,
    tool_executor: ToolExecutor,
    session: SessionManager,
    config: RuntimeConfig,
}

impl ConversationRuntime {
    pub async fn run_turn(&mut self, user_message: String) -> Result<AgentResponse>;
    pub async fn run_conversation(&mut self, messages: Vec<Message>) -> Result<Summary>;
}

pub struct PromptBuilder {
    // 构建系统提示的公开接口
}

impl PromptBuilder {
    pub fn build(&mut self, context: &PromptContext) -> Result<String>;
}
```

**依赖**:

- `provider::ProviderManager` - LLM 调用
- `tools::ToolExecutor` - 工具执行
- `session::SessionManager` - 状态管理
- `types::*` - 数据类型

**被依赖者**:

- `commands::agent` - Tauri 命令
- 测试模块
- 网关模块 (Phase 2)

**内部结构**:

```
agent/
├── __init__.rs        # 模块声明
├── types.rs           # 私有类型
├── conversation.rs    # 核心循环
├── prompt.rs          # 提示构建
├── compression.rs     # 上下文压缩 (Phase 2)
└── mod.rs            # 公开接口
```

---

### 2.2 Provider Module (provider/)

**职责**: LLM 提供商管理、凭证、模型路由

**关键文件**:

- `client.rs` - ProviderClient (统一接口)
- `manager.rs` - ProviderManager (选择和路由)
- `providers/` - 各个提供商实现

**公开 API**:

```rust
pub struct ProviderManager {
    clients: Arc<DashMap<ProviderKind, Arc<ProviderClient>>>,
    router: Arc<ModelRouter>,
}

impl ProviderManager {
    pub async fn resolve_provider(
        &self,
        preferred_model: Option<&str>,
    ) -> Result<Arc<ProviderClient>>;

    pub async fn create_message(
        &self,
        request: MessageRequest,
    ) -> Result<MessageResponse>;
}

pub struct ProviderClient {
    kind: ProviderKind,
    config: ProviderConfig,
}

impl ProviderClient {
    pub async fn create_message(&self, request: MessageRequest) -> Result<MessageResponse>;
    pub async fn create_message_stream(&self, ...) -> impl Stream<Item = Result<StreamEvent>>;
}
```

**支持的提供商**:

```rust
pub enum ProviderKind {
    Anthropic { use_bedrock: bool },
    OpenAI { compatible: Option<String> },
    Grok,
    OpenRouter,
    Custom { endpoint: String },
    // Phase 2+: Google Gemini, Llama API, 本地模型
}
```

**路由策略**:

```rust
pub enum RoutingStrategy {
    Auto {
        prefer_capability: Capability,
        budget_constraint: Option<Money>,
        latency_constraint: Option<Duration>,
    },
    Static(HashMap<String, ProviderKind>),
    Adaptive { /* ... */ }
}
```

**依赖**:

- 标准 HTTP 库 (reqwest)
- OAuth 库
- JSON 序列化

**被依赖者**:

- `agent::ConversationRuntime` - 调用 LLM
- `commands::model` - 模型切换命令

**内部结构**:

```
provider/
├── mod.rs
├── client.rs              # 统一客户端
├── manager.rs             # 管理器
├── router.rs              # 路由逻辑
├── credential.rs          # 凭证管理
├── oauth.rs               # OAuth 流程
└── providers/
    ├── anthropic.rs       # Anthropic 适配
    ├── openai.rs          # OpenAI 适配
    ├── grok.rs            # Grok 适配
    ├── openrouter.rs      # OpenRouter 适配
    └── custom.rs          # 自定义端点
```

---

### 2.3 Tools Module (tools/)

**职责**: 工具注册、执行、权限、后端管理

**关键文件**:

- `registry.rs` - ToolRegistry (中央注册)
- `executor.rs` - ToolExecutor (执行引擎)
- 各个工具文件

**公开 API**:

```rust
pub struct ToolRegistry {
    tools: Arc<DashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn register(&self, name: &str, tool: Arc<dyn Tool>) -> Result<()>;
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn get_schemas(&self) -> Vec<ToolSchema>;
    pub async fn execute(&self, call: ToolCall) -> Result<ToolResult>;
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult>;
}

pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    parallelizer: ParallelExecutor,
    cacher: ToolCache,  // Phase 2
}

impl ToolExecutor {
    pub async fn execute_batch(&self, calls: Vec<ToolCall>) -> Result<Vec<ToolResult>>;
    pub async fn execute_sequential(&self, calls: Vec<ToolCall>) -> Result<Vec<ToolResult>>;
}
```

**工具组织**:

```
tools/
├── mod.rs
├── registry.rs         # 中央注册表 (import-time 注册)
├── executor.rs         # 执行引擎
├── permission.rs       # 权限检查
├── approval.rs         # 用户批准流程
│
├── terminal/           # Terminal 工具集
│   ├── bash.rs
│   ├── process.rs
│   └── mod.rs
│
├── file/               # File 工具集
│   ├── read_file.rs
│   ├── write_file.rs
│   ├── edit_file.rs
│   └── mod.rs
│
├── web/                # Web 工具集
│   ├── search.rs
│   ├── extract.rs
│   └── mod.rs
│
├── browser/            # Browser 工具集 (Phase 2)
│   ├── navigate.rs
│   ├── click.rs
│   └── mod.rs
│
├── vision/             # Vision 工具集 (Phase 2)
│   ├── analyze.rs
│   └── mod.rs
│
└── mcp/                # MCP 工具动态加载 (Phase 2)
    ├── client.rs
    └── wrapper.rs
```

**依赖**:

- `agent::types` - 数据类型
- Platform-specific 库 (tokio, std::process 等)

**被依赖者**:

- `agent::ConversationRuntime` - 执行工具
- `commands::tools` - 工具管理命令

---

### 2.4 Session Module (session/)

**职责**: 会话存储、历史、元数据

**关键文件**:

- `storage.rs` - 数据库操作
- `manager.rs` - SessionManager (业务逻辑)
- `schema.rs` - SQLite schema

**公开 API**:

```rust
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
}

impl SessionManager {
    pub async fn create_session(&self, user_id: &str, title: &str) -> Result<Session>;
    pub async fn restore_session(&self, session_id: &str) -> Result<Session>;
    pub async fn save_message(&self, session_id: &str, message: &Message) -> Result<()>;
    pub async fn list_sessions(&self, user_id: &str) -> Result<Vec<SessionSummary>>;
    pub async fn delete_session(&self, session_id: &str) -> Result<()>;
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<()>;
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;
    async fn add_message(&self, session_id: &str, message: &Message) -> Result<()>;
    async fn search(&self, user_id: &str, query: &str) -> Result<Vec<Message>>;
}

pub struct Session {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub metadata: SessionMetadata,
}
```

**依赖**:

- `sqlx` - SQLite 驱动
- `sqlite` - 数据库
- `agent::types` - 消息类型

**被依赖者**:

- `agent::ConversationRuntime` - 保存历史
- `commands::session` - 会话管理命令
- 网关 (Phase 2)

**内部结构**:

```
session/
├── mod.rs
├── types.rs            # Session, Message, 等等
├── storage.rs          # SQLiteSessionStore
├── manager.rs          # SessionManager
├── schema.rs           # 数据库 schema 和迁移
├── compression.rs      # 会话压缩 (Phase 2)
└── fts.rs              # FTS5 搜索 (Phase 2)
```

---

### 2.5 Memory Module (memory/) [Phase 2]

**职责**: SOUL/MEMORY/USER 数据管理

**公开 API**:

```rust
pub struct MemoryManager {
    soul: SoulStore,      // 永久的个性和能力
    memory: MemoryStore,  // 长期回忆
    user: UserStore,      // 用户信息建模
}

pub struct Soul {
    pub personality: String,
    pub capabilities: Vec<String>,
    pub values: Vec<String>,
}

pub struct Memory {
    pub facts: Vec<Fact>,
    pub relationships: Vec<Relationship>,
    pub skills: Vec<Skill>,
}

pub struct UserProfile {
    pub name: String,
    pub preferences: HashMap<String, String>,
    pub history: Vec<Interaction>,
}
```

**依赖**:

- `session::SessionStore` - 查询历史
- Vector DB (Phase 2) - 语义搜索

---

### 2.6 Plugin Module (plugin/)

**职责**: 插件发现、加载、注册

**公开 API**:

```rust
pub struct PluginManager {
    plugins: Arc<DashMap<String, Arc<dyn Plugin>>>,
    sources: Vec<PluginSource>,
}

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn register(&self, context: &PluginContext) -> Result<()>;
}

pub struct PluginContext {
    pub tool_registry: Arc<ToolRegistry>,
    pub hook_manager: Arc<HookManager>,
    pub cli_registry: Arc<CommandRegistry>,
}
```

**插件类型**:

1. **Tool Plugins** - 贡献工具
2. **Hook Plugins** - 注册生命周期 hook
3. **Context Engine Plugins** - 自定义压缩引擎 (单选)
4. **Memory Plugins** - 自定义记忆后端 (单选)

**发现搜索路径**:

```
1. ~/.hermes/plugins/        (用户插件)
2. ./.hermes/plugins/        (项目插件)
3. pip entry_points          (通过 pip 安装的插件)
```

---

## 3. 模块集成接口

### 3.1 Tauri Commands 作为模块网关

Tauri Commands 在 `src-tauri/src/commands/` 中定义，充当**模块公开接口**：

```rust
// commands/agent.rs
#[tauri::command]
pub async fn run_agent_turn(
    state: tauri::State<'_, AppState>,
    user_message: String,
) -> Result<AgentResponse, String> {
    // 1. 获取当前会话
    let mut session = state.session_manager.restore_session(&session_id).await?;

    // 2. 调用 agent module
    let mut runtime = state.agent_runtime.clone();
    let response = runtime.run_turn(user_message).await?;

    // 3. 保存到 session module
    state.session_manager.save_message(&session_id, &response.message).await?;

    // 4. 返回结果到前端
    Ok(response)
}
```

**模块之间通过 AppState 共享**:

```rust
pub struct AppState {
    pub agent_runtime: Arc<Mutex<ConversationRuntime>>,
    pub provider_manager: Arc<ProviderManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub memory_manager: Arc<MemoryManager>,
    pub plugin_manager: Arc<PluginManager>,
}
```

### 3.2 Types 模块作为公约

所有模块共享统一的类型定义：

```
agent/types.rs
├─ Message (role, content, tool_calls)
├─ ToolCall (function name + arguments)
├─ ToolResult
└─ AgentResponse

provider/types.rs
├─ ProviderKind
├─ MessageRequest
├─ MessageResponse
└─ StreamEvent

tools/types.rs
├─ ToolSchema
├─ ToolCall
├─ ToolResult
└─ ToolExecutionContext

session/types.rs
├─ Session
├─ SessionMetadata
└─ CostTracking

memory/types.rs
├─ Soul
├─ Memory
└─ UserProfile
```

**统一的消息格式（OpenAI 兼容）**:

```rust
pub struct Message {
    pub role: Role,  // user, assistant, system
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
}
```

### 3.3 注册表模式避免循环依赖

关键模块（tools, plugins, hooks）使用**注册表模式**：

```
导入时结构：

tools/registry.rs (no deps)
    ↑
tools/bash.rs, file.rs, ... (each -> registry.register())
    ↑
runtime/tools_discovery.rs (imports registry + all tools)
    ↑
agent/conversation.rs (uses registry)

⚠️ 循环依赖避免：
- bash.rs 不导入 agent/
- agent/ 不导入 bash.rs
- 都只依赖 registry/
```

---

## 4. 模块间通信模式

### 4.1 同步流（Agent 执行循环）

```
User Input (Tauri UI)
    │
    ▼
commands/agent.rs (run_agent_turn)
    │
    ├─ session_manager.restore_session()
    │
    ├─ agent_runtime.run_turn(user_msg)
    │  │
    │  ├─ prompt_builder.build()
    │  │   └─ memory_manager.load()
    │  │
    │  ├─ provider_manager.resolve_provider()
    │  │   └─ provider_client.create_message()
    │  │
    │  ├─ parse tool_calls
    │  │
    │  ├─ tool_executor.execute_batch()
    │  │   └─ tool_registry.dispatch()
    │  │
    │  └─ loop if tool_calls
    │
    ├─ session_manager.save_message()
    │
    └─ Return response to UI
```

### 4.2 异步事件（Plugin 系统）[Phase 2]

```
agent/ emits lifecycle events
    │
    ├─ on_conversation_start
    │   └─ plugin_manager.emit()
    │       └─ plugins/*/ listen & modify system_prompt
    │
    ├─ on_tool_call
    │   └─ plugin_manager.emit()
    │       └─ plugins/*/ validate or transform tool_call
    │
    ├─ on_tool_result
    │   └─ plugin_manager.emit()
    │       └─ plugins/*/ cache result or forward
    │
    └─ on_conversation_finish
        └─ plugin_manager.emit()
            └─ plugins/*/ save to memory
```

---

## 5. 独立成库的 Crate (可选)

某些模块可以独立发布为 crate（对标 Hermes 的可复用库）：

```
if2Ai/rust/crates/
├── api/                # Provider abstraction
│   └── Can be published: `if2ai-api`
│
├── runtime/            # Core agent runtime
│   └── Can be published: `if2ai-runtime`
│
├── tools/              # Tool registry and stdlib tools
│   └── Can be published: `if2ai-tools`
│
├── plugins/            # Plugin system
│   └── Can be published: `if2ai-plugins`
│
└── session/            # Session persistence
    └── Can be published: `if2ai-session`
```

**发布策略**:

- Phase 1: 内部使用
- Phase 2: 作为 workspace crates
- Phase 3: 发布到 crates.io (可选)

---

## 6. 测试模块边界

### 6.1 单元测试 (模块内)

```
agent/conversation.rs
  └─ #[cfg(test)] mod tests
      ├─ test_prompt_building()
      ├─ test_tool_execution()
      └─ test_retry_logic()

tools/registry.rs
  └─ #[cfg(test)] mod tests
      ├─ test_tool_registration()
      ├─ test_tool_dispatch()
      └─ test_permission_check()
```

### 6.2 集成测试 (Harness 框架)

```
harness/
├─ evaluators/
│  ├─ agent_loop_evaluator.py
│  ├─ tool_system_evaluator.py
│  └─ session_evaluator.py
│
└─ runners/
   ├─ end_to_end_runner.py
   └─ module_integration_runner.py
```

**集成测试场景**:

```python
def test_full_agent_loop():
    """测试：User Message → Prompt → Provider → Tool → Response"""
    harness.run(
        prompt="What files are in current directory?",
        expected_tools=["bash"],
        assertions=[
            assert_tool_called("bash"),
            assert_response_contains("file"),
        ]
    )
```

---

## 7. 依赖管理规则

### 7.1 导入规则

```
严格的分层：
┌─────────────────────┐
│  Entry Points       │ (可导入任何东西)
│  commands/*         │
└──────────┬──────────┘
           ↓
┌─────────────────────┐
│  Core Modules       │ (可导入 types 和其他 core)
│  agent, provider    │
│  tools, session     │
└──────────┬──────────┘
           ↓
┌─────────────────────┐
│  Types Modules      │ (只有类型，不导入其他)
│  */types.rs         │
└─────────────────────┘
```

### 7.2 禁止的导入

❌ `agent/ → gateway/` (前后端顺序)  
❌ `tools/ → agent/` (除非通过注册表)  
❌ `session/ → agent/` (circular)  
✅ 使用 `Arc<dyn Trait>` 或 `Arc<AppState>` 代替

### 7.3 版本管理

Cargo.toml 中的依赖版本要求：

```toml
[workspace]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
# 内部 crates
if2ai-api = { path = "../rust/crates/api", version = "0.1" }
if2ai-tools = { path = "../rust/crates/tools", version = "0.1" }
```

---

## 8. 模块演进路线

### Phase 1 (现在)

- ✅ types modules
- ✅ agent module (核心循环)
- ✅ provider module (多提供商)
- ✅ tools module (注册表)
- ✅ session module (存储)

### Phase 2 (2-4 周)

- ✅ memory module (SOUL/MEMORY/USER)
- ✅ plugin module 完善
- ⏳ hook system
- ⏳ compression module

### Phase 3 (4-8 周)

- ❌ gateway module (多平台)
- ❌ cron module (定时任务)
- 库发布 (crates.io)

---

## 9. 快速参考

### 添加新模块的步骤

1. **定义 Types** (`mod/types.rs`)
2. **实现 Public Trait** (`mod/lib.rs` 或 `mod/mod.rs`)
3. **实现 Internals** (各个文件)
4. **注册到 AppState** (`main.rs`)
5. **暴露 Tauri Command** (`commands/mod.rs`)
6. **编写测试** (`mod/tests.rs`)
7. **更新文档** (本文件 + 模块文档)

### 添加新 Tauri Command 的步骤

1. 实现业务逻辑（在相应的 module/）
2. 创建 Command wrapper （在 `commands/`）
3. 使用 `#[tauri::command]` 装饰
4. 在 `main.rs` 注册命令
5. 在前端调用命令

```rust
// commands/my_command.rs
#[tauri::command]
async fn my_command(
    state: tauri::State<'_, AppState>,
    param: String,
) -> Result<Response, String> {
    // 调用 module 业务逻辑
    state.module_manager.do_something(&param).await
        .map_err(|e| e.to_string())
}

// main.rs
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::my_command::my_command,
        ])
        .run(/* ... */)
}
```

---

## 参考链接

- [System Architecture Framework](./system-architecture-framework.md)
- [Agent Loop Design](./agent-loop.md)
- [Provider Resolution Design](./provider-resolution.md)
- [Tool System Design](./tool-system.md)
- [Session Persistence Design](./session-persistence.md)

---

**版本**: 1.0 | **最后更新**: 2026-04-11
