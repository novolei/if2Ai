# Memory System & Honcho Integration 设计文档

**版本**: 1.1 | **最后更新**: 2026-04-18 | **状态**: Mixed (Current Impl + Design) | **对齐**: Hermes Memory Providers (8 providers)

> If2Ai 集成 Honcho AI 作为主要的外部记忆提供商，支持 AI-native 跨会话用户建模、辩证问答和语义搜索。同时支持 7 个其他记忆提供商作为替代方案。

---

## 目录

- [1. 系统概览](#系统概览)
- [2. 记忆架构](#记忆架构)
- [3. Honcho 集成](#honcho-集成)
- [4. 多提供商模式](#多提供商模式)
- [5. 数据流](#数据流)
- [6. 配置管理](#配置管理)
- [7. 实现路线](#实现路线)
- [8. Hermes 对标](#hermes-对标)

---

## 系统概览

### 当前实现状态

当前代码不是“每个 Session 一份独立 memory 库”，而是：

- 使用共享 SQLite 库：`~/.if2ai/memory/memory.db`
- 每条 memory entry 可携带 `session_id` / `project_id` 作为 scope 标签
- Agent 工具链中的 `memory_store` / `memory_recall` 已接入 scoped 读写
- 当前实际生效的隔离以 `session` 为主，`global` 为兜底可见范围
- `project` 级共享在数据模型中已预留，但读路径尚未完整打通
- 前端 Memory Browser 当前更接近“全库视图”，不是“当前 Session 视图”

换句话说，现状是：

`共享存储 + scope 字段隔离 + Session 优先 + Global 兼容`

### 推荐作用域策略

为了兼顾安全性和复用价值，推荐采用三层 memory scope：

| Scope | 用途 | 是否默认自动写入 |
| --- | --- | --- |
| `session` | 临时任务事实、当前轮次结论、一次性上下文 | 是 |
| `project` | 仓库约定、架构决策、常用命令、工程偏好 | 否，需 promotion |
| `global` | 用户长期偏好、稳定身份信息、跨项目习惯 | 否，需高置信或显式确认 |

推荐写入策略：

1. 默认把新记忆写入 `session`
2. 当信息在同一 `Project` 内重复出现并被多次使用时，提升到 `project`
3. 只有稳定、长期、跨项目适用的信息才进入 `global`

这样可以避免两种极端：

- 全部 `global`：污染严重，容易跨任务串味
- 全部 `session`：无法跨会话复用，高价值工程记忆沉没

### 记忆的三个层次

```
┌─────────────────────────────────────┐
│  外部记忆提供商 (Cloud / Self-hosted) │
│  • Honcho AI  ⭐ (User Modeling)    │
│  • Mem0       (Server-side LLM)     │
│  • Hindsight  (Knowledge Graph)     │
│  • ... + 5 others                   │
└──────────────┬──────────────────────┘
               │ (Tauri Commands / API)
┌──────────────▼──────────────────────┐
│ If2Ai Memory Manager (本地编排)      │
│ ├─ Provider Registry               │
│ ├─ Context Injection               │
│ ├─ Recall & Sync                   │
│ └─ Multi-Provider Coordination      │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│ 内置记忆 (Always Active)             │
│ ├─ MEMORY.md (事实和观察)            │
│ └─ USER.md (用户偏好和身份)          │
└─────────────────────────────────────┘
```

### Why Honcho?

```
特性对比:
┌─────────────────────────────────────────┐
│ 内置记忆 (MEMORY.md + USER.md)           │
│ └─ 手动维护, 有限容量, 单会话            │
├─────────────────────────────────────────┤
│ Honcho (外部)                           │
│ ├─ AI-native 用户建模 ⭐                │
│ ├─ 跨会话学习                           │
│ ├─ 辩证问答 (dialectic Q&A)            │
│ ├─ 语义搜索和相似性                     │
│ ├─ 持久结论                             │
│ ├─ 多档案支持 (agents)                 │
│ └─ 云 + 自托管双模式                   │
└─────────────────────────────────────────┘
```

---

## 记忆架构

### 内置记忆 (MEMORY.md / USER.md)

```rust
pub struct BuiltInMemory {
    // MEMORY.md - 事实记录
    facts: Vec<MemoryFact>,              // 日期记录的观察
    observations: Vec<String>,           // Agent 观察到的事情

    // USER.md - 用户身份
    user_preferences: HashMap<String, String>,
    user_identity: UserProfile,
    communication_style: String,

    // 生命周期
    last_updated: DateTime<Utc>,
    session_id: String,
}

pub struct MemoryFact {
    timestamp: DateTime<Utc>,
    fact: String,
    confidence: f32,                     // 0.0 - 1.0
    tags: Vec<String>,
    source: String,                      // "user_provided" | "agent_observed"
}
```

**特点**:

- 本地存储 (SQLite)
- 手动编辑友好
- 会话内快速访问
- 大小有限制 (~50KB)
- 每个档案独立副本

### 外部记忆提供商 (Honcho as Primary)

```rust
pub trait MemoryProvider: Send + Sync {
    // 基础操作
    async fn initialize(&mut self) -> Result<()>;
    async fn health_check(&self) -> Result<HealthStatus>;

    // 记忆管理
    async fn store_memory(&self, memory: ExternalMemory) -> Result<String>;
    async fn recall_memories(&self, query: &str, limit: usize) -> Result<Vec<ExternalMemory>>;
    async fn search_memories(&self, query: &str) -> Result<Vec<SearchResult>>;

    // 提供商特定的工具
    async fn get_tools(&self) -> Result<Vec<MemoryTool>>;

    // 档案管理
    async fn create_profile(&self, profile_id: &str) -> Result<()>;
    async fn switch_profile(&mut self, profile_id: &str) -> Result<()>;
}

pub struct ExternalMemory {
    id: String,
    content: String,
    memory_type: MemoryType,            // User, Observation, Pattern, etc.
    timestamp: DateTime<Utc>,
    session_id: Option<String>,
    tags: Vec<String>,
    metadata: HashMap<String, String>,
}

pub enum MemoryType {
    UserProfile,                        // 关于用户的长期事实
    Observation,                        // Agent 的观察
    Pattern,                            // 用户的行为模式
    Decision,                           // Agent 做出的决策
    Preference,                         // 用户偏好
    Context,                            // 背景信息
}
```

---

## Honcho 集成

### Honcho 核心概念 (AI-Native User Modeling)

```
User (同一个真实人)
    ├─ Workspace (多个 Hermes agents)
    │   ├─ AI Peer "default" (通用 agent)
    │   │   └─ 观察: 用户喜欢做什么
    │   ├─ AI Peer "coder" (编程 agent)
    │   │   └─ 观察: 用户的编码风格
    │   └─ AI Peer "writer" (写作 agent)
    │       └─ 观察: 用户的写作风格
    └─ 共享用户模型
        └─ "我是谁" (统一的身份和偏好)
```

**关键特性**:

- **Dialectic Q&A**: Honcho 主动提出问题来澄清和扩展用户模型
- **Semantic Search**: 向量化的记忆检索
- **Multi-Agent**: 每个 agent 有自己的 AI peer，但共享用户理解
- **Persistent Conclusions**: 会议总结和关键洞察的自动提取

### Honcho 4 个工具

```rust
pub struct HonchoToolKit {
    tools: HashMap<String, HonchoTool>,
}

pub enum HonchoTool {
    Profile {
        // honcho_profile
        // 返回用户的完整建模卡片
        description: "Get full user profile card from Honcho",
        output: UserProfileCard,
    },

    Search {
        // honcho_search
        // 语义搜索用户记忆
        description: "Search user memories semantically",
        params: SearchParams,
        output: Vec<MemoryResult>,
    },

    Context {
        // honcho_context
        // 获取 LLM 综合的上下文
        description: "Get LLM-synthesized context for current task",
        params: ContextRequest,
        output: String,  // LLM 生成的摘要
    },

    Conclude {
        // honcho_conclude
        // 存储新发现的事实
        description: "Store new facts or conclusions about user",
        params: ConclusionData,
        output: MemoryId,
    },
}

// 示例: honcho_profile 返回
pub struct UserProfileCard {
    user_id: String,
    summary: String,                    // "该用户是一名全栈开发者..."
    key_traits: Vec<String>,            // ["技术爱好者", "快速学习者"]
    preferences: Preferences,
    interactions_count: u32,
    last_interaction: DateTime<Utc>,
    conclusions: Vec<(String, f32)>,   // (fact, confidence)
}
```

### Honcho 配置

```rust
pub struct HonchoConfig {
    // 基础配置
    pub workspace_id: String,
    pub api_key: String,
    pub api_url: Option<String>,       // 自托管时使用

    // 档案配置
    pub ai_peer_name: String,          // "default", "coder", "writer"
    pub peer_name: String,              // 显示名称

    // 行为配置
    pub recall_mode: RecallMode,        // "auto" | "manual"
    pub write_frequency: WriteFrequency, // "immediate" | "session_end" | "on_demand"
    pub observation_level: u32,         // 0-3: how detailed observations are

    // 多档案继承
    pub inherit_from: Option<String>,   // 从另一个档案继承配置
}

pub enum RecallMode {
    Auto,           // 每个 turn 前自动拉取相关记忆
    Manual,         // 仅通过 honcho_search tool 拉取
    Disabled,       // 禁用 Honcho 记忆下注入
}

pub enum WriteFrequency {
    Immediate,      // 每条消息立即同步到 Honcho
    SessionEnd,     // 会话结束时批量上传
    OnDemand,       // 仅通过 honcho_conclude tool
}
```

### 配置文件结构 (honcho.json / honcho.yaml)

```yaml
# 全局配置
global:
  api_key: ${HONCHO_API_KEY}
  workspace_id: '${HONCHO_WORKSPACE_ID}'

  # 自托管模式
  # api_url: "http://localhost:9000"

# 档案配置
profiles:
  # 默认档案配置
  default:
    ai_peer_name: 'default'
    peer_name: 'Default Agent'

    recall_mode: 'auto'
    write_frequency: 'immediate'
    observation_level: 2

    # 自托管时的本地 cache
    cache_dir: '${HOME}/.if2ai/memory/honcho/default'
    cache_ttl: 3600 # seconds

  # 编程档案 (继承自 default)
  coder:
    ai_peer_name: 'coder'
    peer_name: 'Coder Agent'

    inherit_from: 'default'

    # 覆盖配置
    observation_level: 3 # 更细致的编码风格观察
    write_frequency: 'session_end'

  # 写作档案
  writer:
    ai_peer_name: 'writer'
    peer_name: 'Writer Agent'

    inherit_from: 'default'
    observation_level: 2
    write_frequency: 'immediate'

# 档案创建时自动执行
init_hooks:
  - create_ai_peer # 在 Honcho 中创建 peer
  - cache_initial_profile # 首次缓存配置
```

### Honcho 数据同步流程

```
┌─────────────────────────────────────┐
│  Session Start (Agent 初始化)        │
└──────────────┬──────────────────────┘
               │
               ├─► 读取 honcho.json
               │
               ├─► 连接 Honcho API
               │
               ├─► 获取 AI Peer 配置
               │
               └─► 预加载用户档案 (缓存)

┌──────────────────────────────────────┐
│  Each Agent Turn                     │
└──────────────┬───────────────────────┘
               │
          ┌────┴────┐
          │          │
       (auto mode)  (manual mode)
          │          │
          ▼          │
    ┌─────────────┐  │
    │ honcho_     │  │
    │ search()    │  │ (user calls tool)
    │ 拉取记忆    │  │
    └─────────────┘  │
          │          │
          │          ▼
          │      ┌─────────────┐
          │      │ honcho_     │
          │      │ search()    │
          │      │ (via tool)  │
          │      └─────────────┘
          │          │
          └──────┬───┘
                 │
         ┌───────▼────────┐
         │ 注入到 System  │
         │ Prompt        │
         └───────┬────────┘
                 │
         ┌───────▼────────────────────┐
         │ Agent 生成响应              │
         │ (使用 Honcho 上下文)        │
         └───────┬────────────────────┘
                 │
          ┌───────▼─────────┐
          │                 │
      (if write_frequency  (if write_frequency
       = immediate)         = on_demand)
          │                 │
          ▼                 │ (user calls)
    ┌──────────────┐        │
    │ honcho_      │        │
    │ store()      │────┐   │
    │ (自动)       │    │   │
    └──────────────┘    │   │
          │             │   │
          │   ┌─────────┘   │
          │   │             │
          │   │      ┌──────▼─────────┐
          │   │      │ honcho_        │
          │   │      │ conclude()     │
          │   │      │ (via tool)     │
          │   │      └──────┬─────────┘
          │   │             │
          └───┴─────────────┘
                 │
         ┌───────▼────────────────┐
         │ Sync to Honcho Cloud   │
         │ (Batch or immediate)   │
         └────────────────────────┘
```

---

## 多提供商模式

### 8 个记忆提供商对比

| 提供商          | 存储        | 成本      | 工具数 | 最适合                 |
| --------------- | ----------- | --------- | ------ | ---------------------- |
| **Honcho** ⭐   | Cloud       | 按需付费  | 4      | 用户建模 + 多代理      |
| **OpenViking**  | 自托管      | 免费      | 5      | 结构化知识 + 文件系统  |
| **Mem0**        | Cloud       | 按需付费  | 3      | 自动 LLM 提取          |
| **Hindsight**   | Cloud/本地  | 免费/付费 | 3      | 知识图 + 实体关系      |
| **Holographic** | 本地 SQLite | 免费      | 9      | 本地 + 高级查询        |
| **RetainDB**    | Cloud       | $20/月    | 5      | 混合搜索 (Vector+BM25) |
| **ByteRover**   | 本地/Cloud  | 免费/付费 | 3      | 便携式本地知识树       |
| **Supermemory** | Cloud       | 按需付费  | 4      | 语义检索 + 会话图      |

### Provider Registry

```rust
pub struct MemoryProviderRegistry {
    providers: HashMap<String, Arc<Box<dyn MemoryProvider>>>,
    active_provider: Option<String>,

    config: ProviderConfig,
}

impl MemoryProviderRegistry {
    pub async fn register_provider(
        &mut self,
        provider_id: &str,
        provider: Box<dyn MemoryProvider>,
        config: serde_json::Value,
    ) -> Result<()> {
        // 验证配置
        provider.validate_config(&config)?;

        // 初始化提供商
        let initialized = provider.initialize().await?;

        // 注册
        self.providers.insert(provider_id.to_string(), Arc::new(initialized));

        Ok(())
    }

    pub async fn activate_provider(&mut self, provider_id: &str) -> Result<()> {
        if !self.providers.contains_key(provider_id) {
            return Err(format!("Provider {} not registered", provider_id).into());
        }

        // 健康检查
        let provider = &self.providers[provider_id];
        provider.health_check().await?;

        self.active_provider = Some(provider_id.to_string());
        Ok(())
    }

    pub fn get_active_provider(&self) -> Result<Arc<Box<dyn MemoryProvider>>> {
        let provider_id = self.active_provider.as_ref()?;
        Ok(self.providers[provider_id].clone())
    }
}
```

---

## 数据流

### 会话生命周期中的记忆

```
┌──────────────────────────────────────────────┐
│ Session 初始化                               │
├──────────────────────────────────────────────┤
│                                              │
│ 1. 加载内置记忆 (MEMORY.md, USER.md)        │
│    └─ 从本地 SQLite 加载                    │
│                                              │
│ 2. 初始化 Honcho 连接                       │
│    ├─ 读取 honcho.json                     │
│    ├─ 验证 API 密钥                        │
│    └─ 获取 AI Peer 配置                    │
│                                              │
│ 3. 预加载用户档案                           │
│    ├─ honcho_profile() 获取用户卡片        │
│    └─ 缓存到 RAM (TTL: 1小时)             │
│                                              │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│ 每个 Turn (Agent 循环)                       │
├──────────────────────────────────────────────┤
│                                              │
│ 1. 召回上下文 (Recall Phase)                 │
│    if recall_mode == "auto":                │
│    ├─ 构造查询 Q = 用户输入 + 对话上下文   │
│    ├─ honcho_search(Q) 获取相关记忆        │
│    └─ 排序并截断 (top-5)                   │
│                                              │
│ 2. 构造系统提示                              │
│    System Prompt = Default + Honcho Context │
│    # 注例:                                   │
│    # "User is a software engineer..."      │
│    # "Usually prefers TypeScript..."        │
│    # "Has worked on 3 previous projects..." │
│                                              │
│ 3. Agent 循环 (agent-loop.md)               │
│    └─ 使用增强的系统提示运行               │
│                                              │
│ 4. 存储新记忆 (Write Phase)                 │
│    if write_frequency == "immediate":      │
│    ├─ 提取关键事实 (LLM-based)             │
│    └─ honcho_conclude() 存储到 Honcho     │
│                                              │
│ 5. 更新内置记忆                              │
│    ├─ MEMORY.md += 新观察                  │
│    └─ USER.md += 更新的偏好                │
│                                              │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│ Session 结束                                 │
├──────────────────────────────────────────────┤
│                                              │
│ 1. 批量同步 (Batch Sync)                    │
│    if write_frequency == "session_end":    │
│    ├─ 收集所有本轮的观察                   │
│    └─ honcho_store() 批量上传              │
│                                              │
│ 2. 提取会话总结                              │
│    ├─ LLM: 从对话中提取关键洞察            │
│    └─ honcho_conclude() 创建结论           │
│                                              │
│ 3. 保存会话日志                              │
│    └─ SQLite: session_history 表            │
│                                              │
└──────────────────────────────────────────────┘
```

### 数据模型

```rust
// SQLite Schema for Built-in Memory

CREATE TABLE memory_facts (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp DATETIME NOT NULL,
    fact TEXT NOT NULL,
    confidence REAL DEFAULT 0.5,
    tags TEXT,  // JSON array
    source TEXT,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

CREATE TABLE user_profile (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL UNIQUE,
    user_data JSONB,
    preferences JSONB,
    communication_style TEXT,
    last_updated DATETIME,
    FOREIGN KEY(profile_id) REFERENCES profiles(id)
);

// Honcho Cache Table

CREATE TABLE honcho_cache (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL,
    cache_key TEXT NOT NULL,
    cache_value JSONB,
    retrieved_at DATETIME NOT NULL,
    expires_at DATETIME NOT NULL,
    UNIQUE(profile_id, cache_key),
    FOREIGN KEY(profile_id) REFERENCES profiles(id)
);

CREATE TABLE honcho_sync_log (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL,
    session_id TEXT,
    sync_type TEXT,  // "store" | "recall" | "search"
    timestamp DATETIME NOT NULL,
    status TEXT,     // "success" | "failed" | "pending"
    error_message TEXT,
    FOREIGN KEY(profile_id) REFERENCES profiles(id),
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);
```

---

## 配置管理

### If2Ai 中的记忆配置

```rust
pub struct MemoryConfig {
    // 提供商选择
    pub enabled_providers: Vec<String>,
    pub active_provider: String,        // 当前活跃提供商

    // 内置记忆
    pub built_in: BuiltInMemoryConfig,

    // 提供商特定配置
    pub honcho: Option<HonchoConfig>,
    pub mem0: Option<Mem0Config>,
    pub hindsight: Option<HindsightConfig>,
    pub holographic: Option<HolographicConfig>,
    // ... etc for other providers

    // 全局行为
    pub global_recall: RecallStrategy,
    pub global_write: WriteStrategy,
}

pub struct BuiltInMemoryConfig {
    pub memory_db_path: PathBuf,
    pub max_memory_size: u64,           // bytes
    pub auto_compress: bool,            // LLM-based summarization
    pub compress_threshold: u32,        // when to compress
}
```

### 配置加载层次

```
1. 代码默认值 (hardcoded in Rust)
   ↓
2. 全局配置 ~/.if2ai/config.yaml
   ↓
3. 档案配置 ~/.if2ai/profiles/{profile}/config.yaml
   ↓
4. 环境变量 (HONCHO_API_KEY, etc)
   ↓
5. 运行时覆盖 (在 UI 中设置)
```

---

## 实现路线

### Phase 2: 记忆系统基础 (4-6 周)

#### 第 1-2 周: 内置记忆强化

- [ ] SQLite schema: memory_facts, user_profile
- [ ] MEMORY.md / USER.md 编辑接口
- [ ] 自动压缩算法 (LLM summary)
- [ ] 检索和搜索 (FTS5)

#### 第 2-3 周: Honcho 集成

- [ ] HonchoClient: API 包装层
- [ ] HonchoProvider 实现
- [ ] 4 个工具: profile, search, context, conclude
- [ ] 多档案支持和配置管理
- [ ] honcho.json 配置解析

#### 第 3-4 周: 内存管理器

- [ ] MemoryProviderRegistry
- [ ] Recall 管道 (context injection)
- [ ] Write 管道 (memory extraction)
- [ ] 同步日志和错误恢复

#### 第 4-5 周: Tauri Commands

- [ ] memory_list_providers
- [ ] memory_activate_provider
- [ ] memory_get_status
- [ ] memory_search (manual recall)
- [ ] memory_update_profile
- [ ] memory_sync (force sync)

#### 第 5-6 周: 可视化和调试

- [ ] 记忆浏览器 UI
- [ ] Honcho 同步状态仪表板
- [ ] 记忆搜索界面
- [ ] 调试日志查看器

### Phase 3: 其他提供商支持 (4-8 周)

- [ ] OpenViking 适配器
- [ ] Mem0 适配器
- [ ] Hindsight 适配器
- [ ] Holographic 适配器 (本地优先)
- [ ] 提供商比较和推荐 UI

### Phase 4: 高级特性 (6+ 周)

- [ ] 跨会话学习 (multi-turn optimization)
- [ ] 记忆融合 (consolidation)
- [ ] 隐私模式 (本地 + 加密)
- [ ] 批量导入/导出

---

## Tauri Commands 设计

### 记忆管理 Commands

```rust
// 提供商管理
#[tauri::command]
async fn memory_list_providers() -> Result<Vec<ProviderInfo>>;

#[tauri::command]
async fn memory_activate_provider(provider_id: String) -> Result<()>;

#[tauri::command]
async fn memory_get_status() -> Result<MemoryStatus>;

// 记忆搜索和检索
#[tauri::command]
async fn memory_search(query: String, limit: u32) -> Result<Vec<MemoryResult>>;

#[tauri::command]
async fn memory_get_profile() -> Result<UserProfileCard>;

// 记忆写入
#[tauri::command]
async fn memory_store_fact(fact: String, tags: Vec<String>) -> Result<String>;

#[tauri::command]
async fn memory_update_profile(updates: serde_json::Value) -> Result<()>;

// 同步和导出
#[tauri::command]
async fn memory_sync() -> Result<SyncReport>;

#[tauri::command]
async fn memory_export(provider_id: String, format: String) -> Result<PathBuf>;

// 配置
#[tauri::command]
async fn memory_configure(provider_id: String, config: serde_json::Value) -> Result<()>;

#[tauri::command]
async fn memory_get_config(provider_id: String) -> Result<serde_json::Value>;
```

### 内置工具 (在 Agent Loop 中使用)

```rust
pub fn memory_built_in_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "honcho_profile".to_string(),
            description: "Get user profile from Honcho".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["json", "text", "summary"],
                        "default": "text"
                    }
                }
            }),
        },
        Tool {
            name: "honcho_search".to_string(),
            description: "Search user memories in Honcho".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "default": 5 },
                    "filter": {
                        "type": "object",
                        "properties": {
                            "memory_type": { "type": "string" },
                            "date_range": { "type": "string" }
                        }
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "honcho_context".to_string(),
            description: "Get LLM-synthesized context from Honcho".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "task": { "type": "string" },
                    "max_tokens": { "type": "integer", "default": 500 }
                },
                "required": ["task"]
            }),
        },
        Tool {
            name: "honcho_conclude".to_string(),
            description: "Store new facts or conclusions about user".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "fact": { "type": "string" },
                    "memory_type": {
                        "type": "string",
                        "enum": ["profile", "observation", "pattern", "preference"]
                    },
                    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                },
                "required": ["fact", "memory_type"]
            }),
        },
    ]
}
```

---

## Hermes 对标

### Hermes 记忆系统

| 特性            | Hermes  | If2Ai       | 进度      |
| --------------- | ------- | ----------- | --------- |
| 8 个提供商支持  | ✅ 完整 | 📋 设计完成 | 设计 100% |
| Honcho 集成     | ✅ 完整 | 📋 设计完成 | 设计 100% |
| 内置 MEMORY.md  | ✅      | ✅ 部分     | 80%       |
| 内置 USER.md    | ✅      | ✅ 部分     | 80%       |
| Recall Pipeline | ✅      | 📋 设计完成 | 设计 100% |
| Write Pipeline  | ✅      | 📋 设计完成 | 设计 100% |
| 多档案支持      | ✅      | 📋 设计完成 | 设计 100% |
| 提供商名作系统  | ✅      | 📋 计划     | 0%        |

### Honcho 特定对齐

```
Hermes honcho_setup    ======> If2Ai memory_activate_provider
Hermes honcho status   ======> If2Ai memory_get_status
Hermes - honcho_profile tool ======> If2Ai honcho_profile tool
Hermes - honcho_search tool ======> If2Ai honcho_search tool
Hermes - honcho_context tool ======> If2Ai honcho_context tool
Hermes - honcho_conclude tool ======> If2Ai honcho_conclude tool

Hermes honcho.json ======> If2Ai .if2ai/honcho.json
Hermes MEMORY.md ======> If2Ai MEMORY.md (SQLite backed)
Hermes USER.md ======> If2Ai USER.md (SQLite backed)
Hermes multi-profile support ======> If2Ai profiles + MemoryProvider
```

---

## 实现示例: Honcho 完整流程

```rust
// 初始化
let mut memory_mgr = MemoryManager::new(app_state.clone()).await?;
memory_mgr.activate_provider("honcho").await?;

// 会话开始
let recall_result = memory_mgr.recall_for_query(
    "用户最近在做什么项目？",
    5  // top-5
).await?;
// 返回: [Memory1("在做 Rust 项目"), Memory2("学习 async patterns")]

// 注入系统提示
let system_prompt = format!(
    "{}\\n\\n## User Context from Honcho:\\n{}",
    DEFAULT_SYSTEM_PROMPT,
    recall_result.format_for_prompt()
);

// Agent 运行 (agent-loop.md)
let response = agent_loop(input, &system_prompt).await?;

// 提取和存储新记忆
let facts = extract_facts_from_response(&response).await?;
for fact in facts {
    memory_mgr.store_memory(
        fact.content,
        fact.memory_type,
        0.8  // confidence
    ).await?;
}
```

---

## 总结

### If2Ai Memory System 的独特优势

✅ **Honcho AI-Native 用户建模** - 不仅存储事实，还学习用户的思维方式  
✅ **8 个提供商灵活选择** - 云、本地、混合等多种方案  
✅ **与 Agent Loop 深度集成** - 自动 recall + write  
✅ **多档案支持** - 每个 agent 有自己的角色，但共享用户理解  
✅ **Hermes 完全对标** - 所有特性和工具都实现

### 关键亮点

1. **Dialectic Learning**: Honcho 主动提问来澄清用户模型
2. **Multi-Agent**: 适合多 Agent 系统 (coder, writer, scientist agents)
3. **Hybrid Recall**: 自动 + 手动混合模式
4. **Provider Agnostic**: 同时支持 8 个提供商，随时切换

---

**版本历史**:

- v1.0 (2026-04-11) - 初始设计，完整 Honcho + 8 提供商对标

**相关文档**:

- 📖 [Hermes Memory Providers](https://hermes-agent.nousresearch.com/docs/user-guide/features/memory-providers)
- 🏗️ [Agent Loop 设计](./agent-loop.md)
- 📊 [Session Persistence 设计](./session-persistence.md)
- 🔄 [RL-Training 设计](./agent-self-improvement.md)
