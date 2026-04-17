# ADR-014: Onboarding & Configuration Platform（if2AI）

Status: Accepted
Date: 2026-04
Author: if2AI Architecture

---

# 1. 背景（Context）

if2AI 是一个本地优先的 AI Agent Desktop 应用。

用户首次使用时必须完成以下关键准备：

- 本地运行环境检查
- Embedded 模型下载（保证离线能力）
- 模型 Provider 配置（OpenAI / Ollama / 等）
- 模型选择
- 渠道配置（如 Telegram / Slack / 等）
- Agent 激活并完成第一次聊天

当前问题：

- 无统一 Onboarding 流程
- 配置分散且缺乏验证
- 无法保证"第一次聊天成功"

---

# 2. 决策（Decision）

引入 **Onboarding Platform（平台级模块）**：

Onboarding 不再是 UI Flow，而是：

> 一个由后端状态机驱动的系统初始化流程

---

# 3. 核心架构

## 3.1 状态机（App State Machine）

```rust
pub enum AppState {
    FirstLaunch,
    Onboarding { step: OnboardingStep },
    Ready,
}

pub enum OnboardingStep {
    Welcome,       // step 1
    SystemCheck,   // step 2
    SecurityConfirm, // step 3
    ProviderSetup, // step 4
    ChannelSetup,  // step 5
    Activation,    // step 6
}

impl OnboardingStep {
    pub fn as_index(&self) -> u8 {
        match self {
            Self::Welcome => 1,
            Self::SystemCheck => 2,
            Self::SecurityConfirm => 3,
            Self::ProviderSetup => 4,
            Self::ChannelSetup => 5,
            Self::Activation => 6,
        }
    }

    pub fn from_index(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Welcome),
            2 => Some(Self::SystemCheck),
            3 => Some(Self::SecurityConfirm),
            4 => Some(Self::ProviderSetup),
            5 => Some(Self::ChannelSetup),
            6 => Some(Self::Activation),
            _ => None,
        }
    }
}
```

---

## 3.2 状态流

```
App Start
  ↓
Check ~/.if2ai/state.json
  ↓
if onboarding_completed == false
  → Load current_step
  → if step == 0: Enter Welcome (FirstLaunch)
  → if step > 0: Resume at Onboarding { step }
else
  → Enter Main App (Ready)
```

---

## 3.3 Step 流转规则

| Step            | 类型      | 前置条件             | 完成条件                             | 可回退 |
| --------------- | --------- | -------------------- | ------------------------------------ | ------ |
| Welcome         | UI-only   | 无                   | 用户点击"开始"                       | 否     |
| SystemCheck     | 系统检测  | 无                   | 所有检查通过 + embedded 模型下载完成 | 否     |
| SecurityConfirm | 用户确认  | SystemCheck 完成     | 用户勾选并确认                       | 否     |
| ProviderSetup   | 配置+验证 | SecurityConfirm 完成 | provider + model 测试成功            | 否     |
| ChannelSetup    | 配置+验证 | ProviderSetup 完成   | 至少一个 channel 配置成功            | 否     |
| Activation      | 激活      | 前置步骤全部完成     | 首次聊天消息发送成功                 | 否     |

---

# 4. 后端架构（Rust）

## 4.1 模块目录结构

```
src-tauri/src/
├── commands/
│   ├── mod.rs               # 统一注册 Tauri commands
│   ├── onboarding.rs        # Onboarding 流程控制 commands
│   ├── system_check.rs      # 系统检测 commands
│   ├── provider.rs          # Provider 配置与验证 commands
│   ├── channel.rs           # Channel 配置与验证 commands
│   ├── config.rs            # 统一配置管理 commands
│   └── activation.rs        # Agent 激活 commands
│
├── modules/
│   ├── onboarding/
│   │   ├── mod.rs           # 模块入口
│   │   ├── state.rs         # AppState + OnboardingStep 状态机
│   │   ├── flow.rs          # Step 流转逻辑
│   │   └── validator.rs     # 每步验证规则
│   │
│   ├── config/
│   │   ├── mod.rs           # 模块入口
│   │   ├── service.rs       # ConfigService 实现
│   │   ├── types.rs         # AppConfig / ProviderConfig 等
│   │   └── store.rs         # JSON 文件持久化层
│   │
│   ├── system_check/
│   │   ├── mod.rs
│   │   ├── service.rs       # SystemCheckService
│   │   ├── env.rs           # CPU/GPU/Node.js 检测
│   │   └── model_download.rs # Embedded 模型下载
│   │
│   ├── provider/
│   │   ├── mod.rs
│   │   ├── service.rs       # ProviderService
│   │   ├── registry.rs      # 内置 Provider 注册表
│   │   └── test.rs          # 真实连接测试
│   │
│   ├── channel/
│   │   ├── mod.rs
│   │   ├── service.rs       # ChannelService
│   │   ├── registry.rs      # 内置 Channel 注册表
│   │   └── test.rs          # 真实连接测试
│   │
│   └── activation/
│       ├── mod.rs
│       └── service.rs       # 首次聊天触发
```

---

## 4.2 核心 Trait 定义

### ConfigService

统一管理 `~/.if2ai/` 下的所有配置文件。

```rust
pub trait ConfigService {
    /// 加载完整配置
    fn load_config() -> Result<AppConfig>;

    /// 保存完整配置
    fn save_config(config: &AppConfig) -> Result<()>;

    /// 保存 Provider 配置
    fn save_provider(provider: &ProviderConfig) -> Result<()>;

    /// 保存模型选择
    fn save_model(model: &ModelSelection) -> Result<()>;

    /// 保存渠道配置
    fn save_channel(channel: &ChannelConfig) -> Result<()>;

    /// 验证配置完整性（所有必填字段 + 至少一个 provider + model 已选）
    fn validate_config(config: &AppConfig) -> Result<Vec<String>>;

    /// 重置 Onboarding（删除 state.json + 清空配置）
    fn reset_onboarding() -> Result<()>;
}
```

### SystemCheckService

```rust
pub trait SystemCheckService {
    /// 运行全部系统检测
    fn check_environment() -> Result<SystemReport>;


    /// 下载 Embedded 模型（带进度回调）
    fn download_embedded_model<F>(progress: F) -> Result<()>
    where
        F: Fn(u64, u64);

    /// 检查 Embedded 模型是否已存在
    fn embedded_model_exists() -> bool;
}
```

### ProviderService

```rust
pub trait ProviderService {
    /// 获取所有内置 Provider 列表
    fn list_providers() -> Vec<Provider>;

    /// 根据 provider_id 获取可用模型列表
    fn list_models(provider_id: &str) -> Result<Vec<Model>>;

    /// 配置 Provider（保存 API key / base URL）
    fn configure_provider(config: ProviderConfig) -> Result<()>;

    /// 选择模型
    fn select_model(model_id: &str) -> Result<()>;
}
```

### ChannelService

```rust
pub trait ChannelService {
    /// 获取所有可用渠道列表
    fn list_channels() -> Vec<Channel>;

    /// 配置渠道（保存 token / webhook / secret）
    fn configure_channel(config: ChannelConfig) -> Result<()>;

    /// 获取已配置的渠道列表
    fn list_configured_channels() -> Vec<ChannelConfig>;
}
```

### ConnectivityTestService

**所有测试必须真实执行，禁止 mock。**

```rust
pub trait ConnectivityTestService {
    /// 测试 Provider 连接（真实 API 调用）
    fn test_provider(config: ProviderConfig) -> Result<TestResult>;

    /// 测试模型可用性（发送测试 prompt）
    fn test_model(provider: &ProviderConfig, model_id: &str) -> Result<TestResult>;

    /// 测试 Channel 连接（真实 webhook / API 调用）
    fn test_channel(config: ChannelConfig) -> Result<TestResult>;
}
```

---

## 4.3 核心类型定义

### Onboarding State

```rust
/// 应用全局状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppState {
    FirstLaunch,
    Onboarding { step: u8 },
    Ready,
}

/// Onboarding 状态持久化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub onboarding_completed: bool,
    pub current_step: u8,
    pub completed_steps: Vec<u8>,
}
```

### System Check

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReport {
    pub cpu: CpuInfo,
    pub gpu: GpuInfo,
    pub nodejs: NodeJsInfo,
    pub embedded_model: EmbeddedModelStatus,
    pub overall: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub architecture: String,    // "x86_64" / "aarch64"
    pub cores: u32,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub available: bool,
    pub name: Option<String>,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeJsInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedModelStatus {
    pub downloaded: bool,
    pub progress: Option<f64>,   // 0.0 - 1.0
    pub size_mb: u64,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Fail(String),
    Running,
    Pending,
}
```

### Provider & Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,              // "ollama" / "openai" / "zhipu" / ...
    pub name: String,            // "Ollama (本地)" / "Z.AI (GLM)" / ...
    pub category: ProviderCategory,
    pub status: ProviderStatus,
    pub supports_models: bool,
    pub is_local: bool,
    pub logo_path: Option<String>, // e.g. "src/assets/ProviderLogos/provider_logo_ollama.png"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderCategory {
    Domestic,    // 国内: Z.AI, Moonshot, Qianfan, Xunfei, Volcengine, BytePlus, Baidu
    International, // 国际: OpenAI, Anthropic, Google, OpenRouter
    Local,       // 本地: Ollama
    Custom,      // 自定义
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderStatus {
    Available,
    ApiKeyRequired,
    Unavailable(String), // 原因描述
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    pub modality: ModelModality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelModality {
    Text,
    Vision,
    Multimodal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider_id: String,
    pub model_id: String,
}
```

### Channel

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,              // "feishu" / "qq" / "wechat" / "telegram" / ...
    pub name: String,            // "飞书" / "QQ Bot" / "WeChat" / ...
    pub category: ChannelCategory,
    pub icon: String,
    pub logo_path: Option<String>, // e.g. "src/assets/ChannelLogos/channel_logo_feishu.png"
    pub requires_token: bool,
    pub requires_secret: bool,
    pub requires_webhook: bool,
    pub node_version_required: Option<String>, // e.g. "18+"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelCategory {
    Social,      // 社交渠道: 飞书, QQ, WeChat, Telegram
    Messaging,   // 消息平台: WhatsApp, Teams, Discord, Slack, iMessage, LINE
    Desktop,     // 桌面协作: Signal, Mattermost, Matrix
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_id: String,
    pub bot_token: Option<String>,
    pub app_secret: Option<String>,
    pub webhook_url: Option<String>,
    pub display_name: String,
}
```

### Test Result

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
    pub details: Option<String>,
}
```

### Activation

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationChecklist {
    pub system_check: bool,
    pub security_confirmed: bool,
    pub provider_configured: bool,
    pub channels_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
}
```

### AppConfig（完整配置）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub active_provider: Option<ProviderConfig>,
    pub active_model: Option<ModelSelection>,
    pub channels: Vec<ChannelConfig>,
    pub onboarding: OnboardingState,
    pub security_confirmed: bool,
}
```

---

# 5. Tauri Command 设计

## 5.1 流程控制（onboarding.rs）

```rust
/// 获取当前应用状态
#[tauri::command]
pub fn onboarding_get_state() -> AppState;

/// 进入下一步（后端状态机驱动）
#[tauri::command]
pub fn onboarding_next_step() -> Result<AppState, String>;

/// 返回上一步
#[tauri::command]
pub fn onboarding_prev_step() -> Result<AppState, String>;

/// 标记 Onboarding 完成
#[tauri::command]
pub fn onboarding_complete() -> Result<(), String>;
```

## 5.2 系统检测（system_check.rs）

```rust
/// 运行系统环境检测
#[tauri::command]
pub fn system_check_run() -> Result<SystemReport, String>;

/// 下载 Embedded 模型（前端轮询进度）
#[tauri::command]
pub fn embedded_model_download() -> Result<(), String>;

/// 获取嵌入式模型下载进度
#[tauri::command]
pub fn embedded_model_progress() -> Result<f64, String>;
```

## 5.3 安全确认（onboarding.rs）

```rust
/// 用户确认安全说明
#[tauri::command]
pub fn security_confirm() -> Result<(), String>;
```

## 5.4 Provider 配置（provider.rs）

```rust
/// 获取 Provider 列表
#[tauri::command]
pub fn provider_list() -> Vec<Provider>;

/// 配置 Provider
#[tauri::command]
pub fn provider_configure(config: ProviderConfig) -> Result<(), String>;

/// 测试 Provider 连接
#[tauri::command]
pub fn provider_test(config: ProviderConfig) -> Result<TestResult, String>;

/// 获取 Provider 可用模型列表
#[tauri::command]
pub fn provider_list_models(provider_id: String) -> Result<Vec<Model>, String>;

/// 选择模型
#[tauri::command]
pub fn model_select(provider_id: String, model_id: String) -> Result<(), String>;

/// 测试模型可用性
#[tauri::command]
pub fn model_test(provider_id: String, model_id: String) -> Result<TestResult, String>;
```

## 5.5 Channel 配置（channel.rs）

```rust
/// 获取渠道列表
#[tauri::command]
pub fn channel_list() -> Vec<Channel>;

/// 配置渠道
#[tauri::command]
pub fn channel_configure(config: ChannelConfig) -> Result<(), String>;

/// 测试渠道连接
#[tauri::command]
pub fn channel_test(config: ChannelConfig) -> Result<TestResult, String>;

/// 获取已配置的渠道
#[tauri::command]
pub fn channel_list_configured() -> Vec<ChannelConfig>;
```

## 5.6 激活（activation.rs）

```rust
/// 获取激活检查清单
#[tauri::command]
pub fn activation_get_checklist() -> Result<ActivationChecklist, String>;

/// 唤醒 Agent（首次聊天）
#[tauri::command]
pub fn activation_wake_agent() -> Result<ActivationResult, String>;
```

## 5.7 配置管理（config.rs）

```rust
/// 加载完整配置
#[tauri::command]
pub fn config_load() -> Result<AppConfig, String>;

/// 保存配置
#[tauri::command]
pub fn config_save(config: AppConfig) -> Result<(), String>;

/// 验证配置
#[tauri::command]
pub fn config_validate() -> Result<Vec<String>, String>;

/// 重置 Onboarding
#[tauri::command]
pub fn config_reset_onboarding() -> Result<(), String>;
```

---

# 6. 配置存储（~/.if2ai/）

## 6.1 目录结构

```
~/.if2ai/
├── state.json              # Onboarding 状态
├── config.json             # 完整应用配置
├── providers/
│   └── <provider_id>.json  # 各 Provider 独立配置
├── channels/
│   └── <channel_id>.json   # 各 Channel 独立配置
├── models/
│   └── embedded-rs/        # Embedded 模型目录
├── memory_config.json      # 记忆配置（已有）
└── trajectories/           # 轨迹数据（已有）
```

## 6.2 state.json

```json
{
  "onboarding_completed": false,
  "current_step": 0,
  "completed_steps": [1, 2, 3]
}
```

## 6.3 config.json

```json
{
  "active_provider": {
    "provider_id": "ollama",
    "display_name": "Ollama (本地)",
    "base_url": "http://localhost:11434",
    "api_key": null
  },
  "active_model": {
    "provider_id": "ollama",
    "model_id": "qwen3:4b"
  },
  "channels": [
    {
      "channel_id": "feishu",
      "display_name": "飞书",
      "bot_token": "cli_xxx",
      "app_secret": null,
      "webhook_url": null
    }
  ],
  "onboarding": {
    "onboarding_completed": true,
    "current_step": 6,
    "completed_steps": [1, 2, 3, 4, 5, 6]
  },
  "security_confirmed": true
}
```

## 6.4 Provider 独立配置（providers/<id>.json）

```json
{
  "provider_id": "zai",
  "display_name": "Z.AI (GLM)",
  "api_key": "sk-xxx",
  "base_url": "https://api.z.ai/v1",
  "selected_model": "glm-4.6",
  "configured_at": "2026-04-16T10:00:00Z"
}
```

## 6.5 Channel 独立配置（channels/<id>.json）

```json
{
  "channel_id": "telegram",
  "display_name": "Telegram (Bot API)",
  "bot_token": "123456:ABC-DEF",
  "configured_at": "2026-04-16T10:05:00Z",
  "test_result": {
    "success": true,
    "message": "连接成功",
    "latency_ms": 234
  }
}
```

---

# 7. 前端架构（React + Tauri）

## 7.1 组件目录结构

```
src/
├── modules/
│   └── onboarding/
│       ├── mod.tsx                  # 模块入口
│       ├── OnboardingApp.tsx        # Onboarding 根组件
│       ├── types.ts                 # TypeScript 类型定义
│       ├── hooks/
│       │   └── useOnboarding.ts     # Onboarding 状态管理 Hook
│       ├── components/
│       │   ├── OnboardingLayout.tsx      # 左右分栏布局
│       │   ├── StepHeader.tsx            # 顶部步骤条 + 标题
│       │   ├── StepProgress.tsx          # 步骤进度指示器
│       │   ├── StepNavigation.tsx        # 底部导航（上一步/下一步）
│       │   ├── InfoPanel.tsx             # 右侧橙色信息面板
│       │   ├── FeatureCard.tsx           # 功能特性卡片
│       │   ├── ProviderCard.tsx          # Provider 选择卡片
│       │   ├── ChannelCard.tsx           # Channel 选择卡片
│       │   ├── CheckItem.tsx             # 检测项组件
│       │   ├── DownloadProgress.tsx      # 模型下载进度
│       │   ├── RiskItem.tsx              # 风险说明条目
│       │   ├── ModelSelector.tsx         # 模型选择器
│       │   ├── ActivationChecklist.tsx   # 激活检查清单
│       │   └── SecurityBadge.tsx         # 安全标识
│       └── steps/
│           ├── WelcomeStep.tsx           # Step 1: 欢迎
│           ├── SystemCheckStep.tsx       # Step 2: 系统预检
│           ├── SecurityConfirmStep.tsx   # Step 3: 安全确认
│           ├── ProviderSetupStep.tsx     # Step 4: 模型提供商
│           ├── ChannelSetupStep.tsx      # Step 5: 渠道配置
│           └── ActivationStep.tsx        # Step 6: 唤醒 Agent
```

## 7.2 前端原则

- **不包含业务逻辑** — 所有行为通过 Tauri invoke 调用后端
- **UI 严格遵循设计稿** — 布局、间距、颜色全部使用 design token
- **状态由后端驱动** — 前端只负责展示，不自行判断 step 流转
- **实时反馈** — 每个操作都有 loading / success / error 状态

## 7.3 useOnboarding Hook

```typescript
interface UseOnboardingReturn {
  // 状态
  appState: AppState
  currentStep: number
  completedSteps: number[]

  // 系统检测
  systemReport: SystemReport | null
  isChecking: boolean
  downloadProgress: number
  isDownloading: boolean

  // Provider
  providers: Provider[]
  selectedProvider: Provider | null
  availableModels: Model[]
  selectedModel: Model | null
  providerTestResult: TestResult | null
  modelTestResult: TestResult | null

  // Channel
  channels: Channel[]
  configuredChannels: ChannelConfig[]
  channelTestResult: Record<string, TestResult>

  // 激活
  activationChecklist: ActivationChecklist | null
  activationResult: ActivationResult | null

  // 操作
  runSystemCheck: () => Promise<void>
  downloadEmbeddedModel: () => Promise<void>
  confirmSecurity: () => Promise<void>
  configureProvider: (config: ProviderConfigInput) => Promise<void>
  testProvider: (config: ProviderConfigInput) => Promise<void>
  selectModel: (modelId: string) => Promise<void>
  testModel: (modelId: string) => Promise<void>
  configureChannel: (config: ChannelConfigInput) => Promise<void>
  testChannel: (config: ChannelConfigInput) => Promise<void>
  wakeAgent: () => Promise<void>
  nextStep: () => Promise<void>
  prevStep: () => Promise<void>
}
```

---

# 8. 各 Step 详细设计

## 8.1 Step 1: Welcome（欢迎页）

**设计稿**: [Welcome.png](docs/references/onboarding-steps/Welcome.png)

### 左侧内容区

| 区域            | 内容                                                     |
| --------------- | -------------------------------------------------------- |
| 进度指示        | `1/6` + 6 个步骤编号圆点（当前高亮）                     |
| 标题            | "欢迎来到 UClaw" + 副标题                                |
| 引导文案        | "准备好一只属于你的 AI 小助爪了吗？"                     |
| 信息条          | "向导全程有实时反馈，完成后所有配置数据可在主界面调整。" |
| 统计信息        | "安装方式: 6 步引导" + "预计耗时: 约 3 分钟"             |
| 特性卡片 (4 个) | 无隐私争议、可视化管理、一键安装能力、四选一体验场       |
| CTA             | "开始 6 步安装向导 →"                                    |

### 右侧信息面板（橙色）

- "WELCOME" 标识 + 应用名
- 主标题: "让 UClaw 在几分钟内起飞。"
- 3 条引导要点
- "6 个步骤" 总览列表（带图标）

### 后端交互

- 仅 UI，无后端调用
- 点击 CTA → 调用 `onboarding_next_step()`

---

## 8.2 Step 2: SystemCheck（系统预检）

**设计稿**: [SystemCheck.png](docs/references/onboarding-steps/SystemCheck.png)

### 左侧内容区

| 区域          | 内容                                                       |
| ------------- | ---------------------------------------------------------- |
| 步骤指示      | `2/6` + 步骤进度条                                         |
| 标题          | "先完成系统预检"                                           |
| 说明          | 检查并安装必要的 Embedded 和 Embedding 模型，用于后续检索  |
| Embedded 下载 | 弹出卡片: 模型名 + 进度条 + "检测当前安装状态，已跳过下载" |
| 检查项列表    | CPU/GPU/Node.js 检测结果 + 状态标记                        |
| 底部导航      | "上一步" + "预检完成 →"                                    |

### 右侧信息面板

- "STEP 2: 系统预检"
- "先体检，再安装。"
- 3 条引导要点
- 当前操作卡片: "1/1" + 下载进度

### 后端交互流程

```
1. 组件挂载 → 自动调用 system_check_run()
2. 显示检测中状态 → 轮询 SystemReport
3. 如果 embedded_model.downloaded == false
   → 显示下载 UI + 调用 embedded_model_download()
   → 轮询 embedded_model_progress() 更新进度条
4. 所有检测通过 + 模型下载完成 → 启用"预检完成"按钮
5. 点击"预检完成" → 调用 onboarding_next_step()
```

---

## 8.3 Step 3: SecurityConfirm（安全确认）

**设计稿**: [SecurityConfirm.png](docs/references/onboarding-steps/SecurityConfirm.png)

### 左侧内容区

| 区域            | 内容                                                                                                                                                                                                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 步骤指示        | `3/6` + 步骤进度条                                                                                                                                                                                                                                                                              |
| 标题            | "请确认安全说明"                                                                                                                                                                                                                                                                                |
| 说明            | "AI 配置涉及权限和第三方接入，请仔细阅读以下说明，完成勾选后再继续后续配置。"                                                                                                                                                                                                                   |
| 风险说明 (8 条) | 1. UClaw 默认个人模式运行<br>2. 安装过程在 ~/.openclaw/ 创建文件<br>3. 安装完成后自动启动各服务进程<br>4. 本应用会读取并存储 AI 凭据<br>5. 通讯 Bot 配置需要 App ID、App Secret 等敏感权限<br>6. UClaw 需要 Node.js 18+ 环境<br>7. 首次安装可能需要额外权限<br>8. (可选) 我已阅读并理解以上说明 |
| 底部导航        | "上一步" + "确认并继续 →"                                                                                                                                                                                                                                                                       |

### 右侧信息面板

- "STEP 3: 安全确认"
- "先讲清规则，再开跑。"
- 安全摘要:
  - 已确认，继续 → 权限已确认，操作可继续
  - 权限安全审查
  - 开发者通道
  - 可控权限，零滥用
  - 可控部署

### 后端交互流程

```
1. 用户勾选所有必选项（第 8 项"我已阅读"为确认开关）
2. 点击"确认并继续" → 调用 security_confirm()
3. 后端将 security_confirmed = true 写入 config.json
4. 调用 onboarding_next_step() 进入下一步
```

---

## 8.4 Step 4: ProviderSetup（模型提供商）

**设计稿**: [ProviderSetup.png](docs/references/onboarding-steps/ProviderSetup.png)

### 左侧内容区

| 区域                     | 内容                                                                                                                                                                                                                                                                                                  |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 步骤指示                 | `4/6` + 步骤进度条                                                                                                                                                                                                                                                                                    |
| 标题                     | "选定默认模型来源。"                                                                                                                                                                                                                                                                                  |
| 说明                     | 选择后可随时在"设置 > 模型"中更改或添加新服务商，现在也可以跳过。                                                                                                                                                                                                                                     |
| Provider 卡片网格 (4 列) | **国内来源**: Z.AI (GLM) ✅, Moonshot (Ximi), Qwen (阿里云), MiniMax, Qianfan (百度), Xiaomi AI, Volcano Engine, BytePlus, Ollama (本地) ⭐, Custom (自定义)<br>**国际来源**: OpenAI ⭐, Anthropic (Claude), Google (Gemini), OpenRouter<br>每个卡片显示: 名称 + 状态标签 (API Key / 测试通过 / 推荐) |
| 底部导航                 | "上一步" + "再配置 →"                                                                                                                                                                                                                                                                                 |

### 右侧信息面板

- "STEP 4: 模型服务商"
- "选定默认模型来源。"
- 已选 Provider 详情: 名称 + 状态 + API Key 提示
- 模型列表 (可搜索): 当前 Provider 下可用模型列表 + 选择器
- 底部: "已选择 1 个模型"

### 后端交互流程

```
1. 组件挂载 → 调用 provider_list() 获取所有 Provider
2. 用户点击 Provider 卡片:
   a. 如果是 Ollama (本地) → 检查本地 Ollama 是否运行
      → 调用 provider_test({ provider_id: "ollama" }) 验证连接
      → 成功后调用 provider_list_models("ollama") 获取模型列表
   b. 如果是云 Provider → 弹出 API Key 输入框
      → 调用 provider_test({ provider_id, api_key }) 验证连接
      → 成功后保存: provider_configure(config)
      → 调用 provider_list_models(provider_id) 获取模型列表
3. 用户选择模型 → model_select(provider_id, model_id)
4. 测试模型 → model_test(provider_id, model_id)
5. 全部通过 → 启用"再配置"按钮 → onboarding_next_step()
```

### Provider 注册表（内置）

```rust
pub fn builtin_providers() -> Vec<Provider> {
    vec![
        // 国内来源
        Provider { id: "zai", name: "Z.AI (GLM)", category: Domestic, status: ApiKeyRequired, logo_path: None, ... },
        Provider { id: "moonshot", name: "Moonshot (Ximi)", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_moonshot.png"), ... },
        Provider { id: "qwen", name: "Qwen (阿里云)", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_aliyun.png"), ... },
        Provider { id: "minimax", name: "MiniMax", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_minimax.png"), ... },
        Provider { id: "qianfan", name: "Qianfan (百度)", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_baidu.png"), ... },
        Provider { id: "xiaomi", name: "Xiaomi AI", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_xiaomi.png"), ... },
        Provider { id: "volcengine", name: "Volcano Engine", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_volcengine.png"), ... },
        Provider { id: "byteplus", name: "BytePlus", category: Domestic, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_byteplus.png"), ... },
        Provider { id: "ollama", name: "Ollama (本地)", category: Local, status: Available, is_local: true, logo_path: Some("src/assets/ProviderLogos/provider_logo_ollama.png"), ... },
        // 国际来源
        Provider { id: "openai", name: "OpenAI", category: International, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_openai.png"), ... },
        Provider { id: "anthropic", name: "Anthropic (Claude)", category: International, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_anthropic.png"), ... },
        Provider { id: "google", name: "Google (Gemini)", category: International, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_google.png"), ... },
        Provider { id: "openrouter", name: "OpenRouter", category: International, status: ApiKeyRequired, logo_path: Some("src/assets/ProviderLogos/provider_logo_openrouter.png"), ... },
        // 自定义（无 logo）
        Provider { id: "custom", name: "Custom (自定义)", category: Custom, status: Available, logo_path: None, ... },
    ]
}
```

### Provider Logo 映射表

| Provider ID  | Logo 文件名                    | 有/无                      |
| ------------ | ------------------------------ | -------------------------- |
| `zai`        | —                              | ❌ 无（使用文字 fallback） |
| `moonshot`   | `provider_logo_moonshot.png`   | ✅                         |
| `qwen`       | `provider_logo_aliyun.png`     | ✅                         |
| `minimax`    | `provider_logo_minimax.png`    | ✅                         |
| `qianfan`    | `provider_logo_baidu.png`      | ✅                         |
| `xiaomi`     | `provider_logo_xiaomi.png`     | ✅                         |
| `volcengine` | `provider_logo_volcengine.png` | ✅                         |
| `byteplus`   | `provider_logo_byteplus.png`   | ✅                         |
| `ollama`     | `provider_logo_ollama.png`     | ✅                         |
| `openai`     | `provider_logo_openai.png`     | ✅                         |
| `anthropic`  | `provider_logo_anthropic.png`  | ✅                         |
| `google`     | `provider_logo_google.png`     | ✅                         |
| `openrouter` | `provider_logo_openrouter.png` | ✅                         |

> 额外可用但未在 ADR 中使用的 Provider Logo：`cloudflare`, `huggingface`, `mistral`, `opencode`, `vercel`, `xai`

---

## 8.5 Step 5: ChannelSetup（渠道配置）

**设计稿**: [ChannelSetup.png](docs/references/onboarding-steps/ChannelSetup.png)

### 左侧内容区

| 区域                | 内容                                                                                                                                                                         |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 步骤指示            | `5/6` + 步骤进度条                                                                                                                                                           |
| 标题                | "接入社交渠道"                                                                                                                                                               |
| 说明                | 选择希望接入的渠道，确保每个"设置 > 渠道"都能正确验证。                                                                                                                      |
| 常用渠道 (3 列卡片) | Feishu/Lark (飞书) ✅, QQ Bot, WeChat (微信), Telegram (Bot API)                                                                                                             |
| 其他渠道 (4 列卡片) | WhatsApp (QR link), Microsoft Teams, Discord (Bot API), Slack (Socket Mode), iMessage (imsg), LINE (Messaging API), Signal (signal-cli), Mattermost (plugin), Matrix (glgnt) |
| 底部导航            | "上一步" + "完成部署 →"                                                                                                                                                      |

### 右侧信息面板

- "STEP 5: 通讯渠道"
- "把 UClaw 接到你常用的平台。"
- 已选渠道列表 (卡片): 渠道名 + 连接状态
- 点击渠道卡片 → 弹出配置表单 (Bot Token / App Secret / Webhook URL)
- 配置后自动调用 channel_test() 验证

### Channel 注册表（内置）

```rust
pub fn builtin_channels() -> Vec<Channel> {
    vec![
        // 社交渠道
        Channel { id: "feishu", name: "飞书", category: Social, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_feishu.png"), ... },
        Channel { id: "qq", name: "QQ Bot", category: Social, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_qq.png"), ... },
        Channel { id: "wechat", name: "WeChat (微信)", category: Social, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_wechat.png"), ... },
        Channel { id: "telegram", name: "Telegram (Bot API)", category: Social, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_telegram.png"), ... },
        // 消息平台
        Channel { id: "whatsapp", name: "WhatsApp (QR link)", category: Messaging, requires_token: false, logo_path: Some("src/assets/ChannelLogos/channel_logo_whatsapp.png"), ... },
        Channel { id: "teams", name: "Microsoft Teams (Bot Framework)", category: Messaging, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_msteams.png"), ... },
        Channel { id: "discord", name: "Discord (Bot API)", category: Messaging, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_discord.png"), ... },
        Channel { id: "slack", name: "Slack (Socket Mode)", category: Messaging, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_slack.png"), ... },
        Channel { id: "imessage", name: "iMessage (imsg)", category: Messaging, requires_token: false, logo_path: Some("src/assets/ChannelLogos/channel_logo_imessage.png"), ... },
        Channel { id: "line", name: "LINE (Messaging API)", category: Messaging, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_line.png"), ... },
        // 桌面协作
        Channel { id: "signal", name: "Signal (signal-cli)", category: Desktop, requires_token: false, logo_path: Some("src/assets/ChannelLogos/channel_logo_signal.png"), ... },
        Channel { id: "mattermost", name: "Mattermost (plugin)", category: Desktop, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_mattermost.png"), ... },
        Channel { id: "matrix", name: "Matrix (glgnt)", category: Desktop, requires_token: true, logo_path: Some("src/assets/ChannelLogos/channel_logo_matrix.png"), ... },
    ]
}
```

### Channel Logo 映射表

| Channel ID   | Logo 文件名                   | 有/无 |
| ------------ | ----------------------------- | ----- |
| `feishu`     | `channel_logo_feishu.png`     | ✅    |
| `qq`         | `channel_logo_qq.png`         | ✅    |
| `wechat`     | `channel_logo_wechat.png`     | ✅    |
| `telegram`   | `channel_logo_telegram.png`   | ✅    |
| `whatsapp`   | `channel_logo_whatsapp.png`   | ✅    |
| `teams`      | `channel_logo_msteams.png`    | ✅    |
| `discord`    | `channel_logo_discord.png`    | ✅    |
| `slack`      | `channel_logo_slack.png`      | ✅    |
| `imessage`   | `channel_logo_imessage.png`   | ✅    |
| `line`       | `channel_logo_line.png`       | ✅    |
| `signal`     | `channel_logo_signal.png`     | ✅    |
| `mattermost` | `channel_logo_mattermost.png` | ✅    |
| `matrix`     | `channel_logo_matrix.png`     | ✅    |

---

### 后端交互流程

```
1. 组件挂载 → 调用 channel_list() 获取所有渠道
2. 用户点击渠道卡片 → 弹出配置表单
3. 填写 Bot Token / App Secret / Webhook URL
4. 点击"保存并测试" → channel_configure(config) → channel_test(config)
5. 测试成功后显示绿色状态标记
6. 至少配置一个渠道后 → 启用"完成部署"按钮 → onboarding_next_step()
```

---

## 8.6 Step 6: Activation（唤醒 Agent）

**设计稿**: [Activation.png](docs/references/onboarding-steps/Activation.png)

### 左侧内容区

| 区域         | 内容                                                                                              |
| ------------ | ------------------------------------------------------------------------------------------------- |
| 步骤指示     | `6/6` + 步骤进度条（全部绿色）                                                                    |
| 标题         | "唤醒 UClaw"                                                                                      |
| 说明         | "完成所有步骤的设置，现在点击唤醒，让 UClaw Agent 上线。"                                         |
| 配置汇总清单 | ✓ 系统环境 (通过)<br>✓ 安全确认 (完成)<br>✓ 模型服务商 (Ollama 1s)<br>✓ 通讯渠道 (飞书 1个已验证) |
| 唤醒按钮     | "+ 唤醒 UClaw" (大按钮)                                                                           |
| 底部导航     | "上一步"                                                                                          |

### 右侧信息面板

- "STEP 6: 激活"
- "唤醒 UClaw，并完成渠道配对。"
- 完成状态卡片: "一切就绪，等你唤醒！"
  - 健康状态: 所有指标正常
  - 多端连接: 已连接飞书
  - 可定制 Agent: 已配置模型
  - 持续进化: 记忆系统就绪

### 后端交互流程

```
1. 组件挂载 → 调用 activation_get_checklist() 获取完成状态
2. 检查清单全部 ✓ → 启用"唤醒 UClaw"按钮
3. 用户点击"唤醒 UClaw" → 调用 activation_wake_agent()
4. 后端:
   a. 验证配置完整性 (config_validate)
   b. 启动 Agent Runtime
   c. 发送首次测试消息
   d. 等待响应
   e. 如果成功 → 设置 onboarding_completed = true
   f. 保存最终配置
5. 前端收到成功响应 → 显示完成动画 → 跳转到主界面
```

---

# 9. UI 设计约束（必须遵守）

## 9.0 设计稿参考（最高优先级）

⚠️ **每个 Step 的 UI 实现必须严格对照设计稿图片，不允许自行发挥。**

| Step | 设计稿路径 | 说明 |
|------|-----------|------|
| Step 1: Welcome | `docs/references/onboarding-steps/Welcome.png` | 欢迎页 — 6 步引导总览 |
| Step 2: SystemCheck | `docs/references/onboarding-steps/SystemCheck.png` | 系统预检 — Embedded 模型下载进度 |
| Step 3: SecurityConfirm | `docs/references/onboarding-steps/SecurityConfirm.png` | 安全确认 — 8 条风险说明 + 勾选 |
| Step 4: ProviderSetup | `docs/references/onboarding-steps/ProviderSetup.png` | 模型提供商 — Provider 卡片网格 + 模型选择 |
| Step 5: ChannelSetup | `docs/references/onboarding-steps/ChannelSetup.png` | 渠道配置 — 社交/消息/桌面三类渠道卡片 |
| Step 6: Activation | `docs/references/onboarding-steps/Activation.png` | 唤醒 Agent — 配置汇总清单 + 唤醒按钮 |

**前端实现时必须遵守**：

1. **实现前必须逐像素对照设计稿** — 布局结构、组件位置、间距比例、颜色、字体大小、圆角、阴影全部与设计稿一致
2. **左右分栏布局不可改动** — 左侧操作区 (70%) 白色背景，右侧状态面板 (30%) 橙色渐变
3. **每个 Step 的右侧面板内容不同** — 必须严格按照对应图片中的右侧面板文案、图标、卡片结构实现
4. **顶部进度条 + 底部导航栏** — 每张图片中都有，结构一致但内容不同
5. **组件复用原则** — 相同 UI 模式（卡片、按钮、输入框）在所有 Step 中保持视觉一致
6. **禁止"大致差不多"** — 设计稿中有的元素必须实现，设计稿中没有的元素不添加

**设计稿缺失时的处理**：

- 如果设计稿中某处细节不够清晰，优先查阅 [Paico UI 设计系统](docs/references/Paico%20UI/) 中的 design token
- 如果设计稿中没有的交互状态（hover / focus / disabled），遵循 Paico 组件库的默认行为
- 如果设计稿中有但未实现的元素（如 Loading spinner），使用 Paico 组件库的对应组件

---

## 9.1 Layout

- **左栏**: 操作区 (70%) — 白色背景，信息密集 + 操作控件
- **右栏**: 状态反馈区 (30%) — 橙色渐变背景 (#FF6B35 → #FF4500)
- **固定 6-step flow** — 不可增删步骤
- **顶部导航栏**: 步骤进度条 (1/6 ~ 6/6) + 6 个圆点指示器
- **底部导航栏**: "上一步" (左侧) + CTA 按钮 (右侧)

## 9.2 视觉原则

- 左侧：信息密集 + 操作
- 右侧：情绪 + 状态 + 引导
- CTA：唯一高权重按钮（橙色主色）
- Icon：必须轻（stroke=1.5, 16px）
- 卡片圆角：8px
- 间距：使用 design token，禁止硬编码

## 9.2.1 Logo 资产使用规范

**资产路径**：

| 类型          | 路径                                           | 数量  |
| ------------- | ---------------------------------------------- | ----- |
| Provider Logo | `src/assets/ProviderLogos/provider_logo_*.png` | 18 个 |
| Channel Logo  | `src/assets/ChannelLogos/channel_logo_*.png`   | 13 个 |

**前端引用方式**：

```tsx
// Provider 卡片 Logo
<img src="/assets/ProviderLogos/provider_logo_ollama.png" alt="Ollama" />

// Channel 卡片 Logo
<img src="/assets/ChannelLogos/channel_logo_feishu.png" alt="飞书" />
```

**Logo 使用规则**：

- Logo 尺寸：卡片中 24x24px，右侧面板中 32x32px
- Logo 背景：白色圆角矩形容器（圆角 6px），带 1px 浅灰边框
- 无 Logo 的 Provider（如 `zai`、`custom`）：使用首字母缩写 + 品牌色文字 fallback
- Logo 加载失败：显示 1px 虚线边框占位符 + 文字名称
- 禁止对 Logo 做 CSS filter / 变色处理（保持原始品牌色）
- 右侧橙色面板中：Logo 背景容器使用白色底（与橙色背景形成对比）

## 9.3 禁止事项

- 改动布局结构
- 改动间距 token
- 改动层级
- 使用非 token 颜色
- 左侧白色区域使用橙色
- 右侧橙色区域使用白色背景控件

---

# 10. App 生命周期

## 10.1 启动流程

```
App Launch
  ↓
Tauri setup: 读取 ~/.if2ai/state.json
  ↓
if onboarding_completed == false
  → AppState::Onboarding { step: current_step }
  → 前端渲染 OnboardingApp
  → 禁止打开 Settings
  → 禁止进入主界面
else
  → AppState::Ready
  → 前端渲染 Main App
```

## 10.2 Onboarding 期间限制

- 禁止打开 Settings 面板
- 禁止进入主界面（聊天窗口）
- 必须按顺序完成流程（不可跳过步骤）
- 可以回退到上一步（但不可跳过前置步骤）

## 10.3 完成后

- 进入主界面
- Settings 可修改配置
- 可通过 Settings → "重置 Onboarding" 重新开始

---

# 11. Reset Onboarding

Settings 的"关于我们"页面提供：

```
重置 Onboarding
  → 调用 config_reset_onboarding()
  → 后端删除 ~/.if2ai/state.json 中 onboarding 相关数据
  → 后端清空 config.json 中的配置
  → 前端收到确认后重启应用
  → 重新进入 Onboarding Step 1
```

---

# 12. 错误处理

## 12.1 网络错误

| 场景                     | 处理                            |
| ------------------------ | ------------------------------- |
| Provider API 测试失败    | 显示红色错误提示 + 重试按钮     |
| Channel webhook 测试失败 | 显示具体错误信息 + 配置修正建议 |
| Embedded 模型下载失败    | 显示重试按钮 + 手动下载链接     |

## 12.2 系统检测失败

| 场景                      | 处理                      |
| ------------------------- | ------------------------- |
| Node.js 未安装            | 显示安装指南链接          |
| GPU 不可用                | 降级为 CPU 模式，继续流程 |
| Embedded 模型下载空间不足 | 显示磁盘空间警告          |

## 12.3 激活失败

| 场景                | 处理                |
| ------------------- | ------------------- |
| 首次聊天超时        | 显示超时提示 + 重试 |
| Provider 配置不完整 | 回退到 Step 4       |
| Channel 配置不完整  | 回退到 Step 5       |

---

# 13. 安全性

## 13.1 凭据存储

- API Key / Bot Token 等敏感信息存储在 `~/.if2ai/` 下的独立 JSON 文件
- 文件权限设置为 0600（仅当前用户可读写）
- 不在日志中输出敏感信息

## 13.2 安全确认

- Step 3 强制用户阅读并确认安全说明
- 未经确认无法进入配置步骤

---

# 14. 风险与后果

## 优点

- 保证首次可用
- 配置标准化
- UX 极大提升
- 后端驱动状态机，前端只做展示，架构清晰

## 风险

- 实现复杂度高
- 后端责任增加
- 状态同步复杂
- 需要支持多种 Provider 和 Channel 的适配

---

# 15. 与现有代码兼容性

## 15.1 现有配置系统全景

Codebase 中存在 **三套** 配置文件路径，Onboarding 必须与它们共存：

| 路径 | 用途 | 负责模块 | 写入者 |
|------|------|----------|--------|
| `~/.claude/settings.json` | Claude Code 兼容层 — env 变量回退读取 | `claw_provider.rs` | 用户 / Claude Code |
| `~/.claude/settings.json` | Claw 主配置 — model / permission / MCP / hooks | `runtime/config.rs` | ConfigLoader |
| `~/.claude/settings.local.json` | 项目级局部配置 | `runtime/config.rs` | 用户 |
| `~/.if2ai/memory_config.json` | 记忆 Token 预算配置 | `commands/settings.rs` | Onboarding / Settings |
| `~/.if2ai/trajectories/` | 轨迹数据 | `commands/settings.rs` | Agent Runtime |

## 15.2 兼容性设计原则

### 原则 1：Onboarding 写入 `~/.if2ai/config.json`，同时桥接 `~/.claude/settings.json`

```
Onboarding ConfigService.save_config()
  ├→ 写入 ~/.if2ai/config.json     ← 主配置（Onboarding 原生格式）
  └→ 桥接写入 ~/.claude/settings.json ← 兼容层（RuntimeConfig 可读取的格式）
```

桥接映射规则：

| Onboarding 字段 | `~/.claude/settings.json` 映射 |
|----------------|------------------------------|
| `active_provider.api_key` | `env.ANTHROPIC_API_KEY`（ClawApi）或 `env.OPENAI_API_KEY`（OpenAI 兼容） |
| `active_provider.base_url` | `env.ANTHROPIC_BASE_URL` 或 `env.OPENAI_BASE_URL` |
| `active_model.model_id` | `model` |
| `security_confirmed` | 不桥接（仅 Onboarding 内部使用） |

### 原则 2：保留 `~/.claude/settings.json` 回退读取链

现有 `claw_provider.rs` 中的 `read_claude_settings_env_non_empty()` 继续有效：

```rust
// claw_provider.rs 现有逻辑 — 不改动
fn read_claude_settings_env_non_empty(key: &str) -> Result<Option<String>, ApiError> {
    // 读取 ~/.claude/settings.json 中的 env 字段
    // 支持 ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL, ANTHROPIC_MODEL
}
```

Onboarding **不修改**此文件，仅作为只读回退源。如果用户之前在 Claude Code 中配置过模型凭据，Onboarding 可以**读取并预填充** Provider 配置表单。

### 原则 3：复用现有 Provider 抽象

```rust
// api/providers/mod.rs — 现有代码
pub trait Provider {
    fn send_message<'a>(&'a self, request: &'a MessageRequest)
        -> ProviderFuture<'a, MessageResponse>;
    fn stream_message<'a>(&'a self, request: &'a MessageRequest)
        -> ProviderFuture<'a, Self::Stream>;
}

// api/providers/manager.rs — 现有 ProviderManager
pub struct ProviderManager { /* ... */ }
```

Onboarding 的 `ConnectivityTestService` **直接使用**现有的 `ClawApiClient` 和 `OpenAiCompatClient` 做真实连接测试：

```rust
// 测试 ClawApi (Anthropic) Provider
use crate::modules::api::ClawApiClient;
let client = ClawApiClient::new(config.api_key.clone())
    .with_base_url(config.base_url.clone_or_default());
client.send_message(&test_request).await?;

// 测试 OpenAI 兼容 Provider
use crate::modules::api::OpenAiCompatClient;
let config = OpenAiCompatConfig {
    api_key: config.api_key.clone(),
    base_url: config.base_url.clone(),
    model: model_id.clone(),
};
let client = OpenAiCompatClient::new(config);
client.send_message(&test_request).await?;
```

### 原则 4：模型 ID 兼容 `MODEL_REGISTRY`

```rust
// api/providers/mod.rs — 现有模型注册表
const MODEL_REGISTRY: &[(&str, ProviderMetadata)] = &[
    ("opus", ProviderMetadata { provider: ProviderKind::ClawApi, ... }),
    ("sonnet", ProviderMetadata { provider: ProviderKind::ClawApi, ... }),
    ("haiku", ProviderMetadata { provider: ProviderKind::ClawApi, ... }),
    ("claude-opus-4-6", ...),
    ("grok", ProviderMetadata { provider: ProviderKind::Xai, ... }),
    // ...
];

pub fn resolve_model_alias(model: &str) -> String { /* ... */ }
```

Onboarding 的模型选择必须通过 `resolve_model_alias()` 规范化：

```rust
// Onboarding 保存模型时
use crate::modules::api::providers::resolve_model_alias;
let canonical_model = resolve_model_alias(&model_selection.model_id);
// 存入 config.json 和桥接到 settings.json
```

### 原则 5：复用现有 `~/.if2ai/` 数据

```
~/.if2ai/
├── memory_config.json    ← 已有（commands/settings.rs 读写）— Onboarding 不改动
├── trajectories/         ← 已有（commands/settings.rs 读写）— Onboarding 不改动
├── state.json            ← Onboarding 新增
├── config.json           ← Onboarding 新增
├── providers/            ← Onboarding 新增
├── channels/             ← Onboarding 新增
└── models/               ← Onboarding 新增（embedded 模型）
```

## 15.3 ConfigService 桥接实现

```rust
// modules/config/service.rs
use crate::modules::runtime::config::{ConfigLoader, RuntimeConfig};
use crate::modules::api::providers::{resolve_model_alias, metadata_for_model, ProviderKind};

impl ConfigService for If2AiConfigService {
    fn save_config(config: &AppConfig) -> Result<()> {
        // 1. 写入 Onboarding 原生配置
        write_if2ai_config(config)?;

        // 2. 桥接到 ~/.claude/settings.json
        if let Some(provider) = &config.active_provider {
            if let Some(model) = &config.active_model {
                let metadata = metadata_for_model(&model.model_id);
                let env_key = match metadata.map(|m| m.provider) {
                    Some(ProviderKind::ClawApi) => ("ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL"),
                    Some(ProviderKind::Xai) => ("XAI_API_KEY", "XAI_BASE_URL"),
                    Some(ProviderKind::OpenAi) => ("OPENAI_API_KEY", "OPENAI_BASE_URL"),
                    _ => return Ok(()), // 未知 provider，跳过桥接
                };

                let mut settings = read_claw_settings()?;
                if let Some(api_key) = &provider.api_key {
                    settings.env.insert(env_key.0.to_string(), api_key.clone());
                }
                if let Some(base_url) = &provider.base_url {
                    settings.env.insert(env_key.1.to_string(), base_url.clone());
                }
                let canonical_model = resolve_model_alias(&model.model_id);
                settings.env.insert("model".to_string(), canonical_model);
                write_claw_settings(&settings)?;
            }
        }
        Ok(())
    }
}
```

## 15.4 兼容性测试

Onboarding 完成后，必须验证以下场景：

| 测试场景 | 验证方式 |
|----------|----------|
| Onboarding 配置后，`RuntimeConfig::load()` 能正确读取 model | 加载 `~/.claude/settings.json` 验证 `model` 字段 |
| Onboarding 配置 Anthropic API Key 后，`ClawApiClient::from_env()` 可用 | 调用 `send_message` 测试 |
| 用户之前在 `~/.claude/settings.json` 中配过 API Key，Onboarding 预填充 | 读取并显示在 Provider 表单中 |
| Onboarding 不破坏已有 `memory_config.json` 和 `trajectories/` | 文件校验和不变 |
| `CLAW_DISABLE_CLAUDE_SETTINGS_FALLBACK=1` 时，Onboarding 仍正常工作 | 环境变量隔离测试 |

---

# 16. UClaw 参考实现

## ⚠️ 配置文件路径隔离（最高优先级）

> **if2AI 的所有配置文件必须写入 `~/.if2ai/`，绝不允许写入 `~/.uclaw/`。**
>
> UClaw 参考代码中出现的 `~/.uclaw/` 路径**仅用于展示其数据结构和字段设计**，
> 实现时**必须将路径替换为 `~/.if2ai/`**：
>
> | UClaw 参考路径 | if2AI 实际路径 |
> |---------------|---------------|
> | `~/.uclaw/config.json` | `~/.if2ai/config.json` |
> | `~/.uclaw/providers.yaml` | `~/.if2ai/providers.yaml` |
> | `~/.uclaw/models.json` | `~/.if2ai/models.json` |
> | `~/.uclaw/auth.json` | `~/.if2ai/auth.json` |
> | `~/.uclaw/channels-config.json` | `~/.if2ai/channels-config.json` |
> | `~/.uclaw/state.json` | `~/.if2ai/state.json` |
>
> **验证方法**：代码中搜索 `~/.uclaw` 或 `".uclaw"` — 如果找到任何匹配，说明路径写错了，必须立即修正。

## 16.1 参考代码库位置

> ⚠️ 以下 UClaw 项目是 if2AI Onboarding 的**参考实现来源**，实现时应直接对照其代码逻辑，但**所有后端服务必须用 Rust 实现，不允许使用 Node.js。**

| 用途 | UClaw 项目路径 | if2AI 对应模块 |
|------|---------------|---------------|
| Provider 配置 + 模型获取 + 连接测试 | `uclaw-rs/src/api/routes/providers.rs` | `src-tauri/src/modules/provider/` |
| Channel 配置 + 连接测试（5 平台） | `uclaw-rs/src/api/routes/channels.rs` | `src-tauri/src/modules/channel/` |
| Channel 配置持久化 | `uclaw-rs/src/channels/config.rs` | `src-tauri/src/modules/channel/config.rs` |
| Channel Manager（动态启停） | `uclaw-rs/src/channels/manager.rs` | `src-tauri/src/modules/channel/manager.rs` |
| ConfigLoader（providers.yaml / models.json / auth.json） | `uclaw-rs/src/llm/config_loader.rs` | `src-tauri/src/modules/config/loader.rs` |
| Runtime Settings 加载 | `uclaw-rs/src/config/settings.rs` | `src-tauri/src/modules/runtime/config.rs`（已有） |
| Auth Profile 轮换 | `uclaw-rs/src/config/auth_profile.rs` | `src-tauri/src/modules/config/auth.rs` |

## 16.2 Provider 配置流程参考（必须对照实现）

参考：`uclaw-rs/src/api/routes/providers.rs`

### 核心实现要点

```
前端: 选择 Provider → 输入 API Key + Base URL → 点击"测试"
       ↓
后端: POST /api/providers/test  → 调用 `{base_url}/models` 验证连接
       ↓
      根据 provider 类型分发：
       ├─ Ollama: 特殊处理 → GET `{base_url}/api/tags`（不使用 /models）
       │            自动追加 `/v1` 后缀，api_key = "local"
       ├─ Anthropic: 不走 /models，从内置 registry 返回模型列表
       │            使用 `anthropic-messages` API 协议
       └─ OpenAI 兼容: GET `{base_url}/models` + Bearer Token 认证
             ↓
      测试成功 → persist_provider_files() → persist_models_file()
                写入 providers.yaml + auth.json + models.json
```

### 持久化三文件结构（参考 UClaw 方案）

> 📌 **UClaw 路径**：`~/.uclaw/`（仅供参考字段结构）
> **if2AI 路径**：`~/.if2ai/`（实际写入位置）

```
~/.if2ai/
├── providers.yaml    # Provider 定义（base_url, api, api_key）
├── models.json       # 模型注册表（pi-coding-agent ModelsConfigSchema 兼容格式）
└── auth.json         # 各 Provider 的 API Key（覆盖 providers.yaml）
```

```yaml
# providers.yaml
providers:
  ollama:
    api_key: local
    base_url: http://localhost:11434/v1
    api: openai-completions
  openai:
    api_key: sk-xxx
    base_url: https://api.openai.com/v1
    api: openai-completions
```

```json
// models.json — 兼容 pi-coding-agent ModelsConfigSchema
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "local",
      "models": [
        { "id": "qwen3:4b", "name": "qwen3:4b", "input": ["text", "image"], "contextWindow": 128000 }
      ]
    }
  }
}
```

```json
// auth.json
{
  "openai": { "apiKey": "sk-xxx" },
  "ollama": { "apiKey": "local" }
}
```

### 连接测试错误码规范（直接复用）

| 错误码 | HTTP 状态 | 含义 |
|--------|-----------|------|
| `provider_config_invalid` | 400 | 配置不完整（缺 base_url / api_key） |
| `provider_auth_error` | 401/403 | API Key 无效 |
| `provider_timeout` | 408/504 | 连接超时 |
| `provider_network_error` | 0 | 网络不可达 |
| `provider_upstream_unavailable` | 500-599 | 上游服务不可用 |

## 16.3 Channel 配置流程参考（必须对照实现）

参考：`uclaw-rs/src/api/routes/channels.rs`

### 已实现的 5 平台连接测试

| 平台 | 测试 API | 验证方式 |
|------|----------|----------|
| Telegram | `https://api.telegram.org/bot{token}/getMe` | 返回 bot 名称 |
| Feishu | `https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal` | 返回 access_token |
| QQ | `https://bots.qq.com/app/getAppAccessToken` + `@me` | 返回 username |
| WhatsApp | `https://graph.facebook.com/v17.0/{phone_id}` | 返回 phone number |
| MS Teams | `https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token` | 返回 access_token |

### Channel 配置持久化

> 📌 **UClaw 路径**：`~/.uclaw/channels-config.json`（仅供参考字段结构）
> **if2AI 路径**：`~/.if2ai/channels-config.json`（实际写入位置）

```json
{
  "enabled": true,
  "platforms": {
    "telegram": {
      "enabled": true,
      "bot_token": "123456:ABC-DEF",
      "webhook_secret": "xxx",
      "bot_username": "@my_bot"
    },
    "feishu": {
      "enabled": true,
      "app_id": "cli_xxx",
      "app_secret": "xxx",
      "encrypt_key": "xxx",
      "verification_token": "xxx"
    }
  },
  "routing": {
    "owner_user_ids": { "telegram": ["user_123"] },
    "default_agent_id": "default",
    "debounce_ms": 300,
    "rate_limit_per_minute": 20
  },
  "public_awareness": ""
}
```

### 动态启停机制（ChannelManager）

```rust
// 配置保存后自动执行：
// 1. 如果平台被禁用 → stop_platform() 停止轮询
// 2. 如果平台已启用 → 构建适配器 → register() → start_platform_polling()
// 支持的适配器：telegram, feishu, qq, slack, discord, whatsapp, msteams,
//               signal, irc, email, dingtalk, wechat, matrix
```

## 16.4 Embedded 模型下载参考

UClaw 未直接实现 Embedded 模型下载，但 `uclaw-rs/src/llm/config_loader.rs` 中的 `ConfigLoader` 展示了从文件加载 provider 和模型注册表的标准模式。if2AI 的 Embedded 模型下载应参考此模式：

```rust
// 参考 ConfigLoader 的加载模式
// 1. 模型文件存放在 ~/.if2ai/models/embedded-rs/
// 2. 下载进度通过回调上报（progress: Fn(u64, u64)）
// 3. 下载完成后更新 state.json 中的 embedded_model 状态
// 4. 使用 tokio + reqwest 实现异步下载（禁止 Node.js）
```

## 16.5 后端技术栈强制约束

```
✅ 允许: Rust (Tauri backend)
❌ 禁止: Node.js / Python / 任何 sidecar 进程

所有 Onboarding 后端服务必须满足：
├── HTTP 请求: reqwest (Rust crate)
├── JSON 处理: serde / serde_json (Rust crate)
├── YAML 处理: serde_yaml (Rust crate)
├── 文件 I/O: std::fs (Rust std)
├── 异步运行时: tokio (Rust crate)
└── 进程管理: 禁止调用 Node.js 子进程
```

UClaw 的 `sidecar` 架构（Node.js sidecar 进程）在 if2AI 中**完全弃用**。所有逻辑必须在 Rust 后端进程中直接执行。

## 16.6 实现对照清单

实现每个 Onboarding 后端模块时，必须对照 UClaw 参考代码：

| if2AI 模块 | 必须对照的 UClaw 文件 | 核心复用逻辑 |
|------------|----------------------|-------------|
| `modules/provider/test.rs` | `api/routes/providers.rs:test_provider` | 连接测试 + 错误码分类 |
| `modules/provider/registry.rs` | `api/routes/providers.rs:fetch_models` | Ollama / Anthropic / OpenAI 三种协议适配 |
| `modules/provider/service.rs` | `api/routes/providers.rs:persist_provider_files` | providers.yaml + auth.json + models.json 三文件持久化 |
| `modules/channel/test.rs` | `api/routes/channels.rs:test_platform` | 5 平台真实 API 测试 |
| `modules/channel/service.rs` | `api/routes/channels.rs:put_channel_config` | 平台凭据保存 + 动态启停 |
| `modules/channel/config.rs` | `channels/config.rs:ChannelsConfig` | PlatformConfig 结构定义 |
| `modules/channel/manager.rs` | `channels/manager.rs:ChannelManager` | 适配器轮询管理 |
| `modules/config/loader.rs` | `llm/config_loader.rs:ConfigLoader` | providers.yaml + models.json + auth.json 加载 |

---

# 17. 未来扩展

- 多 Profile 支持（不同工作空间的独立配置）
- 云同步配置（加密后跨设备同步）
- 插件式 Provider（社区贡献的 Provider 插件）
- 动态 Channel Marketplace（渠道插件市场）
- Onboarding 步骤可配置化（通过 JSON 定义步骤流）

---

# 18. 架构完整性补充（Architectural Completeness Gaps）

> 本节为 ADR 审核阶段识别的架构缺失项，按优先级分级（P0 = 必须在本 Phase 实现，P1/P2 = 可后续完善）。

## 18.1 [P0] ChannelAdapter Trait 定义

**问题**：ADR 第 4.2 节定义了 4 个 trait，但 Channel 的底层适配器 trait 完全缺失。UClaw 参考代码中 `ChannelAdapter` trait + `ConnectMode`（Polling/Webhook）是动态启停的核心。

**补充定义**（应添加到第 4.2 节 `ChannelService` 之后）：

```rust
/// Channel 连接模式
pub enum ConnectMode {
    /// 轮询拉取（Telegram、Feishu、QQ 等）
    Polling,
    /// Webhook 回调（Slack Socket Mode、Discord 等）
    Webhook,
}

/// Channel 底层协议适配器
/// 每个平台（Telegram/Feishu/QQ/...）必须实现此 trait。
pub trait ChannelAdapter: Send + Sync {
    /// 平台唯一标识，如 "telegram"、"feishu"
    fn platform_id(&self) -> &str;

    /// 连接模式（Polling / Webhook）
    fn connect_mode(&self) -> ConnectMode;

    /// 真实连接测试
    fn test_connection(&self, config: &PlatformConfig) -> Result<TestResult>;

    /// 构建渠道配置
    fn build_config(&self, creds: serde_json::Value) -> Result<ChannelConfig>;

    /// 轮询模式运行（仅 Polling 模式需要实现）
    /// on_message: 每条消息的回调函数
    /// cancel: CancellationToken 用于停止轮询
    async fn run_polling<F>(&self, on_message: F, cancel: CancellationToken)
    where
        F: Fn(ChannelEnvelope) + Send + Sync;
}

/// Channel 消息信封 — 统一各平台的消息格式
pub struct ChannelEnvelope {
    pub channel: String,        // 平台 ID
    pub chat_id: String,        // 会话 ID
    pub sender: SenderInfo,
    pub raw: serde_json::Value, // 原始消息 JSON
}

pub struct SenderInfo {
    pub id: String,
    pub display_name: String,
    pub username: Option<String>,
}

/// 平台配置（ChannelAdapter 使用，区别于 ChannelConfig 的 Onboarding 层级）
pub struct PlatformConfig {
    pub bot_token: Option<String>,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub webhook_secret: Option<String>,
    pub webhook_url: Option<String>,
    pub access_token: Option<String>,
    pub phone_number_id: Option<String>,
    pub app_id_teams: Option<String>,
    pub app_password: Option<String>,
    pub bridge_url: Option<String>,
    pub bridge_secret: Option<String>,
    pub encrypt_key: Option<String>,
    pub verification_token: Option<String>,
    pub bot_username: Option<String>,
}
```

**ChannelManager 职责**（参考 UClaw `channels/manager.rs`）：

```rust
/// Channel 生命周期管理器 — 负责所有平台的动态启停
pub struct ChannelManager {
    adapters: DashMap<String, Arc<dyn ChannelAdapter>>,
    tasks: DashMap<String, (JoinHandle<()>, CancellationToken)>,
}

impl ChannelManager {
    /// 启动所有 Polling 模式的平台
    pub fn start_all_polling(&self);
    /// 启动/重启单个平台轮询
    pub fn start_platform_polling(&self, platform: &str, adapter: Arc<dyn ChannelAdapter>);
    /// 停止单个平台轮询
    pub fn stop_platform(&self, platform: &str);
    /// 全部停止（应用关闭时调用）
    pub fn stop_all(&self);
}
```

## 18.2 [P0] 配置持久化双模式统一

**问题**：ADR 第 6 节定义了 `config.json` 作为主配置，但第 16.2 节引入了 `providers.yaml` + `models.json` + `auth.json` 三文件结构。两套持久化机制并存，关系不明确。

**补充规范**：

```
┌─────────────────────────────────────────────────────────────┐
│                    ConfigService::save_config()              │
├───────────────────────┬─────────────────────────────────────┤
│ Layer 1: 快捷配置      │ Layer 2: 规范配置（Runtime 读取）    │
│ ~/.if2ai/config.json  │ ~/.if2ai/providers.yaml              │
│ (Onboarding UI 格式)   │ ~/.if2ai/auth.json                  │
│                       │ ~/.if2ai/models.json                 │
├───────────────────────┼─────────────────────────────────────┤
│ 读取优先: Layer 1      │ 读取优先: Layer 2                    │
│ 写入: 同时维护两套      │ 写入: ConfigService 内部同步维护      │
└───────────────────────┴─────────────────────────────────────┘
```

**关系定义**：

| 维度 | config.json (Layer 1) | 三文件 (Layer 2) |
|------|----------------------|-----------------|
| 目的 | Onboarding UI 快捷读写 | Runtime 规范格式（pi-coding-agent ModelsConfigSchema 兼容） |
| 写入者 | `ConfigService::save_config()` | `ConfigService::save_config()` 内部同步写入 |
| 读取者 | `ConfigService::load_config()` | `ConfigLoader`（已有 `runtime/config.rs`） |
| 格式 | 单一 JSON，扁平结构 | YAML + JSON，分层结构 |
| 冲突解决 | Layer 1 为用户最终选择 | Layer 2 为权威源，`ConfigLoader` 以 Layer 2 为准 |

**同步保证**：

```rust
impl ConfigService for If2AiConfigService {
    fn save_config(config: &AppConfig) -> Result<()> {
        // 1. 写入 Layer 1（config.json）
        write_if2ai_config(config)?;

        // 2. 同步写入 Layer 2（三文件）
        sync_to_triple_files(config)?;

        // 3. 桥接到 ~/.claude/settings.json（兼容层）
        bridge_to_claw_settings(config)?;

        Ok(())
    }
}
```

## 18.3 [P0] Provider List Models 签名修正

**问题**：ADR 第 5.4 节 `provider_list_models(provider_id: String)` 签名缺少 `base_url` 和 `api_key` 参数。从 UClaw 参考代码（`providers.rs:fetch_models`）可以看到三种协议差异：

| Provider 类型 | 接口 | 需要 API Key | 需要 base_url |
|--------------|------|-------------|--------------|
| Ollama | `GET /api/tags` | 否（用 "local" 占位） | 是 |
| Anthropic | 内置 registry | 是（测试连接时用） | 是 |
| OpenAI 兼容 | `GET /models` | 是 | 是 |

**修正签名**：

```rust
/// 获取 Provider 可用模型列表
#[tauri::command]
pub fn provider_list_models(
    provider_id: String,
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<Model>, String>;
```

**对应前端 useOnboarding Hook 也应修正**：

```typescript
listModels: (providerId: string, baseUrl: string, apiKey?: string) => Promise<Model[]>
```

## 18.4 [P0] 橙色面板 WCAG 对比度修复

**问题**：右侧面板使用 `#FF6B35 → #FF4500` 渐变。白色文字 `#FFFFFF` 对 `#FF4500` 的对比度约为 **2.8:1**，**不符合 WCAG AA 标准（要求 4.5:1）**。

**修复方案**：

| 场景 | 背景色 | 文字色 | 对比度 | 状态 |
|------|--------|--------|--------|------|
| Light Mode 面板 | `#CC4400`（加深橙色） | `#FFFFFF` | 4.6:1 | ✅ WCAG AA |
| Light Mode 面板（替代） | `#FF4500`（原渐变） | `#1A0A00`（深棕） | 5.8:1 | ✅ WCAG AA |
| 按钮/高亮元素 | `#FF6B35`（保持不变） | `#FFFFFF` | 3.0:1 | ⚠️ 仅装饰性 |

**Design Token 更新**：

```css
/* paico-design-tokens/colors.css */
--onboarding-panel-bg-start: #CC4400;  /* 加深起点 */
--onboarding-panel-bg-end: #993300;    /* 加深终点 */
--onboarding-panel-text: #FFFFFF;
--onboarding-card-bg: rgba(255, 255, 255, 0.95); /* 面板内卡片 */
--onboarding-card-text: #1A1A1A;
```

## 18.5 [P1] 中断恢复状态（Error Recovery State）

**问题**：当前 `OnboardingState` 只有 `completed_steps`，没有记录每步的中间态和失败原因。断网/关闭应用后，已输入的 API Key 等配置丢失。

**补充类型**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub onboarding_completed: bool,
    pub current_step: u8,
    pub completed_steps: Vec<u8>,
    // ── 中断恢复字段 ──
    /// 每步的中间态草稿（断点续传用）
    pub draft_configs: HashMap<u8, serde_json::Value>,
    /// 最近一次失败记录
    pub last_failure: Option<OnboardingFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingFailure {
    pub step: u8,
    pub error: String,
    pub error_code: Option<String>,   // 如 "provider_timeout", "provider_auth_error"
    pub retriable: bool,
    pub occurred_at: String,          // ISO 8601
}
```

**state.json 示例**：

```json
{
  "onboarding_completed": false,
  "current_step": 4,
  "completed_steps": [1, 2, 3],
  "draft_configs": {
    "4": {
      "selected_provider": "ollama",
      "base_url": "http://localhost:11434",
      "api_key": null
    },
    "5": [
      {
        "channel_id": "telegram",
        "bot_token": "123456:ABC-",
        "test_result": null
      }
    ]
  },
  "last_failure": {
    "step": 4,
    "error": "连接超时，请检查 Ollama 是否正在运行",
    "error_code": "provider_timeout",
    "retriable": true,
    "occurred_at": "2026-04-16T10:30:00Z"
  }
}
```

**恢复策略**：

| 场景 | 恢复行为 |
|------|---------|
| 用户在 Step 4 输入了 API Key 但未测试 | 预填充表单，保留草稿 |
| 模型下载到 80% 时断网 | 显示"上次下载中断在 80%，点击继续" |
| 渠道测试超时后关闭应用 | 显示失败原因 + 重试按钮 |
| 所有步骤完成后标记 completed | 清除 draft_configs（节省磁盘） |

## 18.6 [P1] Tauri Event 推送机制

**问题**：ADR 中前端通过"轮询"获取下载进度（`embedded_model_progress()`），效率低且缺少实时性。

**补充事件定义**：

```rust
// 后端推送事件
const ONBOARDING_STEP_CHANGED: &str = "onboarding://step_changed";
const ONBOARDING_DOWNLOAD_PROGRESS: &str = "onboarding://download_progress";
const ONBOARDING_TEST_COMPLETED: &str = "onboarding://test_completed";
const ONBOARDING_ERROR: &str = "onboarding://error";
```

**事件负载**：

```typescript
// 前端监听
interface OnboardingEvents {
  'onboarding://step_changed': {
    from_step: number;
    to_step: number;
  };
  'onboarding://download_progress': {
    model_name: string;
    downloaded_bytes: number;
    total_bytes: number;
    percent: number;  // 0-100
  };
  'onboarding://test_completed': {
    test_type: 'provider' | 'channel' | 'model';
    target_id: string;
    success: boolean;
    latency_ms?: number;
    error?: string;
  };
  'onboarding://error': {
    step: number;
    code: string;
    message: string;
    retriable: boolean;
  };
}
```

**使用方式**：

```typescript
// useOnboarding Hook 中注册监听
useEffect(() => {
  const unlistenProgress = await listen('onboarding://download_progress', (event) => {
    setDownloadProgress(event.payload);
  });
  return () => unlistenProgress();
}, []);
```

**替代方案**：在 Tauri 2 中，如果 Event 系统不可用，可回退到轮询模式（每 500ms 调用一次 `embedded_model_progress()`）。

## 18.7 [P1] Channel 凭证脱敏（Redaction）

**问题**：ADR 第 5.5 节 `channel_list_configured()` 返回 `Vec<ChannelConfig>`（含明文 token），存在安全风险。

**补充脱敏类型**：

```rust
/// Channel 配置脱敏版本（返回前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfigRedacted {
    pub channel_id: String,
    pub display_name: String,
    /// 是否有 bot token（不暴露实际值）
    pub has_bot_token: bool,
    /// 是否有 app secret
    pub has_app_secret: bool,
    /// 是否有 access token
    pub has_access_token: bool,
    /// 非敏感字段完整返回
    pub webhook_url: Option<String>,
    pub bot_username: Option<String>,
    pub app_id: Option<String>,
    pub phone_number_id: Option<String>,
    /// 上次测试结果（不含敏感信息）
    pub last_test_result: Option<TestResult>,
}

impl ChannelConfig {
    /// 脱敏转换
    pub fn redact(&self) -> ChannelConfigRedacted {
        ChannelConfigRedacted {
            channel_id: self.channel_id.clone(),
            display_name: self.display_name.clone(),
            has_bot_token: self.bot_token.is_some(),
            has_app_secret: self.app_secret.is_some(),
            has_access_token: self.webhook_url.is_some(),
            webhook_url: self.webhook_url.clone(),
            bot_username: None, // 从测试响应中获取
            app_id: None,
            phone_number_id: None,
            last_test_result: None,
        }
    }
}
```

**修正 command 签名**：

```rust
/// 获取已配置的渠道（脱敏版，返回前端安全）
#[tauri::command]
pub fn channel_list_configured() -> Vec<ChannelConfigRedacted>;
```

## 18.8 [P1] Channel Routing 默认值

**问题**：UClaw channels config 中有 `routing` 字段（owner_user_ids, default_agent_id, debounce_ms, rate_limit_per_minute），ADR 未提及。

**补充规范**：Onboarding Step 5 不要求用户配置 routing，但保存渠道配置时必须写入默认值：

```rust
/// Channel Routing 默认配置
pub fn default_routing() -> ChannelRouting {
    ChannelRouting {
        owner_user_ids: HashMap::new(),  // 用户后续在 Settings 中配置
        default_agent_id: "default".to_string(),
        debounce_ms: Some(300),
        rate_limit_per_minute: Some(20),
    }
}
```

**写入时机**：首次配置任何渠道时，自动写入 `~/.if2ai/channels-config.json` 中的 `routing` 字段。

## 18.9 [P1] activation_wake_agent() 职责拆分

**问题**：第 8.6 节后端交互流程列出了 5 个子步骤（验证配置 → 启动 Runtime → 发消息 → 等响应 → 标记完成），单一 command 职责过载。

**修正拆分**：

```rust
/// 验证激活前置条件
#[tauri::command]
pub fn activation_validate() -> Result<ActivationChecklist, String>;

/// 启动 Agent Runtime（启动 channel 轮询 + 初始化 memory）
#[tauri::command]
pub fn activation_start() -> Result<ActivationResult, String>;

/// 发送测试消息并等待响应（复用已有 start_agent_stream）
#[tauri::command]
pub fn activation_test_message() -> Result<TestResult, String>;

/// 标记 Onboarding 完成（写 state + 清除草稿）
#[tauri::command]
pub fn activation_complete() -> Result<(), String>;
```

**前端调用流程**：

```
用户点击"唤醒 UClaw"
  ↓ activation_validate()   → 验证清单全部 ✓
  ↓ activation_start()      → 启动 runtime
  ↓ activation_test_message() → 发送 "Hello" 并等待回复
  ↓ activation_complete()   → 标记 onboarding_completed = true
  ↓ 跳转到主界面
```

## 18.10 [P1] "UClaw" → "if2AI" 品牌统一

**问题**：ADR 第 8 节各 Step 的文案大量使用 "UClaw"，与产品名称不一致。

**修正对照表**：

| 原文案 | 修正为 |
|--------|--------|
| "欢迎来到 UClaw" | "欢迎来到 if2AI" |
| "准备好一只属于你的 AI 小助爪了吗？" | "准备好你的 AI 助手了吗？" |
| "让 UClaw 在几分钟内起飞。" | "让 if2AI 在几分钟内就绪。" |
| "唤醒 UClaw" | "唤醒 if2AI Agent" |
| "把 UClaw 接到你常用的平台。" | "把你常用的平台接入 if2AI。" |
| 右侧面板 "WELCOME" 应用名 | "if2AI" |

**注意**：右侧面板的引导文案（如 "先讲清规则，再开跑"、"先体检，再安装"）风格可从 UClaw 参考，但应针对 if2AI 品牌调性重新编写。

## 18.11 [P1] 异步操作 Loading / Error 状态视觉规范

**问题**：ADR 第 7.4 节仅说"每个操作都有 loading/success/error 状态"，但未定义具体视觉表现。

**补充规范**：

| 操作 | Loading 状态 | Success 状态 | Error 状态 |
|------|-------------|-------------|-----------|
| **Provider 连接测试** | 按钮内 loading spinner + 按钮 disabled | 按钮变为绿色 ✓ + "连接成功" 文字 1.5s | 按钮下方红色 inline text + 重试按钮 |
| **模型下载** | 进度条 + "已下载 X MB / 共 Y MB" + "正在下载..." | 进度条满格 + "下载完成" + ✓ 标记 | 进度条变红 + "下载失败" + 重试按钮 + 手动下载链接 |
| **渠道连接测试** | 卡片内 loading spinner（右上角） | 卡片边框变绿 + 绿色 ✓ 状态标记 | 卡片边框变红 + 具体错误信息 + 修正建议 |
| **系统预检** | 每行显示 "检测中..." + 旋转圆点 | 该行变绿 + ✓ | 该行变红 + ✗ + 原因 + 解决链接 |
| **唤醒 Agent** | 全屏 overlay + 旋转动画 + "正在唤醒 Agent..." | overlay 淡出 + 绿色 ✓ + "已就绪" | overlay 显示错误详情 + 重试按钮 |

**Error 视觉层次**：

| 错误级别 | 表现方式 | 阻断性 |
|---------|---------|--------|
| 表单验证错误（API Key 为空） | 输入框下方红色 inline text | 阻止提交 |
| 网络错误（Provider 超时） | 卡片内错误提示 + Toast 通知 + 重试按钮 | 阻止下一步 |
| 系统检测失败（Node.js 缺失） | 检测项行内警告 + 安装指南链接 | 可选择继续或中止 |
| 致命错误（磁盘空间不足） | Modal 弹窗阻断 + 清理建议 | 阻断流程 |

## 18.12 [P1] Dark Mode 设计规范

**问题**：ADR 第 9.2 节禁止"左侧白色区域使用橙色"，但未定义 Dark Mode 下的视觉表现。

**补充规范**：

| 元素 | Light Mode | Dark Mode |
|------|-----------|-----------|
| 左侧操作区背景 | `#FFFFFF` | `#1A1A1A` |
| 左侧操作区文字 | `#1A1A1A` | `#E0E0E0` |
| 右侧面板渐变 | `#FF6B35 → #FF4500` | `#CC4400 → #882200`（降低饱和度） |
| 右侧面板文字 | `#FFFFFF` | `#FFE0D0`（暖白，确保对比度） |
| 卡片背景 | `#FFFFFF` + 1px `#E0E0E0` 边框 | `#2A2A2A` + 1px `#3A3A3A` 边框 |
| 输入框背景 | `#F5F5F5` | `#2A2A2A` |
| 成功标记（绿色） | `#22C55E` | `#4ADE80` |
| 错误标记（红色） | `#EF4444` | `#F87171` |
| 主 CTA 按钮 | `#FF6B35` 背景 + `#FFFFFF` 文字 | `#FF8855` 背景 + `#1A1A1A` 文字 |
| 焦点环 | `#3B82F6` 2px | `#60A5FA` 2px |

**Dark Mode 切换时机**：跟随系统主题设置，Onboarding 期间不可手动切换（因为 Settings 面板不可用）。

---

# 19. Phase 实现决策项（Deferred to Implementation）

> 以下 P2 事项在本 Phase 实现时由 executor 根据代码实际情况决策完善，不阻塞 PR merge。

## 19.1 [P2] 配置格式版本化

```json
{
  "version": 1,
  "active_provider": { ... }
}
```

**决策点**：
- 何时需要 version bump：新增字段、删除字段、字段类型变更
- migration 逻辑放在 `ConfigService::load_config()` 中：如果 version < current，执行 upgrade 脚本
- 初始版本定为 `1`

## 19.2 [P2] 并发访问保护

**决策点**：
- 方案 A：`fs2` crate 文件锁（简单，仅防多进程）
- 方案 B：`tokio::sync::Mutex` 包装写入操作（推荐，防同进程并发）
- 方案 C：两者结合（最安全）

推荐方案 B 优先实现，因为 Onboarding 期间用户无法打开 Settings，跨进程并发风险低。

## 19.3 [P2] 步骤间转场动画

**决策点**：
- 前进：slide-left（250ms ease-out）
- 回退：slide-right（250ms ease-out）
- 是否需要 CSS `prefers-reduced-motion` 降级为 instant swap

## 19.4 [P2] 响应式断点

**决策点**：
- 最小窗口：≥ 960×640（低于此尺寸显示 resize 提示）
- ≥ 1200px：Provider 卡片 4 列
- 960–1199px：Provider 卡片 3 列
- 800–959px：Provider 卡片 2 列，右侧面板折叠为顶部 drawer
- < 800px：单栏模式（不推荐，Onboarding 应在 ≥ 800px 下运行）

## 19.5 [P2] 键盘导航规范

**决策点**：
- Tab 顺序：表单输入 → 测试按钮 → 下一步按钮
- Enter 在表单中：不触发测试（避免误触），仅在焦点在按钮上时触发
- Escape：Onboarding 期间禁用（防止意外关闭）
- 焦点环：2px `#3B82F6` outline（Dark Mode 为 `#60A5FA`）

## 19.6 [P2] 微交互规范

| 组件 | Hover | Active | Disabled | Focused |
|------|-------|--------|----------|---------|
| Provider 卡片 | 边框 `#FF6B35` + 2px 阴影 | 下沉 1px (`transform: translateY(1px)`) | 灰度 50% + cursor: not-allowed | 2px `#3B82F6` outline |
| CTA 按钮 | 背景加深 10% | 下沉 2px | 灰度 50% + cursor: not-allowed | 外发光 `#FF6B35` 4px |
| 步骤圆点 | 放大 1.1x (`transform: scale(1.1)`) | — | — | — |
| 输入框 | 边框从 `#E0E0E0` → `#B0B0B0` | — | 背景 `#F0F0F0` + cursor: not-allowed | 2px `#3B82F6` outline |
| Channel 卡片 | 同 Provider 卡片 | 同 Provider 卡片 | 同 Provider 卡片 | 同 Provider 卡片 |

## 19.7 [P2] Onboarding 遥测/漏斗分析

**决策点**：
- 仅在用户 opt-in 后收集
- 数据结构嵌入 `state.json` 的 `analytics` 字段
- 字段：started_at, step_durations, errors
- 不上报外部服务器，仅存储在本地 `~/.if2ai/analytics.json`

---

# 20. 结论

Onboarding 是 if2AI 的"First Run OS"，必须作为平台能力实现，而非 UI 功能。

核心原则：

1. **后端状态机驱动** — 前端不控制 step 流转
2. **真实连接测试** — 禁止 mock 成功
3. **统一配置服务** — 所有配置通过 ConfigService
4. **不可跳过** — 必须完成全部 6 步才能进入主界面
