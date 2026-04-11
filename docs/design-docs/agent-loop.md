# Agent Loop & Conversation Runtime

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes Agent `run_agent.py` (~9200 行)  
**实现语言**: Rust  
**关键文件**: `crates/runtime/src/conversation.rs` + `crates/api/src/client.rs`

---

## 1. 系统概览

### 1.1 在 Hermes 中的角色

Hermes Agent Loop（核心的 `AIAgent` 类）负责：

- 🔄 同步协调 Agent 循环
- 📝 维护对话历史（OpenAI 消息格式）
- 🔗 集成 LLM 提供商选择
- 🛠️ 工具调用和执行
- 💾 会话持久化与压缩
- 🚀 中断和降级处理

### 1.2 在 Claw Code 中的现状

**现有实现**（`src/conversation.rs`）：

```
✅ ConversationRuntime - 会话运行时
✅ ApiClient - 多提供商支持
✅ Tool 执行上下文
✅ 消息历史管理
✅ Session 保存/加载
✅ 权限检查
⏳ 迭代预算跟踪
⏳ 上下文压缩触发
⏳ 提供商降级
```

**代码规模**：~1,500 行（包含 runtime 的所有模块）

---

## 2. 架构设计

### 2.1 核心数据流

```
用户输入 (API/CLI)
    ↓
[ConversationRuntime::run_turn()]
    ├─ 1. 添加用户消息到历史
    ├─ 2. 构建系统提示 (prompt_builder)
    ├─ 3. 调用 LLM API (ApiClient)
    ├─ 4. 解析响应 (parse tool_calls)
    └─ 5. 执行工具（如果有）→ 回到 3，直到完成
    ↓
[Session::save()]  // SQLite 或其他
    ↓
返回最终响应
```

### 2.2 关键类型定义

#### ConversationRuntime

```rust
pub struct ConversationRuntime {
    api: Arc<ApiClient>,           // LLM 客户端
    system_prompt: String,         // 系统身份
    tools: ToolExecutor,           // 工具执行者
    session: Session,              // 会话状态
    config: RuntimeConfig,         // 运行时配置
    max_turns: usize,              // 最大迭代数
}

impl ConversationRuntime {
    /// 核心 agent 循环
    pub async fn run_turn(
        &mut self,
        user_message: String,
    ) -> Result<AssistantEvent, RuntimeError> {
        // 步骤 1-5（见上）
    }

    /// 完整对话（多轮）
    pub async fn run_conversation(
        &mut self,
        user_message: String,
    ) -> Result<TurnSummary, RuntimeError> {
        // 管理完整的 agent 循环
    }
}
```

#### Message 类型（OpenAI 兼容）

```rust
pub struct ConversationMessage {
    pub role: MessageRole,     // "user", "assistant", "tool"
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}
```

#### ToolCall 和执行

```rust
pub struct ToolCall {
    pub id: String,
    pub function: ToolDefinition,
    pub input: serde_json::Value,
}

pub struct ToolResultContentBlock {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: ToolCall) -> Result<String, ToolError>;
}
```

---

## 3. 详细实现规范

### 3.1 ConversationRuntime::run_turn() 伪代码

```python
async fn run_turn(user_message: String) -> Result<AssistantEvent> {
    # Step 1: 验证和存储用户消息
    user_msg = ConversationMessage {
        role: MessageRole::User,
        content: Some(user_message),
        tool_calls: None,
        tool_call_id: None,
    }
    self.session.add_message(user_msg)?;

    # Step 2: 构建系统提示
    system_prompt = self.build_system_prompt()?;
    api_messages = [
        { role: "system", content: system_prompt },
        ...self.session.messages,
    ];

    # Step 3: 调用 LLM
    iteration = 0
    loop:
        iteration += 1
        if iteration > self.max_turns {
            return Err("Max iterations exceeded");
        }

        # 检查是否需要压缩
        if self.should_compact() {
            self.compact_session()?;
        }

        # 调用 API
        response = await self.api.create_message(
            model: self.config.model,
            system: system_prompt,
            messages: api_messages,
            tools: self.tools.get_schemas(),
        )?;

        # 添加助手响应到历史
        assistant_msg = ConversationMessage {
            role: MessageRole::Assistant,
            content: response.content,
            tool_calls: response.tool_calls,
            tool_call_id: None,
        };
        self.session.add_message(assistant_msg)?;

        # Step 4: 检查是否有工具调用
        if response.tool_calls.is_empty() {
            # 没有工具调用，返回最终响应
            self.session.save()?;
            return Ok(AssistantEvent::MessageComplete {
                content: response.content,
                iterations: iteration,
            });
        }

        # Step 5: 执行工具
        for tool_call in response.tool_calls:
            # 检查权限
            if !self.permissions.allow_tool(&tool_call.function.name) {
                # 请求用户批准（或自动拒绝）
            }

            # 执行工具
            result = await self.tools.execute(tool_call)?;

            # 添加工具结果到历史
            tool_result = ConversationMessage {
                role: MessageRole::Tool,
                content: Some(result),
                tool_calls: None,
                tool_call_id: Some(tool_call.id),
            };
            self.session.add_message(tool_result)?;

            # 继续循环，获取下一个响应
            api_messages = [系统提示, ...更新的历史];
            # 回到循环开始
}
```

### 3.2 Prompt Builder 实现

```rust
pub fn build_system_prompt(&self) -> Result<String> {
    // 遵循 Hermes prompt_builder.py 的结构

    // Slot 1: SOUL.md (主身份)
    let mut prompt = load_soul_md().unwrap_or_default();

    // Slot 2: 工具描述
    let tools_desc = self.tools.format_descriptions();
    prompt.push_str(&format!("\n\n## Available Tools\n{}", tools_desc));

    // Slot 3: 上下文文件 (AGENTS.md, .cursorrules 等)
    let context = load_context_files()?;
    prompt.push_str(&format!("\n\n## Context\n{}", context));

    // Slot 4: 技能（如果有）
    let skills = load_user_skills()?;
    if !skills.is_empty() {
        prompt.push_str(&format!("\n\n## Available Skills\n{}", skills));
    }

    // Slot 5: 用户记忆（MEMORY.md, USER.md）
    let memory = load_persistent_memory();
    if !memory.is_empty() {
        prompt.push_str(&format!("\n\n## Your Memory\n{}", memory));
    }

    Ok(prompt)
}
```

### 3.3 错误处理和降级

```rust
pub enum RuntimeError {
    ApiError(String),              // LLM API 调用失败
    ToolError(String),             // 工具执行失败
    PermissionDenied(String),      // 权限拒绝
    SessionError(String),          // 会话保存/加载失败
    ConfigError(String),           // 配置错误
    MaxIterationsExceeded,         // 超过最大迭代数
}

// 降级策略（Hermes fallback_model）
impl ConversationRuntime {
    async fn api_call_with_fallback(
        &self,
        request: MessageRequest,
    ) -> Result<MessageResponse> {
        // 尝试主提供商
        match self.api.create_message(&request).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                // 如果是 429/503/5xx，尝试降级提供商
                if self.should_fallback(&e) {
                    tracing::warn!("Falling back to alternative provider: {}", e);
                    return self.api.create_message_fallback(&request).await;
                }
                return Err(RuntimeError::ApiError(e.to_string()));
            }
        }
    }
}
```

---

## 4. 与 Hermes 的对齐

### 4.1 消息格式兼容性

✅ **完全兼容**：

- OpenAI 消息格式 (role/content/tool_calls)
- Tool choice 支持 (auto/required/none)
- Tool result content blocks

✅ **部分支持**：

- 流式响应（需扩展）
- Extended thinking (Claude)
- Multi-modal content (vision blocks)

### 4.2 功能覆盖

| 功能            | Hermes  | If2Ai 现状 | 计划         |
| --------------- | ------- | ---------- | ------------ |
| 基础 Agent 循环 | ✅      | ✅ 90%     | ✅ 完成      |
| 消息历史管理    | ✅      | ✅ 100%    | ✅ 完成      |
| 工具执行        | ✅ 并发 | ⏳ 顺序    | Phase 2 并发 |
| 系统提示构建    | ✅      | ✅ 75%     | ⏳ 完善      |
| 会话持久化      | ✅      | ✅         | ✅ 完成      |
| 中断处理        | ✅      | ⏳ 基础    | ⏳ 完善      |
| 预算追踪        | ✅      | ❌         | Phase 2      |
| 上下文压缩      | ✅      | ❌         | Phase 2      |
| Provider 降级   | ✅      | ⏳ 部分    | Phase 2      |
| Prompt 缓存     | ✅      | ❌         | Phase 2      |

### 4.3 关键差异和计划

**Hermes 的高级特性**（If2Ai Phase 2+）：

1. **Context Compression** - LLM 总结中间消息
2. **Budget Pressure** - 迭代预算警告（70%, 90%）
3. **Tool Concurrency** - 并行工具执行
4. **Prompt Caching** - Anthropic/OpenAI 缓存
5. **Provider Failover** - 自动切换提供商

---

## 5. 集成检查清单

### Phase 1（当前）

- [x] ConversationRuntime 完整实现
- [x] OpenAI 兼容消息格式
- [x] Tool 执行框架
- [ ] 完整的 system prompt builder
- [ ] 权限和批准系统
- [ ] 完整的会话序列化

### Phase 2

- [ ] Tool 并发执行
- [ ] Context compression (借助 Gemini 等)
- [ ] Budget pressure warnings
- [ ] Provider fallback
- [ ] Prompt caching (Anthropic)

### Phase 3+

- [ ] Extended thinking
- [ ] Multi-modal support
- [ ] Advanced scheduling
- [ ] Memory system (Honcho)

---

## 6. 代码位置映射

| Hermes 模块             | 行数 | If2Ai 位置                         | 状态   |
| ----------------------- | ---- | ---------------------------------- | ------ |
| run_agent.py            | 9200 | crates/runtime/src/conversation.rs | ✅ 80% |
| prompt_builder.py       | 600  | crates/runtime/src/prompt.rs       | ⏳ 50% |
| model_tools.py          | 400  | crates/tools/src/lib.rs            | ✅ 90% |
| agent/context_engine.py | 300  | crates/runtime/src/compact.rs      | ⏳ 30% |
| hermes_state.py         | 500  | crates/runtime/src/session.rs      | ✅ 85% |

---

## 7. 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_turn_no_tools() {
        // 测试简单的文本回应（无工具调用）
    }

    #[tokio::test]
    async fn test_tool_execution() {
        // 测试工具调用和执行
    }

    #[tokio::test]
    async fn test_session_persistence() {
        // 测试会话保存和恢复
    }

    #[tokio::test]
    async fn test_error_handling() {
        // 测试错误情况（API 失败等）
    }
}
```

### 集成测试（Harness）

```python
def test_agent_correctness():
    """Agent 能给出合理的回答"""
    runtime = ConversationRuntime::new(...)
    response = runtime.run_turn("What is 2+2?")
    assert "4" in response.content

def test_tool_calling():
    """Agent 能调用工具"""
    runtime = ConversationRuntime::new(...)
    response = runtime.run_turn("List files in /tmp")
    assert len(response.tool_calls) > 0
```

---

## 8. 性能指标

| 指标          | 目标   | 度量方法  |
| ------------- | ------ | --------- |
| 平均响应时间  | <2s    | benchmark |
| 内存使用      | <500MB | valgrind  |
| Tool 执行时间 | <1s    | profiling |
| 会话加载时间  | <100ms | timing    |

---

## 参考资源

- [Hermes Agent Loop](https://hermes-agent.nousresearch.com/docs/developer-guide/agent-loop)
- [Claw Code conversation.rs](../../rust/crates/runtime/src/conversation.rs)
- [OpenAI Message Format](https://platform.openai.com/docs/guides/function-calling)
- [Anthropic Messages API](https://docs.anthropic.com/)

---

**下一步**: 阅读 [Provider Resolution](./provider-resolution.md) 了解多提供商支持的设计
