# Session Persistence & Conversation History

**版本**: 1.0  
**最后更新**: 2026-04-11  
**对标**: Hermes Session Storage (~600 行)  
**实现语言**: Rust + SQLite  
**关键文件**: `crates/runtime/src/session.rs` + database layer

---

## 1. 系统概览

### 1.1 在 Hermes 中的角色

Hermes Session System 负责：

- 💾 对话历史的持久化存储
- 🔍 高效的会话查询和检索
- 📊 会话元数据管理（创建时间、成本、token 计数）
- 🗂️ 会话分支和压缩（基于向量相似性）
- 🔄 会话恢复和继续
- 📈 统计分析和性能指标

### 1.2 在 Claw Code 中的现状

**现有实现**（`src/runtime/session.rs`）：

```
✅ Session 结构体和序列化
✅ SQLite 存储支持
✅ 消息历史管理
✅ 离线持久化
⏳ 会话压缩基础
⏳ 向量搜索集成
⏳ 成本跟踪
⏳ 性能分析
```

**代码规模**：~800 行（session.rs + 相关模块）

---

## 2. 架构设计

### 2.1 会话层次结构

```
┌─────────────────────────────────┐
│       Session Collection        │  (用户的所有会话)
└────────────────┬────────────────┘
                 │
    ┌────────────┼────────────┐
    ▼            ▼            ▼
┌────────┐  ┌────────┐  ┌────────┐
│Session1│  │Session2│  │Session3│
│(Branch)│  │(Active)│  │(Archive)│
└───┬────┘  └───┬────┘  └────────┘
    │           │
    ▼           ▼
  ┌──────────────────────┐
  │ Conversation History │
  │ (双向链表或数组)      │
  └─────────┬────────────┘
            │
    ┌───────┼───────┐
    ▼       ▼       ▼
┌───────────────────────────────┐
│  Messages (Turn 1, 2, 3, ...) │
│  ├─ Turn 1: User + Assistant  │
│  ├─ Turn 2: User + Assistant  │
│  └─ Turn N: User + Assistant  │
└───────────────────────────────┘
```

### 2.2 数据库模式

#### Sessions 表

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,        -- UUID
    user_id TEXT NOT NULL,      -- 用户标识
    title TEXT,                 -- 会话标题
    status TEXT,                -- ACTIVE, PAUSED, ARCHIVED, DELETED

    -- 时间戳
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    last_accessed_at TIMESTAMP,

    -- 元数据
    model TEXT,                 -- "claude-3-opus", "gpt-4", etc.
    total_turns INTEGER DEFAULT 0,
    total_input_tokens INTEGER DEFAULT 0,
    total_output_tokens INTEGER DEFAULT 0,
    total_cost_usd REAL DEFAULT 0,

    -- 配置
    system_prompt TEXT,
    config JSONB,               -- 运行时配置（JSON）

    -- 关系
    parent_session_id TEXT,     -- 如果是分支，指向父会话

    INDEX idx_user_created (user_id, created_at DESC),
    INDEX idx_status (status),
    INDEX idx_parent (parent_session_id)
);

-- 消息表
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_number INTEGER,        -- 在该会话中的轮次

    -- 内容
    role TEXT,                  -- "user", "assistant", "system"
    content TEXT,               -- 消息内容

    -- 工具调用
    tool_calls JSONB,           -- 本轮调用的工具（JSON）
    tool_results JSONB,         -- 工具执行结果（JSON）

    -- 元数据
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd REAL,

    timestamps: {
        created_at TIMESTAMP,
        completed_at TIMESTAMP,  -- 本轮完成时间
    }

    -- 向量嵌入（用于语义搜索）
    embedding VECTOR(1536),     -- pgvector 格式

    INDEX idx_session (session_id, turn_number),
    INDEX idx_role (session_id, role),
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE CASCADE
);

-- 压缩历史表（用于记录被压缩的消息）
CREATE TABLE compression_history (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    original_turn_start INTEGER,
    original_turn_end INTEGER,
    compressed_turn_number INTEGER,

    original_content TEXT,
    compressed_summary TEXT,
    compression_ratio REAL,     -- 压缩前后比例

    created_at TIMESTAMP,

    FOREIGN KEY (session_id) REFERENCES sessions (id)
);

-- 搜索索引（FTS5 全文搜索）
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    session_id UNINDEXED
);
```

### 2.3 关键类型定义

#### Session 结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    // 身份
    pub id: SessionId,
    pub user_id: String,
    pub title: String,

    // 状态
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,

    // 模型和配置
    pub model: String,
    pub system_prompt: String,
    pub config: RuntimeConfig,

    // 对话历史
    pub messages: Vec<Message>,
    pub current_turn: usize,

    // 成本追踪
    pub cost_tracking: CostTracking,

    // 关系
    pub parent_session_id: Option<SessionId>,

    // 元数据
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Paused,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,

    // 工具相关
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,

    // 计量
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost: f64,

    // 时间
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracking {
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub cost_by_model: HashMap<String, f64>,
    pub total_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub total_turns: usize,
    pub average_tokens_per_turn: usize,
    pub estimated_completion_time: Duration,
    pub tags: Vec<String>,

    // 分析数据
    pub user_messages_count: usize,
    pub assistant_messages_count: usize,
    pub tools_used: HashSet<String>,
    pub models_used: HashSet<String>,
}
```

#### SessionStore 接口

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 创建新会话
    async fn create(&self, session: &Session) -> Result<()>;

    /// 获取会话
    async fn get(&self, session_id: &SessionId) -> Result<Option<Session>>;

    /// 列出用户的所有会话
    async fn list_for_user(
        &self,
        user_id: &str,
        filter: SessionFilter,
    ) -> Result<Vec<SessionSummary>>;

    /// 更新会话
    async fn update(&self, session: &Session) -> Result<()>;

    /// 添加消息到会话
    async fn add_message(
        &self,
        session_id: &SessionId,
        message: &Message,
    ) -> Result<()>;

    /// 删除会话
    async fn delete(&self, session_id: &SessionId) -> Result<()>;

    /// 搜索会话
    async fn search(
        &self,
        user_id: &str,
        query: &str,
    ) -> Result<Vec<SearchResult>>;

    /// 系统维护：压缩历史
    async fn compress_history(
        &self,
        session_id: &SessionId,
        target_turns: usize,
    ) -> Result<CompressionResult>;
}

// SQLite 实现
pub struct SqliteSessionStore {
    pool: SqlitePool,
}

impl SqliteSessionStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Self::migrate(&pool).await?;
        Ok(SqliteSessionStore { pool })
    }

    pub async fn migrate(pool: &SqlitePool) -> Result<()> {
        sqlx::query(SCHEMA_SQL).execute(pool).await?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO sessions
            (id, user_id, title, status, created_at, updated_at, model, system_prompt, config)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            session.id,
            session.user_id,
            session.title,
            session.status.to_string(),
            session.created_at,
            session.updated_at,
            session.model,
            session.system_prompt,
            serde_json::to_string(&session.config)?
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_message(
        &self,
        session_id: &SessionId,
        message: &Message,
    ) -> Result<()> {
        // 插入消息
        sqlx::query!(
            r#"
            INSERT INTO messages
            (id, session_id, turn_number, role, content, tool_calls, tool_results,
             input_tokens, output_tokens, cost_usd, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            message.id,
            session_id,
            message.turn_number,
            message.role.to_string(),
            message.content,
            serde_json::to_string(&message.tool_calls)?,
            serde_json::to_string(&message.tool_results)?,
            message.input_tokens,
            message.output_tokens,
            message.cost,
            message.created_at
        )
        .execute(&self.pool)
        .await?;

        // 更新会话元数据
        sqlx::query!(
            r#"
            UPDATE sessions
            SET updated_at = ?, total_turns = total_turns + 1,
                total_input_tokens = total_input_tokens + ?,
                total_output_tokens = total_output_tokens + ?,
                total_cost_usd = total_cost_usd + ?
            WHERE id = ?
            "#,
            Utc::now(),
            message.input_tokens,
            message.output_tokens,
            message.cost,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_for_user(
        &self,
        user_id: &str,
        filter: SessionFilter,
    ) -> Result<Vec<SessionSummary>> {
        let rows = sqlx::query_as::<_, (String, String, String, i64, f64)>(
            r#"
            SELECT id, title, status, total_turns, total_cost_usd
            FROM sessions
            WHERE user_id = ?
            AND status NOT IN ('DELETED')
            ORDER BY updated_at DESC
            LIMIT ?
            "#
        )
        .bind(user_id)
        .bind(filter.limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(id, title, status, turns, cost)| {
            SessionSummary {
                id,
                title,
                status,
                turns: turns as usize,
                cost_usd: cost,
            }
        }).collect())
    }
}
```

---

## 3. 会话生命周期

### 3.1 创建会话

```rust
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    config: SessionConfig,
}

impl SessionManager {
    /// 创建新会话
    pub async fn create_session(
        &self,
        user_id: &str,
        model: &str,
        system_prompt: String,
    ) -> Result<Session> {
        let session = Session {
            id: SessionId::new_v4(),
            user_id: user_id.to_string(),
            title: format!("Chat {}", chrono::Local::now().format("%Y-%m-%d %H:%M")),
            status: SessionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_accessed_at: Utc::now(),
            model: model.to_string(),
            system_prompt,
            ..Default::default()
        };

        self.store.create(&session).await?;
        Ok(session)
    }

    /// 从现有会话继续（分支）
    pub async fn branch_session(
        &self,
        base_session_id: &SessionId,
        branch_point: Option<usize>,
    ) -> Result<Session> {
        // 获取基础会话
        let mut base = self.store.get(base_session_id).await?
            .ok_or(SessionError::NotFound)?;

        // 如果指定了分支点，截断历史
        if let Some(point) = branch_point {
            base.messages.truncate(point);
        }

        // 创建新会话
        let mut new_session = base.clone();
        new_session.id = SessionId::new_v4();
        new_session.parent_session_id = Some(base_session_id.clone());
        new_session.created_at = Utc::now();
        new_session.messages.clear();  // 可选：清空历史或保留

        self.store.create(&new_session).await?;
        Ok(new_session)
    }

    /// 恢复会话
    pub async fn restore_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Session> {
        let mut session = self.store.get(session_id).await?
            .ok_or(SessionError::NotFound)?;

        session.last_accessed_at = Utc::now();
        self.store.update(&session).await?;

        Ok(session)
    }
}
```

### 3.2 添加消息和成本追踪

```rust
impl SessionManager {
    /// 记录用户消息
    pub async fn add_user_message(
        &self,
        session: &mut Session,
        content: String,
    ) -> Result<()> {
        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            tool_calls: None,
            tool_results: None,
            input_tokens: 0,  // 用户消息通常不计费
            output_tokens: 0,
            cost: 0.0,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };

        session.messages.push(message.clone());
        session.current_turn += 1;
        session.updated_at = Utc::now();

        self.store.add_message(&session.id, &message).await?;
        Ok(())
    }

    /// 记录助手消息（包含工具调用和结果）
    pub async fn add_assistant_message(
        &self,
        session: &mut Session,
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
        input_tokens: usize,
        output_tokens: usize,
        model: &str,
    ) -> Result<()> {
        let cost = Self::calculate_cost(model, input_tokens, output_tokens);

        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            tool_calls,
            tool_results: None,
            input_tokens,
            output_tokens,
            cost,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };

        // 更新成本跟踪
        session.cost_tracking.total_input_tokens += input_tokens;
        session.cost_tracking.total_output_tokens += output_tokens;
        *session.cost_tracking.cost_by_model.entry(model.to_string()).or_insert(0.0) += cost;
        session.cost_tracking.total_cost_usd += cost;

        session.messages.push(message.clone());
        session.updated_at = Utc::now();

        self.store.add_message(&session.id, &message).await?;
        Ok(())
    }

    /// 计算成本（基于 token 数）
    fn calculate_cost(model: &str, input_tokens: usize, output_tokens: usize) -> f64 {
        match model {
            "claude-3-opus" => {
                (input_tokens as f64 * 0.015 / 1000.0) +  // $15 per 1M input tokens
                (output_tokens as f64 * 0.075 / 1000.0)   // $75 per 1M output tokens
            }
            "gpt-4" => {
                (input_tokens as f64 * 0.03 / 1000.0) +
                (output_tokens as f64 * 0.06 / 1000.0)
            }
            _ => 0.0,
        }
    }
}
```

---

## 4. 会话搜索和检索

### 4.1 全文搜索

```rust
impl SqliteSessionStore {
    pub async fn search(
        &self,
        user_id: &str,
        query: &str,
    ) -> Result<Vec<SearchResult>> {
        // 使用 FTS5 全文搜索
        let rows = sqlx::query!(
            r#"
            SELECT m.id, m.session_id, m.content, s.title
            FROM messages_fts fts
            JOIN messages m ON fts.rowid = m.rowid
            JOIN sessions s ON m.session_id = s.id
            WHERE fts.content MATCH ?
            AND s.user_id = ?
            ORDER BY rank
            LIMIT 50
            "#,
            query,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| SearchResult {
            message_id: row.id,
            session_id: row.session_id,
            content: row.content,
            session_title: row.title,
        }).collect())
    }

    // 向量搜索（使用向量数据库或 pgvector）
    pub async fn semantic_search(
        &self,
        user_id: &str,
        query_embedding: &[f32; 1536],
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        // 计算相似性并返回最相关的 k 个结果
        // 使用 cosine similarity 或 HNSW 索引
        // (伪代码，实际需要向量数据库支持)

        unimplemented!("Vector search requires pgvector or separate vector DB")
    }
}
```

---

## 5. 会话压缩

```rust
pub struct CompressionConfig {
    /// 触发压缩的消息数阈值
    pub compression_trigger: usize,
    /// 压缩后保留的上下文轮数
    pub keep_recent_turns: usize,
    /// 使用的压缩模型
    pub compression_model: String,
}

impl SessionManager {
    /// 压缩会话历史
    pub async fn compress_session(
        &self,
        session_id: &SessionId,
        config: &CompressionConfig,
    ) -> Result<CompressionResult> {
        let mut session = self.store.get(session_id).await?
            .ok_or(SessionError::NotFound)?;

        let total_messages = session.messages.len();
        if total_messages < config.compression_trigger {
            return Ok(CompressionResult {
                compressed: false,
                reason: "Message count below threshold".to_string(),
            });
        }

        // 确定要压缩的范围
        let compression_start = 0;
        let compression_end = total_messages.saturating_sub(config.keep_recent_turns);

        // 收集要压缩的消息
        let messages_to_compress: Vec<_> = session.messages
            .iter()
            .take(compression_end)
            .collect();

        // 使用 LLM 生成摘要
        let summary = self.generate_summary(&messages_to_compress).await?;

        // 记录压缩历史
        sqlx::query!(
            r#"
            INSERT INTO compression_history
            (id, session_id, original_turn_start, original_turn_end,
             compressed_turn_number, original_content, compressed_summary, compression_ratio)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            uuid::Uuid::new_v4().to_string(),
            session_id,
            compression_start,
            compression_end,
            0,  // 新的压缩消息变为第一条
            format!("{:?}", messages_to_compress),
            summary,
            messages_to_compress.len() as f64
        )
        .execute(&self.pool.clone())
        .await?;

        Ok(CompressionResult {
            compressed: true,
            reason: format!("Compressed {} messages", compression_end),
            original_size: compression_end,
            compressed_size: 1,
        })
    }

    async fn generate_summary(&self, messages: &[&Message]) -> Result<String> {
        // 使用 LLM 生成摘要
        let content = messages.iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        // 调用 API 生成摘要
        let summary = self.api_client
            .summarize(&content, "concise")
            .await?;

        Ok(summary)
    }
}
```

---

## 6. 序列化和导出

### 6.1 JSON 导出

```rust
impl Session {
    pub fn to_json(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self)
    }

    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# {}\n\n", self.title));
        output.push_str(&format!("**Created**: {}\n", self.created_at.format("%Y-%m-%d %H:%M")));
        output.push_str(&format!("**Model**: {}\n", self.model));
        output.push_str(&format!("**Cost**: ${:.4}\n\n", self.cost_tracking.total_cost_usd));

        for (turn, message) in self.messages.iter().enumerate() {
            output.push_str(&format!("## Turn {}: {}\n\n", turn + 1, message.role));
            output.push_str(&format!("{}\n\n", message.content));

            if let Some(calls) = &message.tool_calls {
                output.push_str("**Tools Used**:\n");
                for call in calls {
                    output.push_str(&format!("- {}\n", call.function.name));
                }
                output.push_str("\n");
            }
        }

        output
    }
}
```

---

## 7. 与 Hermes 的对齐

| 功能       | Hermes               | If2Ai 现状      | 计划         |
| ---------- | -------------------- | --------------- | ------------ |
| 会话存储   | ✅ SQLite/PostgreSQL | ✅ SQLite       | ✅ 完成      |
| 消息历史   | ✅ 双链/数组         | ✅ Vec<Message> | ✅ 完成      |
| 成本追踪   | ✅ 模型级别          | ✅ 70%          | Phase 2 完善 |
| 会话元数据 | ✅                   | ✅ 80%          | Phase 2 完善 |
| 会话压缩   | ✅ LLM-based         | ⏳ 基础         | Phase 2      |
| 向量搜索   | ✅                   | ❌              | Phase 2-3    |
| 全文搜索   | ✅ FTS5              | ✅ 70%          | Phase 2      |
| 会话分支   | ✅                   | ⏳ 基础         | Phase 2      |
| 导出/备份  | ✅                   | ✅ 80%          | Phase 1 完善 |

---

## 8. 性能考虑

### 8.1 查询优化

```rust
// 创建丰富索引
CREATE INDEX idx_session_user ON sessions(user_id, updated_at DESC);
CREATE INDEX idx_message_session ON messages(session_id, turn_number);
CREATE INDEX idx_message_timestamp ON messages(created_at DESC);
CREATE INDEX idx_cost_tracking ON sessions(user_id, total_cost_usd DESC);

// 使用连接池
pub struct SqliteSessionStore {
    pool: SqlitePool,  // 支持并发连接
}
```

### 8.2 数据库连接管理

```rust
pub async fn create_pool(database_url: &str, pool_size: u32) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(database_url)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);  // Write-Ahead Logging

    let pool = SqlitePoolOptions::new()
        .max_connections(pool_size)
        .connect_with(options)
        .await?;

    Ok(pool)
}
```

---

## 9. 集成检查清单

### Phase 1

- [x] Session 结构和序列化
- [x] 基础 SQLite 存储
- [x] 消息历史管理
- [ ] 完整的成本追踪

### Phase 2

- [ ] 会话压缩（使用 LLM）
- [ ] 向量搜索集成
- [ ] 会话分支完整实现
- [ ] 性能优化（查询缓存）

### Phase 3+

- [ ] PostgreSQL 支持
- [ ] 分布式会话存储
- [ ] 会话数据湖
- [ ] 实时同步

---

## 参考资源

- [Hermes Session Storage](https://hermes-agent.nousresearch.com/docs/architecture/session-storage)
- [SQLite Documentation](https://www.sqlite.org/docs.html)
- [sqlx - SQL Database Toolkit](https://github.com/launchbadge/sqlx)
- [PostgreSQL Full-Text Search](https://www.postgresql.org/docs/current/textsearch.html)

---

**下一步**: 阅读 [Prompt Builder](./prompt-builder.md) 了解系统提示和提示工程的设计
