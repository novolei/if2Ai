# Provider Resolution & LLM Client

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes Provider Runtime (~500 行)  
**实现语言**: Rust  
**关键文件**: `crates/api/src/` (providers, client, types)  

---

## 1. 系统概览

### 1.1 在 Hermes 中的角色

Hermes Provider 系统负责：
- 🔌 支持 18+ LLM 提供商
- 🔐 OAuth 和 API 密钥管理
- 🔀 提供商选择和别名解析
- 💾 凭证缓存和刷新
- 🚀 自动降级和故障转移
- 🎯 模型别名和路由

### 1.2 在 Claw Code 中的现状

**现有实现**（`src/providers/`）：
```
✅ ProviderClient - 统一客户端接口
✅ Anthropic 兼容支持 (AWS, Prompt Caching)
✅ OpenAI 兼容支持 (OpenRouter, 自定义端点)
✅ Grok/X.AI 支持
✅ OAuth 流程
✅ 流式响应（SSE）
⏳ 凭证池（轮换策略）
⏳ 自动降级
```

**代码规模**：~1,200 行（api 模块）

---

## 2. 架构设计

### 2.1 提供商类型层次

```
                       ProviderClient
                            |
                   ┌────────┼────────┐
                   |        |        |
            Anthropic    OpenAI   Custom
              (AWS)     (OpenRouter)  (自定义端点)
               |          |           |
          ┌───┴───┐   ┌────┴────┐   ├──────┐
          |       |   |         |   |      |
        Claude GPT-4  o1     Grok  Ollama  自定义
       (native) (via  (o1-   (x.ai) (本地)  (各种)
                 compat) preview)
```

### 2.2 关键类型定义

#### ProviderClient 统一接口
```rust
pub struct ProviderClient {
    kind: ProviderKind,
    config: ProviderConfig,
    http_client: reqwest::Client,
}

pub enum ProviderKind {
    Anthropic(AnthropicConfig),
    OpenAi(OpenAiCompatConfig),
    ClawProvider(ClawProviderConfig),    // 默认路由
    Custom(CustomProviderConfig),
}

impl ProviderClient {
    /// 创建消息（核心 API）
    pub async fn create_message(
        &self,
        request: MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        match self.kind {
            ProviderKind::Anthropic(ref cfg) => {
                self.create_message_anthropic(cfg, request).await
            }
            ProviderKind::OpenAi(ref cfg) => {
                self.create_message_openai().await
            }
            // ... 其他提供商
        }
    }

    /// 流式消息
    pub async fn create_message_stream(
        &self,
        request: MessageRequest,
    ) -> Result<MessageStream, ApiError>;

    /// 模型列表
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ApiError>;
}
```

#### 提供商配置
```rust
pub struct ProviderConfig {
    pub base_url: String,              // API 端点
    pub api_key: String,               // 认证密钥
    pub model: String,                 // 默认模型
    pub timeout: Duration,             // 请求超时
    pub retry_policy: RetryPolicy,     // 重试策略
    pub feature_flags: FeatureFlags,   // 功能标志
}

pub struct FeatureFlags {
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_thinking: bool,       // Claude/o1
    pub supports_batching: bool,
    pub max_tokens: usize,
}
```

### 2.3 API 模式支持

```rust
pub enum ApiMode {
    /// OpenAI 标准格式 (GPT-4, OpenRouter 等)
    ChatCompletions,
    
    /// Anthropic Messages API (Claude 等)
    AnthropicMessages,
    
    /// 其他兼容格式
    Custom(String),
}

pub struct MessageRequest {
    pub model: String,
    pub messages: Vec<InputMessage>,
    pub system: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub stream: bool,
}

pub struct MessageResponse {
    pub id: String,
    pub model: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    pub stop_reason: StopReason,
}
```

---

## 3. 详细实现规范

### 3.1 Provider Resolution 流程

```
用户选择提供商
  ↓
环境变量检查 (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
  ↓
凭证文件检查 (~/.claw/auth.json, ~/.aws/credentials)
  ↓
OAuth 令牌检查（如需要）
  ↓
构建 ProviderClient
  ├─ 验证 API 密钥
  ├─ 检查配额限制
  └─ 尝试列出模型（连接测试）
  ↓
缓存凭证
  ↓
准备就绪
```

### 3.2 多提供商支持实现

```rust
pub struct ProviderManager {
    providers: HashMap<String, ProviderClient>,
    default_provider: String,
    fallback_chain: Vec<String>,  // 降级链
}

impl ProviderManager {
    /// 自动检测可用提供商
    pub async fn auto_detect() -> Result<Self> {
        let mut providers = HashMap::new();

        // 检查 Anthropic
        if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            providers.insert(
                "anthropic".to_string(),
                ProviderClient::new_anthropic(&key)?,
            );
        }

        // 检查 OpenAI (直接或通过 OpenRouter)
        if let Ok(key) = env::var("OPENAI_API_KEY") {
            providers.insert(
                "openai".to_string(),
                ProviderClient::new_openai(&key)?,
            );
        }

        // 检查 OpenRouter (通用)
        if let Ok(key) = env::var("OPENROUTER_API_KEY") {
            providers.insert(
                "openrouter".to_string(),
                ProviderClient::new_openrouter(&key)?,
            );
        }

        // 检查 Grok/X.AI
        if let Ok(key) = env::var("XAI_API_KEY") {
            providers.insert(
                "grok".to_string(),
                ProviderClient::new_xai(&key)?,
            );
        }

        Ok(ProviderManager {
            providers,
            default_provider: "anthropic".to_string(),  // 优先
            fallback_chain: vec![
                "openrouter".to_string(),
                "openai".to_string(),
                "grok".to_string(),
            ],
        })
    }

    /// 选择提供商（带降级）
    pub async fn get_provider(
        &self,
        name: &str,
    ) -> Result<ProviderClient> {
        if let Some(provider) = self.providers.get(name) {
            return Ok(provider.clone());
        }

        // 如果请求的提供商不存在，使用默认值
        self.providers
            .get(&self.default_provider)
            .ok_or_else(|| ApiError::ProviderNotFound(name.to_string()))
    }

    /// 尝试带降级的 API 调用
    pub async fn create_message_with_fallback(
        &self,
        request: MessageRequest,
    ) -> Result<MessageResponse> {
        let mut last_error = None;

        // 优先尝试默认提供商
        for provider_name in std::iter::once(&self.default_provider)
            .chain(self.fallback_chain.iter())
        {
            match self.get_provider(provider_name) {
                Ok(provider) => match provider.create_message(&request).await {
                    Ok(response) => return Ok(response),
                    Err(e) => {
                        tracing::warn!(
                            "Provider {} failed: {}, trying fallback",
                            provider_name,
                            e
                        );
                        last_error = Some(e);
                    }
                },
                Err(_) => continue,
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ApiError::NoProvidersAvailable
        }))
    }
}
```

### 3.3 模型别名和路由

```rust
pub struct ModelRouter {
    aliases: HashMap<String, String>,
    providers: HashMap<String, Vec<String>>,  // 提供商 -> 模型列表
}

impl ModelRouter {
    /// 解析模型别名
    pub fn resolve_model(&self, model_name: &str) -> String {
        if let Some(alias) = self.aliases.get(model_name) {
            alias.clone()
        } else {
            model_name.to_string()
        }
    }

    /// 获取模型的推荐提供商
    pub fn recommend_provider(&self, model: &str) -> Option<String> {
        let model = self.resolve_model(model);
        
        for (provider, models) in &self.providers {
            if models.iter().any(|m| m == &model) {
                return Some(provider.clone());
            }
        }
        None
    }
}

// 全局别名表
lazy_static::lazy_static! {
    static ref MODEL_ALIASES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("claude", "claude-3-5-sonnet-20241022");
        m.insert("gpt4", "gpt-4o");
        m.insert("gpt", "gpt-4o-mini");
        m.insert("o1", "o1-preview");
        m.insert("sonnet", "claude-3-5-sonnet-20241022");
        m.insert("opus", "claude-3-opus-20250219");
        m.insert("haiku", "claude-3-5-haiku-20241022");
        m
    };
}
```

### 3.4 凭证管理

```rust
pub struct CredentialStore {
    path: PathBuf,  // ~/.claw/credentials.json
    cache: RwLock<HashMap<String, StoredCredential>>,
}

#[derive(Serialize, Deserialize)]
pub struct StoredCredential {
    pub provider: String,
    pub credential_type: CredentialType,
    pub value: String,
    pub expires_at: Option<SystemTime>,
}

pub enum CredentialType {
    ApiKey,
    OAuthToken,
    OAuthRefreshToken,
    AwsAccessKey,  // AWS Bedrock
}

impl CredentialStore {
    /// 保存凭证（加密）
    pub async fn save(&self, cred: StoredCredential) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.insert(cred.provider.clone(), cred.clone());
        
        // 保存到文件（加密）
        self.persist(&cache)?;
        Ok(())
    }

    /// 获取凭证（带过期检查）
    pub async fn get(&self, provider: &str) -> Option<StoredCredential> {
        let cache = self.cache.read().await;
        
        if let Some(cred) = cache.get(provider) {
            // 检查是否过期
            if let Some(expires) = cred.expires_at {
                if SystemTime::now() < expires {
                    return Some(cred.clone());
                }
            } else {
                return Some(cred.clone());
            }
        }
        None
    }

    /// 刷新过期令牌（OAuth）
    pub async fn refresh_token(
        &self,
        provider: &str,
    ) -> Result<String> {
        // 调用提供商的刷新端点
        // 更新凭证存储
        todo!()
    }
}
```

---

## 4. 支持的提供商详细信息

### 4.1 Anthropic (Claude)

```rust
pub struct AnthropicConfig {
    pub api_key: String,
    pub base_url: String,    // 默认: https://api.anthropic.com
    pub api_version: String, // 默认: 2024-06-01
    pub available_models: Vec<&'static str>,
}

impl AnthropicConfig {
    pub fn available_models() -> Vec<&'static str> {
        vec![
            "claude-3-5-sonnet-20241022",      // 推荐
            "claude-3-5-haiku-20241022",
            "claude-3-opus-20250219",
            "claude-3-5-sonnet-20240620",
        ]
    }
    
    pub fn features(&self) -> FeatureFlags {
        FeatureFlags {
            supports_streaming: true,
            supports_tools: true,
            supports_vision: true,
            supports_thinking: true,    // Extended thinking
            supports_batching: true,
            max_tokens: 200_000,
        }
    }
}
```

### 4.2 OpenAI (GPT)

```rust
pub struct OpenAiConfig {
    pub api_key: String,
    pub base_url: String,    // 默认: https://api.openai.com/v1
    pub organization: Option<String>,
    pub available_models: Vec<&'static str>,
}

impl OpenAiConfig {
    pub fn available_models() -> Vec<&'static str> {
        vec![
            "gpt-4o",              // 推荐
            "gpt-4o-mini",
            "gpt-4-turbo",
            "o1-preview",
            "o1-mini",
        ]
    }
}
```

### 4.3 OpenRouter (通用网关)

```rust
pub struct OpenRouterConfig {
    pub api_key: String,
    pub base_url: String,    // https://openrouter.ai/api/v1
    pub preferred_provider: Option<String>,
}

// OpenRouter 支持 100+ 模型，包括：
//  - Claude (Anthropic)
//  - GPT (OpenAI)
//  - Mistral
//  - Llama
//  - 等等
```

### 4.4 Grok (X.AI)

```rust
pub struct GrokConfig {
    pub api_key: String,
    pub base_url: String,    // https://api.x.ai/v1
    pub available_models: Vec<&'static str>,
}

impl GrokConfig {
    pub fn available_models() -> Vec<&'static str> {
        vec![
            "grok-3",              // 最新
            "grok-2-1212",
            "grok-2-vision-1212",
        ]
    }
}
```

### 4.5 自定义端点 (Ollama, 私有部署)

```rust
pub struct CustomConfig {
    pub base_url: String,           // http://localhost:11434/v1 等
    pub api_key: Option<String>,    // 可选认证
    pub available_models: Vec<String>,
    pub supports_streaming: bool,
}

// 示例：
// Ollama           http://localhost:11434/v1
// vLLM             http://localhost:8000/v1
// LM Studio        http://localhost:1234/v1
// Ollama (通过 OpenAI 兼容)     http://localhost:11434/v1
```

---

## 5. 与 Hermes 的对齐

| 功能 | Hermes | If2Ai 现状 | 计划 |
|------|--------|----------|------|
| 多提供商支持（18+） | ✅ | ✅ 70% | Phase 2 完整 |
| OAuth 流程 | ✅ | ✅ | ✅ 完成 |
| API 密钥管理 | ✅ | ✅ | ✅ 完成 |
| 凭证缓存 | ✅ | ✅ | ✅ 完成 |
| 自动降级 | ✅ | ⏳ 部分 | Phase 2 完善 |
| 模型别名 | ✅ | ✅ 70% | Phase 2 完整 |
| 凭证池（轮换） | ✅ | ❌ | Phase 2 |
| 流式响应 | ✅ | ✅ | ✅ 完成 |
| 速率限制处理 | ✅ | ⏳ 基础 | Phase 2 |
| 成本追踪 | ✅ | ❌ | Phase 3 |

---

## 6. 集成检查清单

### Phase 1
- [x] 基础 ProviderClient
- [x] Anthropic 支持
- [x] OpenAI 兼容支持
- [x] OAuth 流程
- [x] 流式响应
- [ ] 完整的凭证管理
- [ ] 完整的模型别名

### Phase 2
- [ ] 凭证池 + 轮换
- [ ] 自动降级逻辑
- [ ] 更多提供商（Grok, Ollama 等）
- [ ] 速率限制处理
- [ ] 成本计算

### Phase 3+
- [ ] 多区域部署
- [ ] 负载均衡
- [ ] 提供商管理 UI

---

## 7. 配置示例

### 环境变量
```bash
# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_BASE_URL="https://api.anthropic.com"  # 可选

# OpenAI
export OPENAI_API_KEY="sk-proj-..."
export OPENAI_ORG_ID="org-..."  # 可选

# OpenRouter
export OPENROUTER_API_KEY="sk-or-..."

# Grok/X.AI
export XAI_API_KEY="xai-..."

# 自定义端点
export CUSTOM_API_KEY="..."
export CUSTOM_API_URL="http://localhost:8000/v1"
```

### 配置文件 (~/.claw/config.yaml)
```yaml
default_provider: anthropic
providers:
  anthropic:
    model: claude-3-5-sonnet-20241022
    api_key: $ANTHROPIC_API_KEY
    features:
      extended_thinking: true
      vision: true

  openai:
    model: gpt-4o
    api_key: $OPENAI_API_KEY
    organization: $OPENAI_ORG_ID

  custom:
    base_url: http://localhost:11434/v1  # Ollama
    model: llama2
    api_key: optional

fallback_chain:
  - openrouter
  - openai
  - grok
```

---

## 8. 测试策略

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_auto_detect_providers() {
        let manager = ProviderManager::auto_detect().await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    #[ignore]  // 需要真实 API 密钥
    async fn test_create_message_anthropic() {
        let provider = ProviderClient::new_anthropic(...)?;
        let response = provider.create_message(...).await;
        assert!(response.is_ok());
    }

    #[test]
    fn test_model_routing() {
        let router = ModelRouter::default();
        assert_eq!(router.resolve_model("claude"), "claude-3-5-sonnet-20241022");
    }
}
```

---

## 参考资源

- [Hermes Provider Runtime](https://hermes-agent.nousresearch.com/docs/developer-guide/provider-runtime)
- [Anthropic Documentation](https://docs.anthropic.com/)
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [OpenRouter Documentation](https://openrouter.ai/docs)

---

**下一步**: 阅读 [Tool System](./tool-system.md) 了解工具注册和执行的设计
