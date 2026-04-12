# If2Ai Product Backlog

> **Executor 读取规则**：本文件是功能需求和任务清单的唯一来源。  
> 每个 backlog 条目包含 `design_ref`（必读设计文档）、`impl_targets`（实现文件）、`acceptance`（验收标准）。  
> 与 `docs/exec-plans/active/` 中的 YAML slice 配合使用。

**最后同步**: 2026-04-11 | **同步来源**: docs/design-docs/ 全部 18 份设计文档

---

## 阶段路线图

```
Phase 1 (当前) → Phase 2 → Phase 3
核心框架         高级特性     扩展生态

P1: 修复编译 + ConversationRuntime + Provider + Tools + Session + Tauri IPC
P2: ContextCompression + ErrorHandling + PromptBuilder + MemorySystem + MessagingGateway (Telegram + Discord)
P3: AgentSelfImprovement + IDE/LSP Bridge + JSON-RPC API Server
```

---

## Phase 1 — 核心框架 (CURRENT)

> 对应 exec-plan: `docs/exec-plans/active/phase-1-foundation.yaml`

### BL-101 修复 Rust 编译错误

| 字段             | 值                                                                                                                    |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- |
| **优先级**       | P0 — 阻塞后续所有任务                                                                                                 |
| **状态**         | 🔴 pending                                                                                                            |
| **design_ref**   | `COMPILATION_ERRORS_CHECKLIST.md`                                                                                     |
| **impl_targets** | `src-tauri/src/modules/api/providers/claw_provider.rs`, `openai_compat.rs`, `src-tauri/src/modules/runtime/config.rs` |

**任务描述**：修复 52 个 Rust 编译错误，使 `cargo check -p if2ai-backend` 零错误通过。

**已知错误类型及修复方式**：

```rust
// E0433: 模块路径错误
// 旧: use runtime::OAuthTokenSet;
// 新: use crate::modules::runtime::OAuthTokenSet;

// E0015: const fn 调用非 const 函数
// claw_provider.rs:690, openai_compat.rs:910
// 删除 is_retryable() 函数签名中的 const 关键字

// E0282: 类型推断失败
// 为闭包添加显式类型注解
// .filter(|value: &String| !value.is_empty())
```

**验收标准**：

- `cargo check -p if2ai-backend` 无错误
- `cargo check -p if2ai-backend` 无 warning（或 warning 数量不增加）

---

### BL-102 ConversationRuntime 核心循环

| 字段             | 值                                                                          |
| ---------------- | --------------------------------------------------------------------------- |
| **优先级**       | P0                                                                          |
| **状态**         | 🔴 pending                                                                  |
| **design_ref**   | `docs/design-docs/agent-loop.md` + `docs/design-docs/agent-orchestrator.md` |
| **impl_targets** | `src-tauri/src/modules/runtime/conversation.rs`                             |

**任务描述**：实现 `ConversationRuntime` 核心对话循环，这是整个 Agent 系统的心脏。

**必须实现的接口**（来自 agent-loop.md）：

```rust
pub struct ConversationRuntime {
    api: Arc<ApiClient>,
    system_prompt: String,
    tools: ToolExecutor,
    session: Session,
    config: RuntimeConfig,
    max_turns: usize,
}

impl ConversationRuntime {
    /// 核心 Agent 循环 — 单轮次执行
    pub async fn run_turn(
        &mut self,
        user_message: String,
    ) -> Result<TurnResult>;

    /// 流式输出版本
    pub async fn run_turn_streaming(
        &mut self,
        user_message: String,
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<TurnResult>;
}
```

**循环状态机**（来自 agent-orchestrator.md）：

1. 加载/初始化对话历史
2. 构建系统提示 (`PromptBuilder`)
3. 调用 LLM API（流式）
4. 解析响应，提取 tool_calls
5. 若有工具调用 → 执行工具 → 回到步骤 3
6. 若无工具调用 → 循环结束
7. 检查迭代预算（`max_turns`）
8. 保存 session

**验收标准**：

- `cargo test -p if2ai-backend runtime::conversation` 全部通过
- 能处理无工具调用的单轮对话
- 能处理 1+ 次工具调用的多轮对话
- 超出 `max_turns` 时返回 `Err(AgentError::BudgetExhausted)`

---

### BL-103 ProviderManager — 多 LLM 支持

| 字段             | 值                                                                            |
| ---------------- | ----------------------------------------------------------------------------- |
| **优先级**       | P0                                                                            |
| **状态**         | 🔴 pending                                                                    |
| **design_ref**   | `docs/design-docs/provider-resolution.md` + `docs/design-docs/llm-routing.md` |
| **impl_targets** | `src-tauri/src/modules/api/providers/` (全部文件)                             |

**任务描述**：实现统一的 LLM 提供商管理，支持 OpenAI / Anthropic / 国内提供商，含故障转移。

**必须实现的 Provider 接口**（来自 provider-resolution.md）：

```rust
pub trait LLMProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        config: &RequestConfig,
    ) -> Result<LLMResponse>;

    async fn stream_complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        config: &RequestConfig,
        tx: Sender<StreamChunk>,
    ) -> Result<()>;

    fn name(&self) -> &str;
    fn is_available(&self) -> bool;  // 检查 API key 是否存在
}
```

**支持的提供商**（来自 llm-routing.md）：

| 提供商      | 环境变量             | 优先级 |
| ----------- | -------------------- | ------ |
| OpenAI      | `OPENAI_API_KEY`     | 高     |
| Anthropic   | `ANTHROPIC_API_KEY`  | 高     |
| OpenRouter  | `OPENROUTER_API_KEY` | 中     |
| Kimi        | `KIMI_API_KEY`       | 中     |
| Deepseek    | `DEEPSEEK_API_KEY`   | 中     |
| LocalOllama | (无需密钥)           | 低     |

**故障转移逻辑**：

- 按优先级选择第一个 `is_available()` 的提供商
- 请求失败时自动尝试下一个提供商
- 所有提供商失败时返回 `Err(AgentError::AllProvidersExhausted)`

**验收标准**：

- `cargo test -p if2ai-backend api::providers` 全部通过
- 至少 OpenAI + Anthropic 两个提供商正确实现
- Mock provider 用于测试

---

### BL-104 ToolRegistry + 基础工具集

| 字段             | 值                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------ |
| **优先级**       | P0                                                                                   |
| **状态**         | 🔴 pending                                                                           |
| **design_ref**   | `docs/design-docs/tool-system.md`                                                    |
| **impl_targets** | `src-tauri/src/modules/tools/registry.rs`, `src-tauri/src/modules/tools/executor.rs` |

**任务描述**：实现工具注册表和执行器，并提供 3 个基础工具。

**必须实现的核心结构**（来自 tool-system.md）：

```rust
pub struct ToolEntry {
    pub name: String,
    pub toolset: String,
    pub description: String,
    pub schema: JsonValue,         // OpenAI function calling format
    pub handler: Arc<ToolHandler>,
    pub check_fn: Option<fn() -> bool>,
    pub timeout_secs: Option<u32>,
    pub disabled: bool,
}

pub struct ToolRegistry {
    tools: Arc<DashMap<String, ToolEntry>>,
}

impl ToolRegistry {
    pub fn register(&self, entry: ToolEntry) -> Result<()>;
    pub fn get(&self, name: &str) -> Option<ToolEntry>;
    pub fn get_definitions(&self, toolsets: &[String]) -> Vec<ToolDefinition>;
    pub fn execute(&self, name: &str, input: JsonValue) -> Result<ToolResult>;
}
```

**必须提供的基础工具**：

| 工具名       | toolset    | 描述                      |
| ------------ | ---------- | ------------------------- |
| `bash`       | `terminal` | 执行 shell 命令（沙箱化） |
| `read_file`  | `files`    | 读取文件内容              |
| `write_file` | `files`    | 写入文件内容              |

**验收标准**：

- `cargo test -p if2ai-backend tools::` 全部通过
- 三个基础工具可以正常注册和调用
- 超时工具调用能正确返回错误

---

### BL-105 SessionManager — 会话持久化

| 字段             | 值                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------- |
| **优先级**       | P0                                                                                     |
| **状态**         | 🔴 pending                                                                             |
| **design_ref**   | `docs/design-docs/session-persistence.md` + `docs/design-docs/data-schema.md`          |
| **impl_targets** | `src-tauri/src/modules/session/manager.rs`, `src-tauri/src/modules/session/storage.rs` |

**任务描述**：基于 SQLite 实现会话和消息的持久化存储。

**数据库 Schema**（来自 data-schema.md）：

```sql
-- 会话表
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,           -- UUID
    title TEXT,
    status TEXT DEFAULT 'active',  -- 'active' | 'completed' | 'error'
    created_at INTEGER NOT NULL,   -- Unix timestamp
    updated_at INTEGER NOT NULL,
    total_tokens INTEGER DEFAULT 0,
    total_cost REAL DEFAULT 0.0
);

-- 消息表
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,            -- 'user' | 'assistant' | 'tool' | 'system'
    content TEXT NOT NULL,
    is_summary INTEGER DEFAULT 0,  -- 压缩摘要标记
    tokens INTEGER,
    created_at INTEGER NOT NULL
);
```

**必须实现的接口**：

```rust
pub struct SessionManager {
    db: Arc<SqlitePool>,
}

impl SessionManager {
    pub async fn create_session(&self, title: Option<String>) -> Result<Session>;
    pub async fn get_session(&self, id: &str) -> Result<Option<Session>>;
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
    pub async fn save_messages(&self, session_id: &str, messages: &[Message]) -> Result<()>;
    pub async fn load_messages(&self, session_id: &str) -> Result<Vec<Message>>;
    pub async fn delete_session(&self, id: &str) -> Result<()>;
}
```

**验收标准**：

- `cargo test -p if2ai-backend session::` 全部通过
- 会话 CRUD 操作正确工作
- 消息序列化/反序列化一致

---

### BL-106 Tauri Commands — IPC 网关

| 字段             | 值                                                                                                        |
| ---------------- | --------------------------------------------------------------------------------------------------------- |
| **优先级**       | P0                                                                                                        |
| **状态**         | 🔴 pending                                                                                                |
| **design_ref**   | `docs/design-docs/entry-points-design.md` + `docs/design-docs/module-boundaries-and-integration.md`       |
| **impl_targets** | `src-tauri/src/commands/agent.rs`, `src-tauri/src/commands/session.rs`, `src-tauri/src/commands/tools.rs` |

**任务描述**：实现 Tauri IPC 命令，作为前端 React 和后端 Rust 之间的桥梁。

**必须实现的命令**（来自 entry-points-design.md）：

```rust
// commands/agent.rs
#[tauri::command]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    session_id: String,
    user_message: String,
) -> Result<AgentTurnResponse, String>;

#[tauri::command]
pub async fn run_agent_turn_stream(
    state: State<'_, AppState>,
    session_id: String,
    user_message: String,
    window: Window,     // 通过 Tauri event emit 流式输出
) -> Result<(), String>;

// commands/session.rs
#[tauri::command]
pub async fn create_session(state: State<'_, AppState>) -> Result<SessionDto, String>;

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String>;

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<MessageDto>, String>;
```

**AppState 结构**（来自 module-boundaries-and-integration.md）：

```rust
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub provider_manager: Arc<ProviderManager>,
    pub runtime_config: Arc<RuntimeConfig>,
}
```

**验收标准**：

- `cargo check -p if2ai-backend` 包含 commands 模块，无错误
- Tauri 应用启动时所有命令正确注册
- 前端可以通过 `invoke('run_agent_turn', {...})` 调用

---

### BL-107 前端 IPC 接入 — React 对接后端

| 字段             | 值                                                                              |
| ---------------- | ------------------------------------------------------------------------------- |
| **优先级**       | P1                                                                              |
| **状态**         | 🔴 pending                                                                      |
| **design_ref**   | `docs/design-docs/entry-points-design.md`                                       |
| **impl_targets** | `src/App.tsx`, `src/hooks/useAgent.ts` (新建), `src/hooks/useSession.ts` (新建) |

**任务描述**：将现有 React UI (`src/App.tsx`) 的 placeholder 替换为真实的 Tauri IPC 调用。

**当前状态**：`src/App.tsx` 中使用 `invoke('greet')` 作为占位符，需要替换。

**必须实现的 hooks**：

```typescript
// src/hooks/useSession.ts
export function useSession() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const createSession = () => invoke<SessionDto>('create_session')
  const listSessions = () => invoke<SessionSummary[]>('list_sessions')
  return { sessions, createSession, listSessions }
}

// src/hooks/useAgent.ts
export function useAgent(sessionId: string) {
  const sendMessage = (msg: string) =>
    invoke<AgentTurnResponse>('run_agent_turn', { sessionId, userMessage: msg })

  // 流式版本：监听 Tauri event
  const sendMessageStream = (msg: string, onChunk: (chunk: string) => void) => {
    const unlisten = listen('agent_stream_chunk', (e) =>
      onChunk(e.payload as string)
    )
    invoke('run_agent_turn_stream', { sessionId, userMessage: msg })
    return unlisten // 调用方负责 unlisten
  }

  return { sendMessage, sendMessageStream }
}
```

**验收标准**：

- 界面可以创建新对话
- 界面可以列出历史会话
- 发送消息后能收到 Agent 回复并显示
- 流式输出时文字逐字出现

---

### BL-108 Phase 1 集成测试

| 字段             | 值                                                                             |
| ---------------- | ------------------------------------------------------------------------------ |
| **优先级**       | P1                                                                             |
| **状态**         | 🔴 pending                                                                     |
| **design_ref**   | `docs/design-docs/testing-strategy.md` + `docs/design-docs/harness-testing.md` |
| **impl_targets** | `src-tauri/src/tests/integration/`, `harness/suites/phase1_basic.yaml`         |

**任务描述**：编写 Phase 1 集成测试，验证完整的 Tauri → Rust 路径。

**必须覆盖的测试场景**：

```rust
// 场景 1: 单轮对话（Mock Provider）
#[tokio::test]
async fn test_single_turn_with_mock_provider()

// 场景 2: 工具调用对话
#[tokio::test]
async fn test_tool_call_round_trip()

// 场景 3: 会话保存和恢复
#[tokio::test]
async fn test_session_save_load()

// 场景 4: 超出 max_turns 预算
#[tokio::test]
async fn test_budget_exhausted()
```

**验收标准**：

- `cargo test -p if2ai-backend` 100% pass
- `python -m harness.runner run --slice 1.9 --workspace .` 通过

---

## Phase 2 — 高级特性

> 待 Phase 1 的 `human_checkpoint` 通过后，由 AI 根据 design-docs 生成对应的 exec-plan YAML。

### BL-201 PromptBuilder — 动态提示词构建

| 字段             | 值                                        |
| ---------------- | ----------------------------------------- |
| **优先级**       | P0（Phase 2 首批）                        |
| **design_ref**   | `docs/design-docs/prompt-builder.md`      |
| **impl_targets** | `src-tauri/src/modules/runtime/prompt.rs` |

**核心功能**：

- 从 `AgentDefinition` 构建角色描述（名称、角色、能力、语气）
- 将工具定义转换为 OpenAI / Claude 格式
- 支持语言切换（中文 / 英文）
- Few-shot example 注入

**关键接口**：

```rust
pub struct PromptBuilder {
    agent_def: AgentDefinition,
    tool_format: ToolFormatStyle,  // OpenAI or Claude
    language: String,
}

impl PromptBuilder {
    pub fn build(&self, tools: &[ToolEntry]) -> Result<String>;
    pub fn build_with_context(&self, tools: &[ToolEntry], context: &str) -> Result<String>;
}
```

---

### BL-202 ContextCompression — 长对话压缩

| 字段             | 值                                             |
| ---------------- | ---------------------------------------------- |
| **优先级**       | P0（Phase 2 首批）                             |
| **design_ref**   | `docs/design-docs/context-compression.md`      |
| **impl_targets** | `src-tauri/src/modules/runtime/compression.rs` |

**压缩触发条件**（来自 context-compression.md）：

- 当前 tokens > `window_size * compression_threshold`（默认 50%）

**三步压缩算法**：

1. **Prune**：移除工具调用的中间消息（保留最终结果）
2. **Protect**：保留头部（前 N 条）+ 尾部（后 M 条）
3. **Summarize**：对中间消息调用一次 LLM 生成摘要，替换原始消息

```rust
pub struct ContextCompressor {
    window_manager: ContextWindowManager,
    summarize_provider: Arc<dyn LLMProvider>,
}

impl ContextCompressor {
    pub fn should_compress(&self, messages: &[Message]) -> bool;
    pub async fn compress(&self, messages: Vec<Message>) -> Result<Vec<Message>>;
}
```

---

### BL-203 ErrorHandling — 统一错误处理和恢复

| 字段             | 值                                       |
| ---------------- | ---------------------------------------- |
| **优先级**       | P0（Phase 2 首批）                       |
| **design_ref**   | `docs/design-docs/error-handling.md`     |
| **impl_targets** | `src-tauri/src/modules/runtime/error.rs` |

**错误分类**（来自 error-handling.md）：

```rust
pub enum AgentError {
    // 可恢复 → 自动重试（指数退避）
    RateLimit { retry_after: Option<Duration> },
    Timeout { elapsed: Duration },
    ConnectionError(String),
    TokenLimitExceeded { current: u32, limit: u32 },

    // 可转移 → 切换提供商
    InvalidApiKey { provider: String },
    ModelUnavailable { provider: String, model: String },
    ProviderDown { provider: String },

    // 致命 → 停止执行，返回错误
    BudgetExhausted { turns_used: usize },
    SecurityViolation(String),
    InvalidToolCall { tool_name: String, reason: String },
}
```

**恢复策略**：可恢复错误最多重试 3 次，可转移错误触发 provider fallback。

---

### BL-204 Memory System — 跨会话记忆

| 字段             | 值                                     |
| ---------------- | -------------------------------------- |
| **优先级**       | P1（Phase 2 第二批）                   |
| **design_ref**   | `docs/design-docs/memory-system.md`    |
| **impl_targets** | `src-tauri/src/modules/memory/` (全部) |

**内置记忆**（无需外部服务）：

- `MEMORY.md`：Agent 观察到的事实
- `USER.md`：用户偏好和身份信息

**Honcho 集成**（可选，需要 API key）：

- 跨会话用户建模
- 辩证问答（dialectic Q&A）
- 语义搜索

**记忆接口**：

```rust
pub trait MemoryProvider: Send + Sync {
    async fn store(&self, key: &str, value: &str) -> Result<()>;
    async fn retrieve(&self, query: &str) -> Result<Vec<MemoryEntry>>;
    async fn inject_context(&self, messages: &mut Vec<Message>) -> Result<()>;
}

pub struct MemoryManager {
    providers: Vec<Box<dyn MemoryProvider>>,
    built_in: BuiltInMemory,    // MEMORY.md + USER.md
}
```

---

### BL-205 MessagingGateway — 多平台消息接入

| 字段             | 值                                       |
| ---------------- | ---------------------------------------- |
| **优先级**       | P2（Phase 2 最后一批）                   |
| **design_ref**   | `docs/design-docs/messaging-gateway.md`  |
| **impl_targets** | `src-tauri/src/modules/plugins/gateway/` |

**第一期支持的平台**（按优先级）：

| 平台     | 环境变量             | 接入方式           |
| -------- | -------------------- | ------------------ |
| Telegram | `TELEGRAM_BOT_TOKEN` | Polling or Webhook |
| Discord  | `DISCORD_BOT_TOKEN`  | WebSocket          |

**统一消息接口**：

```rust
pub trait MessagingAdapter: Send + Sync {
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()>;
    async fn send_message(&self, chat_id: &str, text: &str) -> Result<()>;
    async fn send_typing(&self, chat_id: &str) -> Result<()>;
    fn platform_name(&self) -> &str;
}
```

---

## Phase 3 — 扩展生态

> Phase 3 的具体 exec-plan 在 Phase 2 完成后由 AI + 人工共同规划。

### BL-301 Agent Self-Improvement (RL-Training)

| **design_ref** | `docs/design-docs/agent-self-improvement.md`          |
| -------------- | ----------------------------------------------------- |
| **描述**       | 基于 GRPO 的强化学习，Agent 根据任务结果自我迭代      |
| **依赖**       | BL-108 集成测试（提供训练数据）、BL-204 Memory System |

### BL-302 IDE / LSP Bridge

| **design_ref** | `docs/design-docs/entry-points-design.md` (Phase 3 部分) |
| -------------- | -------------------------------------------------------- |
| **描述**       | 通过 stdio JSON-RPC 接入 VSCode / Cursor 等 IDE          |
| **依赖**       | BL-106 Tauri Commands（命令体系复用）                    |

### BL-303 JSON-RPC API Server

| **design_ref** | `docs/design-docs/entry-points-design.md` (Phase 2 部分) |
| -------------- | -------------------------------------------------------- |
| **描述**       | 暴露 HTTP API 供第三方调用 Agent                         |
| **依赖**       | BL-106 Tauri Commands（逻辑层复用）                      |

---

## Phase 4 — 工具激活与工作目录边界 (Tool & Workdir Boundary)

> 对应 exec-plan: `docs/exec-plans/active/phase-4-tool-and-boundary.yaml`

### BL-401 注册内置工具 + LLM 集成

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P0                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`           |
| **impl_targets** | `src-tauri/src/main.rs`, `src-tauri/src/modules/tools/mod.rs`, `src-tauri/src/commands/agent.rs` |

**任务描述**：激活工具注册表，让内置工具（bash, read_file, json_parse）注册到 ToolRegistry，并让 `MessageRequest` 传递工具定义给 LLM。

**验收标准**：
- `cargo check -p if2ai-backend` 无错误
- `register_builtin_tools()` 在 `tools/mod.rs` 中存在
- `main.rs` 调用 `register_builtin_tools()`
- `agent.rs` 中 `MessageRequest.tools` 为 `Some(...)`

---

### BL-402 ToolContext Arc<Mutex> 机制

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P0                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/project-workdir-boundary.md`   |
| **impl_targets** | `src-tauri/src/modules/tools/context.rs`, `src-tauri/src/modules/tools/registry.rs`, `src-tauri/src/modules/runtime/config.rs` |

**任务描述**：实现 `ToolContext` 结构（workdir + permission_mode），通过 `Arc<Mutex>` 传递给工具 handler，让工具函数能访问当前 project 的 workdir。

**验收标准**：
- `ToolContext` 结构在 `context.rs` 中存在
- `Registry.dispatch` 获取并传递 `ToolContext`
- `RuntimeConfig` 新增 `workdir: Option<PathBuf>` 字段

---

### BL-403 file_read workdir allowlist

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P0                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/project-workdir-boundary.md`   |
| **impl_targets** | `src-tauri/src/modules/tools/builtin/file_read.rs` |

**任务描述**：实现 workdir allowlist，只允许读取 project workdir 内的文件。保留原有的敏感路径 denylist 作为额外保护。

**验收标准**：
- 读取 workdir 外的文件返回错误
- 读取 `/etc/passwd` 等敏感路径返回错误
- handler 签名包含 `ToolContext` 参数

---

### BL-404 bash workdir 限制

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P0                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/project-workdir-boundary.md`   |
| **impl_targets** | `src-tauri/src/modules/tools/builtin/bash.rs`   |

**任务描述**：实现 bash 命令的 workdir 限制，通过 `cd $workdir && $command` 包装原始命令。

**验收标准**：
- bash 命令在 workdir 内执行
- 危险命令（`rm -rf /` 等）仍被拦截
- handler 签名包含 `ToolContext` 参数

---

### BL-405 execute_tool Tauri 命令

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/commands/tools.rs`, `src-tauri/src/commands/mod.rs`, `src/lib/tauri.ts` |

**任务描述**：新增 `execute_tool`, `list_tools`, `get_tool_definitions` 三个 Tauri 命令，让前端可以直接调用工具。

**验收标准**：
- `commands/tools.rs` 中三个命令存在
- 前端 `lib/tauri.ts` 导出 `executeTool`, `listTools`, `getToolDefinitions`
- TypeScript 类型定义正确

---

### BL-406 Per-Project PermissionMode

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/project-workdir-boundary.md`   |
| **impl_targets** | `src-tauri/src/modules/projects/mod.rs`, `src-tauri/src/modules/runtime/permissions.rs` |

**任务描述**：在 `Project` 结构中新增 `permission_mode` 字段，持久化到 `project.json`，并让 `agent.rs` 运行时读取此配置。

**验收标准**：
- `Project.permission_mode` 字段存在
- 默认值为 `WorkspaceWrite`
- 旧 `project.json` 反序列化正确处理缺失字段

---

### BL-407 文件操作类工具（ZeroClaw 复刻）

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P0                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/modules/tools/builtin/file_write.rs`, `src-tauri/src/modules/tools/builtin/file_edit.rs`, `src-tauri/src/modules/tools/builtin/glob_search.rs`, `src-tauri/src/modules/tools/builtin/content_search.rs` |

**任务描述**：从 ZeroClaw 复刻 4 个文件操作工具：`file_write`（写入文件）、`file_edit`（diff patch）、`glob_search`（glob 模式搜索）、`content_search`（内容搜索，支持 regex）。

**验收标准**：
- 4 个工具都有 `entry()` 函数
- `glob_search` 使用 glob crate
- `content_search` 支持 regex 搜索
- 在 workdir 内执行（安全检查）

---

### BL-408 Web 类工具（ZeroClaw 复刻）

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/modules/tools/builtin/web_fetch.rs`, `src-tauri/src/modules/tools/builtin/web_search.rs`, `src-tauri/src/modules/tools/builtin/http_request.rs` |

**任务描述**：从 ZeroClaw 复刻 3 个 Web 工具：`web_fetch`（网页抓取，支持 CSS selector）、`web_search`（网络搜索）、`http_request`（HTTP 请求）。

**验收标准**：
- 3 个工具都有 `entry()` 函数
- `web_fetch` 支持 CSS selector 内容提取
- `web_search` 支持 DuckDuckGo/Brave/SearXNG provider
- 使用 reqwest, scraper, url crate

---

### BL-409 Memory 类工具（ZeroClaw 复刻）

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/modules/memory/`, `src-tauri/src/modules/tools/builtin/memory_store.rs`, `src-tauri/src/modules/tools/builtin/memory_recall.rs`, `src-tauri/src/modules/tools/builtin/memory_forget.rs`, `src-tauri/src/modules/tools/builtin/memory_purge.rs`, `src-tauri/src/modules/tools/builtin/memory_export.rs` |

**任务描述**：从 ZeroClaw 复刻 5 个 Memory 工具 + `MemoryProvider` trait：存储/召回/删除/清除/导出记忆。

**验收标准**：
- `MemoryProvider` trait 定义 5 个方法
- 5 个工具都有 `entry()` 函数
- 支持 Core/Daily/Conversation/Custom 分类
- `memory_export` 支持 JSON 和 Markdown 格式

---

### BL-410 Cron/调度类工具（ZeroClaw 复刻）

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/modules/scheduler/`, `src-tauri/src/modules/tools/builtin/cron_add.rs`, `src-tauri/src/modules/tools/builtin/cron_list.rs`, `src-tauri/src/modules/tools/builtin/cron_remove.rs`, `src-tauri/src/modules/tools/builtin/cron_run.rs`, `src-tauri/src/modules/tools/builtin/cron_runs.rs` |

**任务描述**：从 ZeroClaw 复刻 5 个 Cron 工具 + `Scheduler` trait：创建/列出/删除/立即运行/查看执行记录。

**验收标准**：
- `Scheduler` trait 定义 5 个方法
- 5 个工具都有 `entry()` 函数
- cron expression 解析使用 cron crate
- 支持 cron expression 验证

---

### BL-411 ToolSet 分类系统

| 字段             | 值                                              |
| ---------------- | ----------------------------------------------- |
| **优先级**       | P1                                              |
| **状态**         | 🔴 pending                                      |
| **design_ref**   | `docs/design-docs/tool-activation.md`            |
| **impl_targets** | `src-tauri/src/modules/tools/toolset.rs`, `src-tauri/src/modules/tools/mod.rs`, `src-tauri/src/modules/tools/registry.rs`, `src-tauri/src/commands/tools.rs`, `src/lib/tauri.ts` |

**任务描述**：实现 ToolSet 分类系统，包括 `ToolSet` 结构、`ToolSetRegistry` 管理器、`TOOLSETS` 预定义常量、按 toolset 批量过滤工具定义的 API。

**验收标准**：
- `TOOLSETS` 常量包含 8 个预定义工具集（files, terminal, utility, web, memory, scheduler, minimal, development）
- `ToolSetRegistry` 能正确映射 tool → toolset
- `get_definitions_by_toolsets()` 方法存在且正确过滤
- 前端 `ToolSet` 类型包含 name, description, tools, enabled 字段

---

## 设计文档 ↔ Backlog 映射表

| 设计文档                             | 对应 Backlog 条目                                          | Phase |
| ------------------------------------ | ---------------------------------------------------------- | ----- |
| system-architecture-framework.md     | 整体指导，无直接 backlog                                   | —     |
| module-boundaries-and-integration.md | BL-101 (修复编译), BL-106 (AppState)                       | P1    |
| entry-points-design.md               | BL-106 (Tauri Commands), BL-107 (前端接入), BL-302, BL-303 | P1/P3 |
| agent-loop.md                        | BL-102 (ConversationRuntime)                               | P1    |
| agent-orchestrator.md                | BL-102 (循环逻辑)                                          | P1    |
| provider-resolution.md               | BL-103 (ProviderManager)                                   | P1    |
| llm-routing.md                       | BL-103 (故障转移)                                          | P1    |
| tool-system.md                       | BL-104 (ToolRegistry), BL-401 (工具激活)                      | P1/P4 |
| tool-activation.md                   | BL-401, BL-405, BL-407, BL-408, BL-409, BL-410, BL-411     | P4    |
| project-workdir-boundary.md         | BL-402, BL-403, BL-404, BL-406                              | P4    |
| session-persistence.md               | BL-105 (SessionManager)                                    | P1    |
| data-schema.md                       | BL-105 (SQLite Schema)                                     | P1    |
| prompt-builder.md                    | BL-201 (PromptBuilder)                                     | P2    |
| context-compression.md               | BL-202 (ContextCompression)                                | P2    |
| error-handling.md                    | BL-203 (ErrorHandling)                                     | P2    |
| memory-system.md                     | BL-204 (MemorySystem)                                      | P2    |
| messaging-gateway.md                 | BL-205 (MessagingGateway)                                  | P2    |
| agent-self-improvement.md            | BL-301 (RL-Training)                                       | P3    |
| testing-strategy.md                  | BL-108 (集成测试) + 所有 acceptance                        | 贯穿  |
| harness-testing.md                   | harness/gate.py + harness/runner.py                        | 贯穿  |

---

## Executor 快速查找

**当前需要实现什么？**

```bash
# 查看当前 pending 的 slice
grep -A3 "status: pending" docs/exec-plans/active/phase-1-foundation.yaml | head -20

# 找到对应的 Backlog 条目
# Phase 1:
# slice 1.2 → BL-101
# slice 1.3 → BL-102
# slice 1.4 → BL-103
# slice 1.5 → BL-104
# slice 1.6 → BL-105
# slice 1.7 → BL-106
# slice 1.8 → BL-107
# slice 1.9 → BL-108
#
# Phase 4:
# slice 4.1 → BL-401
# slice 4.2 → BL-402
# slice 4.3 → BL-403
# slice 4.4 → BL-404
# slice 4.5 → BL-405
# slice 4.6 → BL-406
# slice 4.8 → BL-407 (文件操作类工具)
# slice 4.9 → BL-408 (Web 类工具)
# slice 4.10 → BL-409 (Memory 类工具)
# slice 4.11 → BL-410 (Cron/调度类工具)
# slice 4.7  → BL-411 (ToolSet 分类系统)
```

| 功能                   | 状态           | 优先级 | 所有者 |
| ---------------------- | -------------- | ------ | ------ |
| Chat Interface         | ✅ Draft       | P0     | TBD    |
| Agent Dashboard        | ✅ Draft       | P0     | TBD    |
| Settings Panel         | 🔄 In Progress | P1     | TBD    |
| Memory Management      | ⏳ Planned     | P2     | TBD    |
| Batch Execution        | ⏳ Planned     | P2     | TBD    |
| Tool Activation (P4)   | 🔴 Pending     | P0     | TBD    |
| Workdir Boundary (P4)  | 🔴 Pending     | P0     | TBD    |

## 🚀 添加新规范

1. 创建 `feature-name.md` 文件
2. 填充模板部分
3. 从 [AGENTS.md](../../AGENTS.md) 链接到新规范
4. 在此索引中添加条目

---

**版本**: 0.1.0 | **最后更新**: 2026-04-12
