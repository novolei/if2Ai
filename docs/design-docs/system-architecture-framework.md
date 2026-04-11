# If2Ai 系统架构框架 (Hermes 对齐)

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes 整体架构 (architecture.md)  
**核心思想**: 一个 Agent 类，多个入口点，松耦合的模块设计

---

## 1. 系统概览

### 1.1 Hermes 架构的 3 层结构

```
┌─────────────────────────────────────────────────────────┐
│              Entry Points Layer                         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐      │
│  │   CLI      │  │  Gateway   │  │    ACP     │      │
│  │ (Tauri)    │  │ (Messaging)│  │  (IDE)     │      │
│  └──────┬─────┘  └──────┬─────┘  └──────┬─────┘      │
└─────────┼─────────────────┼─────────────┼─────────────┘
          │                 │             │
┌─────────┴─────────────────┴─────────────┴─────────────┐
│              Core Agent Loop Layer                    │
│  ┌──────────────────────────────────────────────────┐ │
│  │  AIAgent (run_conversation)                      │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐    │ │
│  │  │ Prompt   │ │ Provider │ │ Tool         │    │ │
│  │  │ Builder  │ │ Router   │ │ Dispatcher   │    │ │
│  │  └────────┬─┘ └────────┬─┘ └────────────┬─┘    │ │
│  │           └────────────┴────────────────┘      │ │
│  └──────────────────────────────────────────────────┘ │
│                       ▼                               │
│  ┌──────────────────────────────────────────────────┐ │
│  │  Compression & Caching                           │ │
│  │  Context Compression │ Prompt Caching            │ │
│  └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
          │                 │             │
┌─────────┴─────────────────┴─────────────┴─────────────┐
│              Infrastructure Layer                     │
│  ┌───────────┐  ┌──────────┐  ┌────────────────┐    │
│  │ Session   │  │ Tool     │  │ Plugin         │    │
│  │ Storage   │  │ Backends │  │ System         │    │
│  │           │  │          │  │                │    │
│  │ - SQLite  │  │ - Term   │  │ - Memory       │    │
│  │ - FTS5    │  │ - Web    │  │ - Context Eng  │    │
│  │ - Lineage │  │ - MCP    │  │ - Tools        │    │
│  └───────────┘  └──────────┘  └────────────────┘    │
└──────────────────────────────────────────────────────┘
```

### 1.2 If2Ai 的对应设计

```
┌──────────────────────────────────────────────────────────┐
│  Entry Points (Tauri 驱动)                              │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │ Tauri App    │  │ Tauri API    │  │ LSP Bridge   │ │
│  │ (Desktop UI) │  │ (JSON-RPC)   │  │ (IDE)        │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
└─────────┼──────────────────┼──────────────────┼────────┘
          │                  │                  │
┌─────────┴──────────────────┴──────────────────┴────────┐
│  Tauri Commands (IPC Layer)                            │
│  Rust 后端接口                                         │
│                                                        │
│  #[tauri::command]                                     │
│  async fn run_agent_turn(msg: String) -> Result {...}  │
└──────────────────────┬───────────────────────────────┘
                       │
┌──────────────────────┴───────────────────────────────┐
│  AIAgent Core (Rust)                                 │
│                                                      │
│  ┌─────────────────────────────────────────────┐   │
│  │ ConversationRuntime                         │   │
│  │ pub async fn run_turn(&mut self, ...) {...} │   │
│  └──────┬──────────┬──────────┬────────────────┘   │
│         │          │          │                    │
│    ┌────▼─┐  ┌─────▼──┐  ┌───▼─────┐             │
│    │Prompt│  │Provider│  │Tool     │             │
│    │Bldnr │  │Manager │  │Executor │             │
│    └──────┘  └────────┘  └─────────┘             │
│                                                   │
│  ┌──────────────────┐                            │
│  │Context Compressor│(Phase 2)                   │
│  └──────────────────┘                            │
└──────────────────┬───────────────────────────────┘
                   │
┌──────────────────┴───────────────────────────────┐
│  Infrastructure (Support Layers)                 │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │Sessions  │  │Tool      │  │Plugins       │  │
│  │          │  │Backends  │  │              │  │
│  │SQLite    │  │Terminal  │  │Memory        │  │
│  │FTS5      │  │File Ops  │  │MCP           │  │
│  │History   │  │Web       │  │Custom        │  │
│  │Cost      │  │Vision    │  │              │  │
│  │          │  │MCP       │  │              │  │
│  └──────────┘  └──────────┘  └──────────────┘  │
└──────────────────────────────────────────────────┘
```

---

## 2. 9 个核心子系统设计

### 2.1 Agent Loop (核心)

**职责**: 对话循环、消息处理、工具执行、错误处理、重试

**关键类**:
```rust
pub struct ConversationRuntime {
    api: Arc<ProviderManager>,
    prompt_builder: PromptBuilder,
    tool_executor: ToolExecutor,
    session: SessionManager,
    compression: ContextCompressor,
    config: RuntimeConfig,
    max_iterations: usize,
}

impl ConversationRuntime {
    pub async fn run_turn(&mut self, user_message: String) -> Result<AssistantEvent>
    
    // 内部步骤：
    // 1. build_prompt() - 构建系统提示
    // 2. call_llm() - 调用 LLM
    // 3. parse_response() - 解析响应
    // 4a. if tool_calls -> execute_tools() -> loop back
    // 4b. else -> return final_response()
}
```

**对标**: `run_agent.py` (9,200 行)  
**If2Ai**: `crates/runtime/src/conversation.rs` (500+ 行) ✅ 80%

**设计文档**: [agent-loop.md](./agent-loop.md)

---

### 2.2 Prompt System (提示工程)

**职责**: 系统提示构建、动态内容注入、提示优化、缓存

**组件**:
```rust
pub struct PromptBuilder {
    agent_definition: AgentDefinition,      // 角色和能力
    memory_manager: MemoryManager,          // SOUL/MEMORY/USER
    context_files: Vec<ContextFile>,        // AGENTS.md, .hermes.md
    tool_schemas: Vec<ToolSchema>,          // 工具定义
    model_metadata: ModelMetadata,          // 模型特定的说明
}

impl PromptBuilder {
    pub fn build_system_prompt(&mut self) -> String {
        // Step 1: 基础角色定义（从 SOUL.md）
        // Step 2: 能力和工具说明
        // Step 3: 用户信息（从 USER.md）
        // Step 4: 会话记忆（从 MEMORY.md）
        // Step 5: 特殊指令（从 .hermes.md）
        // Step 6: 输出格式规范
    }
}

pub struct PromptCaching {
    // Anthropic 提示缓存支持
    cache_breakpoints: Vec<usize>,
    cache_token_savings: Arc<Mutex<f64>>,
}

pub struct ContextCompressor {
    // LLM-based 上下文压缩
    summarization_model: String,
    compression_trigger: usize,  // token 数
    keep_recent_turns: usize,
}
```

**对标**: `agent/prompt_builder.py` + `context_compressor.py` (500 行)  
**If2Ai**: `crates/runtime/src/prompt.rs` (部分实现)  
**完成度**: ⏳ 30%

**所需增强**:
- [ ] 完整的 SOUL/MEMORY/USER 支持
- [ ] 提示板式模板（role-specific）
- [ ] Anthropic 提示缓存集成
- [ ] LLM-based 压缩

---

### 2.3 Provider Resolution (多提供商)

**职责**: LLM 提供商选择、凭证管理、模型路由、故障转移

**关键类**:
```rust
pub struct ProviderManager {
    clients: Arc<DashMap<ProviderKind, Arc<ProviderClient>>>,
    router: Arc<ModelRouter>,
    credential_pool: Arc<CredentialPool>,
    fallback_order: Vec<ProviderKind>,
}

pub enum ProviderKind {
    Anthropic { use_bedrock: bool },
    OpenAI { compatible_endpoint: Option<String> },
    Grok,
    OpenRouter,
    Custom { endpoint: String },
    // + 13 more providers
}

pub struct ModelRouter {
    // 三种路由策略
    strategy: RoutingStrategy,
}

pub enum RoutingStrategy {
    Auto {
        // 基于能力自动选择
        prefer_cheaper: bool,
        prefer_faster: bool,
        required_capabilities: Vec<Capability>,
    },
    Static(HashMap<String, ProviderKind>),  // 预配置规则
    Adaptive {
        // 基于历史性能
        metrics: PerformanceMetrics,
    }
}
```

**对标**: `runtime_provider.py` + `anthropic_adapter.py` (1,200 行)  
**If2Ai**: `crates/api/src/client.rs` + `providers/` (1,200 行) ✅ 70%

**设计文档**: [provider-resolution.md](./provider-resolution.md)

---

### 2.4 Tool System (工具框架)

**职责**: 工具注册、执行调度、权限检查、后端管理

**关键类**:
```rust
pub struct ToolRegistry {
    tools: Arc<DashMap<String, Arc<dyn Tool>>>,
    toolsets: HashMap<String, ToolSet>,
    permissions: PermissionPolicy,
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
    cacher: ToolCache,
    approval_manager: ToolApprovalManager,
}

impl ToolExecutor {
    pub async fn execute_batch(
        &self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolResult>> {
        // 支持并发执行（如果工具间无依赖）
    }
}

pub struct ToolBackendManager {
    terminal: TerminalBackendSelector,  // 6 backends: local, docker, ssh, modal, daytona, singularity
    web: WebBackendSelector,            // 4 backends: direct, firecrawl, apify, playwright
    browser: BrowserAutomation,         // 11 tools: navigate, click, type, screenshot, etc.
    vision: VisionAnalysis,             // Claude Vision, GPT-4V
    mcp: MCPManager,                    // Dynamic MCP tools
}
```

**对标**: `model_tools.py` + `tools/registry.py` (800 行)  
**If2Ai**: `crates/tools/src/` (600 行) ✅ 90%

**设计文档**: [tool-system.md](./tool-system.md)

**工具数量对比**:
- Hermes: 47 个工具 + 20 个工具集
- If2Ai 计划: 20+ 个工具 + 10+ 个工具集 (Phase 1+2)

---

### 2.5 Session Persistence (会话存储)

**职责**: 对话历史持久化、元数据管理、搜索、压缩追踪

**数据库设计**:
```sql
-- 核心会话表
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    title TEXT,
    model TEXT,
    total_turns INTEGER,
    total_input_tokens INTEGER,
    total_output_tokens INTEGER,
    total_cost_usd REAL,
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    parent_session_id TEXT,  -- 用于分支和压缩
    compression_count INTEGER DEFAULT 0,
    status TEXT,  -- ACTIVE, ARCHIVED, DELETED
    INDEX idx_user_created (user_id, created_at DESC),
    INDEX idx_parent (parent_session_id)
);

-- 消息表
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_number INTEGER,
    role TEXT,  -- user, assistant, system
    content TEXT,
    tool_calls JSONB,
    tool_results JSONB,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd REAL,
    created_at TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    INDEX idx_session_turn (session_id, turn_number)
);

-- 压缩历史表（记录什么被压缩了）
CREATE TABLE compression_history (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    original_turn_start INTEGER,
    original_turn_end INTEGER,
    compressed_summary TEXT,
    compression_ratio REAL,
    created_at TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

-- FTS5 全文搜索
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    session_id UNINDEXED
);
```

**关键特性**:
- ✅ SQLite + FTS5
- ✅ 会话线性追踪（parent/child）
- ✅ 成本计算
- ✅ 压缩历史记录
- ⏳ 向量搜索（Phase 2）

**对标**: `hermes_state.py` (600 行)  
**If2Ai**: `crates/runtime/src/session.rs` (800 行) ✅ 80%

**设计文档**: [session-persistence.md](./session-persistence.md)

---

### 2.6 Messaging Gateway (多平台)

**职责**: 多平台消息路由、用户授权、会话隔离、hook 系统

**架构**:
```rust
pub struct GatewayRunner {
    message_bus: Arc<MessageBus>,
    session_router: SessionRouter,
    auth_manager: AuthManager,
    platform_adapters: Arc<DashMap<String, Arc<dyn PlatformAdapter>>>,
    hook_manager: HookManager,
    cron_scheduler: CronScheduler,
}

pub trait PlatformAdapter: Send + Sync {
    async fn on_message(&self, event: PlatformEvent) -> Result<()>;
    async fn send_message(&self, to: &str, message: &str) -> Result<()>;
    // 实现: Telegram, Discord, Slack, WhatsApp, Signal, etc.
}

pub enum PlatformEvent {
    MessageReceived { platform: String, user_id: String, text: String },
    MessageEdited { ... },
    ReactionAdded { ... },
}

pub struct HookManager {
    // Hook 生命周期事件
    on_agent_start,
    on_tool_call,
    on_tool_result,
    on_agent_finish,
    // 用户可以注册自定义 hook
}
```

**对标**: `gateway/run.py` (7,500 行)  
**If2Ai**: ❌ 0% (Phase 3 功能)

**平台支持计划**:
- Phase 1: 内部 API/JSON-RPC
- Phase 2: Slack, Discord
- Phase 3: Telegram, WhatsApp, 等等

---

### 2.7 Plugin System (插件)

**职责**: 工具扩展、memory provider、context engine、CLI 命令

**发现机制**:
```rust
pub struct PluginManager {
    sources: vec![
        PluginSource::UserHome("~/.hermes/plugins/"),
        PluginSource::Project("./.hermes/plugins/"),
        PluginSource::PipEntryPoints,
    ],
}

pub trait Plugin {
    fn register_tools(&self, registry: &mut ToolRegistry);
    fn register_hooks(&self, hooks: &mut HookManager);
    fn register_commands(&self, cli: &mut CLI);
}

// 特殊化的插件类型（单选）
pub trait MemoryProvider: Send + Sync {
    async fn load_memory(&self, user_id: &str) -> Result<Memory>;
    async fn save_memory(&self, user_id: &str, memory: &Memory) -> Result<()>;
}

pub trait ContextEngine: Send + Sync {
    async fn compress(&self, messages: &[Message]) -> Result<String>;
    async fn retrieve(&self, query: &str) -> Result<Vec<Message>>;
}
```

**对标**: `hermes_cli/plugins.py` + 特化的 memory/context_engine (400 行)  
**If2Ai**: `crates/plugins/src/` (400+ 行) ✅ 85%

**设计文档**: 计划中

---

### 2.8 Cron Scheduler (定时任务)

**职责**: 定时 Agent 任务、job 管理、平台交付

**数据格式**:
```json
{
  "jobs": [
    {
      "id": "daily-standup",
      "schedule": "0 9 * * *",
      "prompt": "Generate a standup summary",
      "attached_skills": ["memory", "summarization"],
      "delivery": {
        "platform": "slack",
        "channel": "#engineering"
      },
      "next_run": "2026-04-12T09:00:00Z",
      "last_run": "2026-04-11T09:00:00Z"
    }
  ]
}
```

**对标**: `cron/scheduler.py` (200+ 行)  
**If2Ai**: ❌ 0% (Phase 3 功能)

---

### 2.9 ACP/IDE Integration (编辑器)

**职责**: VS Code/Zed/JetBrains 集成、stdio JSON-RPC

**协议**:
```rust
pub struct ACPServer {
    // JSON-RPC 2.0 over stdio
    // 实现 LSP 兼容的接口
}

pub enum ACPRequest {
    Initialize { client_info: ClientInfo },
    Complete { context: EditorContext },
    ExecuteCommand { command: String, args: Vec<Value> },
    CancelRequest { id: u32 },
}

pub enum ACPNotification {
    Did_Open { document: Document },
    Did_Change { document: Document, changes: Vec<Change> },
    Did_Close { document: Document },
}
```

**对标**: `acp_adapter/` (200+ 行)  
**If2Ai**: `crates/lsp/src/` (基础实现) ✅ 70%

---

## 3. 数据流和交互

### 3.1 CLI Session 流程

```
User Input
    ↓
(Tauri App) HermesCLI.process_input()
    ↓
ConversationRuntime.run_turn(user_message)
    ↓
├─ PromptBuilder.build_system_prompt()
├─ ProviderManager.select_provider(model_preference)
├─ LLM API Call (chat_completions / anthropic_messages)
└─ Parse response
    ├─ if tool_calls:
    │  └─ ToolExecutor.execute_batch(tool_calls)
    │     ├─ ToolRegistry.dispatch()
    │     ├─ Execute in sandboxed environment
    │     └─ Collect results
    │  └─ LOOP: run_turn_with_tool_results()
    │
    └─ else:
       └─ Final response ready
           ├─ Display in UI
           └─ SessionManager.save_message()
               ├─ Save to SQLite
               ├─ Update cost_tracking
                └─ Create FTS5 index
```

### 3.2 Gateway Message 流程

```
Platform Event (Telegram, Discord, etc.)
    ↓
PlatformAdapter.on_message()
    ↓
GatewayRunner._handle_message()
    ├─ AuthManager.authorize_user()
    ├─ SessionRouter.resolve_session_key()
    ├─ Load session history from SQLite
    └─ ConversationRuntime.run_conversation()
        └─ [Same as CLI flow]
            └─ Collect response
                ├─ PlatformAdapter.send_message()
                └─ HookManager.on_agent_finish()
```

### 3.3 Cron Job 流程

```
Scheduler tick
    ↓
Load due jobs from jobs.json
    ↓
for each job:
    ├─ Create fresh ConversationRuntime (no history)
    ├─ Inject attached skills as system prompt
    ├─ Run job prompt
    ├─ ConversationRuntime.run_conversation()
    └─ Deliver response to target platform
        ├─ Update job.last_run
        ├─ Calculate job.next_run
        └─ Persist in jobs.json
```

---

## 4. 设计原则

### 4.1 Hermes 的 6 大原则

| 原则 | 含义 | If2Ai 适配 |
|-----|------|----------|
| **Prompt 稳定性** | 系统提示在对话中途不变 | ✅ 实现 |
| **可观测性** | 每个工具调用对用户可见 | ✅ 实现 |
| **可中断性** | API 和工具可被中断 | ⏳ 基础实现 |
| **平台无关** | 一个 AIAgent 类服务所有入口点 | ✅ 设计 |
| **松耦合** | 可选系统使用注册表模式 | ✅ 实现 |
| **配置隔离** | 每个配置独立的 HERMES_HOME | ✅ 实现 |

### 4.2 If2Ai 的额外原则

- **类型安全**: Rust 编译时检查
- **异步优先**: Tokio-based 并发
- **Tauri 优先**: 原生桌面应用
- **可测试性**: Harness 框架集成
- **文档即代码**: 设计文档与代码同步

---

## 5. 模块依赖关系

### 5.1 导入依赖链

```
tools/registry.rs  (no deps - 所有工具导入它)
    ↑
tools/*.rs  (每个工具在导入时调用 registry.register())
    ↑
runtime/tools_dispatch.rs  (导入 tools/registry + 触发发现)
    ↑
runtime/conversation.rs, commands/*.rs, gateway/*.rs
    (都依赖 tools_dispatch)
```

**意义**: 工具注册在 import time 发生，在任何 agent 实例创建前

### 5.2 模块导入顺序

```
1. Types Layer (no dependencies)
   ├─ api/types.rs
   ├─ runtime/types.rs
   └─ tools/types.rs

2. Registry Layer (imports types)
   ├─ tools/registry.rs
   ├─ runtime/session_store.rs
   └─ runtime/provider_registry.rs

3. Implementation Layer (imports registry)
   ├─ agent/prompt_builder.rs
   ├─ agent/provider_manager.rs
   ├─ agent/tool_executor.rs
   └─ tools/*.rs

4. Core Layer (imports implementations)
   └─ runtime/conversation.rs

5. Entry Points (imports core)
   ├─ commands/agent.rs
   ├─ commands/tools.rs
   └─ commands/session.rs
```

---

## 6. Hermes vs If2Ai 实现对比

| 层 | Hermes | If2Ai Phase 1 | If2Ai Phase 2 | If2Ai Phase 3 |
|----|--------|--------------|---------------|--------------|
| **Entry Points** | 4 | 1 (Tauri) | 2 (+ API) | 3 (+ LSP) |
| **Agent Loop** | ✅ 9.2K | ✅ 500 | ✅ 1K | ✅ 1.5K |
| **Prompt System** | ✅ 500 | ⏳ 150 | ✅ 400 | ✅ 500 |
| **Provider** | ✅ 1.2K | ✅ 800 | ✅ 1K | ✅ 1.5K |
| **Tools** | ✅ 800 | ✅ 600 | ✅ 1K | ✅ 1.5K |
| **Session** | ✅ 600 | ✅ 400 | ✅ 600 | ✅ 800 |
| **Gateway** | ✅ 7.5K | ❌ 0 | ⏳ 2K | ✅ 5K |
| **Plugin** | ✅ 400 | ✅ 400 | ✅ 500 | ✅ 600 |
| **Cron** | ✅ 200 | ❌ 0 | ❌ 0 | ✅ 300 |
| **ACP** | ✅ 200 | ✅ 100 | ✅ 200 | ✅ 300 |
| **Total** | **~21K** | **~3K** | **~7K** | **~12K** |

---

## 7. If2Ai 的实现路线

### Phase 1: 核心循环 (现在 - 2 周)
✅ Agent Loop  
✅ Provider System  
✅ Tool System  
✅ Session Storage  
⏳ Prompt System 增强

**目标**: 可用的本地 Agent 应用

### Phase 2: 高级特性 (2-4 周)
- Prompt System 完整
- Memory System (SOUL/MEMORY/USER)
- Context Compression
- Error Handling
- Testing Framework
- Gateway 基础 (Slack/Discord)

**目标**: 功能完整、可靠、有记忆的 Agent

### Phase 3: 扩展系统 (4-8 周)
- Gateway 完整 (14+ 平台)
- Cron Scheduler
- Plugin 市场
- IDE 集成完善
- 性能优化

**目标**: 生产级、可扩展的 Agent 平台

---

## 8. 快速导航

### 按子系统快速查找
- **Agent Loop** → [agent-loop.md](./agent-loop.md)
- **Provider** → [provider-resolution.md](./provider-resolution.md)
- **Tools** → [tool-system.md](./tool-system.md)
- **Session** → [session-persistence.md](./session-persistence.md)
- **Prompt** → [prompt-builder.md](./prompt-builder.md) (待)
- **Gateway** → [messaging-gateway.md](./messaging-gateway.md) (待)
- **Plugin** → [plugin-architecture.md](./plugin-architecture.md) (待)
- **Error** → [error-handling.md](./error-handling.md) (待)
- **Testing** → [testing-strategy.md](./testing-strategy.md) (待)

### 按职能快速查找
- **新人** → 本文档 → 选择子系统
- **Agent 开发** → [agent-loop.md](./agent-loop.md) + [provider-resolution.md](./provider-resolution.md)
- **工具开发** → [tool-system.md](./tool-system.md)
- **前端/UI** → 查看 `src/` 和 Tauri commands
- **测试** → [testing-strategy.md](./testing-strategy.md) (待)
- **架构** → 本文档 + [DESIGN_DOCS_INDEX.md](./DESIGN_DOCS_INDEX.md)

---

## 参考资源

- **Hermes 官方**: https://hermes-agent.nousresearch.com/docs/developer-guide/architecture
- **If2Ai 架构**: [ARCHITECTURE.md](../../ARCHITECTURE.md)
- **If2Ai 设计原则**: [DESIGN.md](../../DESIGN.md)
- **所有设计文档**: [DESIGN_DOCS_INDEX.md](./DESIGN_DOCS_INDEX.md)

---

**下一步**: 
1. 阅读具体的子系统设计文档
2. 查看代码实现
3. 运行现有的测试
4. 开始实现 Phase 1 剩余部分

**版本**: 1.0 | **最后更新**: 2026-04-11
