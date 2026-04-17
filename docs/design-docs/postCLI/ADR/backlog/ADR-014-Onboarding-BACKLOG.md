# ADR-014 Onboarding & Configuration Platform — 详细实施 backlog

## 概述

**目标**: 实现 if2AI 的完整 Onboarding 系统——由后端状态机驱动的 6 步系统初始化流程，涵盖环境检测、安全确认、Provider 配置、Channel 配置、Agent 激活。
**ADR**: [ADR-014](./ADR-014-if2AI-Onboarding.md)
**优先级**: P0 (核心平台能力)
**预估工时**: 8-10 天
**现状**: `src-tauri/src/modules/onboarding/` 目录不存在，需要从零创建；现有 Settings 模块已占用 `src-tauri/src/commands/settings.rs`。

---

## 子任务清单

### TASK-014-01: Onboarding State Machine 核心

**目标**: 实现状态机类型定义、持久化层、状态流驱动逻辑。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/onboarding/mod.rs`
- [ ] 创建 `src-tauri/src/modules/onboarding/state.rs`:
  ```rust
  pub enum AppState { FirstLaunch, Onboarding { step: u8 }, Ready }
  pub enum OnboardingStep { Welcome, SystemCheck, SecurityConfirm, ProviderSetup, ChannelSetup, Activation }
  pub struct OnboardingState {
      pub onboarding_completed: bool,
      pub current_step: u8,
      pub completed_steps: Vec<u8>,
      pub draft_configs: HashMap<u8, serde_json::Value>,
      pub last_failure: Option<OnboardingFailure>,
  }
  ```
- [ ] 创建 `src-tauri/src/modules/onboarding/flow.rs`:
  ```rust
  pub struct OnboardingFlow;
  impl OnboardingFlow {
      pub fn next_step(state: &OnboardingState) -> Result<OnboardingState>;
      pub fn prev_step(state: &OnboardingState) -> Result<OnboardingState>;
      pub fn can_proceed(step: u8, state: &OnboardingState) -> bool;
  }
  ```
- [ ] 创建 `src-tauri/src/modules/onboarding/store.rs` — `~/.if2ai/state.json` 读写
- [ ] `OnboardingStep::as_index()` / `from_index()` 方法
- [ ] AppState 序列化/反序列化
- [ ] 编写单元测试

**验收标准**:
- [ ] `OnboardingState` 可正确序列化到 `~/.if2ai/state.json`
- [ ] `next_step()` / `prev_step()` 状态转移正确
- [ ] 状态不可越界（step 1-6）

---

### TASK-014-02: ConfigService 双模式持久化

**目标**: 实现 `~/.if2ai/config.json` (Layer 1) + `providers.yaml`/`auth.json`/`models.json` (Layer 2) 双写。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/config/mod.rs`
- [ ] 创建 `src-tauri/src/modules/config/types.rs`:
  ```rust
  pub struct AppConfig {
      pub version: u8,  // = 1
      pub active_provider: Option<ProviderConfig>,
      pub active_model: Option<ModelSelection>,
      pub channels: Vec<ChannelConfig>,
      pub onboarding: OnboardingState,
      pub security_confirmed: bool,
  }
  pub struct ProviderConfig { ... }
  pub struct ChannelConfig { ... }
  pub struct ModelSelection { ... }
  ```
- [ ] 创建 `src-tauri/src/modules/config/service.rs` — `ConfigService` trait 实现:
  ```rust
  impl ConfigService for If2AiConfigService {
      fn save_config() -> 写 Layer 1 + 同步写 Layer 2 + 桥接 ~/.claude/settings.json
      fn load_config() -> 读 Layer 1（带 version migration）
      fn validate_config() -> 校验完整性
      fn reset_onboarding() -> 删除 state.json + 清空 config
  }
  ```
- [ ] 创建 `src-tauri/src/modules/config/store.rs` — JSON 文件读写（带 `tokio::sync::Mutex` 并发保护）
- [ ] 创建 `src-tauri/src/modules/config/bridge.rs` — 桥接到 `~/.claude/settings.json`:
  ```rust
  pub fn bridge_to_claw_settings(config: &AppConfig) -> Result<()>;
  ```
- [ ] 创建 `src-tauri/src/modules/config/triple_files.rs` — `providers.yaml` + `auth.json` + `models.json` 同步写入
- [ ] 文件权限 0600（敏感文件）
- [ ] 编写单元测试

**验收标准**:
- [ ] `save_config()` 同时写入 config.json + 三文件 + 桥接文件
- [ ] `load_config()` 可正确读取并迁移旧版本
- [ ] `reset_onboarding()` 清空所有数据
- [ ] 并发写入不破坏文件

---

### TASK-014-03: SystemCheckService

**目标**: 实现 CPU/GPU/Node.js 检测 + Embedded 模型下载。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/system_check/mod.rs`
- [ ] 创建 `src-tauri/src/modules/system_check/types.rs`:
  ```rust
  pub struct SystemReport {
      pub cpu: CpuInfo, pub gpu: GpuInfo,
      pub nodejs: NodeJsInfo, pub embedded_model: EmbeddedModelStatus,
      pub overall: CheckStatus,
  }
  pub enum CheckStatus { Pass, Fail(String), Running, Pending }
  ```
- [ ] 创建 `src-tauri/src/modules/system_check/env.rs`:
  ```rust
  pub fn detect_cpu() -> CpuInfo;
  pub fn detect_gpu() -> GpuInfo;
  pub fn detect_nodejs() -> NodeJsInfo;
  pub fn run_full_check() -> SystemReport;
  ```
- [ ] 创建 `src-tauri/src/modules/system_check/model_download.rs`:
  ```rust
  pub fn download_embedded_model<F>(progress: F) -> Result<()>
      where F: Fn(u64, u64) + Send + Sync;
  pub fn embedded_model_exists() -> bool;
  pub fn get_download_progress() -> f64;
  ```
- [ ] 模型下载使用 `tokio + reqwest` 异步实现
- [ ] 下载进度通过 `AtomicU64` 存储，支持轮询/Event 读取
- [ ] 模型存储路径 `~/.if2ai/models/embedded-rs/`
- [ ] 编写单元测试

**验收标准**:
- [ ] CPU/GPU/Node.js 检测返回正确结果
- [ ] 模型下载可中断、可续传
- [ ] 下载进度可被前端轮询读取
- [ ] Node.js 缺失时不阻断流程（降级提示）

---

### TASK-014-04: ChannelAdapter Trait + ChannelManager

**目标**: 实现 Channel 适配器 trait + 消息信封 + ChannelManager 动态启停。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/channel/mod.rs`
- [ ] 创建 `src-tauri/src/modules/channel/adapter.rs`:
  ```rust
  pub enum ConnectMode { Polling, Webhook }
  pub trait ChannelAdapter: Send + Sync {
      fn platform_id(&self) -> &str;
      fn connect_mode(&self) -> ConnectMode;
      fn test_connection(&self, config: &PlatformConfig) -> Result<TestResult>;
      fn build_config(&self, creds: serde_json::Value) -> Result<ChannelConfig>;
      async fn run_polling<F>(&self, on_message: F, cancel: CancellationToken)
          where F: Fn(ChannelEnvelope) + Send + Sync;
  }
  pub struct ChannelEnvelope { channel, chat_id, sender, raw }
  pub struct PlatformConfig { ... }
  ```
- [ ] 创建 `src-tauri/src/modules/channel/manager.rs`:
  ```rust
  pub struct ChannelManager {
      adapters: DashMap<String, Arc<dyn ChannelAdapter>>,
      tasks: DashMap<String, (JoinHandle<()>, CancellationToken)>,
  }
  impl ChannelManager {
      pub fn start_all_polling(&self);
      pub fn start_platform_polling(&self, platform: &str, adapter: Arc<dyn ChannelAdapter>);
      pub fn stop_platform(&self, platform: &str);
      pub fn stop_all(&self);
  }
  ```
- [ ] 创建 `src-tauri/src/modules/channel/types.rs`:
  ```rust
  pub struct Channel { id, name, category, icon, logo_path, requires_token, ... }
  pub enum ChannelCategory { Social, Messaging, Desktop }
  pub struct ChannelConfigRedacted { ... }  // 脱敏版
  pub struct ChannelRouting { owner_user_ids, default_agent_id, debounce_ms, rate_limit_per_minute }
  ```
- [ ] 创建 `src-tauri/src/modules/channel/registry.rs` — `builtin_channels()` 注册表（13 个渠道）
- [ ] 编写单元测试

**验收标准**:
- [ ] `ChannelAdapter` trait 可被各平台适配器实现
- [ ] `ChannelManager` 可启动/停止轮询
- [ ] `ChannelConfigRedacted` 不泄露 token

---

### TASK-014-05: ProviderService + Test + Registry

**目标**: 实现 Provider 服务 + 连接测试 + 模型列表获取 + 注册表。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/provider/mod.rs`
- [ ] 创建 `src-tauri/src/modules/provider/types.rs`:
  ```rust
  pub struct Provider { id, name, category, status, supports_models, is_local, logo_path }
  pub enum ProviderCategory { Domestic, International, Local, Custom }
  pub enum ProviderStatus { Available, ApiKeyRequired, Unavailable(String) }
  pub struct Model { id, name, context_window, max_tokens, modality }
  pub enum ModelModality { Text, Vision, Multimodal }
  ```
- [ ] 创建 `src-tauri/src/modules/provider/registry.rs` — `builtin_providers()` 注册表（14 个 provider）
- [ ] 创建 `src-tauri/src/modules/provider/test.rs`:
  ```rust
  pub async fn test_provider_connection(config: ProviderConfig) -> Result<TestResult>;
  pub async fn test_model_availability(config: ProviderConfig, model_id: String) -> Result<TestResult>;
  // Ollama: GET /api/tags
  // Anthropic: 内置 registry
  // OpenAI 兼容: GET /models + Bearer Token
  ```
- [ ] 创建 `src-tauri/src/modules/provider/service.rs`:
  ```rust
  pub fn list_providers() -> Vec<Provider>;
  pub async fn list_models(provider_id, base_url, api_key) -> Result<Vec<Model>>;
  pub async fn configure_provider(config: ProviderConfig) -> Result<()>;
  pub fn select_model(provider_id, model_id) -> Result<()>;
  ```
- [ ] 复用现有 `ClawApiClient` / `OpenAiCompatClient` 做真实连接测试
- [ ] 复用 `MODEL_REGISTRY` + `resolve_model_alias()` 规范化模型 ID
- [ ] 错误码规范：`provider_config_invalid`, `provider_auth_error`, `provider_timeout`, `provider_network_error`, `provider_upstream_unavailable`
- [ ] 编写单元测试

**验收标准**:
- [ ] 14 个 provider 全部在注册表中
- [ ] Ollama/Anthropic/OpenAI 三种协议测试路径正确
- [ ] 错误码分类正确
- [ ] 不破坏现有 `ClawApiClient` / `OpenAiCompatClient`

---

### TASK-014-06: Channel Test Service (5 平台连接测试)

**目标**: 实现 Telegram/Feishu/QQ/WhatsApp/MS Teams 真实连接测试。

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/channel/test.rs`:
  ```rust
  pub async fn test_channel_connection(config: ChannelConfig) -> Result<TestResult>;
  // 内部按平台分发:
  // Telegram: GET https://api.telegram.org/bot{token}/getMe
  // Feishu: POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
  // QQ: POST https://bots.qq.com/app/getAppAccessToken + GET /users/@me
  // WhatsApp: GET https://graph.facebook.com/v17.0/{phone_id}
  // MS Teams: POST https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token
  ```
- [ ] 参考 UClaw `uclaw-rs/src/api/routes/channels.rs:test_platform` 实现
- [ ] 超时设置：8-10 秒
- [ ] 凭据脱敏：日志中不输出完整 token
- [ ] 未实现在线验证的平台返回 "凭证将在唤醒 Agent 后生效"
- [ ] 编写单元测试（mock HTTP 响应）

**验收标准**:
- [ ] 5 平台真实 API 测试正确
- [ ] 凭据验证失败返回具体错误码
- [ ] 超时处理正确

---

### TASK-014-07: Tauri Commands (全部 22+ commands)

**目标**: 实现全部 Tauri IPC 命令并注册到 `commands/mod.rs`。

**具体任务**:
- [ ] 创建 `src-tauri/src/commands/onboarding.rs`:
  ```rust
  #[tauri::command] pub fn onboarding_get_state() -> AppState
  #[tauri::command] pub fn onboarding_next_step() -> Result<AppState, String>
  #[tauri::command] pub fn onboarding_prev_step() -> Result<AppState, String>
  #[tauri::command] pub fn onboarding_complete() -> Result<(), String>
  #[tauri::command] pub fn security_confirm() -> Result<(), String>
  ```
- [ ] 创建 `src-tauri/src/commands/system_check.rs`:
  ```rust
  #[tauri::command] pub fn system_check_run() -> Result<SystemReport, String>
  #[tauri::command] pub fn embedded_model_download() -> Result<(), String>
  #[tauri::command] pub fn embedded_model_progress() -> Result<f64, String>
  ```
- [ ] 创建 `src-tauri/src/commands/provider.rs`:
  ```rust
  #[tauri::command] pub fn provider_list() -> Vec<Provider>
  #[tauri::command] pub fn provider_configure(config: ProviderConfig) -> Result<(), String>
  #[tauri::command] pub fn provider_test(config: ProviderConfig) -> Result<TestResult, String>
  #[tauri::command] pub fn provider_list_models(provider_id: String, base_url: String, api_key: Option<String>) -> Result<Vec<Model>, String>
  #[tauri::command] pub fn model_select(provider_id: String, model_id: String) -> Result<(), String>
  #[tauri::command] pub fn model_test(provider_id: String, model_id: String) -> Result<TestResult, String>
  ```
- [ ] 创建 `src-tauri/src/commands/channel.rs`:
  ```rust
  #[tauri::command] pub fn channel_list() -> Vec<Channel>
  #[tauri::command] pub fn channel_configure(config: ChannelConfig) -> Result<(), String>
  #[tauri::command] pub fn channel_test(config: ChannelConfig) -> Result<TestResult, String>
  #[tauri::command] pub fn channel_list_configured() -> Vec<ChannelConfigRedacted>
  ```
- [ ] 创建 `src-tauri/src/commands/activation.rs`:
  ```rust
  #[tauri::command] pub fn activation_validate() -> Result<ActivationChecklist, String>
  #[tauri::command] pub fn activation_start() -> Result<ActivationResult, String>
  #[tauri::command] pub fn activation_test_message() -> Result<TestResult, String>
  #[tauri::command] pub fn activation_complete() -> Result<(), String>
  ```
- [ ] 创建 `src-tauri/src/commands/config.rs`:
  ```rust
  #[tauri::command] pub fn config_load() -> Result<AppConfig, String>
  #[tauri::command] pub fn config_save(config: AppConfig) -> Result<(), String>
  #[tauri::command] pub fn config_validate() -> Result<Vec<String>, String>
  #[tauri::command] pub fn config_reset_onboarding() -> Result<(), String>
  ```
- [ ] 在 `src-tauri/src/commands/mod.rs` 中导出所有命令
- [ ] 在 `src-tauri/src/main.rs` 的 `invoke_handler!` 中注册所有命令
- [ ] 在 `AppState` 中添加 `onboarding_flow: OnboardingFlow` 引用

**验收标准**:
- [ ] 所有 22+ commands 编译通过
- [ ] 命令签名与 ADR-014 第 5 + 18.3 + 18.9 节一致
- [ ] `channel_list_configured()` 返回脱敏版
- [ ] `provider_list_models` 包含 base_url + api_key 参数

---

### TASK-014-08: Tauri Event 推送机制

**目标**: 实现后端 → 前端的事件推送通道。

**具体任务**:
- [ ] 在 `src-tauri/src/modules/onboarding/events.rs` 中定义事件常量:
  ```rust
  const ONBOARDING_STEP_CHANGED: &str = "onboarding://step_changed";
  const ONBOARDING_DOWNLOAD_PROGRESS: &str = "onboarding://download_progress";
  const ONBOARDING_TEST_COMPLETED: &str = "onboarding://test_completed";
  const ONBOARDING_ERROR: &str = "onboarding://error";
  ```
- [ ] 在关键路径插入 `app.emit_all()` 调用:
  - `onboarding_next_step()` / `onboarding_prev_step()` → `step_changed`
  - `download_embedded_model()` 进度回调 → `download_progress` (每 500ms)
  - `test_provider_connection()` 完成 → `test_completed`
  - 任何错误 → `onboarding_error`
- [ ] 前端 TypeScript 类型定义（`src/modules/onboarding/types.ts`）
- [ ] 降级方案：如果 Event 不可用，保留轮询兼容层

**验收标准**:
- [ ] 前端可监听 `onboarding://download_progress` 并更新进度条
- [ ] 事件负载格式与 ADR-014 第 18.6 节一致
- [ ] 轮询兼容层仍可用

---

### TASK-014-09: 前端 Onboarding 根组件 + Layout

**目标**: 实现 `OnboardingApp.tsx` + 左右分栏布局 + 步骤导航。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/mod.tsx`
- [ ] 创建 `src/modules/onboarding/OnboardingApp.tsx` — 根组件
- [ ] 创建 `src/modules/onboarding/types.ts` — TypeScript 类型:
  ```typescript
  type AppState = { type: 'FirstLaunch' } | { type: 'Onboarding'; step: number } | { type: 'Ready' }
  interface OnboardingState { onboarding_completed: boolean; current_step: number; completed_steps: number[]; ... }
  interface UseOnboardingReturn { ... }
  ```
- [ ] 创建 `src/modules/onboarding/hooks/useOnboarding.ts` — 状态管理 Hook:
  - 调用 Tauri `invoke()` 获取状态
  - 注册 Event 监听
  - 管理 UI 本地状态（selected provider/model/channel）
- [ ] 创建 `src/modules/onboarding/components/OnboardingLayout.tsx` — 左右分栏（70% / 30%）:
  - 左侧：白色背景操作区
  - 右侧：橙色渐变面板（`#CC4400 → #993300`，WCAG AA 合规）
- [ ] 创建 `src/modules/onboarding/components/StepHeader.tsx` — 顶部步骤条
- [ ] 创建 `src/modules/onboarding/components/StepNavigation.tsx` — 底部导航（上一步/下一步）
- [ ] 创建 `src/modules/onboarding/components/InfoPanel.tsx` — 右侧橙色信息面板
- [ ] 创建 `src/modules/onboarding/components/StepProgress.tsx` — 步骤进度指示器
- [ ] Design Token 全部使用 Paico 设计系统
- [ ] 禁止硬编码颜色/间距

**验收标准**:
- [ ] `OnboardingApp` 可根据后端 `AppState` 渲染对应 step
- [ ] 左右分栏比例正确（70/30）
- [ ] 右侧面板橙色符合 WCAG AA 对比度
- [ ] TypeScript 编译通过

---

### TASK-014-10: Step 1 — Welcome（欢迎页）

**目标**: 实现 Welcome 页面组件，严格对照设计稿。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/WelcomeStep.tsx`
- [ ] 左侧内容:
  - 进度指示 `1/6` + 6 个步骤圆点
  - 标题 "欢迎来到 if2AI" + 副标题
  - 引导文案
  - 4 个特性卡片: 无隐私争议、可视化管理、一键安装能力、四选一体验场
  - 统计信息: "安装方式: 6 步引导" + "预计耗时: 约 3 分钟"
  - CTA 按钮 "开始 6 步安装向导 →"
- [ ] 右侧面板:
  - "WELCOME" + "if2AI"
  - 主标题 "让 if2AI 在几分钟内就绪。"
  - 3 条引导要点
  - "6 个步骤" 总览列表（带图标）
- [ ] 后端交互: 点击 CTA → 调用 `onboarding_next_step()`
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/Welcome.png`

**验收标准**:
- [ ] UI 与设计稿一致
- [ ] 点击 CTA 可进入 Step 2
- [ ] 无后端调用

---

### TASK-014-11: Step 2 — SystemCheck（系统预检）

**目标**: 实现系统预检页面 + 模型下载进度。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/SystemCheckStep.tsx`
- [ ] 左侧内容:
  - 步骤指示 `2/6` + 进度条
  - 标题 "先完成系统预检"
  - Embedded 模型下载卡片（带进度条）
  - CPU/GPU/Node.js 检测项列表
  - 底部导航 "上一步" + "预检完成 →"
- [ ] 右侧面板:
  - "STEP 2: 系统预检"
  - "先体检，再安装。"
  - 下载进度卡片
- [ ] 后端交互流程:
  1. 组件挂载 → `system_check_run()`
  2. 轮询/Event 获取 `SystemReport`
  3. 如果模型未下载 → `embedded_model_download()` + 监听 `download_progress`
  4. 全部通过 → 启用"预检完成"按钮
- [ ] Loading 状态: 每行 "检测中..." + 旋转圆点
- [ ] Error 状态: 行内红色 ✗ + 原因 + 解决链接
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/SystemCheck.png`

**验收标准**:
- [ ] 检测项正确展示
- [ ] 模型下载进度实时更新
- [ ] 全部通过后可进入下一步

---

### TASK-014-12: Step 3 — SecurityConfirm（安全确认）

**目标**: 实现安全确认页面 + 勾选逻辑。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/SecurityConfirmStep.tsx`
- [ ] 左侧内容:
  - 步骤指示 `3/6`
  - 标题 "请确认安全说明"
  - 8 条风险说明（使用 `RiskItem` 组件）
  - 第 8 项 "我已阅读并理解以上说明" 为确认开关
  - 底部导航 "上一步" + "确认并继续 →"（勾选全部后启用）
- [ ] 右侧面板:
  - "STEP 3: 安全确认"
  - "先讲清规则，再开跑。"
  - 安全摘要卡片
- [ ] 后端交互:
  1. 用户勾选全部必选项
  2. 点击"确认并继续" → `security_confirm()`
  3. 后端写入 `security_confirmed = true` → `onboarding_next_step()`
- [ ] 风险说明中的 `~/.openclaw/` 替换为 `~/.if2ai/`
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/SecurityConfirm.png`

**验收标准**:
- [ ] 8 条说明全部可勾选
- [ ] 未全部勾选时"确认并继续"按钮禁用
- [ ] 确认后写入后端状态

---

### TASK-014-13: Step 4 — ProviderSetup（模型提供商）

**目标**: 实现 Provider 选择 + 配置 + 测试 + 模型选择页面。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/ProviderSetupStep.tsx`
- [ ] 创建 `src/modules/onboarding/components/ProviderCard.tsx`
- [ ] 创建 `src/modules/onboarding/components/ModelSelector.tsx`
- [ ] 左侧内容:
  - 步骤指示 `4/6`
  - 标题 "选定默认模型来源。"
  - Provider 卡片网格（4 列）: 国内来源 (9 个) + 国际来源 (4 个) + 自定义
  - 每个卡片: Logo + 名称 + 状态标签 (API Key 必需/测试通过/推荐)
  - 点击卡片 → 弹出配置表单 (API Key + Base URL) → "测试连接"按钮
  - 测试成功后 → 显示模型选择器
  - 底部导航 "上一步" + "再配置 →"
- [ ] 右侧面板:
  - "STEP 4: 模型服务商"
  - 已选 Provider 详情 + 模型列表（可搜索）
- [ ] 后端交互流程:
  1. `provider_list()` → 渲染卡片
  2. 用户点击卡片 → 如果是 Ollama 直接测试；如果是云 Provider 弹出 API Key 输入
  3. `provider_test()` → 成功后 `provider_list_models()` → `provider_configure()`
  4. 用户选择模型 → `model_select()` → `model_test()`
- [ ] Loading 状态: 按钮内 spinner + disabled
- [ ] Error 状态: 按钮下方红色 inline text + 重试按钮
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/ProviderSetup.png`

**验收标准**:
- [ ] 14 个 Provider 卡片全部渲染
- [ ] Logo 加载正确（无 Logo 显示文字 fallback）
- [ ] 连接测试真实执行
- [ ] 模型选择后可进入下一步

---

### TASK-014-14: Step 5 — ChannelSetup（渠道配置）

**目标**: 实现渠道选择 + 配置 + 测试页面。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/ChannelSetupStep.tsx`
- [ ] 创建 `src/modules/onboarding/components/ChannelCard.tsx`
- [ ] 左侧内容:
  - 步骤指示 `5/6`
  - 标题 "接入社交渠道"
  - 常用渠道 (3 列): Feishu, QQ, WeChat, Telegram
  - 其他渠道 (4 列): WhatsApp, Teams, Discord, Slack, iMessage, LINE, Signal, Mattermost, Matrix
  - 点击卡片 → 弹出配置表单 (Bot Token / App Secret / Webhook URL) → "保存并测试"
  - 测试成功后显示绿色状态标记
  - 底部导航 "上一步" + "完成部署 →"
- [ ] 右侧面板:
  - "STEP 5: 通讯渠道"
  - 已选渠道列表 + 连接状态
- [ ] 后端交互流程:
  1. `channel_list()` → 渲染卡片
  2. 用户点击卡片 → 填写凭据 → `channel_configure()` → `channel_test()`
  3. 测试成功 → 写入默认 routing 配置
  4. 至少配置一个渠道 → 启用"完成部署"按钮
- [ ] Loading 状态: 卡片右上角 spinner
- [ ] Error 状态: 卡片边框变红 + 具体错误信息
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/ChannelSetup.png`

**验收标准**:
- [ ] 13 个 Channel 卡片全部渲染
- [ ] Logo 加载正确
- [ ] 5 平台真实 API 测试
- [ ] 配置至少一个渠道后可进入下一步

---

### TASK-014-15: Step 6 — Activation（唤醒 Agent）

**目标**: 实现激活页面 + 配置汇总 + 唤醒流程。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/steps/ActivationStep.tsx`
- [ ] 创建 `src/modules/onboarding/components/ActivationChecklist.tsx`
- [ ] 创建 `src/modules/onboarding/components/SecurityBadge.tsx`
- [ ] 左侧内容:
  - 步骤指示 `6/6` + 全部绿色
  - 标题 "唤醒 if2AI Agent"
  - 配置汇总清单:
    - ✓ 系统环境 (通过)
    - ✓ 安全确认 (完成)
    - ✓ 模型服务商 (Ollama 1s)
    - ✓ 通讯渠道 (飞书 1个已验证)
  - 大按钮 "+ 唤醒 if2AI Agent"
  - 底部导航 "上一步"
- [ ] 右侧面板:
  - "STEP 6: 激活"
  - 完成状态卡片 "一切就绪，等你唤醒！"
  - 健康状态 / 多端连接 / 可定制 Agent / 持续进化
- [ ] 后端交互流程:
  1. `activation_validate()` → 验证清单全部 ✓
  2. 用户点击 → `activation_start()` → 启动 runtime
  3. `activation_test_message()` → 发送测试消息
  4. `activation_complete()` → 标记 `onboarding_completed = true`
  5. 完成动画 → 跳转到主界面
- [ ] Loading 状态: 全屏 overlay + 旋转动画
- [ ] Error 状态: overlay 内错误详情 + 重试按钮
- [ ] 严格对照设计稿 `docs/references/onboarding-steps/Activation.png`

**验收标准**:
- [ ] 汇总清单正确展示
- [ ] 唤醒流程 4 步全部执行
- [ ] 成功后跳转到主界面

---

### TASK-014-16: App 启动集成 + Onboarding 入口

**目标**: 在 Tauri 启动流程中集成 Onboarding 状态检查。

**具体任务**:
- [ ] 修改 `src-tauri/src/main.rs` 的 `setup` 函数:
  ```rust
  fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
      let state = read_onboarding_state();
      if state.onboarding_completed {
          // Enter Ready state
      } else {
          // Enter Onboarding state
      }
      // ... existing setup ...
  }
  ```
- [ ] 修改 `src-tauri/src/commands/mod.rs` 的 `AppState` 结构:
  ```rust
  pub struct AppState {
      // ... existing fields ...
      pub onboarding_flow: Arc<OnboardingFlow>,
  }
  ```
- [ ] 在 `main.rs` 中根据 `AppState` 决定渲染 OnboardingApp 还是 Main App
- [ ] Onboarding 期间禁止打开 Settings 面板
- [ ] Onboarding 期间禁止进入主界面（聊天窗口）
- [ ] 编写集成测试

**验收标准**:
- [ ] 首次启动自动进入 Onboarding
- [ ] 已完成 Onboarding 后自动进入主界面
- [ ] Onboarding 期间 Settings 不可打开

---

### TASK-014-17: Reset Onboarding + 兼容性验证

**目标**: 实现重置 Onboarding 功能 + 验证与现有代码兼容性。

**具体任务**:
- [ ] 在 Settings "关于我们" 页面添加 "重置 Onboarding" 按钮
- [ ] 调用 `config_reset_onboarding()` → 删除 `state.json` + 清空 `config.json`
- [ ] 验证兼容性（5 个场景）:
  - Onboarding 配置后 `RuntimeConfig::load()` 可正确读取 model
  - Onboarding 配置 Anthropic API Key 后 `ClawApiClient::from_env()` 可用
  - 用户之前在 `~/.claude/settings.json` 中配过 API Key，Onboarding 预填充
  - Onboarding 不破坏已有 `memory_config.json` 和 `trajectories/`
  - `CLAW_DISABLE_CLAUDE_SETTINGS_FALLBACK=1` 时 Onboarding 仍正常工作
- [ ] 编写兼容性测试

**验收标准**:
- [ ] 重置后重新进入 Onboarding Step 1
- [ ] 所有 5 个兼容性场景验证通过

---

### TASK-014-18: 全局 UI 组件 + 微交互 + Dark Mode

**目标**: 实现通用 UI 组件 + Dark Mode 适配。

**具体任务**:
- [ ] 创建 `src/modules/onboarding/components/FeatureCard.tsx` — 特性卡片
- [ ] 创建 `src/modules/onboarding/components/CheckItem.tsx` — 检测项
- [ ] 创建 `src/modules/onboarding/components/DownloadProgress.tsx` — 下载进度
- [ ] 创建 `src/modules/onboarding/components/RiskItem.tsx` — 风险说明条目
- [ ] Dark Mode 适配:
  - 左侧背景 `#1A1A1A` / 右侧面板渐变 `#CC4400 → #882200`
  - 所有颜色使用 CSS 变量 / Tailwind dark: 前缀
  - 跟随系统主题
- [ ] 微交互:
  - Provider/Channel 卡片 hover/active/disabled/focused 状态
  - CTA 按钮 hover/active/disabled 状态
  - 步骤圆点 hover 放大 1.1x
  - 焦点环 2px outline
- [ ] 可访问性:
  - 状态标记使用图标（✓ ✗ ⏳）而非仅颜色
  - 键盘 Tab 导航
  - ARIA labels

**验收标准**:
- [ ] Dark Mode 下所有元素可读
- [ ] 微交互正确触发
- [ ] 色盲用户可通过图标识别状态

---

## 优先级排序

| 优先级 | Task | 理由 | 依赖 |
|--------|------|------|------|
| P0 | TASK-014-01 | 状态机是所有功能的基础 | 无 |
| P0 | TASK-014-02 | ConfigService 是配置持久化核心 | TASK-014-01 |
| P0 | TASK-014-03 | SystemCheck 是 Step 2 的后端 | 无 |
| P0 | TASK-014-04 | ChannelAdapter trait 是 Channel 模块的基础 | 无 |
| P0 | TASK-014-05 | ProviderService 是 Step 4 的后端 | 无 |
| P0 | TASK-014-07 | Tauri Commands 是前后端桥梁 | TASK-014-01~06 |
| P1 | TASK-014-06 | Channel Test Service 复用 UClaw 逻辑 | TASK-014-04 |
| P1 | TASK-014-08 | Event 推送增强体验（非阻塞） | TASK-014-01, TASK-014-03 |
| P1 | TASK-014-09 | 前端根组件 + Layout | 无（可并行） |
| P1 | TASK-014-16 | App 启动集成 | TASK-014-01, TASK-014-07 |
| P1 | TASK-014-10 | Step 1 Welcome | TASK-014-09 |
| P1 | TASK-014-11 | Step 2 SystemCheck | TASK-014-03, TASK-014-09 |
| P1 | TASK-014-12 | Step 3 SecurityConfirm | TASK-014-09 |
| P1 | TASK-014-13 | Step 4 ProviderSetup | TASK-014-05, TASK-014-09 |
| P1 | TASK-014-14 | Step 5 ChannelSetup | TASK-014-04, TASK-014-06, TASK-014-09 |
| P1 | TASK-014-15 | Step 6 Activation | TASK-014-07, TASK-014-09 |
| P1 | TASK-014-17 | Reset + 兼容性验证 | TASK-014-02, TASK-014-07 |
| P2 | TASK-014-18 | 全局 UI 组件 + Dark Mode | TASK-014-09~15 |

---

## 依赖关系图

```
TASK-014-01 State Machine ───┬── TASK-014-02 ConfigService ───┬── TASK-014-07 Commands ──┬── TASK-014-16 App 启动
                             │                                │                          ├── TASK-014-17 Reset
                             │                                │                          └── TASK-014-15 Activation
                             │                                └── TASK-014-17 Reset
                             └── TASK-014-08 Events ──────────┘

TASK-014-03 SystemCheck ──────────────────────────────────────────────────────────────────┬── TASK-014-11 Step 2
                                                                                           └── TASK-014-08 Events

TASK-014-04 ChannelAdapter ───┬── TASK-014-06 Channel Test ───┐
                              └── TASK-014-07 Commands ───────┘
                                                             └── TASK-014-14 Step 5

TASK-014-05 ProviderService ──────────────┬── TASK-014-07 Commands ──┬── TASK-014-13 Step 4
                                          └──────────────────────────┘

TASK-014-09 Frontend Root ────────────────┬── TASK-014-10 Step 1
                                          ├── TASK-014-11 Step 2
                                          ├── TASK-014-12 Step 3
                                          ├── TASK-014-13 Step 4
                                          ├── TASK-014-14 Step 5
                                          └── TASK-014-15 Step 6

TASK-014-18 Global UI / Dark Mode ──────── 依赖所有 Step 组件完成
```

---

## 验收总览

- [ ] Onboarding 状态机正确驱动 6 步流程
- [ ] `~/.if2ai/state.json` + `config.json` 持久化正确
- [ ] Layer 1 (config.json) + Layer 2 (三文件) + Bridge (~/.claude/settings.json) 三写同步
- [ ] CPU/GPU/Node.js 检测返回正确结果
- [ ] Embedded 模型可下载且进度实时可见
- [ ] 14 个 Provider 全部在注册表中，3 种协议测试正确
- [ ] 13 个 Channel 全部在注册表中，5 平台真实 API 测试正确
- [ ] 22+ Tauri Commands 编译通过并正确注册
- [ ] Tauri Event 推送可用（降级轮询也可用）
- [ ] 6 个 Step 页面 UI 与设计稿一致
- [ ] 右侧橙色面板 WCAG AA 对比度合规
- [ ] Dark Mode 下所有元素可读
- [ ] 首次启动自动进入 Onboarding
- [ ] 完成 Onboarding 后自动进入主界面
- [ ] 重置 Onboarding 可用
- [ ] 5 个兼容性场景全部通过
- [ ] `cargo fmt + clippy + test` 全部通过
