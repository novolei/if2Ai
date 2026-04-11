# Error Handling & Recovery 设计文档

> 错误分类、恢复策略和弹性设计是构建可靠 Agent 的基础。这个文档定义了完整的错误处理框架。

## 错误分类体系

```
┌─────────────────────────────────────┐
│ Agent Error                         │
├─────────────────────────────────────┤
│                                     │
├─ 可恢复易修复 (Recoverable)         │
│  ├─ RateLimit → 指数退避           │
│  ├─ Timeout → 重试                  │
│  ├─ ConnectionError → 重新连接      │
│  └─ TokenLimit → 压缩上下文         │
│                                     │
├─ 可转移故障转移 (Transferable)      │
│  ├─ InvalidAPIKey → 尝试备用       │
│  ├─ ModelUnavailable → 切换模型     │
│  ├─ ProviderDown → 使用备用提供商   │
│  └─ QuotaExceeded → 轮换提供商      │
│                                     │
└─ 致命无法恢复 (Fatal)              │
   ├─ InvalidToolCall → 拒绝         │
   ├─ SecurityViolation → 中止        │
   ├─ UserInterruption → 保存状态    │
   └─ SystemFailure → 崩溃日志       │
```

## 错误类型定义

```rust
#[derive(Debug, Clone)]
pub enum AgentError {
    // === Layer 1: LLM 错误 ===
    LLMError {
        provider: String,
        model: String,
        error_type: LLMErrorType,
        message: String,
        retriable: bool,
    },

    // === Layer 2: 工具执行错误 ===
    ToolError {
        tool_name: String,
        error_type: ToolErrorType,
        message: String,
        retriable: bool,
    },

    // === Layer 3: 上下文错误 ===
    ContextError {
        error_type: ContextErrorType,
        message: String,
        recovery_action: Option<String>,
    },

    // === Layer 4: 系统错误 ===
    SystemError {
        error_type: SystemErrorType,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LLMErrorType {
    // 网络和超时
    Timeout { duration_secs: u32 },
    ConnectionError(String),
    DNSResolution(String),

    // API 错误
    InvalidAPIKey,
    RateLimited { retry_after_secs: u32 },
    QuotaExceeded,
    InvalidRequest { reason: String },

    // 模型错误
    ModelNotFound,
    ModelDisabled,
    InsufficientTokens,

    // 其他
    ProviderDown,
    UnexpectedResponse(String),
    Unknown { status_code: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolErrorType {
    // 执行错误
    NotFound(String),
    InvalidParameters { reason: String },
    Timeout { duration_secs: u32 },
    ExecutionFailed(String),

    // 权限错误
    PermissionDenied { resource: String },
    SecurityViolation { violation: String },

    // 资源错误
    ResourceUnavailable { reason: String },
    OutOfMemory,

    // 依赖错误
    DependencyNotSatisfied { dependency: String },
    ConfigurationMissing { key: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextErrorType {
    ContextExhausted { current_tokens: u32, max_tokens: u32 },
    CompressionFailed { reason: String },
    InsufficientContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemErrorType {
    DatabaseError(String),
    ConfigurationError(String),
    InternalError(String),
    Panic(String),
}
```

## 恢复策略

```rust
pub trait RecoveryStrategy: Send + Sync {
    async fn execute(&self) -> Result<RecoveryOutcome>;
    fn is_applicable(&self, error: &AgentError) -> bool;
    fn priority(&self) -> u32;  // 优先级（0=最低，100=最高）
}

pub enum RecoveryOutcome {
    Recovered { action: String },
    Transferred { next_provider: String },
    Deferred { wait_time_secs: u32 },
    Failed { reason: String },
}

// === 恢复策略 1: 重试 ===
pub struct RetryStrategy {
    max_attempts: u32,
    backoff_strategy: BackoffStrategy,
}

pub enum BackoffStrategy {
    Fixed { wait_ms: u32 },
    Linear { base_ms: u32, multiplier: u32 },
    Exponential { base_ms: u32, max_ms: u32 },
    Jittered { base_ms: u32, jitter_factor: f32 },
}

impl RetryStrategy {
    pub async fn execute_with_retry<F, T>(
        &self,
        mut f: F,
    ) -> Result<T>
    where
        F: FnMut() -> BoxFuture<'static, Result<T>>,
    {
        for attempt in 1..=self.max_attempts {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retriable() => {
                    if attempt < self.max_attempts {
                        let wait = self.calculate_backoff(attempt);
                        tokio::time::sleep(wait).await;
                        continue;
                    } else {
                        return Err(e);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        match &self.backoff_strategy {
            BackoffStrategy::Fixed { wait_ms } => Duration::millis(*wait_ms as u64),
            BackoffStrategy::Linear { base_ms, multiplier } => {
                Duration::millis((base_ms + (attempt - 1) * multiplier) as u64)
            }
            BackoffStrategy::Exponential { base_ms, max_ms } => {
                let wait = base_ms * 2_u32.pow(attempt - 1);
                Duration::millis(wait.min(*max_ms) as u64)
            }
            BackoffStrategy::Jittered { base_ms, jitter_factor } => {
                let base = base_ms * 2_u32.pow(attempt - 1) as u32;
                let jitter = (base as f32 * jitter_factor) as u32;
                let random = rand::random::<u32>() % jitter;
                Duration::millis((base + random) as u64)
            }
        }
    }
}

// === 恢复策略 2: 提供商转移 ===
pub struct ProviderTransferStrategy {
    fallback_providers: Vec<ProviderConfig>,
    current_fallback_index: usize,
}

impl ProviderTransferStrategy {
    pub async fn execute(&mut self) -> Result<RecoveryOutcome> {
        if self.current_fallback_index >= self.fallback_providers.len() {
            return Ok(RecoveryOutcome::Failed {
                reason: "All fallback providers exhausted".to_string()
            });
        }

        let next_provider = &self.fallback_providers[self.current_fallback_index];
        self.current_fallback_index += 1;

        Ok(RecoveryOutcome::Transferred {
            next_provider: next_provider.provider_name.clone(),
        })
    }
}

// === 恢复策略 3: 上下文压缩 ===
pub struct ContextCompressionStrategy {
    compressor: Arc<ContextCompressor>,
}

impl ContextCompressionStrategy {
    pub async fn execute(
        &self,
        messages: &mut Vec<Message>,
    ) -> Result<RecoveryOutcome> {
        let stats = self.compressor.compress(messages).await?;

        Ok(RecoveryOutcome::Recovered {
            action: format!(
                "Compressed {} messages, freed {} tokens",
                stats.messages_summarized,
                stats.total_tokens_freed
            ),
        })
    }
}

// === 恢复策略 4: 模型降级 ===
pub struct ModelDowngradeStrategy {
    available_models: Vec<(String, String)>,  // (provider, model)
    current_index: usize,
}

impl ModelDowngradeStrategy {
    pub async fn execute(&mut self) -> Result<RecoveryOutcome> {
        if self.current_index >= self.available_models.len() {
            return Ok(RecoveryOutcome::Failed {
                reason: "No fallback models available".to_string(),
            });
        }

        let (provider, model) = &self.available_models[self.current_index];
        self.current_index += 1;

        Ok(RecoveryOutcome::Recovered {
            action: format!("Switched to {}/{}", provider, model),
        })
    }
}
```

## 错误处理中间件

```rust
pub struct ErrorHandler {
    strategies: Vec<Box<dyn RecoveryStrategy>>,
    error_log: Arc<ErrorLog>,
    circuit_breaker: CircuitBreaker,
}

impl ErrorHandler {
    pub async fn handle(&mut self, error: AgentError) -> Result<RecoveryOutcome> {
        // Step 1: 错误分类和日志
        let error_class = self.classify_error(&error);
        self.error_log.log(&error, &error_class);

        // Step 2: 检查熔断器状态
        if self.circuit_breaker.is_open() {
            return Err(AgentError::SystemError {
                error_type: SystemErrorType::InternalError(
                    "Circuit breaker is open, retries disabled".to_string()
                ),
                message: "System is in failure recovery mode".to_string(),
            });
        }

        // Step 3: 选择并执行恢复策略
        let mut applicable_strategies = self.strategies
            .iter()
            .filter(|s| s.is_applicable(&error))
            .collect::<Vec<_>>();

        // 按优先级排序
        applicable_strategies.sort_by_key(|s| std::cmp::Reverse(s.priority()));

        for strategy in applicable_strategies {
            match strategy.execute().await {
                Ok(outcome) => {
                    self.error_log.log_recovery(&error, &outcome);
                    return Ok(outcome);
                }
                Err(e) => {
                    // 这个策略失败，尝试下一个
                    self.error_log.log(&e, &ErrorClass::RecoveryFailed);
                    continue;
                }
            }
        }

        // Step 4: 所有策略都失败 → 记录并返回
        Err(error)
    }

    fn classify_error(&self, error: &AgentError) -> ErrorClass {
        match error {
            AgentError::LLMError { error_type, .. } => {
                match error_type {
                    LLMErrorType::RateLimited { .. } => ErrorClass::RateLimited,
                    LLMErrorType::Timeout { .. } => ErrorClass::Timeout,
                    LLMErrorType::InvalidAPIKey => ErrorClass::InvalidCredentials,
                    LLMErrorType::QuotaExceeded => ErrorClass::QuotaExceeded,
                    _ => ErrorClass::ProviderError,
                }
            }
            AgentError::ToolError { error_type, .. } => {
                match error_type {
                    ToolErrorType::NotFound(_) => ErrorClass::ToolNotFound,
                    ToolErrorType::Timeout { .. } => ErrorClass::Timeout,
                    ToolErrorType::PermissionDenied { .. } => ErrorClass::PermissionError,
                    _ => ErrorClass::ToolError,
                }
            }
            AgentError::ContextError { .. } => ErrorClass::ContextError,
            AgentError::SystemError { .. } => ErrorClass::SystemError,
        }
    }
}

pub enum ErrorClass {
    RateLimited,
    Timeout,
    InvalidCredentials,
    QuotaExceeded,
    ProviderError,
    ToolNotFound,
    ToolError,
    PermissionError,
    ContextError,
    SystemError,
    RecoveryFailed,
}

// === 熔断器模式 ===
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout_secs: u32,
    state: Arc<Mutex<CircuitBreakerState>>,
}

pub enum CircuitBreakerState {
    Closed { failure_count: u32 },
    Open { since: DateTime<Utc> },
    HalfOpen,
}

impl CircuitBreaker {
    pub fn is_open(&self) -> bool {
        matches!(self.state.blocking_lock(), CircuitBreakerState::Open { .. })
    }

    pub fn record_failure(&self) {
        let mut state = self.state.blocking_lock();
        match *state {
            CircuitBreakerState::Closed { ref mut failure_count } => {
                *failure_count += 1;
                if *failure_count >= self.failure_threshold {
                    *state = CircuitBreakerState::Open {
                        since: Utc::now(),
                    };
                }
            }
            _ => {}
        }
    }

    pub fn record_success(&self) {
        let mut state = self.state.blocking_lock();
        *state = CircuitBreakerState::Closed { failure_count: 0 };
    }
}
```

## 错误日志和分析

```rust
pub struct ErrorLog {
    entries: Arc<Mutex<Vec<ErrorLogEntry>>>,
    metrics: ErrorMetrics,
}

pub struct ErrorLogEntry {
    pub timestamp: DateTime<Utc>,
    pub error_class: ErrorClass,
    pub error: AgentError,
    pub recovery_attempted: bool,
    pub recovery_outcome: Option<RecoveryOutcome>,
    pub duration_until_recovery_ms: Option<u32>,
}

pub struct ErrorMetrics {
    pub total_errors: AtomicU32,
    pub recoverable_errors: AtomicU32,
    pub failed_recoveries: AtomicU32,
    pub average_recovery_time_ms: AtomicU32,
}
```

## 完美降级

```rust
pub struct GracefulDegradation {
    // 当无法使用丰富的工具时
    pub fallback_mode: bool,
    pub available_tools: HashSet<String>,
}

impl GracefulDegradation {
    pub async fn activate(&mut self, reason: &str) {
        self.fallback_mode = true;
        // 只启用关键工具（web_search, read_file）
        self.available_tools.retain(|tool| {
            ["web_search", "read_file", "terminal"].contains(&tool.as_str())
        });
        warn!("Graceful degradation activated: {}", reason);
    }

    pub async fn deactivate(&mut self) {
        self.fallback_mode = false;
        // 重新加载所有工具
    }
}
```

## Harness 集成

### 错误恢复评估

```yaml
test_case:
  name: 'Error Recovery'
  scenario: 'Primary LLM 返回速率限制错误，验证自动恢复'
  steps: 1. Mock OpenAI API 返回 429 Too Many Requests
    2. 触发 Agent 推理
    3. 验证系统自动执行指数退避
    4. 验证请求在延迟后重试成功

  evaluators:
    - name: behavior
      config:
        retries_attempted: '>= 1'
        backoff_strategy: 'exponential'
        final_success: true
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: DESIGN.md - Error Handling Principles
