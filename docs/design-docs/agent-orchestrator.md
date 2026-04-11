# Agent Orchestrator 设计文档

> Agent Orchestrator 是整个系统的核心协调引擎，负责对话循环、工具调用、预算管理和 LLM 交互。基于 hermes-agent 的 AIAgent 类（3600+ 行），这是项目最复杂的模块。

## 设计概述

Agent Orchestrator 是一个有状态的运行时环境，管理单个对话会话的完整生命周期。

```
用户输入 → 会话加载 → 提示词构建 → LLM 调用 → 工具响应 → 上下文压缩 → 消息保存 → 输出
  ↑                                                                                │
  └────────────────────────── 循环直到完成或达到预算限制 ────────────────────────┘
```

## 核心职责

### 1. 对话循环管理

**入口点**：`run_conversation(user_message, conversation_history=None)`

```rust
pub async fn run_conversation(
    &mut self,
    user_message: String,
    conversation_history: Option<Vec<Message>>,
) -> Result<ConversationResult> {
    // Phase 1: 预处理
    let mut messages = self.load_or_init_history(conversation_history);
    
    // Phase 2: 主循环（迭代预算管理）
    while !self.iteration_budget.is_exhausted() {
        // 2a. 构建提示词
        let system_prompt = self.prompt_builder.build(&messages)?;
        messages.push(Message::user(user_message.clone()));
        
        // 2b. 调用 LLM（流式处理）
        let response = self.llm_client.stream_response(
            &system_prompt,
            &messages,
            &self.tool_definitions(),
        ).await?;
        
        // 2c. 处理流式响应
        messages.push(Message::assistant(response.content.clone()));
        
        // 2d. 解析和执行工具调用
        if !response.tool_calls.is_empty() {
            let results = self.execute_tools(&response.tool_calls).await?;
            messages.extend(results);
        } else {
            // Agent 已完成推理
            break;
        }
        
        // 2e. 管理上下文
        self.maybe_compress_context(&mut messages)?;
    }
    
    // Phase 3: 持久化
    self.save_session(&messages).await?;
    
    Ok(ConversationResult {
        messages,
        metrics: self.collect_metrics(),
    })
}
```

### 2. 预算追踪系统

三层预算模型，确保资源管理和成本控制：

```rust
pub struct BudgetTracker {
    // 层级 1: 迭代预算（最大 Agent 思考步数）
    pub iteration_budget: IterationBudget {
        max_iterations: u32,           // 默认 20
        current_iteration: u32,
    },
    
    // 层级 2: 上下文窗口预算
    pub context_budget: ContextBudget {
        total_tokens: u32,             // 模型的窗口大小（如 4096）
        used_tokens: u32,              // 已用 token 数
        compression_threshold: f32,    // 默认 50%
    },
    
    // 层级 3: Token/成本预算（可选）
    pub token_budget: TokenBudget {
        max_tokens: Option<u32>,
        max_cost_usd: Option<f32>,
        current_tokens_used: u32,
        current_cost_usd: f32,
    },
}

impl BudgetTracker {
    pub fn check_and_consume(
        &mut self,
        tokens_to_use: u32,
        iteration_increment: u32,
    ) -> Result<()> {
        // 检查所有三层预算
        if self.context_budget.used_tokens + tokens_to_use > self.context_budget.total_tokens {
            return Err(BudgetError::ContextExhausted);
        }
        if self.iteration_budget.current_iteration >= self.iteration_budget.max_iterations {
            return Err(BudgetError::IterationsExhausted);
        }
        
        // 消费预算
        self.context_budget.used_tokens += tokens_to_use;
        self.iteration_budget.current_iteration += iteration_increment;
        
        Ok(())
    }
    
    pub fn should_compress(&self) -> bool {
        let usage_ratio = self.context_budget.used_tokens as f32 
            / self.context_budget.total_tokens as f32;
        usage_ratio > self.context_budget.compression_threshold
    }
}
```

### 3. LLM 客户端生命周期

支持动态模型切换、故障转移和多中断处理：

```rust
pub struct LLMClientLifecycle {
    primary_config: ProviderConfig,
    fallback_chain: Vec<ProviderConfig>,
    current_client: Box<dyn LLMProvider>,
    rate_limit_state: RateLimitTracker,
}

impl LLMClientLifecycle {
    pub async fn call_llm(
        &mut self,
        messages: &[Message],
        tools: &[Tool],
        stream: bool,
    ) -> Result<LLMResponse> {
        loop {
            match self.current_client.complete(messages, tools, stream).await {
                Ok(response) => {
                    self.rate_limit_state.record_success(&response);
                    return Ok(response);
                }
                Err(e) if e.is_rate_limit() => {
                    // 暂停后重试
                    self.rate_limit_state.apply_backoff();
                    self.current_client.sleep_backoff().await;
                    continue;
                }
                Err(e) if e.is_recoverable() => {
                    // 尝试下一个提供商
                    self.switch_to_next_fallback()?;
                    continue;
                }
                Err(e) => {
                    // 致命错误
                    return Err(e);
                }
            }
        }
    }
    
    pub async fn switch_model(
        &mut self,
        provider: &str,
        model: &str,
        credentials: ProviderCredentials,
    ) -> Result<()> {
        // 不需要重新加载会话，只切换 client
        self.current_client = create_provider_client(provider, model, credentials)?;
        Ok(())
    }
}
```

### 4. 工具执行管理

支持并行执行、依赖解析和安全性检查：

```rust
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    max_parallel_workers: usize,  // 默认 8
}

impl ToolExecutor {
    pub async fn execute_batch(
        &self,
        tool_calls: Vec<ToolCall>,
        task_context: &TaskContext,
    ) -> Result<Vec<ToolResult>> {
        // Phase 1: 验证和依赖检查
        let sorted_calls = self.resolve_dependencies(&tool_calls)?;
        
        // Phase 2: 分组执行
        let (parallel, sequential) = self.partition_calls(&sorted_calls);
        
        let mut results = Vec::new();
        
        // Phase 2a: 并行执行（无依赖）
        let parallel_results = futures::future::join_all(
            parallel.iter().map(|call| self.execute_single(call, task_context))
        ).await;
        results.extend(parallel_results);
        
        // Phase 2b: 顺序执行（有依赖或需要同步）
        for call in sequential {
            let result = self.execute_single(&call, task_context).await?;
            results.push(result);
        }
        
        Ok(results)
    }
    
    async fn execute_single(
        &self,
        call: &ToolCall,
        context: &TaskContext,
    ) -> Result<ToolResult> {
        // 安全性检查
        self.registry.validate_tool_call(call)?;
        
        // 执行
        let result = self.registry.dispatch(&call.name, &call.args).await?;
        
        // 结果处理
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            output: result,
            execution_time: duration,
        })
    }
}
```

### 5. 状态管理

保持一致的执行上下文：

```rust
pub struct OrchestratorState {
    // 会话信息
    pub session_id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    
    // 运行时状态
    pub messages: Vec<Message>,
    pub tool_definitions: Vec<Tool>,
    pub budgets: BudgetTracker,
    
    // 执行快照
    pub primary_runtime: RuntimeSnapshot,
    pub fallback_chain: Vec<RuntimeSnapshot>,
    pub rate_limit_state: RateLimitTracker,
    
    // 性能指标
    pub metrics: ExecutionMetrics {
        total_tokens: u32,
        tool_calls_count: u32,
        compression_count: u32,
        error_count: u32,
        duration_secs: f32,
    },
}

pub struct RuntimeSnapshot {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub snapshot_time: DateTime<Utc>,
}
```

## 实现细节

### 错误分类和恢复

```rust
pub enum OrchestratorError {
    // 可恢复（自动重试）
    RateLimit { retry_after: Duration },
    ContextExhausted,           // 触发压缩
    TokenLimitExceeded,         // 切换到更小模型
    
    // 可转移（尝试备用提供商）
    ProviderUnavailable,
    InvalidAPIKey,
    
    // 致命（无法恢复）
    InvalidToolCall,
    UserInterruption,
    SystemFailure,
}

impl OrchestratorError {
    pub fn is_recoverable(&self) -> bool { ... }
    pub fn is_transferable(&self) -> bool { ... }
    pub fn recovery_strategy(&self) -> RecoveryStrategy { ... }
}
```

### 日志和监测

```rust
pub struct OrchestrationMetrics {
    // 每次迭代收集
    iteration_num: u32,
    llm_response_time: Duration,
    tokens_generated: u32,
    tools_called: Vec<String>,
    context_compression_triggered: bool,
    
    // 聚合指标
    pub total_iterations: u32,
    pub total_tokens: u32,
    pub total_tool_calls: u32,
    pub total_compressions: u32,
    pub average_iteration_time: Duration,
}
```

## 与其他模块的集成

```
Agent Orchestrator
├── ToolRegistry (工具调度)
├── PromptBuilder (提示词构建)
├── LLMProvider (LLM 调用)
├── ContextCompressor (上下文管理)
├── MemoryProvider (会话持久化)
└── BudgetTracker (资源管理)
```

## Tauri IPC 集成

```rust
#[tauri::command]
pub async fn run_agent_command(
    state: State<'_, OrchestratorState>,
    user_message: String,
) -> Result<ConversationResult> {
    let mut orchestrator = state.lock().await;
    orchestrator.run_conversation(user_message, None).await
}
```

## 性能特征

| 指标 | 目标 | 说明 |
|------|------|------|
| 首次响应 | < 500ms | 包括 LLM 延迟 |
| 工具执行 | 并行 8 个 | 取决于工具依赖 |
| 内存占用 | < 500MB | 单个会话 |
| 消息历史 | 支持 10000+ 条 | 带自动压缩 |

## 测试策略

### 单元测试
- 预算消费和检查
- 依赖解析算法
- 错误分类逻辑
- 状态转换

### 集成测试
- 完整对话循环
- 工具执行和结果整合
- 上下文压缩触发
- 提供商故障转移

### Harness 评估
- Agent 信息收集能力
- 工具用法合适性
- 预算 SLA 遵守
- 错误恢复成功率

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [docs/references/hermes-agent-analysis.md](../../references/hermes-agent-analysis.md)
