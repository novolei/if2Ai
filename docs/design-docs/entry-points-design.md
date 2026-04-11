# If2Ai 入口点设计 (Entry Points Architecture)

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes 的 4 个入口点  
**If2Ai 适配**: 3 个阶段的入口点

---

## 1. 入口点概览

### 1.1 Hermes 的 4 个入口点

```
┌─────────────────────────────────────┐
│  User/System Interaction Layer      │
├─────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐ │
│  │   CLI        │  │   Gateway    │ │
│  │ (local) │  │ (remote)   │ │
│  ├──────────────┤  ├──────────────┤ │
│  │ HermesCLI    │  │ GatewayRunner│ │
│  │ (8.5K)       │  │ (7.5K)       │ │
│  └─────┬────────┘  └──────┬───────┘ │
├─────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐ │
│  │   Batch      │  │     ACP      │ │
│  │   Runner     │  │   (IDE)      │ │
│  ├──────────────┤  ├──────────────┤ │
│  │ 程序化生成   │  │ Editor 插件  │ │
│  │ 轨迹数据     │  │ (stdio/JSON) │ │
│  └─────┬────────┘  └──────┬───────┘ │
└────────┼───────────────────┼────────┘
         │                   │
         └─────────┬─────────┘
                   ▼
         ┌──────────────────┐
         │   AIAgent        │
         │ (Shared Core)    │
         └──────────────────┘
```

### 1.2 If2Ai 的分阶段入口点

```
Phase 1 (现在):
┌─────────────────────────────────────┐
│  Tauri 桌面应用                      │
└──────────────┬──────────────────────┘
               ▼
        ┌──────────────┐
        │ Tauri IPC    │
        │ Commands     │
        └──────┬───────┘
               ▼
        ┌──────────────┐
        │  AIAgent     │
        │ (Shared)     │
        └──────────────┘

Phase 2 (2-4 周):
┌─────────────────────────────────────────┐
│  Tauri App  │  API Server  │ Batch      │
└──────┬──────────┬──────────┬────────────┘
       │          │          │
       └──────────┼──────────┘
                  ▼
        ┌──────────────────┐
        │  Tauri Commands  │
        │  + JSON-RPC API  │
        │  + Batch Runner  │
        └──────┬───────────┘
               ▼
        ┌──────────────┐
        │  AIAgent     │
        │ (Shared)     │
        └──────────────┘

Phase 3 (4-8 周):
┌─────────────────────────────────────────────┐
│  Tauri  │  API  │  Batch  │  IDE Plugins   │
└──────┬──────┬──────┬──────┬────────────────┘
       │      │      │      │
       └──────┼──────┼──────┘
              ▼      ▼
        ┌──────────────────┐
        │  Tauri Commands  │
        │  + JSON-RPC API  │
        │  + stdio/JSON    │
        └──────┬───────────┘
               ▼
        ┌──────────────┐
        │  AIAgent     │
        │ (Shared)     │
        └──────────────┘
```

---

## 2. Phase 1: Tauri 桌面应用

### 2.1 架构设计

```
┌──────────────────────────────────────┐
│  Tauri Window (Svelte UI)            │
│                                      │
│  Chat Input → Send Message Button    │
│                      │               │
│                      ▼               │
│             invoke('run_agent_turn')│
└──────────────────┬───────────────────┘
                   │
                   │ IPC (JSON)
                   │
┌──────────────────▼───────────────────┐
│  Tauri Backend (Rust)                │
│                                      │
│  #[tauri::command]                   │
│  async fn run_agent_turn(            │
│    state: State<AppState>,           │
│    message: String                   │
│  ) -> Result<AgentResponse>          │
│                                      │
│  ├─ Load session from state          │
│  ├─ Call agent.run_turn()            │
│  ├─ Save to session DB               │
│  └─ Return response                  │
│                                      │
└──────────────────┬───────────────────┘
                   │
                   ▼
        ┌──────────────────────┐
        │  Rust Modules        │
        │  (Agent + Support)   │
        └──────────────────────┘
```

### 2.2 Tauri 命令接口

**初始化**:

```rust
// src-tauri/src/main.rs
fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::agent::run_agent_turn,
            commands::agent::run_conversation,
            commands::session::list_sessions,
            commands::session::delete_session,
            commands::tools::list_tools,
            commands::tools::execute_tool,
            commands::memory::get_memory,
            commands::memory::save_memory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
```

**Command 层** (`src-tauri/src/commands/agent.rs`):

```rust
#[tauri::command]
pub async fn run_agent_turn(
    state: tauri::State<'_, AppState>,
    session_id: String,
    user_message: String,
) -> Result<RunAgentTurnResponse, String> {
    // Step 1: 验证会话存在
    let mut session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| format!("Session load failed: {}", e))?;

    // Step 2: 创建运行时（或复用已有的）
    let mut runtime = state
        .agent_factory
        .create_runtime_for_session(&session)
        .map_err(|e| format!("Runtime creation failed: {}", e))?;

    // Step 3: 执行一turn
    let response = runtime
        .run_turn(user_message)
        .await
        .map_err(|e| format!("Agent execution failed: {}", e))?;

    // Step 4: 保存消息和元数据
    state
        .session_manager
        .add_assistant_message(
            &session_id,
            response.message.clone(),
            response.tool_calls.clone(),
            response.input_tokens,
            response.output_tokens,
        )
        .await
        .map_err(|e| format!("Message save failed: {}", e))?;

    // Step 5: 返回结果
    Ok(RunAgentTurnResponse {
        content: response.message,
        tool_calls: response.tool_calls,
        tokens: TokenUsage {
            input: response.input_tokens,
            output: response.output_tokens,
            cost: response.cost,
        },
    })
}

#[tauri::command]
pub async fn run_conversation(
    state: tauri::State<'_, AppState>,
    session_id: String,
    messages: Vec<String>,
) -> Result<Summary, String> {
    // 连续执行多个 turn
    let mut results = Vec::new();
    for msg in messages {
        let result = run_agent_turn(state.clone(), session_id.clone(), msg).await?;
        results.push(result);
    }

    Ok(Summary {
        total_turns: results.len(),
        total_cost: results.iter().map(|r| r.tokens.cost).sum(),
    })
}
```

### 2.3 会话管理

**建立新会话**:

```
Tauri UI: Click "New Chat" Button
    │
    ▼
invoke('create_session', { title: "..." })
    │
    ▼
run_agent_turn 拿到 state.session_manager.create_session()
    │
    ▼
SessionManager creates:
    ├─ New session ID
    ├─ SQLite entry
    ├─ Empty message history
    └─ Default metadata
    │
    ▼
Return session_id to UI
    │
    ▼
UI stores session_id in Svelte store
    │
    ▼
后续 run_agent_turn() 使用该 session_id
```

**会话持久化**:

```rust
pub struct AppState {
    session_manager: Arc<SessionManager>,
    agent_factory: Arc<AgentFactory>,
    provider_manager: Arc<ProviderManager>,
    // ... etc
}

// 在 main.rs 中初始化
let session_manager = SessionManager::new("./data/sessions.db").await?;
let state = AppState {
    session_manager: Arc::new(session_manager),
    // ...
};
```

### 2.4 前端集成 (React + TypeScript)

```tsx
// src/lib/tauri.ts — Tauri IPC 封装层
import { invoke } from '@tauri-apps/api/tauri';

export interface AgentTurnResponse {
  message: string;
  session_id: string;
  tool_calls?: ToolCall[];
  tokens?: TokenUsage;
}

export interface SessionMeta {
  id: string;
  title: string;
  created_at: string;
}

export async function runAgentTurn(
  sessionId: string,
  userMessage: string
): Promise<AgentTurnResponse> {
  return await invoke<AgentTurnResponse>('run_agent_turn', {
    sessionId,
    userMessage,
  });
}

export async function listSessions(): Promise<SessionMeta[]> {
  return await invoke<SessionMeta[]>('list_sessions');
}

export async function deleteSession(id: string): Promise<void> {
  return await invoke<void>('delete_session', { id });
}

// src/App.tsx — React 组件
import { useState, useCallback } from 'react';
import { runAgentTurn } from './lib/tauri';

interface Message {
  role: 'user' | 'assistant';
  content: string;
}

export default function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputText, setInputText] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [sessionId, setSessionId] = useState<string>('');

  const handleSendMessage = useCallback(async () => {
    if (!inputText.trim() || isLoading) return;

    const userMsg = inputText;
    setInputText('');
    setIsLoading(true);

    // 添加用户消息到 UI
    setMessages(prev => [...prev, { role: 'user', content: userMsg }]);

    try {
      const response = await runAgentTurn(sessionId, userMsg);
      // 添加助手响应到 UI
      setMessages(prev => [...prev, { role: 'assistant', content: response.message }]);
      if (response.session_id && !sessionId) {
        setSessionId(response.session_id);
      }
    } catch (err) {
      // 错误展示（友好错误消息，不暴露内部细节）
      const errorMessage = err instanceof Error ? err.message : 'Agent execution failed';
      setMessages(prev => [...prev, {
        role: 'assistant',
        content: `Error: ${errorMessage}`
      }]);
    } finally {
      setIsLoading(false);
    }
  }, [inputText, isLoading, sessionId]);

  return (
    <div className="chat-container">
      <div className="messages">
        {messages.map((msg, i) => (
          <div key={i} className={`message ${msg.role}`}>
            {msg.content}
          </div>
        ))}
      </div>
      <div className="input-area">
        <input
          value={inputText}
          onChange={e => setInputText(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && !e.shiftKey && handleSendMessage()}
          placeholder="Type a message..."
          disabled={isLoading}
        />
        <button onClick={handleSendMessage} disabled={isLoading || !inputText.trim()}>
          {isLoading ? 'Sending...' : 'Send'}
        </button>
      </div>
    </div>
  );
}
```

**关键实现要点**：
- `src/lib/tauri.ts` 封装所有 `invoke` 调用，App.tsx **不直接调用** `@tauri-apps/api`
- `isLoading` 状态在发送中禁用按钮，防止重复提交
- 错误消息对用户友好（不暴露 Rust 内部错误细节）
- TypeScript 类型与 Rust `RunAgentTurnResponse` 完全一致

---

## 3. Phase 2: API 服务器

### 3.1 架构设计

```
┌─────────────────────────────────┐
│  标准 HTTP/JSON-RPC 客户端      │
│  (Web, Mobile, CLI)             │
└──────────────┬──────────────────┘
               │
               │ HTTP/JSON-RPC
               │
┌──────────────▼──────────────────┐
│  JSON-RPC 2.0 Server            │
│  (Actix-web or Axum)            │
│                                 │
│  POST /rpc                      │
│  {                              │
│    "jsonrpc": "2.0",            │
│    "method": "run_agent_turn",  │
│    "params": {...},             │
│    "id": 1                      │
│  }                              │
│                                 │
│  ├─ Authentication (Bearer token)
│  ├─ Rate limiting               │
│  ├─ Request validation          │
│  └─ Response formatting         │
│                                 │
└──────────────┬──────────────────┘
               │
               ▼
   ┌──────────────────────┐
   │  Shared Command Impl │
   │  (Reuse Tauri Code)  │
   └──────────────────────┘
```

### 3.2 API 端点

```rust
// JSON-RPC 方法

// 会话管理
method: "create_session"
params: { title: "string" }
result: { session_id: "uuid" }

method: "list_sessions"
params: { user_id: "string" }
result: { sessions: [Session] }

method: "delete_session"
params: { session_id: "uuid" }
result: { status: "ok" }

// Agent 控制
method: "run_agent_turn"
params: { session_id: "uuid", message: "string" }
result: { content: "string", tokens: {...} }

method: "run_conversation"
params: { session_id: "uuid", messages: ["msg1", "msg2"] }
result: { summary: {...} }

// 工具管理
method: "list_tools"
params: {}
result: { tools: [ToolSchema] }

method: "execute_tool"
params: { tool_name: "string", arguments: {...} }
result: { output: "string" }

// 记忆管理
method: "get_memory"
params: { user_id: "string", type: "soul|memory|user" }
result: { [type]: {...} }

method: "save_memory"
params: { user_id: "string", memory: {...} }
result: { status: "ok" }
```

### 3.3 实现 (Rust + Actix-web/Axum)

```rust
// src-tauri/src/api/mod.rs
use actix_web::{middleware, web, App, HttpServer};
use jsonrpc_v2::{JsonRpcError, JsonRpcParams, JsonRpcServer};

pub struct ApiServer {
    app_state: Arc<AppState>,
    port: u16,
}

impl ApiServer {
    pub async fn start(self) -> std::io::Result<()> {
        let app_state = self.app_state.clone();

        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(app_state.clone()))
                .wrap(middleware::Logger::default())
                .wrap(middleware::NormalizePath::trim())
                // JSON-RPC 2.0 endpoint
                .service(
                    web::scope("/rpc")
                        .route("/", web::post().to(handle_rpc_request))
                )
                // Health check
                .service(
                    web::scope("/health")
                        .route("/", web::get().to(health_check))
                )
        })
        .bind(("127.0.0.1", self.port))?
        .run()
        .await
    }
}

// JSON-RPC request handler
async fn handle_rpc_request(
    state: web::Data<Arc<AppState>>,
    body: web::Json<serde_json::Value>,
) -> web::Json<serde_json::Value> {
    // 解析 JSON-RPC 请求
    let method = body["method"].as_str().unwrap_or("");
    let params = &body["params"];
    let id = body["id"].clone();

    // 调用对应的 command handler
    let result = match method {
        "run_agent_turn" => {
            handle_run_agent_turn(&state, params).await
        }
        "list_sessions" => {
            handle_list_sessions(&state, params).await
        }
        // ... 其他方法
        _ => {
            return web::Json(json!({
                "jsonrpc": "2.0",
                "error": "Method not found",
                "id": id
            }));
        }
    };

    // 返回 JSON-RPC 响应
    web::Json(json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    }))
}
```

---

## 4. Phase 3: IDE 集成 (LSP/ACP)

### 4.1 架构设计

```
┌──────────────────────────────────┐
│  VS Code / Zed / JetBrains       │
│                                  │
│  Editor Context API:             │
│  ├─ Open files                   │
│  ├─ Selection                    │
│  ├─ Current directory            │
│  └─ Diagnostics                  │
│                                  │
└──────────────┬───────────────────┘
               │
               │ stdio / JSON-RPC 2.0
               │
┌──────────────▼───────────────────┐
│  LSP Server (vs Code extensions) │
│                                  │
│  Implements:                      │
│  ├─ initialize                   │
│  ├─ completion/resolve           │
│  ├─ hover/definition             │
│  ├─ execute_command              │
│  ├─ workspace/executeCommand     │
│  └─ Custom notifications         │
│                                  │
└──────────────┬───────────────────┘
               │
               ▼
   ┌──────────────────────┐
   │  Shared Command Impl │
   │  (Reuse Tauri Code)  │
   └──────────────────────┘
```

### 4.2 LSP 命令

```rust
// Custom commands for IDE plugin

method: "run_in_context"
params: {
  prompt: "string",
  files: ["file1.ts", "file2.ts"],
  insertLocation: "editor|sidebar",
}
result: { output: "string", insertedText: "string" }

method: "refactor_selection"
params: {
  selectedCode: "string",
  language: "typescript|python|rust",
  refactorType: "extract|rename|simplify",
}
result: { refactoredCode: "string" }

method: "explain_code"
params: { code: "string" }
result: { explanation: "string" }

method: "fix_diagnostics"
params: { diagnostics: [Diagnostic] }
result: { fixes: [CodeAction] }
```

### 4.3 VS Code 扩展实现

```typescript
// vscode-extension/src/extension.ts
import * as vscode from 'vscode';
import { LanguageClient, ... } from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
  // 启动 LSP 服务器
  const serverModule = context.asAbsolutePath(
    path.join('..', 'target', 'release', 'if2ai-lsp')
  );

  const serverOptions = {
    run: { command: serverModule },
    debug: { command: serverModule, args: ['--debug'] }
  };

  const clientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'typescript' },
      { scheme: 'file', language: 'python' },
      { scheme: 'file', language: 'rust' },
    ],
  };

  client = new LanguageClient(
    'if2ai-lsp',
    'If2Ai Agent LSP',
    serverOptions,
    clientOptions
  );

  client.start();

  // 注册命令
  context.subscriptions.push(
    vscode.commands.registerCommand('if2ai.runInContext', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;

      const selectedCode = editor.document.getText(editor.selection);
      const response = await client.sendRequest('run_in_context', {
        prompt: 'Explain and improve this code:',
        files: [editor.document.fileName],
        code: selectedCode,
      });

      // 在 sidebar 中显示结果
      showResultPanel(response);
    })
  );
}

function showResultPanel(response: any) {
  const panel = vscode.window.createWebviewPanel(
    'if2aiResult',
    'If2Ai Result',
    vscode.ViewColumn.Beside
  );

  panel.webview.html = getWebviewContent(response);
}
```

---

## 5. 数据流对比

### 5.1 三个入口点的消息流

```
Tauri (Phase 1):
User Input → Svelte UI → invoke('run_agent_turn')
→ Tauri Command → Agent → Response → UI Update

API (Phase 2):
HTTP Client → POST /rpc → JSON-RPC Handler
→ Agent → JSON Response → HTTP Client

IDE (Phase 3):
Editor Context → LSP Client → stdio JSON-RPC
→ LSP Server → Agent → LSP Notification → Editor
```

### 5.2 共享代码

所有三个入口点都底层调用相同的 **Command Handlers**：

```rust
// 核心业务逻辑在这里，三个入口点都复用

mod commands {
    pub mod agent {
        pub async fn run_agent_turn(
            state: &AppState,
            session_id: &str,
            message: &str,
        ) -> Result<AgentResponse> {
            // 核心逻辑，所有入口点都调用这个
        }
    }

    pub mod session { ... }
    pub mod tools { ... }
    pub mod memory { ... }
}

// Tauri 命令直接调用
#[tauri::command]
pub async fn run_agent_turn(
    state: State<AppState>,
    session_id: String,
    message: String,
) -> Result<AgentResponse> {
    commands::agent::run_agent_turn(&state, &session_id, &message).await
}

// JSON-RPC 端点也调用相同的函数
async fn handle_run_agent_turn(
    state: &AppState,
    params: &serde_json::Value,
) -> Result<AgentResponse> {
    let session_id = params["session_id"].as_str().unwrap();
    let message = params["message"].as_str().unwrap();
    commands::agent::run_agent_turn(state, session_id, message).await
}

// LSP 服务器同样调用
async fn handle_lsp_request(
    state: &AppState,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    match method {
        "run_in_context" => {
            let prompt = params["prompt"].as_str().unwrap();
            // 实际上仍然调用底层 Agent
            let response = state.agent.run_turn(prompt).await?;
            Ok(json!({ "output": response }))
        }
        _ => Err(Error::MethodNotFound),
    }
}
```

---

## 6. 隔离和安全

### 6.1 用户隔离

每个用户有自己的会话集合：

```rust
pub struct Session {
    id: String,
    user_id: String,  // ← 关键：用户隔离
    // ... 其他字段
}

// 在 API 层验证用户
#[tauri::command]
pub async fn list_sessions(
    state: State<AppState>,
    auth: AuthToken,  // JWT or session token
) -> Result<Vec<SessionSummary>> {
    let user_id = auth.verify()?.user_id;
    state.session_manager.list_sessions(&user_id).await
}
```

### 6.2 速率限制

在 API 层添加速率限制：

```rust
use actix_web_httpauth::middleware::HttpAuthentication;
use actix_ratelimit::RateLimiter;

App::new()
    .wrap(HttpAuthentication::bearer(validate_token))
    .wrap(RateLimiter::default())  // 防止滥用
    .service(/* ... */)
```

### 6.3 权限检查

某些操作需要特殊权限：

```rust
pub struct AuthToken {
    user_id: String,
    permissions: Vec<Permission>,
}

pub enum Permission {
    ReadSessions,
    WriteSessions,
    ExecuteTools,
    AccessMemory,
    ManagePlugins,
}

// 权限检查
if !auth.has_permission(Permission::ExecuteTools) {
    return Err("Permission denied".into());
}
```

---

## 7. 部署拓扑

### 7.1 Phase 1 (Tauri 桌面应用)

```
User Machine
    │
    ├─ Tauri App (Desktop)
    │   ├─ Rust Backend
    │   │   ├─ Agent
    │   │   ├─ Tools
    │   │   └─ Session DB (local SQLite)
    │   │
    │   └─ Svelte UI
    │
    └─ External APIs (通过网络)
        ├─ OpenAI / Anthropic / etc
        └─ Web Search / Vision APIs
```

### 7.2 Phase 2 (API 服务器)

```
┌─────────────────────────────────────┐
│  Optional Deployment Server         │
├─────────────────────────────────────┤
│  → If2Ai API Server (Rust)          │
│    ├─ Actix-web / Axum             │
│    ├─ Agent + Tools                │
│    └─ SQLite or PostgreSQL DB      │
│                                     │
│  ← Multiple Clients:                │
│    ├─ Tauri Desktop App            │
│    ├─ Web Client                   │
│    ├─ Mobile App (future)          │
│    └─ CLI Tools                    │
└─────────────────────────────────────┘
```

### 7.3 Phase 3 (完整生态)

```
┌──────────────────────────────────────────┐
│  Central If2Ai Platform                  │
├──────────────────────────────────────────┤
│                                          │
│  ┌─ Tauri Desktop ──┐                   │
│  │                  │                   │
│  ├─ API Server ─────┼─── PostgreSQL    │
│  │  (Rust)          │                   │
│  │  - Async workers │                   │
│  │  - Job scheduler │                   │
│  │  - Cron          │                   │
│  │                  │                   │
│  ├─ IDE Plugins ────┤                   │
│  │  (LSP)           │                   │
│  │                  │                   │
│  ├─ Gateway ────────┼─── Multi-Platform │
│  │  (Messaging)     │   Adapters        │
│  │                  │   - Slack         │
│  │                  │   - Discord       │
│  │                  │   - Telegram      │
│  │                  │   - etc           │
│  │                  │                   │
│  └──────────────────┘                   │
│                                          │
└──────────────────────────────────────────┘
```

---

## 8. 快速参考

### 添加新的 Tauri 命令

```rust
// 1. 实现业务逻辑（在 modules/ 中）
impl AgentModule {
    pub async fn do_something(&self, param: &str) -> Result<Response> {
        // 逻辑
    }
}

// 2. 创建 Command 包装（在 commands/ 中）
#[tauri::command]
pub async fn my_command(
    state: State<AppState>,
    param: String,
) -> Result<Response> {
    state.agent_module.do_something(&param).await
}

// 3. 在 main.rs 注册
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![my_command])
```

### 添加新的 API 端点

```rust
// 1. 在 commands/ 中实现（复用业务逻辑）

// 2. 在 api/handlers.rs 中包装
async fn handle_my_endpoint(
    state: web::Data<Arc<AppState>>,
    params: web::Json<RequestParams>,
) -> HttpResponse {
    match commands::my_command::do_something(state, params).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::InternalServerError().json(error_response(e)),
    }
}

// 3. 在 main.rs 注册路由
.service(
    web::scope("/api")
        .post("/my-endpoint", handle_my_endpoint)
)
```

---

## 9. 参考链接

- [System Architecture Framework](./system-architecture-framework.md)
- [Module Boundaries and Integration](./module-boundaries-and-integration.md)
- [Agent Loop Design](./agent-loop.md)

---

**版本**: 1.0 | **最后更新**: 2026-04-11
