# LLM Routing & Provider 设计文档

> 多提供商 LLM 支持是 Agent 的核心特性。这个文档基于 hermes-agent 的 8+ 提供商支持设计，提供完整的提供商检测、路由、故障转移和动态切换机制。

## 系统架构

```
用户请求
    ↓
┌─────────────────────────┐
│ ProviderDetector        │ ← 检测系统 API 密钥
└────────┬────────────────┘
         ↓
┌─────────────────────────────────┐
│ ProviderRouter                  │ ← 选择最佳第一提供商
│ (根据优先级、可用性、配置)       │
└────────┬────────────────────────┘
         ↓
    ┌────────────────────────────────────────┐
    │ Primary LLM Provider (e.g., OpenAI)    │
    └─────────────┬──────────────────────────┘
                  │
         ┌────────┴──────────┐
         │ (Success)         │ (Error)
         ↓                   ↓
    [Return]     ┌──────────────────────────┐
                 │ FallbackResolver         │
                 │ (选择下一个提供商)        │
                 └────────┬─────────────────┘
                          ↓
                 [Fallback 1, 2, 3, ...]
```

## 核心组件

### 1. 支持的 LLM 提供商

基于 hermes-agent 的实现，支持 8+ 提供商：

```rust
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum LLMProvider {
    OpenAI,              // API: api.openai.com
    Anthropic,           // API: api.anthropic.com
    Claude,              // 替代名称 for Anthropic
    OpenRouter,          // API: openrouter.io (多模型聚合)
    GitHubCopilot,       // API: copilot-api.github.com
    Kimi,                // 月之暗面 (中国)
    MiniMax,             // API: api-us.minimaxi.com
    DashScope,           // 阿里云
    Deepseek,            // API: api.deepseek.com
    LocalOllama,         // 本地 LLaMA 推理
    Custom,              // 自定义兼容 OpenAI 的端点
}

pub struct LLMProviderConfig {
    pub provider: LLMProvider,
    pub model: String,                      // e.g., "gpt-4", "claude-3-opus"
    pub base_url: String,
    pub api_key_env: String,                // e.g., "OPENAI_API_KEY"
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    pub timeout_secs: u32,
    pub priority: i32,                      // 优先级（越高越优先）
    pub is_enabled: bool,
}

impl LLMProviderConfig {
    pub fn is_available(&self) -> bool {
        is_enabled && std::env::var(&self.api_key_env).is_ok()
    }

    pub fn api_key(&self) -> Result<String> {
        std::env::var(&self.api_key_env)
            .map_err(|_| ProviderError::MissingApiKey(self.api_key_env.clone()))
    }
}
```

### 2. Provider Detector（提供商检测器）

```rust
pub struct ProviderDetector {
    all_configs: Vec<LLMProviderConfig>,
    cache: Arc<DashMap<String, bool>>,      // 缓存可用性检查
    cache_ttl_secs: u32,                    // 默认 300 秒
}

impl ProviderDetector {
    pub fn detect_available_providers(&self) -> Vec<LLMProvider> {
        self.all_configs
            .iter()
            .filter(|cfg| {
                // 检查环境变量
                if std::env::var(&cfg.api_key_env).is_err() {
                    return false;
                }
                // 检查启用状态
                if !cfg.is_enabled {
                    return false;
                }
                true
            })
            .map(|cfg| cfg.provider.clone())
            .collect()
    }

    pub fn detect_primary(&self) -> Result<LLMProviderConfig> {
        // 返回优先级最高的可用提供商
        self.all_configs
            .iter()
            .filter(|cfg| cfg.is_available())
            .max_by_key(|cfg| cfg.priority)
            .cloned()
            .ok_or(ProviderError::NoProviderAvailable)
    }

    pub async fn probe_provider(&self, provider: &LLMProvider) -> Result<()> {
        // 发送测试请求以验证连接和凭证
        let config = self.get_config(provider)?;
        let client = create_client(&config)?;

        let response = client.complete(
            &[Message::user("test".to_string())],
            &[],
            false,
        ).await?;

        Ok(())
    }
}
```

### 3. Provider Router（路由器）

```rust
pub struct ProviderRouter {
    primary_config: LLMProviderConfig,
    fallback_chain: Vec<LLMProviderConfig>,
    detector: Arc<ProviderDetector>,
    rate_limiter: Arc<RateLimitManager>,
}

impl ProviderRouter {
    pub fn new_from_env() -> Result<Self> {
        let detector = ProviderDetector::new();
        let primary = detector.detect_primary()?;
        let mut all_available = detector.detect_available_providers();
        all_available.remove_first(&primary);  // 移除主提供商

        let fallback_chain = all_available
            .iter()
            .map(|p| detector.get_config(p))
            .collect::<Result<Vec<_>>>()?;

        Ok(ProviderRouter {
            primary_config: primary,
            fallback_chain,
            detector: Arc::new(detector),
            rate_limiter: Arc::new(RateLimitManager::new()),
        })
    }

    pub async fn route_request<T: Request>(
        &mut self,
        request: &T,
        attempt: usize,
    ) -> Result<LLMResponse> {
        let config = if attempt == 0 {
            &self.primary_config
        } else if attempt <= self.fallback_chain.len() {
            &self.fallback_chain[attempt - 1]
        } else {
            return Err(ProviderError::AllFallbacksExhausted);
        };

        // 速率限制检查
        self.rate_limiter.check_and_consume(&config.provider)?;

        // 创建客户端
        let client = create_provider_client(config)?;

        // 发送请求
        client.complete(&request.messages, &request.tools, request.stream).await
    }

    pub async fn switch_provider(
        &mut self,
        provider: &LLMProvider,
        model: &str,
    ) -> Result<()> {
        let config = self.detector.get_config(provider)?;
        self.detector.probe_provider(provider).await?;

        self.primary_config = config;
        self.primary_config.model = model.to_string();

        Ok(())
    }
}
```

### 4. Rate Limiting & Backoff

```rust
pub struct RateLimitManager {
    // 按提供商追踪速率限制
    limits: Arc<DashMap<LLMProvider, ProviderLimit>>,
}

pub struct ProviderLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u32,
    pub current_requests: u32,
    pub current_tokens: u32,
    pub reset_time: DateTime<Utc>,
    pub retry_after: Option<Duration>,
}

impl RateLimitManager {
    pub fn check_and_consume(
        &self,
        provider: &LLMProvider,
        tokens: u32,
    ) -> Result<()> {
        let mut limit = self.limits.get_mut(provider)
            .ok_or(ProviderError::UnknownProvider)?;

        // 检查 window 是否过期
        if Utc::now() > limit.reset_time {
            limit.current_requests = 0;
            limit.current_tokens = 0;
            limit.reset_time = Utc::now() + Duration::minutes(1);
        }

        if limit.current_requests >= limit.requests_per_minute {
            return Err(ProviderError::RateLimited {
                retry_after: limit.reset_time - Utc::now(),
            });
        }

        if limit.current_tokens + tokens > limit.tokens_per_minute {
            return Err(ProviderError::TokenLimitExceeded {
                requested: tokens,
                available: limit.tokens_per_minute - limit.current_tokens,
            });
        }

        limit.current_requests += 1;
        limit.current_tokens += tokens;
        Ok(())
    }

    pub fn apply_backoff(&self, provider: &LLMProvider, retry_count: u32) -> Duration {
        // 指数退避：2^n 秒，最多 60 秒
        let backoff = Duration::seconds(2_i64.pow(retry_count).min(60) as i64);

        let mut limit = self.limits.get_mut(provider).unwrap();
        limit.retry_after = Some(backoff);

        backoff
    }
}
```

### 5. Provider Client Factory

```rust
pub trait LLMClient: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Tool],
        stream: bool,
    ) -> Result<LLMResponse>;

    async fn stream_complete(
        &self,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<impl futures::Stream<Item = Result<StreamChunk>>>;
}

pub fn create_provider_client(
    config: &LLMProviderConfig,
) -> Result<Arc<dyn LLMClient>> {
    match config.provider {
        LLMProvider::OpenAI => {
            Ok(Arc::new(OpenAIClient::new(config.clone())?))
        }
        LLMProvider::Anthropic => {
            Ok(Arc::new(AnthropicClient::new(config.clone())?))
        }
        LLMProvider::OpenRouter => {
            Ok(Arc::new(OpenRouterClient::new(config.clone())?))
        }
        // ... 其他提供商
        LLMProvider::Custom => {
            Ok(Arc::new(CustomCompatClient::new(config.clone())?))
        }
    }
}

// OpenAI 兼容客户端基类
pub struct OpenAICompatClient {
    base_url: String,
    api_key: String,
    model: String,
    temperature: f32,
    http_client: reqwest::Client,
}

impl OpenAICompatClient {
    pub async fn complete(
        &self,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<LLMResponse> {
        let request_body = json!({
            "model": self.model,
            "messages": messages,
            "tools": self.format_tools(tools),
            "temperature": self.temperature,
        });

        let response = self.http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        let data = response.json::<JsonValue>().await?;

        Ok(LLMResponse {
            content: data["choices"][0]["message"]["content"].as_str().unwrap().to_string(),
            tool_calls: self.parse_tool_calls(&data),
            stop_reason: data["choices"][0]["finish_reason"].as_str().unwrap().to_string(),
            usage: Usage {
                prompt_tokens: data["usage"]["prompt_tokens"].as_i64().unwrap_or(0) as u32,
                completion_tokens: data["usage"]["completion_tokens"].as_i64().unwrap_or(0) as u32,
            },
        })
    }
}
```

## 错误处理与恢复

```rust
pub enum ProviderError {
    // 可重试
    RateLimited { retry_after: Duration },
    Timeout,
    ConnectionError(String),
    TemporaryServiceError,

    // 转移到备用提供商
    InvalidAPIKey,
    AccountQuotaExceeded,
    ModelNotAvailable,

    // 致命错误
    MissingApiKey(String),
    NoProviderAvailable,
    AllFallbacksExhausted,
    InvalidConfiguration,
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            RateLimited { .. } | Timeout | ConnectionError(_) | TemporaryServiceError
        )
    }

    pub fn is_transferable(&self) -> bool {
        matches!(self,
            InvalidAPIKey | AccountQuotaExceeded | ModelNotAvailable
        )
    }
}
```

## 配置示例

```yaml
# config.yaml
llm:
  providers:
    - name: openai
      enabled: true
      model: gpt-4-turbo-preview
      api_key_env: OPENAI_API_KEY
      base_url: https://api.openai.com/v1
      priority: 100

    - name: anthropic
      enabled: true
      model: claude-3-opus-20240229
      api_key_env: ANTHROPIC_API_KEY
      base_url: https://api.anthropic.com
      priority: 90

    - name: openrouter
      enabled: true
      model: openai/gpt-4
      api_key_env: OPENROUTER_API_KEY
      base_url: https://openrouter.io/api/v1
      priority: 80

    - name: deepseek
      enabled: false # 需要手动启用
      model: deepseek-chat
      api_key_env: DEEPSEEK_API_KEY
      base_url: https://api.deepseek.com
      priority: 50
```

## Harness 集成

### Provider Routing 评估

```yaml
test_case:
  name: 'Provider Failover'
  scenario: 'Primary provider 返回错误，自动转移到备用提供商'
  steps: 1. Mock OpenAI API 返回 503 Service Unavailable
    2. 执行 Agent 推理
    3. 验证系统自动切换到 Anthropic
    4. 验证响应成功取得

  evaluators:
    - name: behavior
      config:
        providers_used: ['anthropic'] # 备用提供商应被使用
        retries_attempted: 1
```

### 多模型切换

```yaml
test_case:
  name: 'Dynamic Model Switch'
  prompt: 'Analyze this complex code'
  setup:
    primary_model: 'gpt-3.5-turbo' # 快速，便宜
    fallback_model: 'gpt-4-turbo' # 更强大
  trigger: 'Output quality < threshold'
  evaluators:
    - name: model_switching
      config:
        should_switch: true
        target_model: 'gpt-4-turbo'
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [docs/references/hermes-agent-analysis.md](../../references/hermes-agent-analysis.md) - LLM Integration
