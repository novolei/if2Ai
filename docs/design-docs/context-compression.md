# Context Compression & Memory 设计文档

> Context Compression 是处理长对话的关键机制。这个文档基于 hermes-agent 的自适应压缩算法，提供完整的内存管理、压缩触发和递增更新设计。

## 核心概念

Agent 需要在有限的 LLM 上下文窗口内处理无限长的对话：

```
对话阶段 1: {msg1, msg2, msg3, msg4, msg5}  (50 tokens)
     ↓
耗尽 50% 阈值？ → 无，继续

对话阶段 2: {msg6, msg7, msg8, msg9, msg10} (100 tokens 总计)
     ↓
耗尽 50% 阈值？ → 是 → 压缩！

压缩步骤:
1. 修剪: 移除琐碎消息 → 80 tokens
2. 保护: 保留头部（用户意图）和尾部（最近上下文）
3. 总结: 中间部分转为固定大小的摘要 → 40 tokens

结果: {msg1-3（保护）, SUMMARY（摘要）, msg8-10（保护）} = 40 tokens → 恢复空间
```

## 系统架构

```
LLM Response
    ↓
┌──────────────────────┐
│ Token Counter        │ → 计算消耗 tokens
└────────┬─────────────┘
         ↓
    预登用 > 50% 阈值？
         │ No      │ Yes
         ↓         ↓
      继续  ┌─────────────────┐
            │ Compression     │
            │ Pipeline        │
            └────────┬────────┘
                     ↓
         ┌─────────────────────────┐
         │ 1. Prune                │ → 移除不重要消息
         └────────┬────────────────┘
                  ↓
         ┌─────────────────────────┐
         │ 2. Protect Head & Tail  │ →  保留关键部分
         └────────┬────────────────┘
                  ↓
         ┌─────────────────────────┐
         │ 3. Summarize Middle     │ → 生成总结
         └────────┬────────────────┘
                  ↓
         新消息列表（压缩） → 继续对话
```

## 核心组件

### 1. Context Window Manager

```rust
pub struct ContextWindowManager {
    pub window_size: u32,                       // 模型的 max tokens
    pub compression_threshold: f32,             // 默认 0.5 (50%)
    pub reserved_for_response: u32,             // 保留 response tokens
    pub protected_head_size: u32,               // 保留头部大小
    pub protected_tail_size: u32,               // 保留尾部大小
}

impl ContextWindowManager {
    pub fn available_tokens(&self) -> u32 {
        self.window_size - self.reserved_for_response
    }

    pub fn utilization_ratio(&self, current_tokens: u32) -> f32 {
        current_tokens as f32 / self.available_tokens() as f32
    }

    pub fn should_compress(&self, current_tokens: u32) -> bool {
        self.utilization_ratio(current_tokens) > self.compression_threshold
    }

    pub fn compression_needed_tokens(&self, current_tokens: u32) -> u32 {
        // 计算需要释放多少 tokens 来恢复到 30% 利用率
        let target = (self.available_tokens() as f32 * 0.3) as u32;
        if current_tokens > target {
            current_tokens - target
        } else {
            0
        }
    }
}

pub fn estimate_tokens(text: &str) -> u32 {
    // 简单估算: 1 token ≈ 4 个字符 (OpenAI 标准)
    (text.len() as f32 / 4.0).ceil() as u32
}
```

### 2. Compression Pipeline

```rust
pub struct CompressionPipeline {
    window_manager: ContextWindowManager,
    pruner: MessagePruner,
    protector: ProtectionRange,
    summarizer: ContextSummarizer,
}

impl CompressionPipeline {
    pub async fn compress(
        &self,
        messages: &mut Vec<Message>,
        tokens_to_free: u32,
    ) -> Result<CompressionStats> {
        let mut stats = CompressionStats::default();

        // Phase 1: Prune 不重要的消息
        let (pruned_messages, pruned_tokens) = self.pruner.prune(messages);
        stats.tokens_freed_by_pruning = pruned_tokens;

        if pruned_tokens >= tokens_to_free {
            *messages = pruned_messages;
            return Ok(stats);
        }

        // Phase 2: 标记保护范围
        let (head_range, tail_range) = self.protector.identify_ranges(
            &pruned_messages,
            self.window_manager.protected_head_size,
            self.window_manager.protected_tail_size,
        );

        // Phase 3: 确定要总结的范围
        let middle_range = head_range.end..tail_range.start;
        if middle_range.is_empty() {
            *messages = pruned_messages;
            return Ok(stats);
        }

        // Phase 4: 生成总结
        let middle_messages = &pruned_messages[middle_range.clone()];
        let summary = self.summarizer.summarize(middle_messages).await?;
        stats.summary = summary.clone();
        stats.messages_summarized = middle_messages.len();

        // Phase 5: 重建消息列表
        let mut compressed = Vec::new();
        compressed.extend_from_slice(&pruned_messages[..head_range.end]);
        compressed.push(Message::summary(summary));
        compressed.extend_from_slice(&pruned_messages[tail_range.start..]);

        *messages = compressed;
        Ok(stats)
    }
}
```

### 3. Message Pruning（修剪）

```rust
pub struct MessagePruner {
    // 配置评分函数
    importance_threshold: f32,
}

pub enum MessageImportance {
    Critical,           // 必须保留（用户初始请求、关键决策）
    High,              // 应该保留（工具调用结果）
    Medium,            // 可能需要保留（中间对话）
    Low,               // 可以移除（确认消息、重复的工具调用）
}

impl MessagePruner {
    pub fn prune(&self, messages: &[Message]) -> (Vec<Message>, u32) {
        let mut pruned = Vec::new();
        let mut tokens_freed = 0;

        for msg in messages {
            let importance = self.score_importance(msg);

            match importance {
                MessageImportance::Critical => {
                    pruned.push(msg.clone());
                }
                MessageImportance::High => {
                    pruned.push(msg.clone());
                }
                MessageImportance::Medium => {
                    // 可能修剪
                    if should_keep(msg) {
                        pruned.push(msg.clone());
                    } else {
                        tokens_freed += estimate_tokens(&msg.content);
                    }
                }
                MessageImportance::Low => {
                    // 通常修剪
                    tokens_freed += estimate_tokens(&msg.content);
                }
            }
        }

        (pruned, tokens_freed)
    }

    fn score_importance(&self, msg: &Message) -> MessageImportance {
        match msg.role {
            MessageRole::System => MessageImportance::Critical,
            MessageRole::User => {
                // 第一条用户消息 = 关键
                // 后续用户输入 = 高（可能是转向）
                MessageImportance::High
            }
            MessageRole::Assistant => {
                if msg.content.contains("I'll help") || msg.content.contains("Let me") {
                    MessageImportance::Medium  // 可以删除重复的前置
                } else {
                    MessageImportance::High    // 推理内容很重要
                }
            }
            MessageRole::Tool => {
                if msg.content.len() > 500 {
                    // 长工具输出 → 低（可能可以摘要）
                    MessageImportance::Low
                } else {
                    MessageImportance::High    // 短响应要保留
                }
            }
        }
    }
}
```

### 4. Context Summarizer（总结器）

```rust
pub struct ContextSummarizer {
    llm_provider: Arc<dyn LLMClient>,
}

impl ContextSummarizer {
    pub async fn summarize(&self, middle_messages: &[Message]) -> Result<String> {
        // 使用 LLM 来总结中间部分
        let context_text = middle_messages
            .iter()
            .map(|msg| format!("{}: {}", msg.role, msg.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let summary_prompt = format!(
            "Summarize the following conversation in 2-3 sentences, \
             highlighting any decisions, findings, or important context:\n\n{}",
            context_text
        );

        let response = self.llm_provider.complete(
            &[Message::user(summary_prompt)],
            &[],
            false,  // 不流式
        ).await?;

        Ok(response.content)
    }
}

pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tokens: Option<u32>,
    pub is_summary: bool,  // 标记这是一个生成的总结
}

impl Message {
    pub fn summary(content: String) -> Self {
        Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,  // 总摘要扮演 assistant
            content: format!("[CONTEXT_SUMMARY]\n{}", content),
            timestamp: Utc::now(),
            tokens: None,
            is_summary: true,
        }
    }
}
```

## 双层内存系统

```rust
pub struct MemorySystem {
    // 层级 1: 会话内存（紧凑）
    pub session_context: ContextWindowManager,

    // 层级 2: 持久化外部内存（可选，扩展容量）
    pub persistent_memory: Option<Arc<dyn MemoryStore>>,
}

pub trait MemoryStore: Send + Sync {
    // 写入记忆
    async fn store(
        &self,
        session_id: &str,
        key: &str,
        value: &str,
    ) -> Result<()>;

    // 检索记忆
    async fn retrieve(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>>;  // (content, similarity_score)

    // 删除过期记忆
    async fn cleanup_old(&self, older_than_days: u32) -> Result<()>;
}

// 实现: 向量数据库
pub struct VectorMemoryStore {
    db: Arc<VectorDB>,  // Weaviate, Pinecone, etc.
    embedding_model: String,
}

impl MemoryStore for VectorMemoryStore {
    async fn retrieve(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        let query_embedding = self.embedding_model.embed(query).await?;

        self.db.search(
            session_id,
            &query_embedding,
            limit,
        ).await
    }
}
```

## 压缩指标和监测

```rust
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub triggered_at_tokens: u32,
    pub triggered_at_iteration: u32,
    pub tokens_freed_by_pruning: u32,
    pub messages_summarized: usize,
    pub summary: String,
    pub tokens_freed_by_summarization: u32,
    pub total_tokens_freed: u32,
    pub compression_duration_ms: u32,
    pub result_tokens: u32,
}

pub struct CompressionHistory {
    pub compressions: Vec<CompressionStats>,
}

impl CompressionHistory {
    pub fn total_compressions(&self) -> usize {
        self.compressions.len()
    }

    pub fn average_compression_ratio(&self) -> f32 {
        if self.compressions.is_empty() {
            return 1.0;
        }

        let total_freed: u32 = self.compressions.iter()
            .map(|s| s.total_tokens_freed)
            .sum();

        let total_before: u32 = self.compressions.iter()
            .map(|s| s.triggered_at_tokens)
            .sum();

        1.0 - (total_freed as f32 / total_before as f32)
    }

    pub fn effective_capacity(&self) -> u32 {
        // 考虑压缩效率下的有效上下文容量
        let base = 4096;  // 典型的上下文窗口
        let ratio = self.average_compression_ratio();
        (base as f32 / ratio).ceil() as u32
    }
}
```

## 配置示例

```yaml
context_compression:
  window_size: 4096
  reserved_for_response: 512
  compression_threshold: 0.5 # 50% 时触发
  protected_head_size: 512 # 保留前 512 tokens
  protected_tail_size: 512 # 保留后 512 tokens
  pruning_enabled: true
  summarization_enabled: true
  summary_max_tokens: 200

persistent_memory:
  enabled: true
  store: weaviate # or: pinecone, milvus
  embedding_model: sentence-transformers/all-mpnet-base-v2
  similarity_threshold: 0.7
```

## Harness 集成

### Context Compression 评估

```yaml
test_case:
  name: 'Long Conversation Compression'
  scenario: '100+ 条消息的对话，验证压缩有效但不丢失关键信息'
  steps: 1. 生成 100 条消息的对话历史（~8000 tokens）
    2. 触发 compression pipeline
    3. 验证压缩后 tokens < 50%
    4. 验证关键信息（user intent）被保留
    5. 对压缩后的消息继续 Agent 推理
    6. 验证输出质量无显著下降

  evaluators:
    - name: correctness
      config:
        similarity_to_original: '> 0.85' # 输出与原始对话的相似度
    - name: behavior
      config:
        compressions_triggered: '>= 1'
        tokens_freed: '> 4000'
    - name: performance
      config:
        total_token_usage: '< 50% of uncompressed'
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [docs/references/hermes-agent-analysis.md](../../references/hermes-agent-analysis.md) - Context Management
