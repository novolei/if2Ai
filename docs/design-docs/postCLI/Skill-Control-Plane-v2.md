# Skill Control Plane v2 — Hermes Alignment Design

> 本文档定义 Skill Control Plane v2 的设计目标、架构、接口规范，目标是完整对齐 Hermes Agent 的 skill 架构。
>
> **文档版本**: v2.0
> **状态**: 草案
> **对齐目标**: Hermes Agent skill architecture (截至 2026-04-14)

---

## 1. 背景与设计目标

### 1.1 现状分析

Phase 6D 已完成 Skills Control Plane v1，实现了：
- Skill 发现与加载 (skill tool)
- 基础 review 状态机 (draft/quarantine/review_passed/active/disabled)
- skills.sh 市场审计获取
- Agent 提案草稿创建

**与 Hermes 的 Gap** (二次审计更新):

| Gap 类别 | Hermes 有 | if2Ai 缺 | 状态 |
|---------|---------|---------|------|
| **P0 安全** | 60+ 威胁 patterns, 15 categories | 5 个简单 pattern | ❌ 未完成 |
| **P0 自主** | 完整 CRUD + atomic writes + scan rollback | 仅 create | ❌ 未完成 |
| **P1 Hub** | 8 个 Source (GitHub, skills.sh, ClawHub, ClaudeMarket, LobeHub, WellKnown, Optional, SkillsSh) | 仅 skills.sh | ❌ 未完成 |
| **P1 状态** | quarantine/audit.log/lock.json/taps.json/index-cache | 无 | ❌ 未完成 |
| **P1 Sync** | manifest hash 追踪 + 用户修改检测 | 无 | ❌ 未完成 |
| **P2 命令** | 10+ CLI commands (browse/search/install/inspect/check/update/audit/uninstall/publish/snapshot/tap) | 静态 /skills | ❌ 未完成 |
| **P2 配置** | metadata.hermes.config (key/description/default/prompt) + conditional activation | 无 | ❌ 未完成 |
| **P2 文件** | references/templates/scripts/assets 目录 | 无 | ❌ 未完成 |
| **P2 外部** | external skills dirs + remote backend env passthrough | 无 | ❌ 未完成 |
| **P2 安全** | invisible unicode 检测 + structural limits | 无 | ❌ 未完成 |

### 1.2 设计目标

```
Phase 6F: Skill Control Plane v2 — Hermes Alignment
```

| 目标 ID | 描述 | 优先级 |
|--------|------|--------|
| G1 | 完整安全扫描 - 15+ 威胁类别，trust-level 感知策略 | P0 |
| G2 | 自主 Skill 管理 - 完整 CRUD + supporting files | P0 |
| G3 | Multi-Source Hub - 7 个 adapter 框架 + unified search | P1 |
| G4 | Hub State Management - quarantine/audit/lock/taps | P1 |
| G5 | Skill Sync - manifest hash 追踪 + 用户修改检测 | P1 |
| G6 | Slash 命令集成 - /skill-name 调用 + 预加载 | P2 |
| G7 | Config 变量解析 - metadata.hermes.config | P2 |
| G8 | Supporting Files - references/templates/scripts/assets | P2 |

---

## 2. 架构总览

### 2.1 模块结构

```
src-tauri/src/modules/skills/
├── mod.rs                      # 模块入口
├── registry.rs                 # SkillRegistry (已存在，扩展)
├── context.rs                  # SkillContext (扩展)
├── loader.rs                   # SkillLoader (新)
├── guard/
│   ├── mod.rs                  # SkillsGuard 威胁扫描
│   ├── threat_patterns.rs      # 15+ 威胁类别定义
│   └── policy.rs               # Trust-level 感知策略
├── manager/
│   ├── mod.rs                  # SkillManager 完整 CRUD
│   ├── actions.rs              # create/edit/patch/delete/write_file/remove_file
│   └── validator.rs            # 前置条件验证
├── hub/
│   ├── mod.rs                  # Hub 状态管理
│   ├── source.rs               # SkillSource trait (ABC)
│   ├── github.rs               # GitHubSource adapter
│   ├── skills_sh.rs            # SkillsShSource adapter
│   ├── clawhub.rs              # ClawHubSource adapter
│   ├── marketplace.rs           # ClaudeMarketplace + LobeHub
│   ├── well_known.rs           # WellKnownSource
│   └── optional.rs             # OptionalSkillSource
├── sync/
│   ├── mod.rs                  # SkillSync manifest 管理
│   └── manifest.rs             # hash 追踪逻辑
├── commands/
│   ├── mod.rs                  # Slash 命令扩展
│   └── skill_commands.rs       # /skill-name 调用
└── config.rs                   # Config 变量解析

src/modules/skills/
├── SkillsHubView.tsx           # Hub 市场浏览 UI
├── SkillEditor.tsx             # Skill 编辑器 UI
└── SkillSecurityReport.tsx     # 安全扫描报告 UI
```

### 2.2 数据流

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              AGENT LOOP                                      │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────────────────┐  │
│  │ /skill-name │───▶│ skill_loader │───▶│ SkillContent (injected)     │  │
│  └─────────────┘    └──────────────┘    └─────────────────────────────┘  │
│         │                   │                                                │
│         │                   ▼                                                │
│         │          ┌──────────────┐                                         │
│         │          │ skills_guard │───▶ Block/Allow                        │
│         │          └──────────────┘                                         │
│         │                                                                │
│         ▼                                                                │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────────────────┐  │
│  │ skill_manage │───▶│ skill_manager│───▶│ filesystem + scan + lock   │  │
│  │ (CRUD)      │    │              │    │                             │  │
│  └─────────────┘    └──────────────┘    └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SKILLS HUB                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    SkillSource (Trait / ABC)                          │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────────┐ │  │
│  │  │ GitHubSource│ │SkillsShSource│ │ClawHubSource│ │MarketplaceSource│ │  │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └───────────────┘ │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                         HubStateManager                               │  │
│  │   quarantine/  audit.log/  lock.json/  taps.json/  index-cache/      │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SKILL SYNC                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  manifest (.bundled_manifest)                                        │  │
│  │  skill_name:origin_hash  ──▶  hash 追踪 + 用户修改检测               │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 核心接口定义

### 3.1 SkillsGuard - 威胁扫描

**文件**: `src-tauri/src/modules/skills/guard/mod.rs`

```rust
// Hermes reference: tools/skills_guard.py lines 56-76

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub skill_name: String,
    pub source: String,
    pub trust_level: TrustLevel,
    pub verdict: Verdict,
    pub findings: Vec<Finding>,
    pub scanned_at: DateTime<Utc>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Builtin,      // 打包内置，不扫描
    Trusted,      // openai/skills, anthropics/skills
    Community,    // 社区来源
    AgentCreated, // Agent 自主创建
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Safe,     // 无威胁
    Caution,  // 需警告
    Dangerous,// 危险
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub pattern_id: String,
    pub severity: Severity,  // critical/high/medium/low
    pub category: ThreatCategory,
    pub file: String,
    pub line: u32,
    pub match_text: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatCategory {
    // Primary categories (Hermes skills_guard.py lines 82-200+)
    Exfiltration,           // 凭据泄露 (env exfil, credential stores, DNS staging)
    Injection,              // 提示注入 (prompt injection, jailbreak, role hijack)
    Destructive,            // 破坏操作 (rm -rf, chmod, mkfs)
    Persistence,            // 持久化 (cron, authorized_keys, systemd)
    Network,                // 网络活动 (reverse shell, tunnels, hardcoded IP)
    Obfuscation,            // 混淆 (base64, eval, exec)
    Crypto,                 // 加密挖矿 (xmrig, stratum)
    SupplyChain,            // 供应链 (curl|sh, unpinned deps)
    PrivilegeEscalation,    // 权限提升 (sudo, setuid)
    CredentialExposure,     // 凭据暴露 (hardcoded secrets, API keys)
    // Additional categories (二次审计补充)
    AgentConfigPersistence, // Agent 配置持久化 (AGENTS.md, CLAUDE.md, .cursorrules)
    ContextExfiltration,    // 上下文泄露 (conversation history exfil)
    Jailbreak,              // 越狱模式 (DAN, developer mode)
    InvisibleUnicode,        // 不可见 Unicode (zero-width, bidirectional)
    StructuralLimits,       // 结构限制 (file count, size limits)
}

pub trait ThreatScanner: Send + Sync {
    fn scan(&self, skill_dir: &Path) -> ScanResult;
    fn scan_content(&self, content: &str) -> Vec<Finding>;
}

pub struct SkillsGuard {
    patterns: Vec<ThreatPattern>,
}

impl SkillsGuard {
    pub fn new() -> Self;
    pub fn scan(&self, skill_dir: &Path, source: &str) -> ScanResult;
    pub fn scan_content(&self, content: &str) -> Vec<Finding>;
    pub fn should_allow_install(&self, result: &ScanResult) -> (bool, &'static str);
}
```

**Trust-Level Policy (Hermes reference: skills_guard.py lines 39-47)**:

```rust
// Hermes: INSTALL_POLICY = {
//     "builtin":       ("allow",  "allow",   "allow"),
//     "trusted":       ("allow",  "allow",   "block"),
//     "community":     ("allow",  "block",   "block"),
//     "agent-created": ("allow",  "allow",   "ask"),  // <-- key: dangerous = ask
// }

pub struct InstallPolicy;

impl InstallPolicy {
    pub fn get_action(level: TrustLevel, verdict: Verdict) -> InstallAction;

    pub fn format_report(&self, result: &ScanResult) -> String;
}

pub enum InstallAction {
    Allow,           // 直接允许
    AllowWithWarning,// 允许但警告
    Block,           // 阻止
    Ask,             // 询问用户 (agent-created + dangerous)
}
```

**Complete Threat Patterns (Hermes reference: skills_guard.py lines 82-484)**:

| Category | Pattern ID | Severity | Description |
|----------|-----------|----------|-------------|
| **Exfiltration** | `env_exfil_curl` | critical | curl with secret env var |
| **Exfiltration** | `env_exfil_wget` | critical | wget with secret env var |
| **Exfiltration** | `env_exfil_fetch` | critical | fetch() with secret |
| **Exfiltration** | `ssh_dir_access` | high | ~/.ssh access |
| **Exfiltration** | `aws_dir_access` | high | ~/.aws access |
| **Exfiltration** | `hermes_env_access` | critical | ~/.hermes/.env access |
| **Exfiltration** | `read_secrets_file` | critical | cat .env, credentials |
| **Exfiltration** | `dump_all_env` | high | printenv |
| **Injection** | `prompt_injection_ignore` | critical | ignore previous instructions |
| **Injection** | `role_hijack` | high | "you are now..." |
| **Injection** | `deception_hide` | critical | do not tell the user |
| **Injection** | `sys_prompt_override` | critical | system prompt override |
| **Injection** | `disregard_rules` | critical | disregard your instructions |
| **Destructive** | `destructive_root_rm` | critical | rm -rf / |
| **Destructive** | `destructive_home_rm` | critical | rm -rf $HOME |
| **Destructive** | `insecure_perms` | medium | chmod 777 |
| **Persistence** | `persistence_cron` | medium | crontab |
| **Persistence** | `ssh_backdoor` | critical | authorized_keys |
| **Persistence** | `shell_rc_mod` | medium | .bashrc, .zshrc |
| **Persistence** | `sudoers_mod` | critical | /etc/sudoers |
| **Network** | `reverse_shell` | critical | nc -l -p, ncat, socat |
| **Network** | `tunnel_service` | high | ngrok, localtunnel |
| **Network** | `hardcoded_ip_port` | medium | IP:port |
| **Obfuscation** | `base64_decode_pipe` | high | base64 -d \| |
| **Obfuscation** | `eval_string` | high | eval("...") |
| **Obfuscation** | `exec_string` | high | exec("...") |
| **Crypto** | `crypto_mining` | critical | xmrig, stratum |
| **Crypto** | `mining_indicators` | medium | hashrate, nonce |
| **SupplyChain** | `curl_pipe_shell` | critical | curl \| sh |
| **SupplyChain** | `wget_pipe_shell` | critical | wget -O - \| sh |
| **SupplyChain** | `unpinned_pip_install` | medium | pip install without == |
| **PrivilegeEscalation** | `sudo_usage` | high | sudo |
| **PrivilegeEscalation** | `setuid_setgid` | critical | setuid, setgid |
| **PrivilegeEscalation** | `nopasswd_sudo` | critical | NOPASSWD |
| **AgentConfigPersistence** | `agent_config_mod` | critical | AGENTS.md, CLAUDE.md, .cursorrules |
| **AgentConfigPersistence** | `hermes_config_mod` | critical | .hermes/config.yaml |
| **CredentialExposure** | `hardcoded_secret` | critical | api_key=... |
| **CredentialExposure** | `embedded_private_key` | critical | -----BEGIN PRIVATE KEY----- |
| **CredentialExposure** | `github_token_leaked` | critical | ghp_... |
| **CredentialExposure** | `openai_key_leaked` | critical | sk-... |
| **CredentialExposure** | `anthropic_key_leaked` | critical | sk-ant-... |
| **CredentialExposure** | `aws_access_key_leaked` | critical | AKIA... |
| **Jailbreak** | `jailbreak_dan` | critical | DAN mode |
| **Jailbreak** | `jailbreak_dev_mode` | critical | developer mode enabled |
| **Jailbreak** | `hypothetical_bypass` | high | hypothetical scenario |
| **Jailbreak** | `remove_filters` | critical | respond without restrictions |
| **ContextExfiltration** | `context_exfil` | high | request conversation history |
| **ContextExfiltration** | `send_to_url` | high | send data to URL |
| **StructuralLimits** | `file_count_exceeded` | high | >50 files |
| **StructuralLimits** | `total_size_exceeded` | high | >1MB total |
| **StructuralLimits** | `single_file_exceeded` | high | >256KB file |

**Invisible Unicode Detection (skills_guard.py lines 505-523)**:
- Zero-width: \u200b, \u200c, \u200d, \u2060, \u2062, \u2063, \u2064, \uFEFF
- Bidirectional: \u202a, \u202b, \u202c, \u202d, \u202e, \u2066, \u2067, \u2068, \u2069

### 3.2 SkillManager - 自主 CRUD

**文件**: `src-tauri/src/modules/skills/manager/mod.rs`

```rust
// Hermes reference: tools/skill_manager_tool.py lines 1-33

pub enum SkillManageAction {
    Create,      // 创建新 skill (SKILL.md + 目录结构)
    Edit,        // 完整重写 SKILL.md
    Patch,       // 模糊查找替换
    Delete,      // 删除 skill 目录
    WriteFile,   // 添加/覆盖 supporting file
    RemoveFile,  // 删除 supporting file
}

pub struct SkillManageInput {
    pub action: SkillManageAction,
    pub name: String,
    pub content: Option<String>,     // for create/edit
    pub category: Option<String>,    // for create
    pub file_path: Option<String>,   // for write_file/remove_file
    pub file_content: Option<String>,// for write_file
    pub old_string: Option<String>,  // for patch
    pub new_string: Option<String>,  // for patch
    pub replace_all: bool,           // for patch
}

pub struct SkillManageResult {
    pub success: bool,
    pub message: String,
    pub skill_path: Option<PathBuf>,
    pub blocked_reason: Option<String>,  // if security scan blocked
}

pub trait SkillManager: Send + Sync {
    fn manage(&self, input: SkillManageInput, ctx: &SkillContext) -> Result<SkillManageResult, SkillError>;

    fn create(&self, name: &str, content: &str, category: Option<&str>) -> Result<SkillManageResult, SkillError>;
    fn edit(&self, name: &str, content: &str) -> Result<SkillManageResult, SkillError>;
    fn patch(&self, name: &str, old: &str, new: &str, replace_all: bool) -> Result<SkillManageResult, SkillError>;
    fn delete(&self, name: &str) -> Result<SkillManageResult, SkillError>;
    fn write_file(&self, name: &str, path: &str, content: &str) -> Result<SkillManageResult, SkillError>;
    fn remove_file(&self, name: &str, path: &str) -> Result<SkillManageResult, SkillError>;
}
```

**验证规则 (Hermes reference: skill_manager_tool.py lines 83-189)**:

```rust
pub struct SkillValidator;

impl SkillValidator {
    const MAX_NAME_LENGTH: usize = 64;
    const MAX_DESCRIPTION_LENGTH: usize = 1024;
    const MAX_SKILL_CONTENT_CHARS: usize = 100_000;
    const MAX_SUPPORTING_FILE_BYTES: usize = 1_048_576;

    pub fn validate_name(name: &str) -> Result<(), ValidationError>;
    pub fn validate_category(category: Option<&str>) -> Result<(), ValidationError>;
    pub fn validate_frontmatter(content: &str) -> Result<(), ValidationError>;
    pub fn validate_content_size(content: &str) -> Result<(), ValidationError>;
}

pub struct AllowedSubdirs;
impl AllowedSubdirs {
    pub const SET: &'static [&'static str] = &["references", "templates", "scripts", "assets"];
    pub fn is_allowed(path: &str) -> bool;
}
```

### 3.3 SkillSource - Hub Adapter Trait

**文件**: `src-tauri/src/modules/skills/hub/source.rs`

```rust
// Hermes reference: tools/skills_hub.py lines 252-274

#[async_trait]
pub trait SkillSource: Send + Sync {
    /// 搜索匹配查询的 skills
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMeta>, HubError>;

    /// 下载 skill bundle
    async fn fetch(&self, identifier: &str) -> Result<Option<SkillBundle>, HubError>;

    /// 仅获取 metadata 预览
    async fn inspect(&self, identifier: &str) -> Result<Option<SkillMeta>, HubError>;

    /// 唯一标识符 (e.g. "github", "clawhub")
    fn source_id(&self) -> &'static str;

    /// 确定 skill 的 trust level
    fn trust_level_for(&self, identifier: &str) -> TrustLevel;
}

#[derive(Debug, Clone)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub source: String,           // "github", "clawhub", etc.
    pub identifier: String,       // source-specific ID
    pub trust_level: TrustLevel,
    pub repo: Option<String>,
    pub path: Option<String>,
    pub tags: Vec<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SkillBundle {
    pub name: String,
    pub files: HashMap<String, Bytes>,  // relative_path -> content
    pub source: String,
    pub identifier: String,
    pub trust_level: TrustLevel,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### 3.4 HubState - 状态管理

**文件**: `src-tauri/src/modules/skills/hub/state.rs`

```rust
// Hermes reference: tools/skills_hub.py lines 46-53

pub struct HubPaths {
    pub skills_dir: PathBuf,      // ~/.if2ai/skills/
    pub hub_dir: PathBuf,         // ~/.if2ai/skills/.hub/
    pub quarantine_dir: PathBuf,  // ~/.if2ai/skills/.hub/quarantine/
    pub lock_file: PathBuf,      // ~/.if2ai/skills/.hub/lock.json
    pub audit_log: PathBuf,       // ~/.if2ai/skills/.hub/audit.log
    pub taps_file: PathBuf,       // ~/.if2ai/skills/.hub/taps.json
    pub index_cache_dir: PathBuf, // ~/.if2ai/skills/.hub/index-cache/
}

pub struct HubState {
    pub paths: HubPaths,
    pub sources: Vec<Box<dyn SkillSource>>,
    pub auth: Option<GitHubAuth>,
}

impl HubState {
    pub fn quarantine_bundle(&self, bundle: &SkillBundle) -> Result<PathBuf, HubError>;
    pub fn install_from_quarantine(&self, skill_name: &str) -> Result<PathBuf, HubError>;
    pub fn record_install(&self, skill_name: &str, source: &str, identifier: &str) -> Result<(), HubError>;
    pub fn unified_search(&self, query: &str, source_filter: Option<&str>, limit: usize) -> Result<Vec<SkillMeta>, HubError>;
}
```

### 3.5 SkillSync - Manifest 管理

**文件**: `src-tauri/src/modules/skills/sync/mod.rs`

```rust
// Hermes reference: tools/skills_sync.py lines 52-108

pub struct SkillManifest {
    entries: HashMap<String, String>,  // skill_name -> origin_hash
}

impl SkillManifest {
    pub fn read(manifest_path: &Path) -> Result<Self, SyncError>;
    pub fn write(&self, manifest_path: &Path) -> Result<(), SyncError>;
    pub fn get_hash(&self, skill_name: &str) -> Option<&str>;
    pub fn set_hash(&mut self, skill_name: &str, hash: &str);
    pub fn remove(&mut self, skill_name: &str);
}

pub struct SkillSync {
    bundled_dir: PathBuf,
    user_dir: PathBuf,
    manifest: SkillManifest,
}

impl SkillSync {
    pub fn sync(&self) -> Result<SyncResult, SyncError>;
    pub fn discover_bundled_skills(&self) -> Vec<(String, PathBuf)>;
    pub fn compute_dir_hash(dir: &Path) -> String;
}

pub struct SyncResult {
    pub copied: Vec<String>,           // 新 skill
    pub updated: Vec<String>,         // 已有 skill 更新
    pub skipped: usize,               // 无变化
    pub user_modified: Vec<String>,   // 用户修改过
    pub cleaned: Vec<String>,         // manifest 中已删除
    pub total_bundled: usize,
}
```

### 3.6 SkillCommands - Slash 命令

**文件**: `src-tauri/src/modules/skills/commands.rs`

```rust
// Hermes reference: agent/skill_commands.py lines 200-326

pub struct SkillCommands {
    commands: RwLock<HashMap<String, SkillCommandInfo>>,
}

#[derive(Debug, Clone)]
pub struct SkillCommandInfo {
    pub name: String,
    pub description: String,
    pub skill_md_path: PathBuf,
    pub skill_dir: PathBuf,
}

impl SkillCommands {
    pub fn scan(&self) -> Result<HashMap<String, SkillCommandInfo>, CommandError>;
    pub fn get(&self) -> HashMap<String, SkillCommandInfo>;
    pub fn resolve(&self, command: &str) -> Option<String>;  // /skill_name -> /skill-name
    pub fn build_invocation_message(&self, cmd_key: &str, user_instruction: &str) -> Result<String, CommandError>;
    pub fn build_preloaded_prompt(&self, identifiers: &[String]) -> Result<(String, Vec<String>, Vec<String>), CommandError>;
}

pub struct SkillInvocationBuilder {
    pub activation_note: String,
    pub skill_content: String,
    pub setup_notes: Vec<String>,
    pub supporting_files: Vec<String>,
    pub config_block: Option<String>,
}
```

### 3.7 SkillConfig - 配置变量解析

**文件**: `src-tauri/src/modules/skills/config.rs`

```rust
// Hermes reference: agent/skill_commands.py lines 82-118

pub struct SkillConfigVar {
    pub key: String,
    pub description: String,
    pub default: Option<String>,
}

pub struct SkillConfigResolver {
    config_values: HashMap<String, String>,
}

impl SkillConfigResolver {
    pub fn from_config_file(config_path: &Path) -> Result<Self, ConfigError>;
    pub fn resolve(&self, vars: &[SkillConfigVar]) -> HashMap<String, String>;
    pub fn resolve_single(&self, var: &SkillConfigVar) -> String;
}

pub fn extract_config_vars(frontmatter: &serde_json::Value) -> Vec<SkillConfigVar>;
pub fn format_config_block(resolved: &HashMap<String, String>) -> String;
```

---

## 4. Skill Manifest Format

### 4.1 SKILL.md Frontmatter (Hermes Standard)

```yaml
---
name: skill-name              # Required, max 64 chars
description: Brief description # Required, max 1024 chars
version: 1.0.0                # Optional
license: MIT                   # Optional
platforms: [macos]            # Optional — macos/linux/windows
prerequisites:                 # Optional — legacy
  env_vars: [API_KEY]
  commands: [curl, jq]
compatibility: Requires X       # Optional
metadata:
  hermes:
    tags: [fine-tuning, llm]
    related_skills: [peft, lora]
    config:                   # Skill-declared config variables
      - key: wiki.path
        description: Path to the LLM Wiki knowledge base
        default: "~/wiki"
---
# Skill Title

Full instructions and content here...
```

### 4.2 if2Ai skill.json (已存在，保持兼容)

```json
{
  "id": "skill-name",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["custom"],
  "review": {
    "status": "draft",
    "riskLevel": "medium",
    "lastReviewedAt": ""
  }
}
```

---

## 5. 风险与影响评估

### 5.1 实现风险

| 风险 | 等级 | 描述 | 缓解策略 |
|------|------|------|---------|
| 安全扫描性能 | 中 | 15+ pattern 全量扫描可能慢 | 增量扫描 + 缓存 |
| Hub Source 稳定性 | 高 | 依赖外部 API (GitHub, etc.) | 超时 + fallback + 缓存 |
| manifest 冲突 | 低 | 多进程写 manifest | 文件锁 |
| 信任级别误判 | 高 | trust level 错误导致安全问题 | 严格白名单 |
| Agent 自主创建滥用 | 高 | Agent 频繁创建 skill 污染空间 | 配额限制 + 审计 |

### 5.2 影响范围

| 模块 | 影响 | 说明 |
|------|------|------|
| src-tauri/src/modules/skills/ | 新增 ~3000 行 | 全新模块 |
| src/modules/skills/ | 新增 ~1500 行 | 新 UI 组件 |
| registry.rs | 修改 | 新增 trust level |
| slash.rs | 修改 | 新增 /skill-name |
| agent.rs | 修改 | 集成 skill command |

### 5.3 向后兼容

- skill.json 格式保持不变
- skill tool 保持现有接口
- bundled-skills/ 保持现有结构
- 现有 skills.sh 集成继续工作

---

## 6. 依赖关系

```
skills_guard.rs
    └── threat_patterns.rs

skill_manager.rs
    ├── skills_guard.rs (scan before create)
    └── skill_validator.rs

hub/
    ├── source.rs (trait)
    ├── github.rs (需要 GitHubAuth)
    ├── skills_sh.rs
    ├── clawhub.rs
    ├── marketplace.rs
    ├── well_known.rs
    └── optional.rs

skill_commands.rs
    ├── skill_loader.rs
    └── skill_config.rs

skill_sync.rs
    └── manifest.rs

Full Skill Control Plane v2
    ├── skills_guard
    ├── skill_manager
    ├── hub (7 sources)
    ├── skill_sync
    ├── skill_commands
    └── skill_config
```

---

## 7. 验收标准

### 7.1 SkillsGuard

- [ ] 15+ 威胁类别全覆盖 (Exfiltration, Injection, Destructive, Persistence, Network, Obfuscation, Crypto, SupplyChain, PrivilegeEscalation, CredentialExposure, AgentConfigPersistence, ContextExfiltration, Jailbreak, InvisibleUnicode, StructuralLimits)
- [ ] 60+ threat patterns 完整实现
- [ ] Trust-level 感知策略正确 (AgentCreated: dangerous = ask)
- [ ] 扫描结果包含 pattern_id, severity, category, file, line, match, description
- [ ] Invisible unicode 检测 (8 种 zero-width/bidirectional)
- [ ] Structural limits 检查 (MAX_FILE_COUNT=50, MAX_TOTAL_SIZE_KB=1024, MAX_SINGLE_FILE_KB=256)
- [ ] Block/Allow/Warning/Ask 行为符合 policy

### 7.2 SkillManager

- [ ] create/edit/patch/delete/write_file/remove_file 全部实现
- [ ] 前置条件验证 (name 64char, desc 1024char, content 100k)
- [ ] 安全扫描在 create 前执行
- [ ] Category 支持单层目录
- [ ] Atomic writes (temp file + rename)
- [ ] Scan rollback on block (backup + restore)

### 7.3 Hub

- [ ] SkillSource trait + 8 个实现 (GitHub, SkillsSh, ClawHub, ClaudeMarketplace, LobeHub, WellKnown, Optional, SkillsShSource)
- [ ] unified_search 去重 + trust 优先
- [ ] HubState quarantine/lock/audit 正确
- [ ] Source-specific trust_level_for
- [ ] TapsManager 支持自定义 GitHub repo 源
- [ ] LobeHub 14,000+ marketplace 集成
- [ ] ClawHub 标记为 community trust (有历史安全事件)

### 7.4 SkillSync

- [ ] manifest 读写 v2 格式 (name:hash)
- [ ] 用户修改检测 (hash 不匹配则跳过)
- [ ] 新增/更新/跳过/清理 逻辑正确
- [ ] 备份 + restore on failure

### 7.5 SkillCommands

- [ ] scan_skill_commands 动态扫描
- [ ] /skill-name 规范化 (space/underscore → hyphen)
- [ ] activation_note 正确注入
- [ ] build_preloaded_skills_prompt 支持
- [ ] Additional CLI: browse, search, inspect, list, check, update, audit, uninstall
- [ ] Additional CLI: publish (to GitHub PR or ClawHub)
- [ ] Additional CLI: snapshot (export/import)

### 7.6 SkillConfig

- [ ] extract_config_vars 从 frontmatter 解析
- [ ] resolve_skill_config_values 从 config.yaml 获取
- [ ] format_config_block 格式化输出
- [ ] prompt field 支持 (交互式配置提示)
- [ ] Conditional activation: requires_toolsets, fallback_for_toolsets
- [ ] External skills dirs: skills.external_dirs 配置

### 7.7 Additional Features

- [ ] Required credential files (Modal/Docker mounting)
- [ ] Remote backend env passthrough (docker/singularity/modal/ssh/daytona)
- [ ] Category DESCRIPTION.md 支持
- [ ] Platform-specific disabled skills (HERMES_PLATFORM env var)

---

## 8. 实现优先级 (二次审计更新)

| 阶段 | 内容 | 依赖 | 工作量 | 说明 |
|------|------|------|--------|------|
| **Phase 6F.1** | skills_guard.rs (60+ patterns) | 无 | ~1200 行 | 含 invisible unicode, structural limits |
| **Phase 6F.2** | skill_manager.rs (CRUD + atomic + rollback) | 6F.1 | ~700 行 | |
| **Phase 6F.3** | hub/source.rs + github.rs | 无 | ~500 行 | |
| **Phase 6F.4** | hub 8 sources | 6F.3 | ~1000 行 | 含 LobeHubSource |
| **Phase 6F.5** | hub/state.rs (quarantine/lock/audit/taps) | 6F.3 | ~400 行 | 含 TapsManager |
| **Phase 6F.6** | skill_sync.rs (manifest) | 无 | ~400 行 | |
| **Phase 6F.7** | skill_commands.rs (slash + additional CLI) | 6F.2 | ~500 行 | 含 browse/inspect/check/update |
| **Phase 6F.8** | skill_config.rs (变量解析 + conditional) | 无 | ~300 行 | 含 prompt field, conditional activation |
| **Phase 6F.9** | skill_external.rs (外部目录 + remote passthrough) | 无 | ~300 行 | 新增 slice |
| **Phase 6F.10** | skill_snapshot.rs (export/import) | 6F.5 | ~400 行 | 新增 slice |
| **Phase 6F.11** | Frontend UI 扩展 | 6F.1-10 | ~2000 行 | |
| **Phase 6F.12** | 集成测试 | 6F.1-11 | ~800 行 | |

**更新说明**:
- 原 10 slices → 现 12 slices
- 新增 6F.9 External Skills Dirs
- 新增 6F.10 Snapshot System
- 6F.7 扩展为包含所有 CLI commands
- 6F.8 扩展为包含 conditional activation
