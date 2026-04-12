# Tool Activation — 前端工具调用与注册激活

**版本**: 1.0
**最后更新**: 2026-04-12
**状态**: Implementation-ready
**依赖**: Phase 1 slices 1.5 (ToolRegistry), 1.7 (Tauri Commands), 1.9 (集成测试)

---

## 1. 背景与问题

### 1.1 现状

`tool-system.md` 定义了完整的工具基础设施：

```
ToolRegistry（DashMap 注册表）
ToolExecutor trait
3 个内置工具：bash, read_file, json_parse
MCP Client 支持
```

**但实际状态是**：

1. `ToolRegistry` 在 `main.rs` 中创建时是**空的** — 没有任何工具注册
2. `ConversationRuntime` 的 `MessageRequest` 中 `tools: None` — 即使注册了也不传给 LLM
3. 前端无法直接触发工具调用 — 只能通过 Agent 循环间接使用
4. 没有 `execute_tool` Tauri 命令暴露给前端

### 1.2 需要做什么

| 优先级 | 任务 | 当前状态 |
|--------|------|----------|
| P0 | 注册内置工具到 `tool_registry` | ❌ 未做 |
| P0 | 让 `MessageRequest` 包含工具定义 | ❌ `tools: None` |
| P1 | 新增 `execute_tool` Tauri 命令 | ❌ 不存在 |
| P1 | 前端 `lib/tauri.ts` 新增工具调用接口 | ❌ 不存在 |
| P2 | 前端工具选择 UI | ⏸️ 后续迭代 |

---

## 2. 系统设计

### 2.1 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                         Frontend (React)                        │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │ useTools()   │    │ ChatUI       │    │ ToolSelectorUI   │  │
│  │ - list()     │    │ (消息展示)   │    │ (工具选择面板)   │  │
│  │ - execute()   │    └──────────────┘    └──────────────────┘  │
│  │ - definitions│                                           │  │
│  └──────┬───────┘                                           │  │
└─────────┼───────────────────────────────────────────────────┘  │
          │ invoke('execute_tool', { name, args })              │
          ▼                                                     │
┌─────────────────────────────────────────────────────────────────┐
│                     Tauri Commands (IPC)                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ execute_tool(name, args) → Result<String, ToolError>     │  │
│  │ list_tools() → Vec<ToolDefinition>                        │  │
│  │ get_tool_definitions() → Vec<Value>                      │  │
│  └──────────────────────────┬───────────────────────────────┘  │
└──────────────────────────────┼──────────────────────────────────┘
                               │ Arc<AppState>
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         AppState                                │
│  ┌──────────────────┐    ┌─────────────────────────────────┐   │
│  │ SessionManager    │    │ ToolRegistry (已注册)            │   │
│  └──────────────────┘    │  ├─ bash (terminal)              │   │
│                          │  ├─ read_file (files)            │   │
│                          │  ├─ json_parse (utility)         │   │
│                          │  └─ [MCP tools via mcp_client]  │   │
│                          └─────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   ConversationRuntime                           │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ MessageRequest {                                        │   │
│  │   tools: Some(tool_registry.get_definitions(...)),  ✅   │   │
│  │   ...                                                    │   │
│  │ }                                                        │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 核心改动点

#### 2.2.1 main.rs — 注册内置工具

```rust
// src-tauri/src/main.rs

// 改动前（空注册表）
let tool_registry = modules::tools::ToolRegistry::new();

// 改动后（注册内置工具）
let tool_registry = modules::tools::ToolRegistry::new();
modules::tools::register_builtin_tools(&tool_registry);

// 新增函数：src-tauri/src/modules/tools/mod.rs
pub fn register_builtin_tools(registry: &ToolRegistry) {
    registry.register(bash::entry()).ok();
    registry.register(file_read::entry()).ok();
    registry.register(json_parse::entry()).ok();
}
```

#### 2.2.2 agent.rs — 传递工具定义给 LLM

```rust
// src-tauri/src/commands/agent.rs

// 改动前
let request = MessageRequest {
    tools: None,  // ❌ 不传工具
    ..
};

// 改动后
let allowed_tools: Option<Vec<String>> = None; // 或从 project config 读取
let request = MessageRequest {
    tools: Some(state.tool_registry.get_definitions(allowed_tools)),  // ✅
    ..
};
```

#### 2.2.3 commands/tools.rs — 新增工具调用命令（新增文件）

```rust
// src-tauri/src/commands/tools.rs (新建)

#[tauri::command]
pub async fn execute_tool(
    state: State<'_, AppState>,
    name: String,
    args: String,  // JSON string
) -> Result<String, String>

#[tauri::command]
pub fn list_tools(
    state: State<'_, AppState>,
) -> Result<Vec<ToolDefinition>, String>

#[tauri::command]
pub fn get_tool_definitions(
    state: State<'_, AppState>,
    allowed: Option<Vec<String>>,
) -> Result<Vec<Value>, String>
```

#### 2.2.4 lib/tauri.ts — 前端接口

```typescript
// src/lib/tauri.ts

export interface ToolDefinition {
  name: string;
  description: string;
  input_schema: object;
}

// 新增接口
export async function executeTool(name: string, args: object): Promise<string>
export async function listTools(): Promise<ToolDefinition[]>
export async function getToolDefinitions(allowed?: string[]): Promise<object[]>
```

---

## 3. 工具注册流程

### 3.1 ToolEntry 结构（已存在）

```rust
// src-tauri/src/modules/tools/registry.rs

pub struct ToolEntry {
    pub name: String,           // "bash", "read_file"
    pub toolset: String,         // "terminal", "files", "utility"
    pub description: String,
    pub input_schema: Value,    // JSON Schema for validation
    pub max_result_size: Option<usize>,
    pub timeout_secs: Option<u32>,
    pub disabled: bool,
    pub handler: ToolHandler,
}
```

### 3.2 内置工具定义

#### bash tool

```rust
// src-tauri/src/modules/tools/builtin/bash.rs

pub fn entry() -> ToolEntry {
    ToolEntry {
        name: "bash".into(),
        toolset: "terminal".into(),
        description: "Execute a bash command in the host environment".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The bash command to execute" },
                "timeout": { "type": "number", "description": "Timeout in seconds (default: 30, max: 300)" }
            },
            "required": ["command"]
        }),
        max_result_size: Some(1024 * 1024),  // 1MB
        timeout_secs: Some(300),               // 5 min max
        disabled: false,
        handler: Arc::new(execute_bash),
    }
}
```

#### read_file tool

```rust
// src-tauri/src/modules/tools/builtin/file_read.rs

pub fn entry() -> ToolEntry {
    ToolEntry {
        name: "read_file".into(),
        toolset: "files".into(),
        description: "Read the contents of a file".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file" },
                "offset": { "type": "number", "description": "Byte offset to start reading from" },
                "limit": { "type": "number", "description": "Maximum number of bytes to read" }
            },
            "required": ["path"]
        }),
        max_result_size: Some(1024 * 1024),  // 1MB
        timeout_secs: Some(30),
        disabled: false,
        handler: Arc::new(execute_read_file),
    }
}
```

#### json_parse tool

```rust
// src-tauri/src/modules/tools/builtin/json_parse.rs

pub fn entry() -> ToolEntry {
    ToolEntry {
        name: "json_parse".into(),
        toolset: "utility".into(),
        description: "Parse and format a JSON string".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "JSON string to parse" },
                "pretty": { "type": "boolean", "description": "Pretty print the output (default: true)" }
            },
            "required": ["input"]
        }),
        max_result_size: Some(1024 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler: Arc::new(execute_json_parse),
    }
}
```

---

## 4. 工具集（ToolSet）分类系统

### 4.1 现状

现有 `ToolEntry` 已有 `toolset: String` 字段，但缺少统一管理机制：

```rust
// 已有字段（registry.rs:59）
pub toolset: String,  // 如 "terminal", "files", "utility"
```

缺少：
- `ToolSet` 结构定义
- 预定义工具集常量
- 按工具集批量查询/启用的 API

### 4.2 ToolSet 数据结构

```rust
// src-tauri/src/modules/tools/toolset.rs (新建)

/// Represents a named group of tools with common characteristics.
#[derive(Debug, Clone)]
pub struct ToolSet {
    /// ToolSet name (e.g., "files", "terminal", "web")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Tool names belonging to this set
    pub tools: Vec<String>,
    /// Whether this ToolSet is enabled by default
    pub enabled: bool,
}

/// Predefined toolset classification constants
/// 来源参考：zeroclaw-master/src/tools/mod.rs TOOLSETS
pub const TOOLSETS: &[(&str, &str, &[&str])] = &[
    // (name, description, tool_names)
    ("files", "File operations: read, write, edit, search", &[
        "read_file", "file_write", "file_edit", "glob_search", "content_search"
    ]),
    ("terminal", "Terminal/shell execution", &[
        "bash"
    ]),
    ("utility", "Utility tools: JSON parsing, calculations", &[
        "json_parse", "calculator"
    ]),
    ("web", "Web fetching and searching", &[
        "web_fetch", "web_search", "http_request"
    ]),
    ("memory", "Long-term memory storage and retrieval", &[
        "memory_store", "memory_recall", "memory_forget", "memory_purge", "memory_export"
    ]),
    ("scheduler", "Cron/scheduled task management", &[
        "cron_add", "cron_list", "cron_remove", "cron_run", "cron_runs"
    ]),
    // Composite toolsets
    ("minimal", "Minimal set for basic tasks", &[
        "read_file", "bash"
    ]),
    ("development", "Full development stack", &[
        "files", "terminal"
    ]),
];
```

### 4.3 ToolSetRegistry 实现

```rust
// src-tauri/src/modules/tools/toolset.rs (新建)

use std::collections::HashMap;

pub struct ToolSetRegistry {
    /// name → ToolSet mapping
    toolsets: HashMap<String, ToolSet>,
    /// tool name → toolset name reverse lookup
    tool_to_toolset: HashMap<String, String>,
}

impl ToolSetRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            toolsets: HashMap::new(),
            tool_to_toolset: HashMap::new(),
        };
        registry.register_default_toolsets();
        registry
    }

    fn register_default_toolsets(&mut self) {
        for (name, description, tools) in TOOLSETS {
            let toolset = ToolSet {
                name: name.to_string(),
                description: description.to_string(),
                tools: tools.iter().map(|s| s.to_string()).collect(),
                enabled: true,
            };
            for tool_name in &toolset.tools {
                self.tool_to_toolset.insert(tool_name.clone(), name.to_string());
            }
            self.toolsets.insert(name.to_string(), toolset);
        }
    }

    /// Get all toolset names.
    pub fn toolset_names(&self) -> Vec<String> {
        self.toolsets.keys().cloned().collect()
    }

    /// Get a toolset by name.
    pub fn get(&self, name: &str) -> Option<&ToolSet> {
        self.toolsets.get(name)
    }

    /// Get toolset for a specific tool.
    pub fn toolset_of(&self, tool_name: &str) -> Option<String> {
        self.tool_to_toolset.get(tool_name).cloned()
    }

    /// Get all tool names in a toolset.
    pub fn tools_in_toolset(&self, toolset_name: &str) -> Vec<String> {
        self.toolsets
            .get(toolset_name)
            .map(|ts| ts.tools.clone())
            .unwrap_or_default()
    }

    /// Get all enabled toolset names.
    pub fn enabled_toolsets(&self) -> Vec<String> {
        self.toolsets
            .values()
            .filter(|ts| ts.enabled)
            .map(|ts| ts.name.clone())
            .collect()
    }

    /// Get all tool names from multiple toolsets.
    /// Returns union of all tools in the specified toolsets.
    pub fn tools_from_toolsets(&self, toolsets: &[String]) -> Vec<String> {
        let mut tools = Vec::new();
        for ts_name in toolsets {
            if let Some(ts) = self.toolsets.get(ts_name) {
                for tool in &ts.tools {
                    if !tools.contains(tool) {
                        tools.push(tool.clone());
                    }
                }
            }
        }
        tools
    }
}
```

### 4.4 与 ToolRegistry 的集成

```rust
// src-tauri/src/modules/tools/mod.rs

pub mod toolset;  // 新增

pub use toolset::{ToolSet, ToolSetRegistry, TOOLSETS};
```

```rust
// 在 ToolRegistry 中新增按 toolset 过滤的方法

impl ToolRegistry {
    /// Get tool definitions filtered by toolsets.
    /// Returns all tools belonging to any of the specified toolsets.
    pub fn get_definitions_by_toolsets(
        &self,
        toolsets: &[String],
        toolset_registry: &ToolSetRegistry,
    ) -> Vec<Value> {
        let allowed = toolset_registry.tools_from_toolsets(toolsets);
        self.get_definitions(Some(&allowed))
    }

    /// Get tool definitions for a single toolset.
    pub fn get_definitions_in_toolset(
        &self,
        toolset_name: &str,
        toolset_registry: &ToolSetRegistry,
    ) -> Vec<Value> {
        let tools = toolset_registry.tools_in_toolset(toolset_name);
        self.get_definitions(Some(&tools))
    }
}
```

### 4.5 前端 TypeScript 接口

```typescript
// src/lib/tauri.ts

export interface ToolSet {
  name: string;
  description: string;
  tools: string[];
  enabled: boolean;
}

/**
 * Get all available toolsets with their tools.
 */
export async function listToolsets(): Promise<ToolSet[]> {
  return invoke<ToolSet[]>('list_toolsets');
}

/**
 * Get tool definitions filtered by toolsets.
 */
export async function getToolDefinitionsByToolsets(
  toolsets: string[]
): Promise<object[]> {
  return invoke<object[]>('get_tool_definitions_by_toolsets', { toolsets });
}
```

### 4.6 Rust Tauri 命令

```rust
// src-tauri/src/commands/tools.rs (扩展)

#[tauri::command]
pub fn list_toolsets(
    state: State<'_, AppState>,
) -> Result<Vec<ToolSet>, String> {
    Ok(state.toolset_registry.enabled_toolsets()
        .iter()
        .filter_map(|name| state.toolset_registry.get(name))
        .cloned()
        .collect())
}

#[tauri::command]
pub fn get_tool_definitions_by_toolsets(
    state: State<'_, AppState>,
    toolsets: Vec<String>,
) -> Result<Vec<serde_json::Value>, String> {
    state.tool_registry
        .get_definitions_by_toolsets(&toolsets, &state.toolset_registry)
        .map_err(|e| e.to_string())
}
```

---

## 5. 前端工具调用接口

### 5.1 TypeScript Interface

```typescript
// src/lib/tauri.ts

export interface ToolCallResult {
  success: boolean;
  output?: string;
  error?: string;
}

/**
 * Execute a tool directly from the frontend.
 * Used for: manual tool invocation, debugging, direct file operations.
 */
export async function executeTool(
  name: string,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  try {
    const result = await invoke<string>('execute_tool', { name, args: JSON.stringify(args) });
    return { success: true, output: result };
  } catch (e) {
    return { success: false, error: String(e) };
  }
}

/**
 * List all available tools with their definitions.
 */
export async function listTools(): Promise<ToolDefinition[]> {
  return invoke<ToolDefinition[]>('list_tools');
}

/**
 * Get tool definitions in OpenAI format for LLM consumption.
 */
export async function getToolDefinitions(allowed?: string[]): Promise<object[]> {
  return invoke<object[]>('get_tool_definitions', { allowed });
}
```

### 5.2 React Hook（可选，后续迭代）

```typescript
// src/hooks/useTools.ts (future iteration)

export function useTools() {
  const [tools, setTools] = useState<ToolDefinition[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    listTools().then(setTools);
  }, []);

  const execute = async (name: string, args: Record<string, unknown>) => {
    setLoading(true);
    try {
      return await executeTool(name, args);
    } finally {
      setLoading(false);
    }
  };

  return { tools, execute, loading };
}
```

---

## 6. 错误处理

### 6.1 ToolError 类型（已存在）

```rust
// src-tauri/src/modules/tools/registry.rs

pub enum ToolError {
    NotFound(String),           // 工具不存在
    Disabled(String),           // 工具已被禁用
    Timeout(String),            // 执行超时
    OutputTooLarge { size: usize, max: usize },
    Register(String),           // 注册失败
    Handler(String),            // 处理函数错误
}
```

### 6.2 前端错误映射

| ToolError | 前端处理 |
|-----------|----------|
| `NotFound` | 提示"工具不存在" |
| `Disabled` | 提示"工具已禁用" |
| `Timeout` | 提示"执行超时，请重试" |
| `OutputTooLarge` | 提示"输出过大，已截断" |
| `Handler` | 提示"工具执行错误" + 错误详情 |

---

## 7. 安全性考虑

### 7.1 当前内置工具的限制

| 工具 | 已有保护 | 缺失 |
|------|----------|------|
| `bash` | 危险命令 denylist（fork bombs 等） | 无 workdir 限制 |
| `read_file` | 敏感路径 denylist（`/etc/passwd` 等） | 无 workdir allowlist |
| `json_parse` | 无 | — |

### 7.2 未来安全增强（不在本 Phase）

- Workdir allowlist 限制（见 `project-workdir-boundary.md`）
- API 配额限制
- 工具使用审计日志
- 每工具可配置的权限模式

---

## 8. 与 Phase 1 tool-system.md 的差异

| 方面 | Phase 1 (tool-system.md) | 本 Phase |
|------|-------------------------|----------|
| 工具注册 | 定义了 API，未实现 | 实现 `register_builtin_tools()` |
| LLM 集成 | `tools: None` | `tools: Some(...)` 传给 LLM |
| 前端调用 | 无 | 新增 `execute_tool` 命令 |
| MCP 工具 | 定义了 client | 未激活（MCP 后续迭代） |

---

## 9. 实现顺序

1. **P0**: `register_builtin_tools()` + 修改 `agent.rs` 传工具定义
2. **P1**: 新增 `commands/tools.rs` 三个命令
3. **P1**: 前端 `lib/tauri.ts` 接口
4. **P2**: 前端工具选择 UI（可选）

---

## 10. 新工具克隆清单（从 ZeroClaw 复刻）

> **来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/`
> **目标**: `src-tauri/src/modules/tools/builtin/`

### 10.1 工具分类总览

| 类别 | 工具数 | 优先级 | 实现 Slice |
|------|--------|--------|-----------|
| **文件操作类** | 4 | P0 | 4.3 |
| **搜索类** | 2 | P0 | 4.3 |
| **Web 类** | 3 | P1 | 4.8 |
| **Memory 类** | 5 | P1 | 4.9 |
| **Cron/调度类** | 7 | P1 | 4.10 |
| **Browser 类** | 4 | P2 | 后续 |
| **Git/代码类** | 6 | P2 | 后续 |
| **第三方集成类** | 8+ | P2 | 后续 |
| **杂项工具** | 10+ | P3 | 后续 |

---

### 10.2 文件操作类工具

#### 9.2.1 file_write

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/file_write.rs`

```rust
pub struct FileWriteTool {
    security: Arc<SecurityPolicy>,
}

impl FileWriteTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self { ... }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str { "file_write" }

    fn description(&self) -> &str {
        "Write content to a file in the workspace. Creates or overwrites the file."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path from workspace root" },
                "content": { "type": "string", "description": "Content to write to the file" },
                "append": { "type": "boolean", "description": "Append to existing file instead of overwriting" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str())?;
        let content = args.get("content").and_then(|v| v.as_str())?;
        let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

        // Security check: path must be within workspace
        let full_path = self.security.workspace_dir.join(path);
        if !full_path.starts_with(&self.security.workspace_dir) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Path outside workspace".into()) });
        }

        let write_mode = if append { std::fs::OpenOptions::new().append(true).create(true) } else { std::fs::OpenOptions::new().write(true).create(true).truncate(true) };

        match write_mode.open(&full_path) {
            Ok(mut file) => {
                use std::io::Write;
                if file.write_all(content.as_bytes()).is_ok() {
                    Ok(ToolResult { success: true, output: format!("Written to {path} ({} bytes)", content.len()), error: None })
                } else {
                    Ok(ToolResult { success: false, output: String::new(), error: Some("Write failed".into()) })
                }
            }
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Failed to open file: {e}")) })
        }
    }
}
```

---

#### 9.2.2 file_edit

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/file_edit.rs`

```rust
pub struct FileEditTool {
    security: Arc<SecurityPolicy>,
}

impl FileEditTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self { ... }

    pub fn apply_diff(content: &str, diff: &str) -> Result<String, String> {
        // Parse unified diff format and apply changes
        // Returns the modified content
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str { "file_edit" }

    fn description(&self) -> &str {
        "Edit a file by applying a unified diff patch"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path from workspace root" },
                "diff": { "type": "string", "description": "Unified diff patch to apply" }
            },
            "required": ["path", "diff"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str())?;
        let diff = args.get("diff").and_then(|v| v.as_str())?;

        // Security check
        let full_path = self.security.workspace_dir.join(path);
        if !full_path.starts_with(&self.security.workspace_dir) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Path outside workspace".into()) });
        }

        // Read original content
        let original = tokio::fs::read_to_string(&full_path).await
            .map_err(|e| format!("Failed to read file: {e}"))?;

        // Apply diff
        match Self::apply_diff(&original, diff) {
            Ok(new_content) => {
                tokio::fs::write(&full_path, &new_content).await
                    .map_err(|e| format!("Failed to write file: {e}"))?;
                Ok(ToolResult { success: true, output: format!("Applied diff to {path}"), error: None })
            }
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(e) })
        }
    }
}
```

---

#### 9.2.3 glob_search

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/glob_search.rs`

```rust
pub struct GlobSearchTool {
    security: Arc<SecurityPolicy>,
}

impl GlobSearchTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self { ... }
}

#[async_trait]
impl Tool for GlobSearchTool {
    fn name(&self) -> &str { "glob_search" }

    fn description(&self) -> &str {
        "Search for files matching a glob pattern in the workspace"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.ts')" },
                "path": { "type": "string", "description": "Root directory to search from (default: workspace root)" },
                "max_results": { "type": "number", "description": "Maximum number of results (default: 100)" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let pattern = args.get("pattern").and_then(|v| v.as_str())?;
        let root = args.get("path").and_then(|v| v.as_str())
            .map(|p| self.security.workspace_dir.join(p))
            .unwrap_or_else(|| self.security.workspace_dir.clone());
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        // Security: ensure root is within workspace
        if !root.starts_with(&self.security.workspace_dir) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Path outside workspace".into()) });
        }

        let mut results = Vec::new();
        // Use glob crate to find matching files
        for entry in glob::glob(&root.join(pattern).display().to_string())? {
            if let Ok(path) = entry {
                results.push(path.display().to_string());
                if results.len() >= max_results {
                    break;
                }
            }
        }

        let output = results.join("\n");
        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

#### 9.2.4 content_search

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/content_search.rs`

```rust
pub struct ContentSearchTool {
    security: Arc<SecurityPolicy>,
}

impl ContentSearchTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self { ... }
}

#[async_trait]
impl Tool for ContentSearchTool {
    fn name(&self) -> &str { "content_search" }

    fn description(&self) -> &str {
        "Search for text content within files in the workspace"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Text or regex pattern to search for" },
                "path": { "type": "string", "description": "Root directory to search from" },
                "file_pattern": { "type": "string", "description": "File glob pattern to match (e.g., '*.rs')" },
                "regex": { "type": "boolean", "description": "Treat query as regex (default: false)" },
                "case_sensitive": { "type": "boolean", "description": "Case sensitive search (default: false)" },
                "max_results": { "type": "number", "description": "Maximum number of results (default: 50)" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args.get("query").and_then(|v| v.as_str())?;
        let root = args.get("path").and_then(|v| v.as_str())
            .map(|p| self.security.workspace_dir.join(p))
            .unwrap_or_else(|| self.security.workspace_dir.clone());
        let file_pattern = args.get("file_pattern").and_then(|v| v.as_str()).unwrap_or("*");
        let is_regex = args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false);
        let case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        // Security check
        if !root.starts_with(&self.security.workspace_dir) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Path outside workspace".into()) });
        }

        let pattern = if is_regex {
            if case_sensitive { regex::Regex::new(query)? } else { regex::RegexBuilder::new(query).case_insensitive(true).build()? }
        } else {
            let escaped = regex::escape(query);
            if case_sensitive { regex::Regex::new(&escaped)? } else { regex::RegexBuilder::new(&escaped).case_insensitive(true).build()? }
        };

        let mut results = Vec::new();
        // Walk directory and search
        let walker = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        for entry in walker {
            let path = entry.path();
            // Skip large files (> 1MB)
            if let Ok(metadata) = entry.metadata() {
                if metadata.len() > 1024 * 1024 { continue; }
            }

            if let Ok(content) = tokio::fs::read_to_string(path).await {
                for (line_num, line) in content.lines().enumerate() {
                    if pattern.is_match(line) {
                        let rel_path = path.strip_prefix(&root).unwrap_or(path).display().to_string();
                        results.push(format!("{}:{}: {}", rel_path, line_num + 1, line.trim()));
                        if results.len() >= max_results {
                            break;
                        }
                    }
                }
            }
            if results.len() >= max_results {
                break;
            }
        }

        let output = results.join("\n");
        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

### 10.3 Web 类工具

#### 9.3.1 web_fetch

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/web_fetch.rs`

```rust
pub struct WebFetchTool {
    client: reqwest::Client,
    allowed_domains: Vec<String>,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            allowed_domains: vec![
                "wikipedia.org".into(),
                "github.com".into(),
                "stackoverflow.com".into(),
                // ... configurable
            ],
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }

    fn description(&self) -> &str {
        "Fetch the content of a web page"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" },
                "selector": { "type": "string", "description": "CSS selector to extract specific content" },
                "max_length": { "type": "number", "description": "Maximum content length (default: 50000)" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let url = args.get("url").and_then(|v| v.as_str())?;
        let selector = args.get("selector").and_then(|v| v.as_str());
        let max_length = args.get("max_length").and_then(|v| v.as_u64()).unwrap_or(50000) as usize;

        // Domain check
        if !self.is_allowed_domain(url) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Domain not allowed".into()) });
        }

        let response = self.client.get(url).send().await?;
        let html = response.text().await?;

        let content = if let Some(sel) = selector {
            // Use scraper crate to extract
            scraper::Html::parse_document(&html)
                .select(&scraper::Selector::parse(sel)?)
                .first()
                .map(|e| e.inner_html())
                .unwrap_or_default()
        } else {
            html
        };

        let truncated = if content.len() > max_length {
            format!("{}...[truncated {} chars]", &content[..max_length], content.len() - max_length)
        } else {
            content
        };

        Ok(ToolResult { success: true, output: truncated, error: None })
    }

    fn is_allowed_domain(&self, url: &str) -> bool {
        if let Ok(parsed) = url::Url::parse(url) {
            self.allowed_domains.iter().any(|d| parsed.host_str() == Some(d))
        } else {
            false
        }
    }
}
```

---

#### 9.3.2 web_search

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/web_search_tool.rs`

```rust
pub struct WebSearchTool {
    client: reqwest::Client,
    provider: SearchProvider,
}

pub enum SearchProvider {
    Brave { api_key: String },
    SearXNG { base_url: String },
    DuckDuckGo,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web for information"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "max_results": { "type": "number", "description": "Maximum results to return (default: 10)" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args.get("query").and_then(|v| v.as_str())?;
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = match &self.provider {
            SearchProvider::Brave { api_key } => self.brave_search(query, max_results, api_key).await?,
            SearchProvider::SearXNG { base_url } => self.searxng_search(query, max_results, base_url).await?,
            SearchProvider::DuckDuckGo => self.duckduckgo_search(query, max_results).await?,
        };

        let output = results.into_iter()
            .map(|r| format!("- {}: {}", r.title, r.url))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

#### 9.3.3 http_request

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/http_request.rs`

```rust
pub struct HttpRequestTool {
    client: reqwest::Client,
    allowed_domains: Vec<String>,
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &str { "http_request" }

    fn description(&self) -> &str {
        "Make HTTP requests to web APIs"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "method": { "type": "string", "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"] },
                "url": { "type": "string", "description": "Request URL" },
                "headers": { "type": "object", "description": "HTTP headers" },
                "body": { "type": "string", "description": "Request body" }
            },
            "required": ["method", "url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let method = args.get("method").and_then(|v| v.as_str())?;
        let url = args.get("url").and_then(|v| v.as_str())?;
        let headers = args.get("headers").and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k,v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect());
        let body = args.get("body").and_then(|v| v.as_str());

        // Domain check
        if !self.is_allowed_domain(url) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Domain not allowed".into()) });
        }

        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            _ => return Ok(ToolResult { success: false, output: String::new(), error: Some("Invalid method".into()) }),
        };

        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                request = request.header(&k, &v);
            }
        }

        if let Some(b) = body {
            request = request.body(b.to_string());
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await?;
                Ok(ToolResult { success: true, output: format!("Status: {}\n\n{}", status, body), error: None })
            }
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(e.to_string()) })
        }
    }
}
```

---

### 10.4 Memory 类工具

#### 9.4.1 memory_store

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/memory_store.rs`

```rust
pub struct MemoryStoreTool {
    memory: Arc<dyn Memory>,
    security: Arc<SecurityPolicy>,
}

pub enum MemoryCategory {
    Core,        // Critical persistent facts
    Daily,       // Daily context and notes
    Conversation, // Current conversation context
    Custom(String),
}

#[async_trait]
impl Tool for MemoryStoreTool {
    fn name(&self) -> &str { "memory_store" }

    fn description(&self) -> &str {
        "Store a fact, preference, or note in long-term memory"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Unique key for this memory" },
                "content": { "type": "string", "description": "The information to remember" },
                "category": { "type": "string", "description": "Memory category: 'core', 'daily', 'conversation', or custom" }
            },
            "required": ["key", "content"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let key = args.get("key").and_then(|v| v.as_str())?;
        let content = args.get("content").and_then(|v| v.as_str())?;
        let category = match args.get("category").and_then(|v| v.as_str()) {
            Some("core") | None => MemoryCategory::Core,
            Some("daily") => MemoryCategory::Daily,
            Some("conversation") => MemoryCategory::Conversation,
            Some(other) => MemoryCategory::Custom(other.to_string()),
        };

        match self.memory.store(key, content, category, None).await {
            Ok(()) => Ok(ToolResult { success: true, output: format!("Stored: {key}"), error: None }),
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Failed: {e}")) })
        }
    }
}
```

---

#### 9.4.2 memory_recall

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/memory_recall.rs`

```rust
pub struct MemoryRecallTool {
    memory: Arc<dyn Memory>,
    security: Arc<SecurityPolicy>,
}

#[async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &str { "memory_recall" }

    fn description(&self) -> &str {
        "Recall stored memories that match a query"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Query to search memories" },
                "category": { "type": "string", "description": "Filter by category (optional)" },
                "max_results": { "type": "number", "description": "Maximum results (default: 10)" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args.get("query").and_then(|v| v.as_str())?;
        let category = args.get("category").and_then(|v| v.as_str());
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.memory.recall(query, category, max_results).await?;

        let output = results.into_iter()
            .map(|r| format!("[{}] {}: {}", r.category, r.key, r.content))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

#### 9.4.3 memory_forget

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/memory_forget.rs`

```rust
pub struct MemoryForgetTool {
    memory: Arc<dyn Memory>,
    security: Arc<SecurityPolicy>,
}

#[async_trait]
impl Tool for MemoryForgetTool {
    fn name(&self) -> &str { "memory_forget" }

    fn description(&self) -> &str {
        "Delete specific memories by key"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Memory key to delete" }
            },
            "required": ["key"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let key = args.get("key").and_then(|v| v.as_str())?;

        match self.memory.delete(key).await {
            Ok(()) => Ok(ToolResult { success: true, output: format!("Deleted: {key}"), error: None }),
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Failed: {e}")) })
        }
    }
}
```

---

#### 9.4.4 memory_purge

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/memory_purge.rs`

```rust
pub struct MemoryPurgeTool {
    memory: Arc<dyn Memory>,
    security: Arc<SecurityPolicy>,
}

#[async_trait]
impl Tool for MemoryPurgeTool {
    fn name(&self) -> &str { "memory_purge" }

    fn description(&self) -> &str {
        "Clear all memories in a category"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "category": { "type": "string", "description": "Category to purge ('core', 'daily', 'conversation', or 'all')" }
            },
            "required": ["category"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let category = args.get("category").and_then(|v| v.as_str())?;

        match category {
            "all" => self.memory.purge_all().await?,
            cat => self.memory.purge_category(cat).await?,
        }

        Ok(ToolResult { success: true, output: format!("Purged category: {category}"), error: None })
    }
}
```

---

#### 9.4.5 memory_export

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/memory_export.rs`

```rust
pub struct MemoryExportTool {
    memory: Arc<dyn Memory>,
    security: Arc<SecurityPolicy>,
}

#[async_trait]
impl Tool for MemoryExportTool {
    fn name(&self) -> &str { "memory_export" }

    fn description(&self) -> &str {
        "Export all memories as JSON"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "category": { "type": "string", "description": "Category to export (optional, exports all if omitted)" },
                "format": { "type": "string", "enum": ["json", "markdown"], "description": "Export format" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let category = args.get("category").and_then(|v| v.as_str());
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("json");

        let memories = self.memory.export(category).await?;

        let output = match format {
            "markdown" => {
                memories.iter()
                    .map(|m| format!("## {}\n\n{}\n", m.key, m.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => serde_json::to_string_pretty(&memories)?,
        };

        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

### 10.5 Cron/调度类工具

#### 9.5.1 cron_add

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/cron_add.rs`

```rust
pub struct CronAddTool {
    scheduler: Arc<dyn Scheduler>,
}

#[async_trait]
impl Tool for CronAddTool {
    fn name(&self) -> &str { "cron_add" }

    fn description(&self) -> &str {
        "Create a scheduled cron job"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Unique job ID" },
                "schedule": { "type": "string", "description": "Cron expression (e.g., '0 9 * * *' for daily at 9am)" },
                "command": { "type": "string", "description": "Command or tool call to execute" },
                "description": { "type": "string", "description": "Job description" }
            },
            "required": ["id", "schedule", "command"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let id = args.get("id").and_then(|v| v.as_str())?;
        let schedule = args.get("schedule").and_then(|v| v.as_str())?;
        let command = args.get("command").and_then(|v| v.as_str())?;
        let description = args.get("description").and_then(|v| v.as_str()).unwrap_or_default();

        match self.scheduler.add(id, schedule, command, description).await {
            Ok(()) => Ok(ToolResult { success: true, output: format!("Added cron job: {id}"), error: None }),
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Failed: {e}")) })
        }
    }
}
```

---

#### 9.5.2 cron_list

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/cron_list.rs`

```rust
pub struct CronListTool {
    scheduler: Arc<dyn Scheduler>,
}

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str { "cron_list" }
    fn description(&self) -> &str { "List all scheduled cron jobs" }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let jobs = self.scheduler.list().await?;
        let output = jobs.iter()
            .map(|j| format!("{} - {} ({}) [{}]", j.id, j.description, j.schedule, j.status))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

#### 9.5.3 cron_remove

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/cron_remove.rs`

```rust
pub struct CronRemoveTool {
    scheduler: Arc<dyn Scheduler>,
}

#[async_trait]
impl Tool for CronRemoveTool {
    fn name(&self) -> &str { "cron_remove" }
    fn description(&self) -> &str { "Remove a scheduled cron job" }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Job ID to remove" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let id = args.get("id").and_then(|v| v.as_str())?;
        match self.scheduler.remove(id).await {
            Ok(()) => Ok(ToolResult { success: true, output: format!("Removed: {id}"), error: None }),
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(e.to_string()) })
        }
    }
}
```

---

#### 9.5.4 cron_run

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/cron_run.rs`

```rust
pub struct CronRunTool {
    scheduler: Arc<dyn Scheduler>,
}

#[async_trait]
impl Tool for CronRunTool {
    fn name(&self) -> &str { "cron_run" }
    fn description(&self) -> &str { "Trigger a cron job immediately" }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Job ID to run" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let id = args.get("id").and_then(|v| v.as_str())?;
        match self.scheduler.run_now(id).await {
            Ok(result) => Ok(ToolResult { success: true, output: result, error: None }),
            Err(e) => Ok(ToolResult { success: false, output: String::new(), error: Some(e.to_string()) })
        }
    }
}
```

---

#### 9.5.5 cron_runs

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/cron_runs.rs`

```rust
pub struct CronRunsTool {
    scheduler: Arc<dyn Scheduler>,
}

#[async_trait]
impl Tool for CronRunsTool {
    fn name(&self) -> &str { "cron_runs" }
    fn description(&self) -> &str { "List recent executions of a cron job" }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Job ID" },
                "limit": { "type": "number", "description": "Max recent runs (default: 10)" }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let id = args.get("id").and_then(|v| v.as_str())?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let runs = self.scheduler.get_runs(id, limit).await?;
        let output = runs.iter()
            .map(|r| format!("{} - {} (exit: {})", r.timestamp, r.status, r.exit_code))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

### 10.6 杂项工具

#### 9.6.1 calculator

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/calculator.rs`

```rust
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }

    fn description(&self) -> &str {
        "Perform arithmetic and statistical calculations"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string", "description": "Mathematical expression to evaluate" },
                "mode": { "type": "string", "enum": ["eval", "stats"], "description": "'eval' for arithmetic, 'stats' for statistics" }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let expression = args.get("expression").and_then(|v| v.as_str())?;
        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("eval");

        let result = match mode {
            "stats" => {
                // Parse numbers and calculate mean, median, stddev
                let numbers: Vec<f64> = expression
                    .split(|c: char| !c.is_numeric() && c != '.' && c != '-')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if numbers.is_empty() {
                    return Ok(ToolResult { success: false, output: String::new(), error: Some("No numbers found".into()) });
                }

                let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
                let median = if numbers.len() % 2 == 0 {
                    (numbers[numbers.len()/2-1] + numbers[numbers.len()/2]) / 2.0
                } else {
                    numbers[numbers.len()/2]
                };
                let variance = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / numbers.len() as f64;
                let stddev = variance.sqrt();

                format!("Count: {}\nMean: {:.2}\nMedian: {:.2}\nStdDev: {:.2}", numbers.len(), mean, median, stddev)
            }
            _ => {
                // Simple arithmetic evaluation using meval crate
                match expression.parse::<f64>() {
                    Ok(val) => val.to_string(),
                    Err(_) => {
                        // Try evaluating as expression
                        meval::eval_str(expression)?.to_string()
                    }
                }
            }
        };

        Ok(ToolResult { success: true, output: result, error: None })
    }
}
```

---

#### 9.6.2 screenshot

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/screenshot.rs`

```rust
pub struct ScreenshotTool;

#[async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &str { "screenshot" }

    fn description(&self) -> &str {
        "Capture a screenshot"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Output path for the screenshot (default: screenshot.png)" },
                "full_screen": { "type": "boolean", "description": "Capture full screen (default: true)" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("screenshot.png");
        let full_screen = args.get("full_screen").and_then(|v| v.as_bool()).unwrap_or(true);

        // Use xcap or screenshots crate on Linux/macOS/Windows
        let img = screenshots::Screen::all()
            .into_iter()
            .next()
            .and_then(|s| s.capture().ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to capture screen"))?;

        img.save(path).map_err(|e| anyhow::anyhow!("Failed to save: {}", e))?;

        Ok(ToolResult { success: true, output: format!("Saved to {path} ({}x{})", img.width(), img.height()), error: None })
    }
}
```

---

#### 9.6.3 weather

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/weather_tool.rs`

```rust
pub struct WeatherTool {
    client: reqwest::Client,
}

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str { "weather" }

    fn description(&self) -> &str {
        "Get weather information for a location using wttr.in"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location": { "type": "string", "description": "City name or location (default: auto-detect)" },
                "format": { "type": "string", "enum": ["short", "medium", "full"], "description": "Output format" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let location = args.get("location").and_then(|v| v.as_str()).unwrap_or("");
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("medium");

        let url = if location.is_empty() {
            "https://wttr.in/?format=j1".to_string()
        } else {
            format!("https://wttr.in/{}?format=j1", location.replace(" ", "+"))
        };

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        let output = match format {
            "short" => {
                let current = &json["current_condition"][0];
                format!("{}°C, {}",
                    current["temp_C"].as_str().unwrap_or("N/A"),
                    current["weatherDesc"][0]["value"].as_str().unwrap_or("N/A"))
            }
            "full" => serde_json::to_string_pretty(&json).unwrap_or_default(),
            _ => {
                // medium format
                let current = &json["current_condition"][0];
                format!("Weather in {}: {}°C, {}% humidity, {}",
                    location,
                    current["temp_C"].as_str().unwrap_or("N/A"),
                    current["humidity"].as_str().unwrap_or("N/A"),
                    current["weatherDesc"][0]["value"].as_str().unwrap_or("N/A"))
            }
        };

        Ok(ToolResult { success: true, output, error: None })
    }
}
```

---

#### 9.6.4 pdf_read

**来源**: `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/pdf_read.rs`

```rust
pub struct PdfReadTool {
    security: Arc<SecurityPolicy>,
}

#[async_trait]
impl Tool for PdfReadTool {
    fn name(&self) -> &str { "pdf_read" }

    fn description(&self) -> &str {
        "Extract text content from a PDF file"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path to the PDF file" },
                "pages": { "type": "string", "description": "Page range (e.g., '1-5' or 'all', default: 'all')" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str())?;
        let pages = args.get("pages").and_then(|v| v.as_str()).unwrap_or("all");

        let full_path = self.security.workspace_dir.join(path);
        if !full_path.starts_with(&self.security.workspace_dir) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Path outside workspace".into()) });
        }

        let file = std::fs::File::open(&full_path)
            .map_err(|e| anyhow::anyhow!("Failed to open: {}", e))?;

        let mut reader = pdf_extract::extract_text_from_pdf(&full_path)
            .map_err(|e| anyhow::anyhow!("Failed to extract: {}", e))?;

        let output = if pages == "all" {
            reader
        } else {
            // Parse page range and extract only those pages
            let page_range = Self::parse_page_range(pages);
            let all_pages: Vec<String> = reader.lines().map(|l| l.unwrap_or_default()).collect();
            all_pages.into_iter()
                .skip(page_range.start.saturating_sub(1))
                .take(page_range.end.saturating_sub(page_range.start) + 1)
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult { success: true, output, error: None })
    }

    fn parse_page_range(range: &str) -> std::ops::RangeInclusive<usize> {
        let parts: Vec<&str> = range.split('-').collect();
        let start = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
        let end = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(start);
        start..=end
    }
}
```

---

## 11. 新工具实现优先级与依赖

### 11.1 Phase 4.3 扩展 — 文件操作类（第一批）

| 工具 | 源码文件 | 实现文件 | 优先级 |
|------|----------|----------|--------|
| `file_write` | `zeroclaw-master/src/tools/file_write.rs` | `src-tauri/src/modules/tools/builtin/file_write.rs` | P0 |
| `file_edit` | `zeroclaw-master/src/tools/file_edit.rs` | `src-tauri/src/modules/tools/builtin/file_edit.rs` | P0 |
| `glob_search` | `zeroclaw-master/src/tools/glob_search.rs` | `src-tauri/src/modules/tools/builtin/glob_search.rs` | P0 |
| `content_search` | `zeroclaw-master/src/tools/content_search.rs` | `src-tauri/src/modules/tools/builtin/content_search.rs` | P0 |

### 11.2 Phase 4.8 — Web 类工具

| 工具 | 源码文件 | 实现文件 | 优先级 |
|------|----------|----------|--------|
| `web_fetch` | `zeroclaw-master/src/tools/web_fetch.rs` | `src-tauri/src/modules/tools/builtin/web_fetch.rs` | P1 |
| `web_search` | `zeroclaw-master/src/tools/web_search_tool.rs` | `src-tauri/src/modules/tools/builtin/web_search.rs` | P1 |
| `http_request` | `zeroclaw-master/src/tools/http_request.rs` | `src-tauri/src/modules/tools/builtin/http_request.rs` | P1 |

### 11.3 Phase 4.9 — Memory 类工具

| 工具 | 源码文件 | 实现文件 | 优先级 |
|------|----------|----------|--------|
| `memory_store` | `zeroclaw-master/src/tools/memory_store.rs` | `src-tauri/src/modules/tools/builtin/memory_store.rs` | P1 |
| `memory_recall` | `zeroclaw-master/src/tools/memory_recall.rs` | `src-tauri/src/modules/tools/builtin/memory_recall.rs` | P1 |
| `memory_forget` | `zeroclaw-master/src/tools/memory_forget.rs` | `src-tauri/src/modules/tools/builtin/memory_forget.rs` | P1 |
| `memory_purge` | `zeroclaw-master/src/tools/memory_purge.rs` | `src-tauri/src/modules/tools/builtin/memory_purge.rs` | P1 |
| `memory_export` | `zeroclaw-master/src/tools/memory_export.rs` | `src-tauri/src/modules/tools/builtin/memory_export.rs` | P1 |

### 11.4 Phase 4.10 — Cron/调度类工具

| 工具 | 源码文件 | 实现文件 | 优先级 |
|------|----------|----------|--------|
| `cron_add` | `zeroclaw-master/src/tools/cron_add.rs` | `src-tauri/src/modules/tools/builtin/cron_add.rs` | P1 |
| `cron_list` | `zeroclaw-master/src/tools/cron_list.rs` | `src-tauri/src/modules/tools/builtin/cron_list.rs` | P1 |
| `cron_remove` | `zeroclaw-master/src/tools/cron_remove.rs` | `src-tauri/src/modules/tools/builtin/cron_remove.rs` | P1 |
| `cron_run` | `zeroclaw-master/src/tools/cron_run.rs` | `src-tauri/src/modules/tools/builtin/cron_run.rs` | P1 |
| `cron_runs` | `zeroclaw-master/src/tools/cron_runs.rs` | `src-tauri/src/modules/tools/builtin/cron_runs.rs` | P1 |

### 11.5 Phase 4.11 — 杂项工具

| 工具 | 源码文件 | 实现文件 | 优先级 |
|------|----------|----------|--------|
| `calculator` | `zeroclaw-master/src/tools/calculator.rs` | `src-tauri/src/modules/tools/builtin/calculator.rs` | P2 |
| `screenshot` | `zeroclaw-master/src/tools/screenshot.rs` | `src-tauri/src/modules/tools/builtin/screenshot.rs` | P2 |
| `weather` | `zeroclaw-master/src/tools/weather_tool.rs` | `src-tauri/src/modules/tools/builtin/weather.rs` | P2 |
| `pdf_read` | `zeroclaw-master/src/tools/pdf_read.rs` | `src-tauri/src/modules/tools/builtin/pdf_read.rs` | P2 |

---

## 12. 新工具 Cargo 依赖

```toml
# src-tauri/Cargo.toml — 新增依赖

[dependencies]
# Web 工具
reqwest = { version = "0.12", features = ["json"] }
scraper = "0.21"          # HTML parsing for web_fetch
url = "2.5"               # URL parsing

# 文件搜索
glob = "0.3"
walkdir = "2.5"
regex = "1.11"

# 调度
cron = "0.15"

# PDF
pdf-extract = "0.7"

# 计算
meval = "0.2"

# 截图
screenshots = "0.8"
xcap = "0.0.15"          # Linux support
```

---

**版本**: 1.2 | **最后更新**: 2026-04-12
**参考**:
- [tool-system.md](./tool-system.md) — 完整工具基础设施定义
- `/Users/ryanliu/Documents/IfAI/zeroclaw-master/src/tools/` — 原始工具实现
