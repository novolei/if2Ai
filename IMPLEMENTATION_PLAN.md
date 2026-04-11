# If2Ai Phase 1 详细实现计划

**文档版本**: 1.0  
**最后更新**: 2025年1月  
**预计完成**: 4-6 周  
**目标**：实现最小可用的 Agent 系统 (MVP)

---

## 📋 阶段概览

### 目标功能集
1. ✅ 单一 LLM 提供商 (OpenAI)
2. ✅ 基本 Agent 循环 (Prompt → API → Response)
3. ✅ 5-8 个核心工具
4. ✅ 会话持久化 (SQLite)
5. ✅ 基础 UI (Chat 界面)
6. ✅ Harness 集成测试

### 不在范围内
- ❌ 多 LLM 提供商切换
- ❌ Prompt 缓存 / 上下文压缩
- ❌ Gateway 多平台消息
- ❌ 插件系统
- ❌ Web UI / Remote

---

## 📅 周计划详细

### 第 1 周：项目基础设置 + Agent 循环框架

#### 周一-周二：Rust 项目结构设置

**目标**：建立可扩展的模块架构

**任务**：
1. 扩展 `Cargo.toml` 依赖
2. 创建模块目录结构
3. 定义核心类型

**文件修改**：

**src-tauri/Cargo.toml** - 添加依赖：
```toml
[dependencies]
# ... existing ...

# Core dependencies
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tokio-util = "0.7"
reqwest = { version = "0.11", features = ["json", "stream"] }
futures = "0.3"

# Database
rusqlite = { version = "0.30", features = ["bundled", "chrono", "uuid"] }
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["v4", "serde"] }

# Error handling & utilities
anyhow = "1"
thiserror = "1"

# LLM / API
async-openai = "0.14"  # OpenAI client library
```

**src-tauri/src/modules/mod.rs**：
```rust
pub mod agent;
pub mod tools;
pub mod memory;
pub mod providers;
pub mod types;

pub use agent::AIAgent;
pub use types::{Message, ToolCall, ToolResult};
```

**新建** `src-tauri/src/modules/types.rs`：
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// OpenAI-compatible message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,  // "system", "user", "assistant", "tool"
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,  // JSON string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub response: String,
    pub messages: Vec<Message>,
    pub tool_calls: Vec<ToolCall>,
    pub iterations: usize,
}

#[derive(Debug)]
pub struct ConversationHistory {
    pub session_id: Uuid,
    pub messages: Vec<Message>,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }
}
```

**验收标准**：
- [ ] Cargo.toml 编译无错
- [ ] 所有模块路径都能导入
- [ ] 类型定义支持 serde 序列化

---

#### 周三-周四：Agent Loop 核心实现

**目标**：实现最小可用的 Agent 循环

**新建** `src-tauri/src/modules/agent.rs`：
```rust
use crate::modules::types::*;
use crate::modules::providers::OpenAIProvider;
use crate::modules::tools::ToolRegistry;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequest};
use std::sync::Arc;
use uuid::Uuid;

pub struct AIAgent {
    provider: Arc<OpenAIProvider>,
    tools: Arc<ToolRegistry>,
    model: String,
    max_iterations: usize,
}

impl AIAgent {
    pub fn new(
        api_key: String,
        model: String,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            provider: Arc::new(OpenAIProvider::new(api_key)),
            tools,
            model,
            max_iterations: 10,
        }
    }

    /// 主 Agent 循环
    pub async fn run_conversation(
        &self,
        user_message: String,
        history: &mut ConversationHistory,
    ) -> Result<AgentResponse, anyhow::Error> {
        // Step 1: 添加用户消息
        history.add_message(Message {
            role: "user".to_string(),
            content: Some(user_message),
            tool_calls: None,
            tool_call_id: None,
        });

        let mut iteration = 0;
        let mut final_response = String::new();

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                anyhow::bail!("Max iterations reached: {}", self.max_iterations);
            }

            // Step 2: 构建系统提示
            let system_prompt = self.build_system_prompt();

            // Step 3: 调用 LLM
            let response = self
                .provider
                .chat_completion(
                    &system_prompt,
                    history.messages.clone(),
                    self.tools.get_schemas(),
                    &self.model,
                )
                .await?;

            // Step 4: 解析响应
            let response_message = Message {
                role: "assistant".to_string(),
                content: response.content.clone(),
                tool_calls: response.tool_calls.clone(),
                tool_call_id: None,
            };
            history.add_message(response_message.clone());

            // Step 5: 检查是否有工具调用
            if let Some(tool_calls) = &response.tool_calls {
                if tool_calls.is_empty() {
                    // 没有工具调用，返回最终响应
                    final_response = response.content.unwrap_or_default();
                    break;
                }

                // 执行工具
                for tool_call in tool_calls {
                    let result = self.tools.execute(&tool_call).await?;
                    history.add_message(Message {
                        role: "tool".to_string(),
                        content: Some(result.content),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
                // 继续循环以获取下一个响应
            } else {
                // 没有工具调用的纯文本响应
                final_response = response.content.unwrap_or_default();
                break;
            }
        }

        Ok(AgentResponse {
            response: final_response,
            messages: history.messages.clone(),
            tool_calls: vec![],
            iterations: iteration,
        })
    }

    fn build_system_prompt(&self) -> String {
        format!(
            r#"You are a helpful AI assistant. You have access to the following tools:

{}

When you need to use a tool, call it using the function calling interface. Always be helpful and explain your reasoning."#,
            self.tools.get_descriptions().join("\n")
        )
    }
}
```

**验收标准**：
- [ ] AIAgent 能初始化
- [ ] run_conversation 签名正确
- [ ] 编译无错且有 test 支持

---

#### 周五：提供商集成基础

**新建** `src-tauri/src/modules/providers.rs`：
```rust
use crate::modules::types::*;
use async_openai::client::OpenAIClient;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequest};
use serde_json::json;

pub struct OpenAIProvider {
    client: OpenAIClient<async_openai::config::OpenAIConfig>,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        let config = async_openai::config::OpenAIConfig::new()
            .with_api_key(api_key);
        Self {
            client: OpenAIClient::with_config(config),
        }
    }

    pub async fn chat_completion(
        &self,
        system_prompt: &str,
        history: Vec<Message>,
        tool_schemas: Vec<serde_json::Value>,
        model: &str,
    ) -> Result<ChatCompletionResponse, anyhow::Error> {
        let mut messages = vec![ChatCompletionRequestMessage::System(
            async_openai::types::ChatCompletionRequestSystemMessage::Text(
                async_openai::types::ChatCompletionRequestSystemMessageContent::Text(
                    system_prompt.to_string(),
                ),
            ),
        )];

        // TODO: 转换历史记录为 OpenAI 消息格式

        let request = CreateChatCompletionRequest {
            model: model.to_string(),
            messages,
            tools: if tool_schemas.is_empty() {
                None
            } else {
                Some(
                    tool_schemas
                        .into_iter()
                        .map(|schema| {
                            // TODO: 转换为 ChatCompletionTool
                            unimplemented!()
                        })
                        .collect(),
                )
            },
            ..Default::default()
        };

        // TODO: 实现完整的 API 调用逻辑
        unimplemented!()
    }
}

pub struct ChatCompletionResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}
```

**验收标准**：
- [ ] OpenAIProvider 能初始化
- [ ] 类型定义完整
- [ ] 为完整实现预留的占位符清晰

---

**该周验收标准**：
- [ ] 项目编译成功 (`cargo build`)
- [ ] 所有模块路径可用
- [ ] 核心类型定义完整
- [ ] Agent 循环框架就位 (虽然不完整)

**该周交付物**：
- [ ] 更新的 Cargo.toml
- [ ] modules/ 目录结构
- [ ] types.rs 完整类型
- [ ] agent.rs 循环框架
- [ ] providers.rs 初始化

---

### 第 2 周：完成核心 Agent + 工具系统

#### 周一-周二：完成 OpenAI 集成

**目标**：能够调用实际的 OpenAI API

**文件修改**：`src-tauri/src/modules/providers.rs`

实现完整的 `chat_completion` 方法。参考 [async_openai 文档](https://docs.rs/async-openai/)。

**关键任务**：
- 将 `Message` 转换为 OpenAI `ChatCompletionRequestMessage`
- 处理 tool_calls JSON 解析
- 实现流式响应（可选 Phase 2）

**测试**：
```bash
# src-tauri/src/modules/providers.rs 中添加测试
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]  // 需要 OPENAI_API_KEY
    async fn test_openai_integration() {
        let provider = OpenAIProvider::new(
            std::env::var("OPENAI_API_KEY").unwrap()
        );
        
        let response = provider.chat_completion(
            "You are a helpful assistant",
            vec![],
            vec![],
            "gpt-4",
        ).await;
        
        assert!(response.is_ok());
    }
}
```

**验收标准**：
- [ ] API 调用不返回错误
- [ ] 响应能正确解析
- [ ] Tool calls 能正确提取

---

#### 周三：Tool Registry + 执行

**新建** `src-tauri/src/modules/tools/mod.rs`：
```rust
pub mod registry;
pub mod terminal;
pub mod file_ops;

pub use registry::ToolRegistry;
```

**新建** `src-tauri/src/modules/tools/registry.rs`：
```rust
use crate::modules::types::ToolCall;
use std::collections::HashMap;
use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, args: serde_json::Value) -> Result<String, anyhow::Error>;
    fn schema(&self) -> serde_json::Value;
    fn name(&self) -> &str;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn execute(&self, tool_call: &ToolCall) -> Result<String, anyhow::Error> {
        let tool = self
            .tools
            .get(&tool_call.function.name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_call.function.name))?;

        let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)?;
        tool.execute(args).await
    }

    pub fn get_schemas(&self) -> Vec<serde_json::Value> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn get_descriptions(&self) -> Vec<String> {
        self.tools
            .values()
            .map(|t| format!("- {}: {}", t.name(), t.schema().pointer("/description")))
            .collect()
    }
}
```

**新建** `src-tauri/src/modules/tools/terminal.rs`：
```rust
use super::Tool;
use async_trait::async_trait;
use serde_json::json;
use std::process::Command;

pub struct TerminalTool;

#[async_trait]
impl Tool for TerminalTool {
    async fn execute(&self, args: serde_json::Value) -> Result<String, anyhow::Error> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr))
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "name": "terminal",
            "description": "Execute a shell command",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }
        })
    }

    fn name(&self) -> &str {
        "terminal"
    }
}
```

**验收标准**：
- [ ] ToolRegistry 编译成功
- [ ] Terminal 工具能执行命令
- [ ] 工具 schema 合法 JSON

---

#### 周四-周五：内存系统 + Tauri 命令

**新建** `src-tauri/src/modules/memory.rs`：
```rust
use crate::modules::types::*;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use uuid::Uuid;

pub struct MemoryStore {
    connection: Connection,
}

impl MemoryStore {
    pub fn new(db_path: &str) -> Result<Self, anyhow::Error> {
        let connection = Connection::open(db_path)?;
        
        // 初始化表
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_calls TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id)
            );
            "#,
        )?;

        Ok(Self { connection })
    }

    pub fn save_session(&self, history: &ConversationHistory) -> Result<(), anyhow::Error> {
        let session_id = history.session_id.to_string();
        let now = Utc::now().to_rfc3339();

        // 保存会话记录
        self.connection.execute(
            "INSERT OR REPLACE INTO sessions (session_id, created_at, updated_at) VALUES (?1, ?2, ?3)",
            [&session_id, &now, &now],
        )?;

        // 保存消息
        for message in &history.messages {
            let content = message.content.as_deref();
            let tool_calls = message
                .tool_calls
                .as_ref()
                .map(|calls| serde_json::to_string(calls).unwrap_or_default());

            self.connection.execute(
                "INSERT INTO messages (session_id, role, content, tool_calls, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![&session_id, &message.role, content, tool_calls, &now],
            )?;
        }

        Ok(())
    }

    pub fn load_session(&self, session_id: &str) -> Result<ConversationHistory, anyhow::Error> {
        let mut stmt = self.connection.prepare(
            "SELECT role, content, tool_calls FROM messages WHERE session_id = ?1 ORDER BY id",
        )?;

        let messages = stmt
            .query_map([session_id], |row| {
                Ok(Message {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    tool_calls: row
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    tool_call_id: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ConversationHistory {
            session_id: Uuid::parse_str(session_id)?,
            messages,
        })
    }
}
```

**修改** `src-tauri/src/commands/mod.rs`：
```rust
mod agent_commands;

pub use agent_commands::*;
```

**新建** `src-tauri/src/commands/agent_commands.rs`：
```rust
use crate::modules::{AIAgent, tools::ToolRegistry, types::*};
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub agent: Mutex<Option<AIAgent>>,
    pub history: Mutex<ConversationHistory>,
}

#[tauri::command]
pub async fn initialize_agent(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut tools = ToolRegistry::new();
    // TODO: 注册工具
    
    let agent = AIAgent::new(api_key, "gpt-4".to_string(), std::sync::Arc::new(tools));
    *state.agent.lock().unwrap() = Some(agent);
    
    Ok("Agent initialized".to_string())
}

#[tauri::command]
pub async fn send_message(
    message: String,
    state: State<'_, AppState>,
) -> Result<AgentResponse, String> {
    let agent = state
        .agent
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Agent not initialized")?
        .clone();
    
    let mut history = state.history.lock().unwrap();
    
    agent
        .run_conversation(message, &mut history)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_conversation_history(
    state: State<'_, AppState>,
) -> Result<Vec<Message>, String> {
    let history = state.history.lock().unwrap();
    Ok(history.messages.clone())
}
```

**修改** `src-tauri/src/main.rs`：
```rust
mod commands;
mod modules;

use commands::{initialize_agent, send_message, get_conversation_history, AppState};
use modules::types::ConversationHistory;
use std::sync::Mutex;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            agent: Mutex::new(None),
            history: Mutex::new(ConversationHistory::new()),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            initialize_agent,
            send_message,
            get_conversation_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**验收标准**：
- [ ] 项目编译成功
- [ ] `tauri dev` 能启动
- [ ] Tool registry 能注册工具

---

**该周验收标准**：
- [ ] OpenAI API 集成完整
- [ ] Tool system 可扩展
- [ ] SQLite 持久化就位
- [ ] Tauri 命令就位
- [ ] 项目能编译和运行

---

### 第 3 周：完整核心集成 + 前端制作

#### 周一-周二：完成剩余工具

在 `src-tauri/src/modules/tools/` 下实现：

1. **web.rs** - web_search + web_extract
2. **file_ops.rs** - read_file, write_file
3. **code_execution.rs** - execute_code (Python/Node.js)

每个工具遵循相同的 `Tool` trait 模式。

**验收标准**：
- [ ] 6-8 个工具都实现了 schema
- [ ] 至少 3 个工具有实际执行逻辑
- [ ] 单元测试覆盖 ≥70%

---

#### 周三-周五：Svelte UI 制作

**前端目标**：
- Chat 消息显示
- 输入框 + 发送按钮
- 工具执行进度
- 对话历史

**src/App.svelte** - 完全重写：
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import ChatView from './components/ChatView.svelte'
  import InputPanel from './components/InputPanel.svelte'

  let apiKey: string = ''
  let initialized: boolean = false
  let messages = []
  let loading: boolean = false

  async function handleInitialize(key: string) {
    try {
      await invoke('initialize_agent', { apiKey: key })
      initialized = true
    } catch (err) {
      console.error(err)
    }
  }

  async function handleSendMessage(text: string) {
    if (!text.trim()) return
    
    loading = true
    try {
      const response = await invoke('send_message', { message: text })
      messages = [...messages, ...response.messages]
    } catch (err) {
      console.error(err)
    }
    loading = false
  }
</script>

{#if !initialized}
  <div class="setup-panel">
    <h1>If2Ai - AI Agent</h1>
    <input 
      type="password" 
      placeholder="Enter OpenAI API Key"
      on:change={(e) => apiKey = e.target.value}
    />
    <button on:click={() => handleInitialize(apiKey)}>
      Initialize
    </button>
  </div>
{:else}
  <div class="chat-container">
    <ChatView {messages} {loading} />
    <InputPanel on:submit={(e) => handleSendMessage(e.detail)} />
  </div>
{/if}

<style>
  .setup-panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 2rem;
  }

  .chat-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
</style>
```

**src/components/ChatView.svelte** - 新建：
```svelte
<script lang="ts">
  export let messages = []
  export let loading = false

  let scrollDiv

  $: if (scrollDiv) {
    scrollDiv.scrollTop = scrollDiv.scrollHeight
  }
</script>

<div class="chat-view" bind:this={scrollDiv}>
  {#each messages as message (message.role + message.content)}
    <div class="message {message.role}">
      <strong>{message.role}</strong>
      <p>{message.content}</p>
    </div>
  {/each}
  {#if loading}
    <div class="message loading">
      <p>Agent 思考中...</p>
    </div>
  {/if}
</div>

<style>
  .chat-view {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }

  .message {
    margin: 0.5rem 0;
    padding: 0.5rem;
    border-radius: 4px;
  }

  .message.user {
    background: #e3f2fd;
    text-align: right;
  }

  .message.assistant {
    background: #f5f5f5;
  }

  .message.loading {
    font-style: italic;
    color: #666;
  }
</style>
```

**src/components/InputPanel.svelte** - 新建：
```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte'

  const dispatch = createEventDispatcher()
  let input = ''

  function handleSubmit() {
    if (input.trim()) {
      dispatch('submit', input)
      input = ''
    }
  }
</script>

<div class="input-panel">
  <input
    type="text"
    placeholder="Type message..."
    bind:value={input}
    on:keydown={(e) => e.key === 'Enter' && handleSubmit()}
  />
  <button on:click={handleSubmit}>Send</button>
</div>

<style>
  .input-panel {
    display: flex;
    gap: 0.5rem;
    padding: 1rem;
    border-top: 1px solid #ddd;
  }

  input {
    flex: 1;
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 4px;
  }

  button {
    padding: 0.5rem 1rem;
    background: #1976d2;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
```

**验收标准**：
- [ ] Chat 界面能接收消息
- [ ] Send 按钮能触发 Tauri 命令
- [ ] 消息能正确显示和滚动

---

**第 3 周验收标准**：
- [ ] 8 个核心工具实现
- [ ] 完整的 Chat UI
- [ ] IPC 通讯工作正常
- [ ] 能进行端到端的对话 (本地测试)

---

### 第 4 周：集成 + 测试 + 文档

#### 周一-周二：端到端集成测试

**创建** `src-tauri/src/lib.rs`：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_loop_basic() {
        // 初始化
        // 发送消息
        // 验证响应
    }

    #[tokio::test]
    async fn test_tool_execution() {
        // 测试工具执行
    }

    #[tokio::test]
    async fn test_session_persistence() {
        // 测试会话保存和恢复
    }
}
```

在 `harness/` 框架中添加完整的集成测试：
```python
# harness/tests/test_agent_correctness.py
class TestAgentCorrectness(HarnessTestCase):
    def test_simple_response(self):
        """Agent 能回答简单问题"""
        response = self.run_agent("What is 2+2?")
        self.assertIn("4", response.text)

    def test_tool_execution(self):
        """Agent 能调用工具"""
        response = self.run_agent("List files in /tmp")
        self.assertTrue(response.tool_calls_made > 0)

    def test_conversation_history(self):
        """Agent 能保持对话历史"""
        self.run_agent("My name is Alice")
        response = self.run_agent("What's my name?")
        self.assertIn("Alice", response.text)
```

#### 周三：性能优化 + 日志

- 增加 tracing 日志到关键路径
- Profile Agent 循环性能
- 优化热路径代码

#### 周四-周五：文档 + Demo

更新文档：
1. **API 文档** - Rust doc comments
2. **集成指南** - 如何添加新工具
3. **部署指南** - 打包和运行
4. **示例** - 预制的演示场景

**验收标准**：
- [ ] 所有测试通过 (`cargo test`)
- [ ] Harness 评估器通过 ≥80% 的标准
- [ ] README 更新完整
- [ ] API 文档全覆盖

---

## 🎯 阶段验收标准（全局）

### 功能完整性
- [ ] Single-turn Agent 循环完整
- [ ] 最少 8 个工具可用
- [ ] SQLite 持久化正常
- [ ] Tauri IPC 通讯稳定
- [ ] 前端 UI 可用

### 代码品质
- [ ] 单元测试 ≥70% 覆盖
- [ ] 集成测试通过
- [ ] 无 clippy 警告
- [ ] 代码格式化 (`cargo fmt`)
- [ ] 文档注释 ≥80%

### Harness 评估
- [ ] 正确性：≥85%
- [ ] 行为：≥80%
- [ ] 性能：<2s 平均响应
- [ ] 可靠性：≥95%

### 可维护性
- [ ] 模块清晰分离
- [ ] 依赖最小化
- [ ] 文档同步更新
- [ ] 架构约束遵守

---

## 📊 里程碑总结表

| 周 | 目标 | 交付物 | 验收 |
|---|------|--------|------|
| 1 | 框架初始化 | AST/types/provider | 编译通过 |
| 2 | 核心集成 | Agent loop/tools/IPC | E2E 大体可行 |
| 3 | 功能完整 | 8 工具 + UI | 完整对话流 |
| 4 | 生产准备 | 测试/文档/打包 | 发布版本 |

---

## 🔧 开发环境检查清单

运行以下命令验证环境就绪：

```bash
# Rust
rustc --version
cargo --version

# Node/Npm
node --version
npm --version

# (可选) Tauri
npm install -g @tauri-apps/cli

# 测试编译
cd src-tauri && cargo build --release
```

---

## 📚 参考资源

| 资源 | 链接 |
|------|------|
| Hermes Agent 源码 | `~/Documents/IfAI/hermes-agent-main` |
| If2Ai 项目 | `/Users/ryanliu/Documents/IfAI/if2Ai` |
| 设计文档 | `docs/design-docs/` |
| Harness 框架 | `harness/README.md` |
| Tauri 文档 | https://tauri.app/docs |
| async-openai | https://docs.rs/async-openai |

---

**下一步**：选择**第 1 周** 的任务开始实现。建议周一早上就开始Cargo.toml 和目录结构的设置。
