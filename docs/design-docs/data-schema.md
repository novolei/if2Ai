# Data Schema & Database 设计文档

> Agent 系统需要持久化存储会话、消息、工具调用历史和性能指标。这个文档设计完整的数据模型和存储架构。

## 核心数据模型

```
用户
 ├─ Session (会话)
 │   ├─ Message (消息)
 │   │   ├─ ToolCall (工具调用)
 │   │   └─ ToolResult (工具结果)
 │   ├─ CompressionEvent (压缩事件)
 │   ├─ ExecutionMetrics (执行指标)
 │   └─ SessionCheckpoint (检查点)
 │
 ├─ Tool (工具)
 │   ├─ ToolDefinition
 │   ├─ ToolUsageStatistics
 │   └─ ToolError (错误日志)
 │
 └─ Memory (记忆库)
     ├─ Fact (事实)
     ├─ Preference (偏好)
     └─ Context (上下文)
```

## 数据库设计

### 1. Messages 表

```sql
CREATE TABLE messages (
    id VARCHAR(36) PRIMARY KEY,              -- UUID
    session_id VARCHAR(36) NOT NULL,         -- FK -> sessions
    role VARCHAR(20) NOT NULL,               -- 'user' | 'assistant' | 'tool' | 'system'
    content LONGTEXT NOT NULL,               -- 消息内容
    is_summary BOOLEAN DEFAULT FALSE,        -- 是否是压缩总结
    tokens INT,                              -- 估计的 token 数
    embedding_id VARCHAR(36),                -- FK -> embeddings
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP,
    
    INDEX idx_session_id (session_id),
    INDEX idx_role_created (role, created_at)
);
```

### 2. Sessions 表

```sql
CREATE TABLE sessions (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,            -- FK -> users
    title VARCHAR(255),
    description TEXT,
    
    -- 执行状态
    status VARCHAR(20),                      -- 'active' | 'completed' | 'error'
    state_json LONGTEXT,                     -- 完整的运行时状态（JSON）
    
    -- 预算
    total_iterations INT,
    total_tokens INT,
    budget_limit_iterations INT,
    budget_limit_tokens INT,
    
    -- 性能
    duration_seconds FLOAT,
    first_response_ms INT,
    tool_calls_count INT,
    compression_count INT,
    errors_count INT,
    
    -- LLM 提供商
    llm_primary_provider VARCHAR(50),
    llm_primary_model VARCHAR(100),
    llm_fallbacks_used TEXT,                 -- JSON array
    
    -- 元数据
    tags JSON,
    config_snapshot JSON,
    
    created_at TIMESTAMP DEFAULT NOW(),
    started_at TIMESTAMP,
    ended_at TIMESTAMP,
    
    INDEX idx_user_id (user_id),
    INDEX idx_created_at (created_at),
    INDEX idx_status (status)
);
```

### 3. Tool Calls 表

```sql
CREATE TABLE tool_calls (
    id VARCHAR(36) PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    message_id VARCHAR(36) NOT NULL,         -- FK -> messages
    
    -- 工具信息
    tool_name VARCHAR(100) NOT NULL,
    tool_params JSON NOT NULL,                -- 传入的参数
    
    -- 执行结果
    status VARCHAR(20),                      -- 'pending' | 'success' | 'error'
    result_text LONGTEXT,
    error_message TEXT,
    
    -- 性能
    execution_time_ms INT,
    input_tokens INT,
    output_tokens INT,
    
    created_at TIMESTAMP DEFAULT NOW(),
    completed_at TIMESTAMP,
    
    INDEX idx_session_id (session_id),
    INDEX idx_tool_name (tool_name),
    INDEX idx_status (status)
);
```

### 4. Compression Events 表

```sql
CREATE TABLE compression_events (
    id VARCHAR(36) PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    
    -- 压缩触发条件
    triggered_at_tokens INT,
    triggered_at_iteration INT,
    trigger_reason VARCHAR(100),             -- 'threshold_exceeded', 'manual'
    
    -- 压缩结果
    tokens_freed INT,
    messages_summarized INT,
    summary_text LONGTEXT,
    
    -- 性能
    compression_time_ms INT,
    
    created_at TIMESTAMP DEFAULT NOW(),
    
    INDEX idx_session_id (session_id),
    INDEX idx_created_at (created_at)
);
```

### 5. Execution Metrics 表

```sql
CREATE TABLE execution_metrics (
    id VARCHAR(36) PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    iteration_num INT,
    
    -- LLM 调用
    llm_response_time_ms INT,
    llm_tokens_generated INT,
    llm_provider VARCHAR(50),
    
    -- 工具执行
    tools_called TEXT,                       -- JSON array
    total_tool_time_ms INT,
    
    -- 上下文
    context_tokens_used INT,
    compression_triggered BOOLEAN,
    
    -- 错误
    error_count INT,
    error_types TEXT,                        -- JSON array
    
    created_at TIMESTAMP DEFAULT NOW(),
    
    INDEX idx_session_id (session_id),
    INDEX idx_iteration (iteration_num)
);
```

## Rust 类型定义

```rust
use sqlx::{FromRow, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    
    pub status: String,                    // "active", "completed", "error"
    pub state_json: Option<String>,        // serde_json::Value
    
    pub total_iterations: i32,
    pub total_tokens: i32,
    pub budget_limit_iterations: i32,
    pub budget_limit_tokens: i32,
    
    pub duration_seconds: Option<f32>,
    pub first_response_ms: Option<i32>,
    pub tool_calls_count: i32,
    pub compression_count: i32,
    pub errors_count: i32,
    
    pub llm_primary_provider: String,
    pub llm_primary_model: String,
    pub llm_fallbacks_used: Option<String>, // JSON
    
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,                      // "user", "assistant", "tool", "system"
    pub content: String,
    pub is_summary: bool,
    pub tokens: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ToolCall {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    
    pub tool_name: String,
    pub tool_params: String,               // JSON
    
    pub status: String,                    // "pending", "success", "error"
    pub result_text: Option<String>,
    pub error_message: Option<String>,
    
    pub execution_time_ms: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CompressionEvent {
    pub id: String,
    pub session_id: String,
    
    pub triggered_at_tokens: i32,
    pub triggered_at_iteration: i32,
    pub trigger_reason: String,
    
    pub tokens_freed: i32,
    pub messages_summarized: i32,
    pub summary_text: Option<String>,
    
    pub compression_time_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}
```

## 数据访问层

```rust
use sqlx::{PgPool, Row};

pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    // 创建会话
    pub async fn create_session(
        &self,
        user_id: &str,
        title: &str,
    ) -> Result<Session> {
        let id = uuid::Uuid::new_v4().to_string();
        
        sqlx::query_as::<_, Session>(
            r#"INSERT INTO sessions (id, user_id, title, status)
               VALUES ($1, $2, $3, 'active')
               RETURNING *"#
        )
        .bind(&id)
        .bind(user_id)
        .bind(title)
        .fetch_one(&self.pool)
        .await
    }
    
    // 保存消息
    pub async fn save_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        tokens: Option<i32>,
    ) -> Result<Message> {
        let id = uuid::Uuid::new_v4().to_string();
        
        sqlx::query_as::<_, Message>(
            r#"INSERT INTO messages (id, session_id, role, content, tokens)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING *"#
        )
        .bind(&id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(tokens)
        .fetch_one(&self.pool)
        .await
    }
    
    // 批量保存工具调用
    pub async fn save_tool_calls(
        &self,
        session_id: &str,
        message_id: &str,
        calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolCall>> {
        let mut tx = self.pool.begin().await?;
        
        for mut call in calls {
            call.id = uuid::Uuid::new_v4().to_string();
            call.session_id = session_id.to_string();
            call.message_id = message_id.to_string();
            
            sqlx::query(
                r#"INSERT INTO tool_calls (id, session_id, message_id, tool_name, tool_params, status)
                   VALUES ($1, $2, $3, $4, $5, 'pending')"#
            )
            .bind(&call.id)
            .bind(session_id)
            .bind(message_id)
            .bind(&call.tool_name)
            .bind(&call.tool_params)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(calls)
    }
    
    // 查询会话历史
    pub async fn get_session_history(
        &self,
        session_id: &str,
    ) -> Result<Vec<Message>> {
        sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE session_id = $1 ORDER BY created_at ASC"
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
    }
    
    // 统计和分析
    pub async fn get_session_stats(
        &self,
        session_id: &str,
    ) -> Result<SessionStats> {
        let row = sqlx::query(
            r#"SELECT
                COUNT(*) as total_messages,
                SUM(CASE WHEN role = 'tool' THEN 1 ELSE 0 END) as tool_messages,
                SUM(tokens) as total_tokens,
                COUNT(DISTINCT tool_name) as unique_tools
               FROM messages
               WHERE session_id = $1"#
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(SessionStats {
            total_messages: row.get(0),
            tool_messages: row.get(1),
            total_tokens: row.get(2),
            unique_tools: row.get(3),
        })
    }
}

pub struct SessionStats {
    pub total_messages: i64,
    pub tool_messages: i64,
    pub total_tokens: Option<i64>,
    pub unique_tools: i64,
}
```

## 存储选项

| 存储 | 用途 | 访问模式 |
|------|------|--------|
| PostgreSQL | 主要数据库（会话、消息、工具调用） | OLTP（频繁读写） |
| Redis | 缓存和会话状态 | 热数据、速率限制 |
| Elasticsearch | 消息和工具调用搜索 | 全文搜索 |
| S3/Minio | 长期归档和日志 | 批量写入 |
| Weaviate | 嵌入和语义搜索 | 向量相似性查询 |

## 迁移策略

```rust
use sqlx::migrate::MigrateDatabase;
use sqlx::Sqlite;

pub async fn run_migrations(database_url: &str) -> Result<()> {
    if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
        Sqlite::create_database(database_url).await?;
    }
    
    let pool = SqlitePool::connect(database_url).await?;
    
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;
    
    Ok(())
}
```

migration 文件结构：
```
migrations/
├── 001_initial_schema.sql
├── 002_add_compression_table.sql
├── 003_add_indexes.sql
└── 004_add_embeddings.sql
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: ARCHITECTURE.md, docs/design-docs/agent-orchestrator.md
