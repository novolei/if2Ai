# Messaging Gateway 设计文档

**版本**: 1.0 | **最后更新**: 2026-04-11 | **状态**: Design Phase | **对齐**: Hermes v0.8.0

> If2Ai Messaging Gateway 允许用户通过多个通讯平台（Telegram, Discord, Slack 等）与 Agent 互动，提供统一的 Session 管理、命令体系和用户认证。

---

## 目录

- [1. 架构概览](#架构概览)
- [2. 平台支持](#平台支持)
- [3. 核心功能](#核心功能)
- [4. Session 管理](#session-管理)
- [5. 命令体系](#命令体系)
- [6. 安全模型](#安全模型)
- [7. 实现路线](#实现路线)
- [8. API 设计](#api-设计)
- [9. Hermes 对标](#hermes-对标)

---

## 架构概览

### 系统架构图

```
┌─────────────────────────────────────────────────────────┐
│                   用户输入来源                            │
├─────────────────────────────────────────────────────────┤
│  Telegram  │  Discord  │  Slack  │  WhatsApp  │  ...   │
├─────────────────────────────────────────────────────────┤
│           Messaging Gateway 网关层                       │
│  ┌─────────────────────────────────────────────────┐    │
│  │ Platform-Specific Adapter Layer                 │    │
│  │ ├─ TelegramAdapter  (polling/webhook)          │    │
│  │ ├─ DiscordAdapter   (websocket)                │    │
│  │ ├─ SlackAdapter     (bolt framework)           │    │
│  │ └─ ...AdapterAdapters                          │    │
│  └─────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────┤
│           Session Manager (per chat/room)              │
│  ├─ SessionStore (Redis/SQLite)                       │
│  ├─ Message History (with compression)               │
│  └─ User Auth & Pairing                              │
├─────────────────────────────────────────────────────────┤
│              Agent Runtime (共享的 AppState)            │
│  └─ 来自 entry-points-design.md 的核心 Agent Loop      │
├─────────────────────────────────────────────────────────┤
│           Cron Scheduler (每 60 秒检查一次)             │
│  ├─ Job Store                                        │
│  └─ Delivery Notification System                    │
└─────────────────────────────────────────────────────────┘
```

### 3 层分层设计

**Layer 1: Platform Abstraction**

- 平台适配器（Telegram, Discord, Slack 等）
- 消息规范化（不同平台的消息格式统一）
- 媒体处理（图片、文件、语音）

**Layer 2: Session & State Management**

- Per-Chat Session 存储
- 用户会话持久化
- Context 压缩和历史管理
- DM Pairing 和授权

**Layer 3: Agent Processing**

- 共享的 AppState 和 Agent Runtime
- 命令解析和执行
- 后台任务支持 (background sessions)

---

## 平台支持

### 完整的平台清单

#### Tier 1: 完全支持 (Hermes v0.8.0 级别)

| 平台         | 文本 | 图片 | 文件 | 线程 | 反应 | 输入态 | 流式 | 优先级 |
| ------------ | ---- | ---- | ---- | ---- | ---- | ------ | ---- | ------ |
| **Telegram** | ✅   | ✅   | ✅   | ✅   | —    | ✅     | ✅   | P0     |
| **Discord**  | ✅   | ✅   | ✅   | ✅   | ✅   | ✅     | ✅   | P0     |
| **Slack**    | ✅   | ✅   | ✅   | ✅   | ✅   | ✅     | ✅   | P0     |
| **WhatsApp** | ✅   | ✅   | ✅   | —    | —    | ✅     | ✅   | P1     |
| **Matrix**   | ✅   | ✅   | ✅   | ✅   | ✅   | ✅     | ✅   | P1     |
| **Feishu**   | ✅   | ✅   | ✅   | ✅   | ✅   | ✅     | ✅   | P1     |

#### Tier 2: 基础支持 (Hermes v0.8.0 级别)

| 平台           | 文本 | 图片 | 文件 | 线程 | 反应 | 输入态 | 流式 |
| -------------- | ---- | ---- | ---- | ---- | ---- | ------ | ---- |
| **Signal**     | ✅   | ✅   | ✅   | —    | —    | ✅     | ✅   |
| **Email**      | ✅   | ✅   | ✅   | ✅   | —    | —      | —    |
| **Mattermost** | ✅   | ✅   | ✅   | ✅   | —    | ✅     | ✅   |
| **WeCom**      | ✅   | ✅   | ✅   | —    | —    | ✅     | ✅   |
| **Weixin**     | ✅   | ✅   | ✅   | —    | —    | ✅     | ✅   |
| **DingTalk**   | —    | —    | —    | —    | —    | ✅     | ✅   |

#### Tier 3: 特殊平台 (不同实现)

| 平台               | 用途     | 特性                    |
| ------------------ | -------- | ----------------------- |
| **SMS (Twilio)**   | 基础文本 | 电话号码验证, 速率限制  |
| **Home Assistant** | 智能家居 | 设备控制, 状态查询      |
| **BlueBubbles**    | iMessage | Mac 桥接, 消息同步      |
| **API Server**     | 通用接口 | OpenAI 兼容, JSON-RPC   |
| **Webhooks**       | 外部集成 | 可定制, 模板 URL 参数化 |

### Phase 1-3 的平台演进

**Phase 1 (当前 - Tauri)**: 无 Gateway，只有 Desktop UI

- Focus: Desktop 应用的 local agent
- Platform: Tauri + Svelte

**Phase 2 (2-4 周)**: 核心 Gateway 实现

- **新增**: Telegram, Discord, Slack (P0 平台)
- **支持**: Session 管理, 认证, 命令
- **代码**: Python/Rust 网关服务

**Phase 3 (4-8 周)**: 扩展平台支持与集成

- **新增**: WhatsApp, Matrix, Signal, Email (Tier 2)
- **新增**: Home Assistant, SmartHome 集成
- **增强**: 语音支持, 更好的媒体处理

---

## 核心功能

### 1. 消息流处理

```
用户消息 (Telegram/Discord/...)
  │
  ├─→ Platform Adapter 接收
  │
  ├─→ Message Normalization
  │   ├─ 规范化格式 (text, images, files)
  │   ├─ 提取 user_id, chat_id, thread_id
  │   └─ 处理特殊标记 (mentions, reactions)
  │
  ├─→ Session Lookup (per chat)
  │   └─ 获取或创建 Session
  │
  ├─→ Security Check
  │   ├─ User 在 allowlist 中?
  │   ├─ 或者已 paired?
  │   └─ 否则: 要求配对
  │
  ├─→ Command Parser
  │   ├─ 如果是 /command → 执行
  │   └─ 否则 → 传递给 Agent
  │
  ├─→ Agent.run_turn()
  │   └─ 从 AppState 运行 (见 agent-loop.md)
  │
  └─→ Response Formatting & Delivery
      ├─ 适配平台格式
      ├─ 处理消息分割 (长度限制)
      ├─ 流式消息更新 (如果支持)
      └─ 发送回消息平台
```

### 2. Session 管理

#### Session 生命周期

```rust
pub struct GatewaySession {
    // 身份标识
    session_id: String,           // 唯一 ID
    platform: String,             // "telegram", "discord" 等
    platform_chat_id: String,    // 平台特定的 chat/room ID
    platform_thread_id: Option<String>,  // 可选的线程 ID

    // 用户和权限
    user_id: String,              // 平台用户 ID
    user_name: String,            // 显示名称
    user_allowed: bool,            // 在 allowlist 中?

    // 状态管理
    status: SessionStatus,         // active, idle, waiting_approval
    created_at: DateTime,
    last_message_at: DateTime,
    is_running: bool,              // Agent 当前在运行?

    // Agent 重用
    agent_session_id: String,      // 关联到 agent-loop.md 中的 ConversationRuntime

    // 配置
    model: String,                 // 当前模型 ("gpt-4", "claude-3" 等)
    provider: String,              // 当前提供商
    personality: String,           // 个性模板

    // 重置策略
    reset_mode: ResetMode,         // daily @ 4am, idle 1440min, 或 both
}

pub enum SessionStatus {
    Active,                    // 准备接收消息
    Running,                   // Agent 正在处理
    WaitingApproval,          // 等待危险命令的批准
    Idle,                      // 设置闲置但未重置
    Archived,                  // 手动存档
}
```

#### Reset 策略 (来自 Hermes)

```yaml
# ~/.hermes/gateway.json
{ 'reset_by_platform': { 'telegram': {
          'mode': 'idle', # daily | idle | both
          'idle_minutes': 240, # 4 小时未活动后重置
          'daily_reset_hour': 4 # 每天 4:00 AM 重置
        }, 'discord': { 'mode': 'both', 'idle_minutes': 60, 'daily_reset_hour': 4 } }, 'default_reset': {
      'mode': 'idle',
      'idle_minutes': 1440 # 24 小时
    } }
```

### 3. 命令系统

#### 内置命令

```
会话管理
  /new, /reset              - 开始新对话
  /resume [name]            - 恢复之前的会话
  /title [name]             - 设置/显示会话标题
  /status                   - 显示 session 信息
  /compress                 - 手动压缩上下文

模型和配置
  /model [provider:model]   - 显示或切换模型 (live switching)
  /provider                 - 显示可用的提供商和认证状态
  /personality [name]       - 切换个性模板
  /reasoning [level|show]   - 改变推理努力 (gpt-o1, claude-thinking)

对话控制
  /retry                    - 重试上一条消息
  /undo                     - 撤销上一条交换 (用户/助手)
  /stop                     - 立即停止运行的 agent
  /approve, /deny           - 批准/拒绝危险命令

背景任务
  /background <prompt>      - 在后台独立运行 prompt
  /reload-mcp               - 重新加载 MCP 服务器

其他
  /help                     - 显示可用命令
  /usage                    - 显示此会话的 token 使用情况
  /insights [days]          - 显示使用统计和分析
  /<skill-name>             - 调用任何已安装的 skill
```

#### Skills 作为子命令

```rust
// skills 通过 plugin 系统自动注册为命令
// 例如: /research, /code_review, /document 等

pub struct SkillCommand {
    name: String,                 // "research"
    description: String,
    required_env_vars: Vec<String>,

    // 首次运行时提示
    on_first_run_prompt: bool,

    // 支持的平台
    enabled_platforms: Vec<String>,  // ["telegram", "discord"]
}
```

### 4. 后台任务支持 (Hermes v0.8.0 特性)

```rust
pub struct BackgroundTask {
    task_id: String,                    // "bg_143022_a1b2c3"
    platform: String,
    chat_id: String,
    user_id: String,

    prompt: String,                     // 用户的 prompt
    status: TaskStatus,                 // running, completed, failed

    started_at: DateTime,
    completed_at: Option<DateTime>,

    // 结果
    result: Option<String>,
    error: Option<String>,

    // 通知设置
    notify_on_complete: bool,          // /background 自动设置为 true
    notification_mode: NotificationMode,
}

pub enum NotificationMode {
    All,      // 输出更新 + 完成消息
    Result,   // 仅完成消息
    Error,    // 仅错误消息
    Off,      // 无消息
}

// 消息流:
// 1. 用户: "/background 研究竞争对手定价"
// 2. Bot:  "🔄 后台任务已启动: '研究竞争对...'"
//          "Task ID: bg_143022_a1b2c3"
// 3. [Agent 在后台独立运行，有自己的 session]
// 4. 当完成时: "✅ 后台任务完成: '研究竞争对手定价'"
//               [输出或链接到结果]
```

### 5. 中断机制 (Interruption)

```
用户在 agent 运行时发送任何消息 → 中断
  │
  ├─→ 如果在执行 terminal 命令
  │   └─ 发送 SIGTERM, 1秒后 SIGKILL
  │
  ├─→ 如果在执行工具链
  │   └─ 取消待执行的工具, 仅运行当前的
  │
  ├─→ 合并消息
  │   └─ 多条中断期间发送的消息合并为一条 prompt
  │
  └─→ /stop 命令
      └─ 中断但不排队后续消息
```

### 6. 工具进度通知 (Tool Progress Notifications)

```yaml
# ~/.hermes/config.yaml
display:
  tool_progress: all # off | new | all | verbose
  tool_progress_command: false # /verbose 在消息中启用?
  background_process_notifications: all # all | result | error | off
```

**进度消息示例**:

```
💻 ls -la...
🔍 web_search...
📄 web_extract...
🐍 execute_code...
✅ 所有工具完成
```

### 7. 类型化 Approval 系统

```rust
pub struct ApprovalRequest {
    approval_id: String,           // 唯一 session key
    command: String,               // 要执行的命令
    description: String,           // "执行 rm -rf /prod?"

    // 优先级和限制
    priority: ApprovalLevel,       // normal | high | critical
    once: bool,                    // 仅此一次审批?
    timeout_seconds: u32,          // 多少秒内需要批准?

    // 平台支持
    supports_buttons: bool,        // 支持原生按钮?
    approval_button_labels: (String, String),  // ("批准", "拒绝")

    // 状态追踪
    status: ApprovalStatus,        // pending | approved | denied | timeout
    approved_by: Option<String>,   // user_id
    approved_at: Option<DateTime>,
}

// Hermes v0.8.0 特性:
// Slack: Thread context preservation
// Telegram: Emoji reactions (reaction buttons)
// 其他: Fallback 到输入命令
```

---

## Session 管理

### Per-Chat Sessions 存储设计

```rust
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, platform: &str, chat_id: &str)
        -> Result<GatewaySession>;

    async fn get_session(&self, session_id: &str)
        -> Result<GatewaySession>;

    async fn get_by_platform_chat(&self, platform: &str, chat_id: &str)
        -> Result<GatewaySession>;

    async fn save_session(&self, session: GatewaySession)
        -> Result<()>;

    async fn reset_session(&self, session_id: &str)
        -> Result<()>;

    async fn list_sessions_for_user(&self, user_id: &str)
        -> Result<Vec<GatewaySession>>;

    async fn archive_session(&self, session_id: &str)
        -> Result<()>;
}
```

### 双重存储: 快速访问 + 持久化

```
┌─────────────────────────────┐
│   In-Memory Cache           │
│  (HashMap<chat_id, Session>)│
│  用于快速查询               │
└──────────────┬──────────────┘
               │
               ├─→ L2: Redis Cache (可选)
               │      TTL: 1 小时
               │      用于分布式部署
               │
               └─→ L3: SQLite/PostgreSQL
                      永久存储
                      历史查询
```

### 多用户线程支持 (Hermes v0.8.0 特性)

```rust
pub struct ThreadedSession {
    platform: String,
    chat_id: String,              // Server channel 或 group
    thread_id: String,            // Thread within chat (Discord, Slack)

    // 多用户支持
    participants: Vec<UserId>,    // [user1, user2, ...]
    initiator: UserId,            // 谁创建了这个 thread

    // 权限
    permissions: HashMap<UserId, PermissionLevel>,

    // Session 关联
    agent_session_id: String,     // 共享的 agent session (多用户)
}
```

---

## 命令体系

### 核心命令实现

```rust
pub trait GatewayCommand: Send + Sync {
    // 命令的唯一标识
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str>;

    // 权限和平台检查
    fn requires_approval(&self) -> bool;
    fn allowed_platforms(&self) -> Vec<&str>;
    fn requires_auth(&self) -> bool;

    // 执行命令
    async fn execute(
        &self,
        session: &mut GatewaySession,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<CommandResponse>;
}

pub struct CommandResponse {
    message: String,                  // 返回给用户的消息
    status: CommandStatus,            // success | error | pending
    requires_acknowledgment: bool,   // 用户需要确认吗?
}

pub struct CommandContext {
    platform: String,
    user_id: String,
    chat_id: String,
    thread_id: Option<String>,
    original_message: String,

    // 引用共享状态
    app_state: Arc<AppState>,
    session_store: Arc<dyn SessionStore>,
}
```

### 命令解析流程

```
消息 "/model gpt-4o"
  │
  ├─→ 正则匹配: "^/(\\w+)(\\s+.*)?"
  │   └─ 命令: "model", 参数: "gpt-4o"
  │
  ├─→ CommandRegistry 查询
  │   └─ 查找 ModelCommand 实现
  │
  ├─→ 权限检查
  │   ├─ 用户在 allowlist?
  │   ├─ 是否需要批准?
  │   └─ 平台支持此命令?
  │
  └─→ 执行命令
      └─ ModelCommand::execute(session, ["gpt-4o"], context)
          └─ 返回 "已切换到 gpt-4o"
```

---

## 安全模型

### 认证和授权

```rust
pub enum AuthLevel {
    Unknown,           // 完全陌生用户
    Paired,            // 通过 DM pairing 验证
    Allowlisted,       // 在 env var allowlist 中
    Admin,             // 管理员 (可配置)
}

pub struct SecurityPolicy {
    // 默认策略: 拒绝未知用户
    default_deny: bool,

    // Allowlist (通过 env vars)
    telegram_allowed_users: Vec<String>,
    discord_allowed_users: Vec<String>,
    slack_allowed_users: Vec<String>,
    // ... 等等

    // DM Pairing
    allow_pairing: bool,
    pairing_code_length: usize,      // 8 个字符
    pairing_code_ttl_seconds: u32,   // 3600 (1 小时)
    pairing_code_rate_limit: u32,    // 每分钟最多 3 个

    // 命令批准
    dangerous_commands: Vec<String>,  // rm, sudo, docker 等
    approval_timeout_seconds: u32,    // 300 (5 分钟)
}
```

### DM Pairing 工作流

```
1. 陌生用户在 Telegram 给 bot DM
2. Bot: "你想配对吗? 配对代码: XKGH5N7P"
   (代码在 1 小时内有效)

3. 用户告诉 admin 代码

4. Admin: hermes pairing approve telegram XKGH5N7P

5. Bot 添加用户到此 Telegram 的 Session allowlist

6. 下次 DM → 自动接受

管理命令:
  hermes pairing list                    # 显示待审批
  hermes pairing approve telegram CODE
  hermes pairing revoke telegram USER_ID

配对代码:
  - 加密随机 (cryptographic randomness)
  - 与用户+时间戳绑定
  - 单次使用
```

---

## 实现路线

### Phase 2: 核心 Gateway (2-4 周)

**目标**: 3 个 P0 平台的完全功能

#### 周 1-2: 基础架构

- [ ] SessionStore 实现 (SQLite backend)
- [ ] Platform Adapter 基类
- [ ] Message Normalization
- [ ] CommandRegistry 和基础命令
- [ ] 认证和 allowlist 系统

#### 周 2-3: Telegram 适配器

```rust
pub struct TelegramAdapter {
    bot_token: String,
    polling_interval_ms: u32,

    // Telegram 特定
    update_id: u64,

    // 会话存储
    sessions: Arc<dyn SessionStore>,
}

impl PlatformAdapter for TelegramAdapter {
    async fn start(&mut self) -> Result<()> {
        // polling 或 webhook 循环
        loop {
            let updates = self.get_updates().await?;
            for update in updates {
                self.handle_update(update).await?;
            }
        }
    }

    async fn send_message(&self, chat_id: i64, text: &str)
        -> Result<MessageId>;
    async fn send_image(&self, chat_id: i64, image_url: &str)
        -> Result<MessageId>;
    async fn edit_message(&self, chat_id: i64, msg_id: i32,
        new_text: &str) -> Result<()>;
}
```

**核心特性**:

- Message polling (或 webhook 如果有 URL)
- Group & supergroup topics (群组话题)
- DM pairing flow
- 内联按钮 (inline buttons)

#### 周 3-4: Discord + Slack

```rust
pub struct DiscordAdapter {
    bot_token: String,
    intent_flags: u32,          // 需要哪些 intents?
    websocket: WebSocket,       // 持续连接

    sessions: Arc<dyn SessionStore>,
}

pub struct SlackAdapter {
    bot_token: String,
    signing_secret: String,

    bolt: SlackBoltFramework,   // 使用 slack-bolt
    sessions: Arc<dyn SessionStore>,
}
```

**核心特性**:

- Discord: Slash commands, threads, reactions
- Slack: App mentions, thread replies, mrkdwn formatting

#### 周 4 新增功能

- [ ] 后台任务支持 (/background)
- [ ] 工具进度通知
- [ ] 中断机制 (Interruption)
- [ ] Session reset 策略

### Phase 3: 平台扩展 + 高级特性 (4-8 周)

#### 额外平台 (Tier 2)

- [ ] WhatsApp (Twilio/Official API)
- [ ] Signal (signal-cli bridge)
- [ ] Matrix (matrix-rust-sdk)
- [ ] Email (IMAP/SMTP)
- [ ] WeCom (企业微信)

#### 高级特性

- [ ] 音频支持 (TTS, voice transcription)
- [ ] 富文本格式 (markdown → 平台格式)
- [ ] 媒体管理 (图片、文件上传/下载)
- [ ] Cron 集成 (定时任务发送)
- [ ] Webhook 入站

---

## API 设计

### Tauri IPC Commands (Messaging Gateway)

```rust
// src-tauri/src/commands/gateway.rs

#[tauri::command]
async fn gateway_start(
    state: State<'_, AppState>,
) -> Result<()> {
    // 启动网关服务
    // 加载配置, 初始化所有 adapters
}

#[tauri::command]
async fn gateway_stop() -> Result<()> {
    // 优雅关闭
}

#[tauri::command]
async fn gateway_status() -> Result<GatewayStatus> {
    pub struct GatewayStatus {
        running: bool,
        connected_platforms: Vec<PlatformStatus>,
        active_sessions: usize,
        cron_jobs: usize,
    }
}

// Session 管理
#[tauri::command]
async fn gateway_list_sessions() -> Result<Vec<SessionInfo>> {}

#[tauri::command]
async fn gateway_get_session(session_id: String)
    -> Result<SessionDetails> {}

#[tauri::command]
async fn gateway_reset_session(session_id: String)
    -> Result<()> {}

#[tauri::command]
async fn gateway_archive_session(session_id: String)
    -> Result<()> {}

// 配置
#[tauri::command]
async fn gateway_setup_wizard() -> Result<()> {
    // 交互式设置向导
}

#[tauri::command]
async fn gateway_get_config() -> Result<GatewayConfig> {}

#[tauri::command]
async fn gateway_save_config(config: GatewayConfig)
    -> Result<()> {}

// 平台特定
#[tauri::command]
async fn gateway_configure_telegram(token: String)
    -> Result<()> {}

#[tauri::command]
async fn gateway_configure_discord(token: String)
    -> Result<()> {}

// ... 等等为每个平台
```

### Rust 内部 API

```rust
// src-tauri/src/modules/gateway/mod.rs

pub struct MessagingGateway {
    adapters: HashMap<String, Box<dyn PlatformAdapter>>,
    sessions: Arc<SessionStore>,
    app_state: Arc<AppState>,
    cron: Arc<CronScheduler>,
    config: GatewayConfig,
}

impl MessagingGateway {
    pub async fn new(app_state: Arc<AppState>)
        -> Result<Self>;

    pub async fn start(&mut self) -> Result<()>;
    pub async fn stop(&mut self) -> Result<()>;

    pub async fn send_message(
        &self,
        platform: &str,
        chat_id: &str,
        message: &str,
    ) -> Result<MessageId>;

    pub async fn get_session(
        &self,
        platform: &str,
        chat_id: &str,
    ) -> Result<GatewaySession>;

    pub async fn execute_command(
        &self,
        session: &mut GatewaySession,
        command: &str,
        args: Vec<String>,
    ) -> Result<CommandResponse>;
}

pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &str;

    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;

    async fn send_message(&self, chat_id: impl ToString,
        text: &str) -> Result<MessageId>;
    async fn send_image(&self, chat_id: impl ToString,
        url: &str) -> Result<MessageId>;
    async fn send_file(&self, chat_id: impl ToString,
        file_path: &Path) -> Result<MessageId>;

    async fn edit_message(&self, chat_id: impl ToString,
        msg_id: MessageId, new_text: &str)
        -> Result<()>;

    async fn render_typing_indicator(&self,
        chat_id: impl ToString) -> Result<()>;

    async fn handle_command(&self, session: &GatewaySession,
        command: &str, args: Vec<String>)
        -> Result<CommandResponse>;
}
```

---

## Hermes 对标

### Hermes v0.8.0 新特性

#### 1. Background Process Auto-Notifications (✨)

**Hermes**:

```python
# /background 启动后台任务
# Agent 可调用 terminal(background=true)
# 当进程完成时，通知用户
notify_on_complete = True
```

**If2Ai 实现**:

```rust
pub struct BackgroundTask {
    prompt: String,
    status: TaskStatus,
    notify_on_complete: bool,     // 自动设置为 true
    notification_mode: NotificationMode,
}

// 消息: "🔄 后台任务启动..."
// 然后: "✅ 后台任务完成" 或 "❌ 错误"
```

**优先级**: P1 (Phase 2 中期)

#### 2. Live Model Switching (/model 命令)

**Hermes**:

```
/model gpt-4o              # 切换到某个模型
/model openrouter:meta/llama    # 指定提供商
/provider             # 显示可用的提供商
```

**If2Ai 实现**:

```rust
#[derive(Clone)]
pub struct ModelCommand {
    provider_manager: Arc<ProviderManager>,
}

impl GatewayCommand for ModelCommand {
    async fn execute(&self, session: &mut GatewaySession,
        args: Vec<String>, ctx: &CommandContext)
        -> Result<CommandResponse> {
        if args.is_empty() {
            // 显示当前模型
            return Ok(CommandResponse {
                message: format!("当前: {} ({})",
                    session.model, session.provider),
                ..Default::default()
            });
        }

        // 解析 provider:model 或仅 model
        let (provider, model) = self.parse_model_spec(&args[0])?;

        // 验证提供商在线
        if !self.provider_manager.is_provider_available(&provider).await? {
            return Err("提供商不可用".into());
        }

        // 切换
        session.provider = provider.clone();
        session.model = model.clone();

        Ok(CommandResponse {
            message: format!("已切换到 {} ({})", model, provider),
            ..Default::default()
        })
    }
}
```

**优先级**: P0 (Phase 2 早期)

#### 3. Approval Buttons

**Hermes v0.8.0**:

- Slack: Native thread context preservation
- Telegram: Emoji reactions for approval status
- Discord: Slash commands for /approve, /deny

**If2Ai 实现**:

```rust
pub struct ApprovalButton {
    platform: String,     // "slack", "telegram", "discord"
    approve_label: String,
    deny_label: String,

    // Platform-specific
    use_native_buttons: bool,  // Slack/Discord 支持原生按钮
    use_emoji_reactions: bool,  // Telegram 用 emoji

    callback_id: String,   // 回调标识
    timeout_seconds: u32,
}

// Telegram: "👍 批准 | 👎 拒绝"
// Discord: Buttons
// Slack: Buttons with thread context
```

**优先级**: P1 (Phase 2 中期)

#### 4. Inactivity-Based Timeouts

**Hermes**:

```python
# 不是基于墙上时间，而是基于实际活动
# 长时间运行但仍在工作的任务永远不会被杀死
# 仅真正空闲的 agents 会超时
```

**If2Ai 实现**:

```rust
pub struct ActivityTracker {
    last_tool_execution: DateTime,
    last_user_input: DateTime,
    last_message_sent: DateTime,
    tool_activity_count: u32,

    idle_timeout_minutes: u32,     // 1440 (24 小时)
}

impl ActivityTracker {
    pub fn should_timeout(&self) -> bool {
        let last_activity = self.last_tool_execution
            .max(self.last_user_input)
            .max(self.last_message_sent);

        let elapsed = Utc::now().signed_duration_since(last_activity);
        elapsed.num_minutes() > self.idle_timeout_minutes as i64
    }
}
```

**优先级**: P1 (Phase 2 中期)

#### 5. 平台强化

**Hermes v0.8.0**:

| 平台           | 新特性                                                             |
| -------------- | ------------------------------------------------------------------ |
| **Matrix**     | Tier 1: reactions, read receipts, rich formatting, room management |
| **Discord**    | Channel controls (ignored_channels, no_thread_channels)            |
| **Telegram**   | Group topics skill binding for supergroup forums                   |
| **Slack**      | Thread engagement + mrkdwn in edit_message                         |
| **Signal**     | Full MEDIA: tag delivery                                           |
| **Mattermost** | File attachments                                                   |
| **Feishu**     | Interactive card approval buttons                                  |

**If2Ai Phase 2 目标**: 至少 Telegram, Discord, Slack 达到 Hermes v0.8.0 级别

### 代码行数对比

| 组件            | Hermes     | If2Ai (估计)     |
| --------------- | ---------- | ---------------- |
| 网关核心        | 2,500+     | 1,500 (简化设计) |
| Telegram 适配器 | 800+       | 400              |
| Discord 适配器  | 700+       | 350              |
| Slack 适配器    | 600+       | 300              |
| Session Store   | 500+       | 300              |
| 命令系统        | 1,200+     | 600              |
| **总计**        | **6,300+** | **3,550**        |

**策略**: If2Ai 使用更高级的 Rust futures 和 tokio，代码更精简；Hermes 是 Python，代码更详细。

---

## 完整的配置示例

```yaml
# ~/.hermes/gateway.json
{ 'enabled_platforms': ['telegram', 'discord', 'slack'], 'telegram': {
      'enabled': true,
      'token': '${TELEGRAM_BOT_TOKEN}',
      'polling_method': 'polling', # polling | webhook
      'polling_interval_ms': 1000
    }, 'discord': { 'enabled': true, 'token': '${DISCORD_BOT_TOKEN}', 'intents': ['GUILD_MESSAGES', 'DIRECT_MESSAGES', 'MESSAGE_CONTENT'] }, 'slack': { 'enabled': true, 'bot_token': '${SLACK_BOT_TOKEN}', 'signing_secret': '${SLACK_SIGNING_SECRET}' }, 'security': { 'default_deny': true, 'allow_pairing': true, 'pairing_code_ttl_seconds': 3600, 'pairing_code_length': 8, 'allowed_users': { 'telegram': ['123456789'], 'discord': ['123456789012345678'], 'slack': ['U123ABC456'] } }, 'reset_by_platform': { 'telegram': { 'mode': 'idle', 'idle_minutes': 240, 'daily_reset_hour': 4 }, 'discord': { 'mode': 'both', 'idle_minutes': 60, 'daily_reset_hour': 4 }, 'slack': { 'mode': 'idle', 'idle_minutes': 1440 } }, 'display': { 'tool_progress': 'all', 'background_process_notifications': 'all', 'typing_indicator': true }, 'cron': { 'enabled': true, 'tick_interval_seconds': 60 } }
```

---

## 总结

### If2Ai Messaging Gateway 特色

✅ **多平台支持** - 15+ 平台的统一接口  
✅ **优雅的架构** - 清晰的 adapter 分离  
✅ **Hermes 对齐** - v0.8.0 特性实现  
✅ **安全优先** - 默认拒绝 + DM pairing  
✅ **可扩展命令** - Plugin system 集成  
✅ **后台任务** - 异步执行 + 自动通知

### 下一步

1. **立即** - 细化 Phase 2 (Telegram/Discord/Slack) 的实现计划
2. **2-4 周** - 实现 Phase 2 核心网关
3. **4-8 周** - 扩展到 Tier 2 平台 + 高级特性

---

**版本历史**:

- v1.0 (2026-04-11) - 初始设计文档，Hermes v0.8.0 对齐

**相关文档**:

- [system-architecture-framework.md](./system-architecture-framework.md) - 系统全景
- [entry-points-design.md](./entry-points-design.md) - Phase 2/3 入口点规划
- [agent-loop.md](./agent-loop.md) - Agent 循环设计
- [session-persistence.md](./session-persistence.md) - Session 存储
