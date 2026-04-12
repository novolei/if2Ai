# Tool System 设计文档

> Tool System 是 Agent 与外部世界交互的关键机制。这个文档基于 hermes-agent 的 40+ 工具实现和 registry pattern，提供完整的工具管理、执行和扩展设计。

---

## ⚠️ Gap Analysis & Activation Status (2026-04-12)

### 现状

`tool-system.md` 定义了完整的工具基础设施，**但存在以下关键 Gap**：

| Gap | 当前状态 | 需要激活的文件 |
|-----|----------|----------------|
| **工具未注册** | `main.rs` 中 `ToolRegistry::new()` 创建空注册表 | `src-tauri/src/main.rs` |
| **LLM 不接收工具** | `MessageRequest { tools: None }` | `src-tauri/src/commands/agent.rs` |
| **无前端调用接口** | 没有 `execute_tool` Tauri 命令 | `src-tauri/src/commands/tools.rs` (新建) |
| **MCP Client 未激活** | 定义了 `mcp_client.rs`，未连接到 registry | 后续迭代 |

### 激活需要的改动

1. **`main.rs`** — 添加 `register_builtin_tools(&tool_registry)`
2. **`agent.rs`** — 修改 `MessageRequest` 传递 `tools: Some(...)`
3. **`commands/tools.rs`** — 新建，暴露 `execute_tool`, `list_tools`, `get_tool_definitions`
4. **`lib/tauri.ts`** — 前端新增工具调用接口

### 设计文档

完整的前端工具调用设计见 [tool-activation.md](./tool-activation.md)。

---

## 系统设计

```
┌─────────────────┐
│  ToolRegistry   │ ← 单例，在 Agent 初始化时加载
│  (动态注册)      │
└────────┬────────┘
         │
    ┌────┴─────────┬──────────────┬──────────────┐
    ▼              ▼              ▼              ▼
┌────────┐    ┌────────┐    ┌────────┐    ┌─────────┐
│  Web   │    │ Files  │    │Terminal│    │Vision   │
│Tools   │    │Tools   │    │Tools   │    │ Tools   │
└────────┘    └────────┘    └────────┘    └─────────┘
    │              │              │              │
    └──────────────┴──────────────┴──────────────┘
                   │
                   ▼
        ┌──────────────────────┐
        │ Tool Executor        │
        │ (并行/顺序执行)       │
        └──────────────────────┘
```

## 核心组件

### 1. Tool Registry（工具注册表）

单例模式，管理所有工具的元数据和执行：

```rust
pub struct ToolEntry {
    // 基础信息
    pub name: String,                          // "web_search"
    pub toolset: String,                       // "web"
    pub description: String,
    pub emoji: String,                         // "🔍"

    // 架构
    pub schema: JsonSchema,                    // OpenAI 兼容 format
    pub input_schema: JsonValue,
    pub output_schema: Option<JsonValue>,

    // 实现
    pub handler: Arc<ToolHandler>,             // 执行函数
    pub is_async: bool,

    // 控制
    pub check_fn: Option<fn() -> bool>,        // 可用性检查（如需 API 密钥）
    pub requires_env: Vec<String>,             // 必需的环境变量
    pub max_result_size: Option<usize>,        // 输出大小限制
    pub timeout_secs: Option<u32>,             // 执行超时
    pub disabled: bool,                        // 管理员禁用标志
}

pub struct ToolRegistry {
    tools: Arc<DashMap<String, ToolEntry>>,
    name_to_toolset: Arc<DashMap<String, String>>,
}

impl ToolRegistry {
    // 注册 API（在 Agent 初始化时调用）
    pub fn register(&self, entry: ToolEntry) -> Result<()> {
        self.name_to_toolset
            .insert(entry.name.clone(), entry.toolset.clone());
        self.tools.insert(entry.name.clone(), entry);
        Ok(())
    }

    // 查询 API
    pub fn get(&self, name: &str) -> Option<ToolEntry> {
        self.tools.get(name).map(|r| r.value().clone())
    }

    pub fn get_definitions(&self,
        toolset_names: &[String],
        disabled_toolsets: &[String],
        quiet: bool,
    ) -> Result<Vec<JsonValue>> {
        // 返回 OpenAI 格式的 tool schema 定义
    }

    pub fn validate_tool_call(&self, call: &ToolCall) -> Result<()> {
        // 检查工具是否存在、参数是否有效
    }

    // 执行 API
    pub async fn dispatch(
        &self,
        name: &str,
        args: &JsonValue,
    ) -> Result<String> {
        let entry = self.get(name)
            .ok_or(ToolError::NotFound(name.to_string()))?;

        // 可用性检查
        if let Some(check) = entry.check_fn {
            if !check() {
                return Err(ToolError::Unavailable(name.to_string()));
            }
        }

        // 环境检查
        for env_var in &entry.requires_env {
            if std::env::var(env_var).is_err() {
                return Err(ToolError::MissingEnv(env_var.clone()));
            }
        }

        // 执行
        let result = (entry.handler)(args).await?;

        // 输出大小检查
        if let Some(max_size) = entry.max_result_size {
            if result.len() > max_size {
                return Err(ToolError::OutputTooLarge {
                    size: result.len(),
                    max: max_size,
                });
            }
        }

        Ok(result)
    }
}
```

### 2. 工具分类系统

```rust
pub struct ToolSet {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,      // 工具名称列表
    pub includes: Vec<String>,   // 包含其他工具集
}

pub struct ToolSetRegistry {
    toolsets: HashMap<String, ToolSet>,
}

impl ToolSetRegistry {
    pub fn resolve(&self, toolset_name: &str) -> Result<HashSet<String>> {
        // 递归解析，返回所有最终工具名称
        let mut result = HashSet::new();
        self._resolve_recursive(toolset_name, &mut result)?;
        Ok(result)
    }
}

// 预定义的工具集分类
pub const TOOLSETS: &[(&str, &[&str])] = &[
    ("web", &["web_search", "web_extract"]),
    ("files", &["read_file", "write_file", "patch", "search_files"]),
    ("terminal", &["terminal", "process"]),
    ("vision", &["vision_analyze", "perception"]),
    ("browser", &["navigate", "click", "type", "screenshot"]),
    ("memory", &["read_memory", "write_memory"]),
    ("code", &["execute_code"]),
    ("delegation", &["delegate_task"]),

    // 组合工具集
    ("minimal", &["web_search", "read_file"]),
    ("research", &["web_search", "web_extract", "read_file"]),
    ("development", &["terminal", "files", "execute_code"]),
    ("full_stack", &["web", "files", "terminal", "vision", "code"]),
];
```

### 3. Tool Handler 类型定义

```rust
pub type ToolHandler = Arc<
    dyn Fn(&JsonValue) -> BoxFuture<'static, Result<String>> + Send + Sync
>;

// 便利宏用于定义工具
#[macro_export]
macro_rules! define_tool {
    (
        name: $name:expr,
        toolset: $toolset:expr,
        description: $desc:expr,
        params: { $($param:ident: $type:ty),* },
        handler: $handler:expr
    ) => {
        ToolEntry {
            name: $name.to_string(),
            toolset: $toolset.to_string(),
            description: $desc.to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    $($param: ...),*
                },
                "required": [...]
            }),
            handler: Arc::new($handler),
            ..Default::default()
        }
    };
}
```

## Tool 分类详解

### A. Web 工具 (2 个)

```rust
Tool: web_search
├─ 功能: 通过 Tavily 或 SerpAPI 进行网络搜索
├─ 参数: query (string), max_results (int)
├─ 返回: Vec<SearchResult> {url, title, snippet, date}
└─ 成本: API 配额

Tool: web_extract
├─ 功能: 提取网页内容
├─ 参数: url (string), selector (optional)
├─ 返回: 页面文本或特定元素内容
└─ 成本: 无（本地处理）
```

### B. 文件工具 (4 个)

```rust
Tool: read_file
├─ 功能: 读取文件内容
├─ 参数: path, offset (optional), limit (optional)
├─ 限制: 文件大小检查, 路径白名单
├─ 返回: 文本内容或二进制数据
└─ 安全: 禁止访问 /etc/passwd 等

Tool: write_file
├─ 功能: 写入或追加文件
├─ 参数: path, content, append (bool)
├─ 限制: 写入权限检查
└─ 返回: 成功或错误

Tool: patch
├─ 功能: 应用统一差异补丁
├─ 参数: path, patch_content
└─ 返回: 修改后的文件内容

Tool: search_files
├─ 功能: 在文件系统中搜索
├─ 参数: query, path, regex (bool)
├─ 返回: Vec<SearchResult {path, preview}>
└─ 限制: 3000 文件上限
```

### C. Terminal 工具 (1 个 + 6 个后端)

```rust
Tool: terminal
├─ 功能: 执行命令行指令
├─ 参数: command (string), backend (optional: local/docker/ssh/modal/daytona/singularity)
├─ 返回: {stdout, stderr, return_code}
├─ 超时: 默认 30 秒
├─ 环境隔离: 支持 Docker 沙箱
└─ 后端支持:
    ├─ local: 本地 shell
    ├─ docker: 容器化执行
    ├─ ssh: 远程服务器
    ├─ modal: 云函数执行
    ├─ daytona: 开发环境
    └─ singularity: HPC 集群
```

### D. Vision 工具 (2 个)

```rust
Tool: vision_analyze
├─ 功能: 分析图像内容（使用 Claude Vision 或 GPT-4V）
├─ 参数: image_url 或 image_base64, task (识别/读取/分析)
├─ 返回: 文本分析结果
└─ 回退: 如果 primary 模型失败，尝试备用模型

Tool: perception
├─ 功能: 更高级的图像理解
├─ 参数: image_data, questions (Vec<String>)
└─ 返回: 多个问题的答案
```

### E. 其他工具

| 工具           | 功能         | 关键特性              |
| -------------- | ------------ | --------------------- |
| navigate       | 浏览器导航   | Puppeteer/Playwright  |
| click, type    | UI 交互      | 坐标或元素定位        |
| screenshot     | 截屏         | Base64 或文件保存     |
| memory         | 读写记忆库   | MEMORY.md + USER.md   |
| execute_code   | 代码执行     | Python/Bash，超时保护 |
| delegate_task  | 创建子 Agent | 预算共享，中断传播    |
| clarify        | 澄清问题     | 要求用户输入          |
| session_search | 搜索会话历史 | 向量搜索或全文搜索    |

## Tool Executor（执行器）

```rust
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    max_parallel_workers: usize,
    dependency_resolver: DependencyResolver,
}

impl ToolExecutor {
    pub async fn execute_batch(
        &self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<(ToolCall, Result<String>)>> {
        // Step 1: 解析依赖关系
        let sorted = self.dependency_resolver.sort(&tool_calls)?;

        // Step 2: 分区为可并行执行的组
        let groups = self.partition_into_groups(&sorted);

        // Step 3: 执行每组（组内并行，组间顺序）
        let mut results = Vec::new();
        for group in groups {
            let group_results = futures::future::join_all(
                group.iter().map(|call| self.execute_single(call))
            ).await;
            results.extend(group_results);
        }

        Ok(results)
    }

    async fn execute_single(&self, call: &ToolCall) -> (ToolCall, Result<String>) {
        match self.registry.dispatch(&call.name, &call.args).await {
            Ok(result) => (call.clone(), Ok(result)),
            Err(e) => (call.clone(), Err(e)),
        }
    }
}
```

## Tool 开发指南

### 添加新工具的步骤

1. **定义 Tool Entry**

   ```rust
   let entry = ToolEntry {
       name: "my_tool".to_string(),
       toolset: "custom".to_string(),
       description: "Does something".to_string(),
       schema: json_schema!(...),
       handler: Arc::new(|args| Box::pin(my_tool_handler(args))),
       ..Default::default()
   };
   ```

2. **实现 Handler**

   ```rust
   async fn my_tool_handler(args: &JsonValue) -> Result<String> {
       let param = args.get("param")?;
       // 业务逻辑
       Ok(result_string)
   }
   ```

3. **注册工具**

   ```rust
   registry.register(entry)?;
   ```

4. **添加单元测试**
   ```rust
   #[tokio::test]
   async fn test_my_tool() {
       let result = my_tool_handler(&json!({"param": "value"})).await;
       assert!(result.is_ok());
   }
   ```

## 安全性考虑

| 问题     | 缓解策略                   |
| -------- | -------------------------- |
| 注入攻击 | 参数类型检查、escapement   |
| 过度执行 | 超时限制、并行数限制       |
| 输出泄漏 | 输出大小检查、敏感数据过滤 |
| 权限提升 | 沙箱隔离（Docker）         |
| API 滥用 | API 密钥管理、速率限制     |

## 与 Harness 的集成

### Tool 行为评估

```yaml
test_case:
  name: 'Web Search Tool'
  prompt: 'What is the capital of France?'
  evaluators:
    - name: behavior
      config:
        tools_used: ['web_search']
        max_tool_calls: 1
    - name: correctness
      config:
        expected_output: 'Paris'
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [docs/references/hermes-agent-analysis.md](../../references/hermes-agent-analysis.md) - Tool System
