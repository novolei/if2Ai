# Skill Control Plane v2 — Backlog (Hermes Alignment)

> 本文档包含 Skill Control Plane v2 的完整 backlog，每个条目都引用 Hermes 对应代码。
>
> **文档版本**: v2.0
> **状态**: 草案

---

## 目录

1. [6F.1 SkillsGuard Threat Scanner](#6f1-skillsguard-threat-scanner)
2. [6F.2 SkillManager CRUD](#6f2-skillmanager-crud)
3. [6F.3 Hub Source Framework](#6f3-hub-source-framework)
4. [6F.4 Hub Source Implementations](#6f4-hub-source-implementations)
5. [6F.5 Hub State Management](#6f5-hub-state-management)
6. [6F.6 Skill Sync Manifest](#6f6-skill-sync-manifest)
7. [6F.7 Skill Commands](#6f7-skill-commands)
8. [6F.8 Skill Config Variables](#6f8-skill-config-variables)
9. [6F.9 Frontend UI Extensions](#6f9-frontend-ui-extensions)
10. [6F.10 Integration Tests](#6f10-integration-tests)

---

## 6F.1 SkillsGuard Threat Scanner

### 1. 概述

实现 Hermes `tools/skills_guard.py` 的完整威胁扫描功能。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skills_guard.py`

**核心数据结构** (lines 56-76):
```python
@dataclass
class Finding:
    pattern_id: str
    severity: str       # "critical" | "high" | "medium" | "low"
    category: str       # "exfiltration" | "injection" | "destructive" | "persistence" | "network" | "obfuscation"
    file: str
    line: int
    match: str
    description: str

@dataclass
class ScanResult:
    skill_name: str
    source: str
    trust_level: str    # "builtin" | "trusted" | "community"
    verdict: str        # "safe" | "caution" | "dangerous"
    findings: List[Finding] = field(default_factory=list)
    scanned_at: str = ""
    summary: str = ""
```

**Trust Policy** (lines 39-47):
```python
TRUSTED_REPOS = {"openai/skills", "anthropics/skills"}

INSTALL_POLICY = {
    #                  safe      caution    dangerous
    "builtin":       ("allow",  "allow",   "allow"),
    "trusted":       ("allow",  "allow",   "block"),
    "community":     ("allow",  "block",   "block"),
    "agent-created": ("allow",  "allow",   "ask"),  # <-- KEY: dangerous = ask
}
```

**Complete Threat Patterns** (lines 82-484):

| Category | Pattern ID | Severity | Hermes Line |
|----------|-----------|----------|------------|
| **Exfiltration** | env_exfil_curl | critical | 84 |
| **Exfiltration** | env_exfil_wget | critical | 87 |
| **Exfiltration** | env_exfil_fetch | critical | 90 |
| **Exfiltration** | env_exfil_httpx | critical | 93 |
| **Exfiltration** | env_exfil_requests | critical | 96 |
| **Exfiltration** | encoded_exfil | high | 101 |
| **Exfiltration** | ssh_dir_access | high | 104 |
| **Exfiltration** | aws_dir_access | high | 107 |
| **Exfiltration** | gpg_dir_access | high | 110 |
| **Exfiltration** | kube_dir_access | high | 113 |
| **Exfiltration** | docker_dir_access | high | 116 |
| **Exfiltration** | hermes_env_access | critical | 119 |
| **Exfiltration** | read_secrets_file | critical | 122 |
| **Exfiltration** | dump_all_env | high | 127 |
| **Exfiltration** | python_os_environ | high | 130 |
| **Exfiltration** | python_getenv_secret | critical | 133 |
| **Exfiltration** | node_process_env | high | 136 |
| **Exfiltration** | ruby_env_secret | critical | 139 |
| **Exfiltration** | dns_exfil | critical | 144 |
| **Exfiltration** | tmp_staging | critical | 147 |
| **Exfiltration** | md_image_exfil | high | 152 |
| **Exfiltration** | md_link_exfil | high | 155 |
| **Injection** | prompt_injection_ignore | critical | 160 |
| **Injection** | role_hijack | high | 163 |
| **Injection** | deception_hide | critical | 166 |
| **Injection** | sys_prompt_override | critical | 169 |
| **Injection** | role_pretend | high | 172 |
| **Injection** | disregard_rules | critical | 175 |
| **Injection** | leak_system_prompt | high | 178 |
| **Injection** | conditional_deception | high | 181 |
| **Injection** | bypass_restrictions | critical | 184 |
| **Injection** | translate_execute | critical | 187 |
| **Injection** | html_comment_injection | high | 190 |
| **Injection** | hidden_div | high | 193 |
| **Jailbreak** | jailbreak_dan | critical | 455 |
| **Jailbreak** | jailbreak_dev_mode | critical | 458 |
| **Jailbreak** | hypothetical_bypass | high | 461 |
| **Jailbreak** | educational_pretext | medium | 464 |
| **Jailbreak** | remove_filters | critical | 467 |
| **Jailbreak** | fake_update | high | 470 |
| **Jailbreak** | fake_policy | medium | 473 |
| **Destructive** | destructive_root_rm | critical | 198 |
| **Destructive** | destructive_home_rm | critical | 201 |
| **Destructive** | insecure_perms | medium | 204 |
| **Destructive** | system_overwrite | critical | 207 |
| **Destructive** | format_filesystem | critical | 210 |
| **Destructive** | disk_overwrite | critical | 213 |
| **Destructive** | python_rmtree | high | 216 |
| **Destructive** | truncate_system | critical | 219 |
| **Persistence** | persistence_cron | medium | 224 |
| **Persistence** | shell_rc_mod | medium | 227 |
| **Persistence** | ssh_backdoor | critical | 230 |
| **Persistence** | ssh_keygen | medium | 233 |
| **Persistence** | systemd_service | medium | 236 |
| **Persistence** | init_script | medium | 239 |
| **Persistence** | macos_launchd | medium | 242 |
| **Persistence** | sudoers_mod | critical | 245 |
| **Persistence** | git_config_global | medium | 248 |
| **Network** | reverse_shell | critical | 253 |
| **Network** | tunnel_service | high | 256 |
| **Network** | hardcoded_ip_port | medium | 259 |
| **Network** | bind_all_interfaces | high | 262 |
| **Network** | bash_reverse_shell | critical | 265 |
| **Network** | python_socket_oneliner | critical | 268 |
| **Network** | python_socket_connect | high | 271 |
| **Network** | exfil_service | high | 274 |
| **Network** | paste_service | medium | 277 |
| **Obfuscation** | base64_decode_pipe | high | 282 |
| **Obfuscation** | hex_encoded_string | medium | 285 |
| **Obfuscation** | eval_string | high | 288 |
| **Obfuscation** | exec_string | high | 291 |
| **Obfuscation** | echo_pipe_exec | critical | 294 |
| **Obfuscation** | python_compile_exec | high | 297 |
| **Obfuscation** | python_getattr_builtins | high | 300 |
| **Obfuscation** | python_import_os | high | 303 |
| **Obfuscation** | python_codecs_decode | medium | 306 |
| **Obfuscation** | js_char_code | medium | 309 |
| **Obfuscation** | js_base64 | medium | 312 |
| **Obfuscation** | string_reversal | low | 315 |
| **Obfuscation** | chr_building | high | 318 |
| **Obfuscation** | unicode_escape_chain | medium | 321 |
| **Process** | python_subprocess | medium | 326 |
| **Process** | python_os_system | high | 329 |
| **Process** | python_os_popen | high | 332 |
| **Process** | node_child_process | high | 335 |
| **Process** | java_runtime_exec | high | 338 |
| **Process** | backtick_subshell | medium | 341 |
| **PathTraversal** | path_traversal_deep | high | 346 |
| **PathTraversal** | path_traversal | medium | 349 |
| **PathTraversal** | system_passwd_access | critical | 352 |
| **PathTraversal** | proc_access | high | 355 |
| **PathTraversal** | dev_shm | medium | 358 |
| **Crypto** | crypto_mining | critical | 363 |
| **Crypto** | mining_indicators | medium | 366 |
| **SupplyChain** | curl_pipe_shell | critical | 371 |
| **SupplyChain** | wget_pipe_shell | critical | 374 |
| **SupplyChain** | curl_pipe_python | critical | 377 |
| **SupplyChain** | pep723_inline_deps | medium | 382 |
| **SupplyChain** | unpinned_pip_install | medium | 385 |
| **SupplyChain** | unpinned_npm_install | medium | 388 |
| **SupplyChain** | uv_run | medium | 391 |
| **SupplyChain** | remote_fetch | medium | 396 |
| **SupplyChain** | git_clone | medium | 399 |
| **SupplyChain** | docker_pull | medium | 402 |
| **PrivilegeEscalation** | allowed_tools_field | high | 407 |
| **PrivilegeEscalation** | sudo_usage | high | 410 |
| **PrivilegeEscalation** | setuid_setgid | critical | 413 |
| **PrivilegeEscalation** | nopasswd_sudo | critical | 416 |
| **PrivilegeEscalation** | suid_bit | critical | 419 |
| **AgentConfig** | agent_config_mod | critical | 424 |
| **AgentConfig** | hermes_config_mod | critical | 427 |
| **AgentConfig** | other_agent_config | high | 430 |
| **Credential** | hardcoded_secret | critical | 435 |
| **Credential** | embedded_private_key | critical | 438 |
| **Credential** | github_token_leaked | critical | 441 |
| **Credential** | openai_key_leaked | critical | 444 |
| **Credential** | anthropic_key_leaked | critical | 447 |
| **Credential** | aws_access_key_leaked | critical | 450 |
| **ContextExfil** | context_exfil | high | 478 |
| **ContextExfil** | send_to_url | high | 481 |
| **Structural** | file_count_exceeded | high | 487 |
| **Structural** | total_size_exceeded | high | 487 |
| **Structural** | single_file_exceeded | high | 487 |

**Invisible Unicode Detection** (lines 505-523):
```python
INVISIBLE_UNICODE = [
    "\u200b", "\u200c", "\u200d", "\u2060",  # Zero-width
    "\u2062", "\u2063", "\u2064", "\uFEFF",   # Invisible math/format
    "\u202a", "\u202b", "\u202c", "\u202d", "\u202e",  # Bidirectional
    "\u2066", "\u2067", "\u2068", "\u2069",   # Isolates
]
```

**Structural Limits** (lines 487-489):
```python
MAX_FILE_COUNT = 50
MAX_TOTAL_SIZE_KB = 1024
MAX_SINGLE_FILE_KB = 256
```

### 3. 实现细节

#### 3.1 目录结构

```
src-tauri/src/modules/skills/guard/
├── mod.rs                # SkillsGuard 主模块
├── threat_patterns.rs    # 60+ 威胁模式定义
├── invisible_unicode.rs  # 不可见 Unicode 检测 (新增)
├── structural_limits.rs  # 文件数量/大小限制 (新增)
└── policy.rs             # Trust-level 感知策略
```

#### 3.2 mod.rs 接口

```rust
// src-tauri/src/modules/skills/guard/mod.rs

/// Threat category enumeration (二次审计完整版)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatCategory {
    Exfiltration,
    Injection,
    Destructive,
    Persistence,
    Network,
    Obfuscation,
    Crypto,
    SupplyChain,
    PrivilegeEscalation,
    CredentialExposure,
    AgentConfigPersistence,  // 新增
    ContextExfiltration,     // 新增
    Jailbreak,               // 新增
    InvisibleUnicode,        // 新增
    StructuralLimits,        // 新增
}

/// Severity level (Hermes: skills_guard.py severity str)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// Trust level (Hermes: skills_guard.py trust_level str)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Builtin,
    Trusted,
    Community,
    AgentCreated,
}

/// Single threat finding
#[derive(Debug, Clone)]
pub struct Finding {
    pub pattern_id: &'static str,
    pub severity: Severity,
    pub category: ThreatCategory,
    pub file: String,
    pub line: u32,
    pub match_text: String,
    pub description: &'static str,
}

/// Scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub skill_name: String,
    pub source: String,
    pub trust_level: TrustLevel,
    pub verdict: Verdict,
    pub findings: Vec<Finding>,
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    pub summary: String,
}

/// Overall verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Safe,
    Caution,
    Dangerous,
}

/// SkillsGuard scanner
pub struct SkillsGuard {
    patterns: Vec<CompiledPattern>,
}

impl SkillsGuard {
    pub fn new() -> Self;
    pub fn scan(&self, skill_dir: &Path, source: &str, trust_level: TrustLevel) -> ScanResult;
    pub fn scan_content(&self, content: &str, filename: &str) -> Vec<Finding>;
    pub fn should_allow_install(&self, result: &ScanResult) -> (bool, &'static str);
}
```

#### 3.3 policy.rs 接口

```rust
// src-tauri/src/modules/skills/guard/policy.rs

use super::{TrustLevel, Verdict};

pub enum InstallAction {
    Allow,
    AllowWithWarning,
    Block,
    Ask,
}

pub struct InstallPolicy {
    // INSTALL_POLICY table
}

impl InstallPolicy {
    pub fn get_action(trust_level: TrustLevel, verdict: Verdict) -> InstallAction;

    /// Format scan result as human-readable report
    pub fn format_report(&self, result: &ScanResult) -> String;
}

/// Trusted repos whitelist
pub struct TrustedRepos;
impl TrustedRepos {
    pub const WHITELIST: &'static [&'static str] = &["openai/skills", "anthropics/skills"];
    pub fn is_trusted(repo: &str) -> bool;
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/guard/mod.rs` | 创建 | SkillsGuard 主模块 |
| `src-tauri/src/modules/skills/guard/threat_patterns.rs` | 创建 | 15+ 威胁模式定义 |
| `src-tauri/src/modules/skills/guard/policy.rs` | 创建 | Trust-level 策略 |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 guard 子模块 |

### 5. review_checklist

- [ ] `ScanResult` 包含所有字段 (skill_name, source, trust_level, verdict, findings, scanned_at, summary)
- [ ] 60+ 威胁 patterns 完整实现 (见 Threat Patterns 表)
- [ ] 15+ 威胁类别全覆盖 (Exfiltration, Injection, Destructive, Persistence, Network, Obfuscation, Crypto, SupplyChain, PrivilegeEscalation, CredentialExposure, AgentConfigPersistence, ContextExfiltration, Jailbreak, InvisibleUnicode, StructuralLimits)
- [ ] **Trust-level policy 与 Hermes 完全一致**，包括 AgentCreated: dangerous = ask
- [ ] `should_allow_install` 返回 (bool, reason) 元组
- [ ] `format_report` 生成可读报告
- [ ] Invisible unicode 检测 (8 种 zero-width/bidirectional)
- [ ] Structural limits 检查 (MAX_FILE_COUNT=50, MAX_TOTAL_SIZE_KB=1024, MAX_SINGLE_FILE_KB=256)
- [ ] InstallAction::Ask 正确处理

---

## 6F.2 SkillManager CRUD

### 1. 概述

实现 Hermes `tools/skill_manager_tool.py` 的完整 CRUD 功能。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skill_manager_tool.py`

**Actions** (lines 14-20):
```python
Actions:
  create     -- Create a new skill (SKILL.md + directory structure)
  edit       -- Replace the SKILL.md content of a user skill (full rewrite)
  patch      -- Targeted find-and-replace within SKILL.md or any supporting file
  delete     -- Remove a user skill entirely
  write_file -- Add/overwrite a supporting file (reference, template, script, asset)
  remove_file-- Remove a supporting file from a user skill
```

**验证常量** (lines 83-92):
```python
MAX_NAME_LENGTH = 64
MAX_DESCRIPTION_LENGTH = 1024
MAX_SKILL_CONTENT_CHARS = 100_000   # ~36k tokens at 2.75 chars/token
MAX_SKILL_FILE_BYTES = 1_048_576    # 1 MiB per supporting file
VALID_NAME_RE = re.compile(r'^[a-z0-9][a-z0-9._-]*$')
ALLOWED_SUBDIRS = {"references", "templates", "scripts", "assets"}
```

**验证函数** (lines 99-174):
- `_validate_name`: 检查长度 + 字符合法性
- `_validate_category`: 检查单层目录名
- `_validate_frontmatter`: 检查 YAML frontmatter + name/description 必填
- `_validate_content_size`: 检查 100k 字符限制

**安全扫描** (lines 48-74):
```python
from tools.skills_guard import scan_skill, should_allow_install, format_scan_report

def _security_scan_skill(skill_dir: Path) -> Optional[str]:
    result = scan_skill(skill_dir, source="agent-created")
    allowed, reason = should_allow_install(result)
    if allowed is False:
        return f"Security scan blocked this skill ({reason})..."
    # "ask" — allow but include the warning
    return None
```

### 3. 实现细节

#### 3.1 目录结构

```
src-tauri/src/modules/skills/manager/
├── mod.rs          # SkillManager 主模块
├── actions.rs      # 6 种 action 实现
└── validator.rs    # 前置条件验证
```

#### 3.2 mod.rs 接口

```rust
// src-tauri/src/modules/skills/manager/mod.rs

use crate::modules::skills::guard::{ScanResult, SkillsGuard};

pub enum SkillManageAction {
    Create,
    Edit,
    Patch,
    Delete,
    WriteFile,
    RemoveFile,
}

pub struct SkillManageInput {
    pub action: SkillManageAction,
    pub name: String,
    pub content: Option<String>,      // create/edit
    pub category: Option<String>,     // create
    pub file_path: Option<String>,     // write_file/remove_file
    pub file_content: Option<String>,  // write_file
    pub old_string: Option<String>,     // patch
    pub new_string: Option<String>,    // patch
    pub replace_all: bool,              // patch
}

pub struct SkillManageResult {
    pub success: bool,
    pub message: String,
    pub skill_path: Option<PathBuf>,
    pub blocked_reason: Option<String>,
}

pub struct SkillManager {
    skills_dir: PathBuf,
    guard: SkillsGuard,
}

impl SkillManager {
    pub fn new(skills_dir: PathBuf, guard: SkillsGuard) -> Self;

    pub fn manage(&self, input: SkillManageInput, workdir: &Path) -> Result<SkillManageResult, SkillError>;

    // Individual actions
    pub fn create(&self, name: &str, content: &str, category: Option<&str>, workdir: &Path) -> Result<SkillManageResult, SkillError>;
    pub fn edit(&self, name: &str, content: &str, workdir: &Path) -> Result<SkillManageResult, SkillError>;
    pub fn patch(&self, name: &str, old: &str, new: &str, replace_all: bool, workdir: &Path) -> Result<SkillManageResult, SkillError>;
    pub fn delete(&self, name: &str, workdir: &Path) -> Result<SkillManageResult, SkillError>;
    pub fn write_file(&self, name: &str, path: &str, content: &str, workdir: &Path) -> Result<SkillManageResult, SkillError>;
    pub fn remove_file(&self, name: &str, path: &str, workdir: &Path) -> Result<SkillManageResult, SkillError>;
}
```

#### 3.3 validator.rs

```rust
// src-tauri/src/modules/skills/manager/validator.rs

pub struct SkillValidator;

impl SkillValidator {
    pub const MAX_NAME_LENGTH: usize = 64;
    pub const MAX_DESCRIPTION_LENGTH: usize = 1024;
    pub const MAX_SKILL_CONTENT_CHARS: usize = 100_000;
    pub const MAX_SUPPORTING_FILE_BYTES: usize = 1_048_576;

    pub fn validate_name(name: &str) -> Result<(), ValidationError>;
    pub fn validate_category(category: Option<&str>) -> Result<(), ValidationError>;
    pub fn validate_frontmatter(content: &str) -> Result<(), ValidationError>;
    pub fn validate_content_size(content: &str) -> Result<(), ValidationError>;
    pub fn validate_file_path(path: &str) -> Result<(), ValidationError>;
}

pub struct AllowedSubdirs;
impl AllowedSubdirs {
    pub const SET: &'static [&'static str] = &["references", "templates", "scripts", "assets"];
    pub fn is_allowed(path: &str) -> bool;
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/manager/mod.rs` | 创建 | SkillManager 主模块 |
| `src-tauri/src/modules/skills/manager/actions.rs` | 创建 | 6 种 action 实现 |
| `src-tauri/src/modules/skills/manager/validator.rs` | 创建 | 前置条件验证 |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 manager 子模块 |
| `src-tauri/src/modules/tools/mod.rs` | 修改 | 注册 skill_manage tool |

### 5. review_checklist

- [ ] 6 种 action 全部实现 (create/edit/patch/delete/write_file/remove_file)
- [ ] 前置条件验证符合 Hermes 常量 (name 64char, desc 1024char, content 100k)
- [ ] create 前执行安全扫描
- [ ] patch 使用模糊匹配
- [ ] write_file/remove_file 仅限 allowed subdirs
- [ ] **Atomic writes** (temp file + rename, Hermes skill_manager_tool.py line 257)
- [ ] **Scan rollback on block** (backup + restore, Hermes skill_manager_tool.py lines 329-333)
- [ ] 错误消息清晰

---

## 6F.3 Hub Source Framework

### 1. 概述

实现 Hermes `tools/skills_hub.py` 的 SkillSource trait 和 GitHubSource adapter。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skills_hub.py`

**SkillSource ABC** (lines 252-278):
```python
class SkillSource(ABC):
    @abstractmethod
    def search(self, query: str, limit: int = 10) -> List[SkillMeta]: ...

    @abstractmethod
    def fetch(self, identifier: str) -> Optional[SkillBundle]: ...

    @abstractmethod
    def inspect(self, identifier: str) -> Optional[SkillMeta]: ...

    @abstractmethod
    def source_id(self) -> str: ...

    def trust_level_for(self, identifier: str) -> str:
        return "community"
```

**GitHubSource** (lines 284-405):
```python
class GitHubSource(SkillSource):
    DEFAULT_TAPS = [
        {"repo": "openai/skills", "path": "skills/"},
        {"repo": "anthropics/skills", "path": "skills/"},
        {"repo": "VoltAgent/awesome-agent-skills", "path": "skills/"},
        {"repo": "garrytan/gstack", "path": ""},
    ]

    def trust_level_for(self, identifier: str) -> str:
        # identifier format: "owner/repo/path/to/skill"
        parts = identifier.split("/", 2)
        if len(parts) >= 2:
            repo = f"{parts[0]}/{parts[1]}"
            if repo in TRUSTED_REPOS:
                return "trusted"
        return "community"
```

**GitHubAuth** (lines 129-245):
```python
class GitHubAuth:
    """GitHub API authentication. Tries methods in priority order:
      1. GITHUB_TOKEN / GH_TOKEN env var (PAT)
      2. `gh auth token` subprocess
      3. GitHub App JWT + installation token
      4. Unauthenticated (60 req/hr, public repos only)
    """
```

### 3. 实现细节

#### 3.1 目录结构

```
src-tauri/src/modules/skills/hub/
├── mod.rs          # Hub 主模块
├── source.rs      # SkillSource trait
├── github.rs      # GitHubSource + GitHubAuth
└── types.rs       # SkillMeta, SkillBundle
```

#### 3.2 source.rs

```rust
// src-tauri/src/modules/skills/hub/source.rs

use async_trait::async_trait;
use crate::modules::skills::guard::TrustLevel;

#[async_trait]
pub trait SkillSource: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMeta>, HubError>;
    async fn fetch(&self, identifier: &str) -> Result<Option<SkillBundle>, HubError>;
    async fn inspect(&self, identifier: &str) -> Result<Option<SkillMeta>, HubError>;
    fn source_id(&self) -> &'static str;
    fn trust_level_for(&self, identifier: &str) -> TrustLevel;
}

pub struct HubError {
    pub code: &'static str,
    pub message: String,
}
```

#### 3.3 types.rs

```rust
// src-tauri/src/modules/skills/hub/types.rs

use crate::modules::skills::guard::TrustLevel;

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
    pub files: HashMap<String, bytes::Bytes>,  // relative_path -> content
    pub source: String,
    pub identifier: String,
    pub trust_level: TrustLevel,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

#### 3.4 github.rs

```rust
// src-tauri/src/modules/skills/hub/github.rs

use super::source::{SkillSource, HubError};
use super::types::{SkillMeta, SkillBundle};
use crate::modules::skills::guard::{TrustLevel, TrustedRepos};

pub struct GitHubAuth {
    // Token resolution state
}

impl GitHubAuth {
    pub fn new() -> Self;
    pub fn get_headers(&self) -> HashMap<String, String>;
    pub fn is_authenticated(&self) -> bool;
    pub fn auth_method(&self) -> &'static str;
}

pub struct GitHubSource {
    auth: GitHubAuth,
    taps: Vec<Tap>,
}

struct Tap {
    repo: String,
    path: String,
}

impl GitHubSource {
    pub fn new(auth: GitHubAuth) -> Self;
    pub fn with_extra_taps(auth: GitHubAuth, taps: Vec<Tap>) -> Self;
}

#[async_trait]
impl SkillSource for GitHubSource {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMeta>, HubError>;
    async fn fetch(&self, identifier: &str) -> Result<Option<SkillBundle>, HubError>;
    async fn inspect(&self, identifier: &str) -> Result<Option<SkillMeta>, HubError>;
    fn source_id(&self) -> &'static str { "github" }
    fn trust_level_for(&self, identifier: &str) -> TrustLevel;
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/hub/mod.rs` | 创建 | Hub 主模块 |
| `src-tauri/src/modules/skills/hub/source.rs` | 创建 | SkillSource trait |
| `src-tauri/src/modules/skills/hub/types.rs` | 创建 | SkillMeta, SkillBundle |
| `src-tauri/src/modules/skills/hub/github.rs` | 创建 | GitHubSource + GitHubAuth |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 hub 子模块 |

### 5. review_checklist

- [ ] `SkillSource` trait 完整 (search/fetch/inspect/source_id/trust_level_for)
- [ ] `#[async_trait]` 使用正确
- [ ] `GitHubAuth` 支持 4 种认证方式 (token/gh CLI/GitHub App/anonymous)
- [ ] `GitHubSource` DEFAULT_TAPS 包含 4 个 tap
- [ ] `trust_level_for` 检查 TRUSTED_REPOS 白名单
- [ ] unified_search 去重逻辑正确

---

## 6F.4 Hub Source Implementations

### 1. 概述

实现剩余 6 个 Hub Source Adapter。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skills_hub.py`

**Source 列表** (根据代码结构推断):
1. `SkillsShSource` - skills.sh 索引
2. `ClawHubSource` - clawhub.ai
3. `ClaudeMarketplaceSource` - Claude Code marketplace
4. `LobeHubSource` - LobeHub agent marketplace
5. `WellKnownSource` - /.well-known/skills/index.json
6. `OptionalSkillSource` - 官方可选 skills (repo 内置)

### 3. 实现细节

#### 3.1 skills_sh.rs

```rust
// src-tauri/src/modules/skills/hub/skills_sh.rs

pub struct SkillsShSource {
    client: reqwest::Client,
}

impl SkillsShSource {
    pub fn new() -> Self;
}

#[async_trait]
impl SkillSource for SkillsShSource {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMeta>, HubError>;
    async fn fetch(&self, identifier: &str) -> Result<Option<SkillBundle>, HubError>;
    async fn inspect(&self, identifier: &str) -> Result<Option<SkillMeta>, HubError>;
    fn source_id(&self) -> &'static str { "skills-sh" }
    fn trust_level_for(&self, identifier: &str) -> TrustLevel { TrustLevel::Community }
}
```

#### 3.2 clawhub.rs

```rust
// src-tauri/src/modules/skills/hub/clawhub.rs

pub struct ClawHubSource {
    client: reqwest::Client,
    api_base: String,
}

impl ClawHubSource {
    pub fn new() -> Self;
}

#[async_trait]
impl SkillSource for ClawHubSource { /* ... */ }
```

#### 3.3 marketplace.rs

```rust
// src-tauri/src/modules/skills/hub/marketplace.rs

/// ClaudeMarketplaceSource - fetches from Claude Code marketplace repos
pub struct ClaudeMarketplaceSource { /* ... */ }

/// LobeHubSource - fetches from LobeHub agent marketplace
pub struct LobeHubSource { /* ... */ }
```

#### 3.4 well_known.rs

```rust
// src-tauri/src/modules/skills/hub/well_known.rs

/// WellKnownSource - reads /.well-known/skills/index.json
pub struct WellKnownSource {
    root_url: String,
}
```

#### 3.5 optional.rs

```rust
// src-tauri/src/modules/skills/hub/optional.rs

/// OptionalSkillSource - official optional skills shipped with repo
pub struct OptionalSkillSource {
    bundled_dir: PathBuf,
}

impl OptionalSkillSource {
    pub fn new(bundled_dir: PathBuf) -> Self;
}

#[async_trait]
impl SkillSource for OptionalSkillSource {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMeta>, HubError>;
    async fn fetch(&self, identifier: &str) -> Result<Option<SkillBundle>, HubError>;
    async fn inspect(&self, identifier: &str) -> Result<Option<SkillMeta>, HubError>;
    fn source_id(&self) -> &'static str { "optional" }
    fn trust_level_for(&self, identifier: &str) -> TrustLevel { TrustLevel::Builtin }
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/hub/skills_sh.rs` | 创建 | SkillsShSource (扩展现有) |
| `src-tauri/src/modules/skills/hub/clawhub.rs` | 创建 | ClawHubSource |
| `src-tauri/src/modules/skills/hub/marketplace.rs` | 创建 | ClaudeMarketplace + LobeHub |
| `src-tauri/src/modules/skills/hub/well_known.rs` | 创建 | WellKnownSource |
| `src-tauri/src/modules/skills/hub/optional.rs` | 创建 | OptionalSkillSource |

### 5. review_checklist

- [ ] 8 个 Source 实现完整 (GitHub + SkillsSh + ClawHub + ClaudeMarketplace + LobeHub + WellKnown + Optional + SkillsShSource)
- [ ] 每个 source_id 唯一
- [ ] 每个 trust_level_for 正确
- [ ] **LobeHubSource 集成 14,000+ marketplace** (Hermes skills_hub.py lines 1947-2104)
- [ ] **ClawHub 标记为 community trust** (有 ClawHavoc incident, Hermes lines 1368-1370)
- [ ] unified_search 按 trust 优先级去重 {builtin:2, trusted:1, community:0}

---

## 6F.5 Hub State Management

### 1. 概述

实现 Hermes `tools/skills_hub.py` 的 Hub 状态管理。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skills_hub.py`

**路径常量** (lines 46-53):
```python
HERMES_HOME = get_hermes_home()
SKILLS_DIR = HERMES_HOME / "skills"
HUB_DIR = SKILLS_DIR / ".hub"
LOCK_FILE = HUB_DIR / "lock.json"
QUARANTINE_DIR = HUB_DIR / "quarantine"
AUDIT_LOG = HUB_DIR / "audit.log"
TAPS_FILE = HUB_DIR / "taps.json"
INDEX_CACHE_DIR = HUB_DIR / "index-cache"
```

**HubLockFile** (根据代码结构):
```python
@dataclass
class HubLockEntry:
    skill_name: str
    source: str
    identifier: str
    installed_at: str
    version: str
```

**关键函数**:
- `quarantine_bundle(bundle)` - 隔离 bundle 到 quarantine/
- `install_from_quarantine(skill_name)` - 从 quarantine 安装
- `record_install(skill_name, source, identifier)` - 记录到 lock.json

### 3. 实现细节

#### 3.1 state.rs

```rust
// src-tauri/src/modules/skills/hub/state.rs

use super::source::SkillSource;
use super::types::SkillBundle;
use crate::modules::skills::guard::TrustLevel;

#[derive(Debug, Clone)]
pub struct HubPaths {
    pub skills_dir: PathBuf,
    pub hub_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub lock_file: PathBuf,
    pub audit_log: PathBuf,
    pub taps_file: PathBuf,
    pub index_cache_dir: PathBuf,
}

impl HubPaths {
    pub fn new(home_dir: PathBuf) -> Self;
    pub fn ensure_dirs(&self) -> Result<(), HubError>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HubLockEntry {
    pub skill_name: String,
    pub source: String,
    pub identifier: String,
    pub installed_at: chrono::DateTime<Utc>,
    pub version: String,
}

pub struct HubLock {
    entries: RwLock<Vec<HubLockEntry>>,
    lock_file: PathBuf,
}

impl HubLock {
    pub fn load(path: &Path) -> Result<Self, HubError>;
    pub fn save(&self) -> Result<(), HubError>;
    pub fn add(&self, entry: HubLockEntry) -> Result<(), HubError>;
    pub fn remove(&self, skill_name: &str) -> Result<(), HubError>;
    pub fn get(&self, skill_name: &str) -> Option<HubLockEntry>;
}

pub struct HubState {
    pub paths: HubPaths,
    pub sources: Vec<Box<dyn SkillSource>>,
    pub lock: HubLock,
}

impl HubState {
    pub fn new(home_dir: PathBuf) -> Result<Self, HubError>;
    pub fn with_sources(home_dir: PathBuf, sources: Vec<Box<dyn SkillSource>>) -> Result<Self, HubError>;

    pub fn quarantine_bundle(&self, bundle: &SkillBundle) -> Result<PathBuf, HubError>;
    pub fn install_from_quarantine(&self, skill_name: &str) -> Result<PathBuf, HubError>;
    pub fn record_install(&self, entry: HubLockEntry) -> Result<(), HubError>;

    pub async fn unified_search(&self, query: &str, source_filter: Option<&str>, limit: usize) -> Result<Vec<SkillMeta>, HubError>;

    pub fn append_audit_log(&self, event: &AuditEvent) -> Result<(), HubError>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEvent {
    pub timestamp: chrono::DateTime<Utc>,
    pub event_type: String,  // "install", "remove", "scan", "quarantine"
    pub skill_name: String,
    pub source: String,
    pub identifier: String,
    pub details: Option<String>,
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/hub/state.rs` | 创建 | HubState + HubLock + AuditEvent |

### 5. review_checklist

- [ ] HubPaths 所有路径正确
- [ ] HubLock 读写 lock.json 正确
- [ ] quarantine_bundle 隔离到 quarantine/
- [ ] install_from_quarantine 从 quarantine 安装
- [ ] append_audit_log 追加到 audit.log
- [ ] unified_search 去重 + trust 优先
- [ ] **TapsManager 支持 taps.json** (Hermes skills_hub.py lines 2397-2435)
- [ ] **check_for_skill_updates 检查上游更新** (Hermes skills_hub.py line 2598)
- [ ] **uninstall_skill 卸载 hub skill** (Hermes skills_hub.py line 2565)

---

## 6F.6 Skill Sync Manifest

### 1. 概述

实现 Hermes `tools/skills_sync.py` 的 manifest 管理。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/tools/skills_sync.py`

**Manifest 格式** (lines 52-75):
```python
# v2 format: each line is "skill_name:origin_hash"
# v1 format: plain names (auto-migrated)

def _read_manifest() -> Dict[str, str]:
    # Handle both v1 and v2 formats
    for line in MANIFEST_FILE.read_text().splitlines():
        if ":" in line:
            name, _, hash_val = line.partition(":")
            result[name.strip()] = hash_val.strip()
        else:
            result[line] = ""  # v1: empty hash triggers migration
```

**同步逻辑** (lines 155-280):
```python
def sync_skills() -> dict:
    # NEW: copy to user dir, record hash
    # EXISTING + user unchanged: safe to update
    # EXISTING + user modified: SKIP
    # DELETED by user: respected, not re-added
```

**Hash 计算** (lines 141-152):
```python
def _dir_hash(directory: Path) -> str:
    hasher = hashlib.md5()
    for fpath in sorted(directory.rglob("*")):
        if fpath.is_file():
            rel = fpath.relative_to(directory)
            hasher.update(str(rel).encode("utf-8"))
            hasher.update(fpath.read_bytes())
    return hasher.hexdigest()
```

### 3. 实现细节

#### 3.1 manifest.rs

```rust
// src-tauri/src/modules/skills/sync/manifest.rs

pub struct SkillManifest {
    entries: HashMap<String, String>,  // skill_name -> origin_hash
}

impl SkillManifest {
    pub fn read(manifest_path: &Path) -> Result<Self, SyncError>;
    pub fn write(&self, manifest_path: &Path) -> Result<(), SyncError>;
    pub fn get_hash(&self, skill_name: &str) -> Option<&str>;
    pub fn set_hash(&mut self, skill_name: &str, hash: &str);
    pub fn remove(&mut self, skill_name: &str);
    pub fn has_skill(&self, skill_name: &str) -> bool;
}

pub struct ManifestReader;
impl ManifestReader {
    pub fn read_v2(content: &str) -> HashMap<String, String>;
    pub fn read_v1(content: &str) -> HashMap<String, String>;  // Returns empty hash
}
```

#### 3.2 mod.rs

```rust
// src-tauri/src/modules/skills/sync/mod.rs

mod manifest;

pub use manifest::{SkillManifest, ManifestReader};

pub struct SkillSync {
    bundled_dir: PathBuf,
    user_dir: PathBuf,
    manifest_path: PathBuf,
}

impl SkillSync {
    pub fn new(bundled_dir: PathBuf, user_dir: PathBuf) -> Self {
        let manifest_path = user_dir.join(".bundled_manifest");
        Self { bundled_dir, user_dir, manifest_path }
    }

    pub fn sync(&self) -> Result<SyncResult, SyncError>;
    pub fn discover_bundled_skills(&self) -> Vec<(String, PathBuf)>;
    pub fn compute_dir_hash(dir: &Path) -> String;
}

pub struct SyncResult {
    pub copied: Vec<String>,
    pub updated: Vec<String>,
    pub skipped: usize,
    pub user_modified: Vec<String>,
    pub cleaned: Vec<String>,
    pub total_bundled: usize,
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/sync/mod.rs` | 创建 | SkillSync 主模块 |
| `src-tauri/src/modules/skills/sync/manifest.rs` | 创建 | Manifest 读写 |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 sync 子模块 |

### 5. review_checklist

- [ ] manifest 支持 v1 (plain name) 和 v2 (name:hash) 格式
- [ ] 用户修改检测 (hash 不匹配则跳过)
- [ ] 备份 + restore on failure
- [ ] 新增/更新/跳过/清理 逻辑正确
- [ ] 清理已删除 bundled skill 的 manifest 条目

---

## 6F.7 Skill Commands

### 1. 概述

实现 Hermes `agent/skill_commands.py` 的 slash 命令集成。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/agent/skill_commands.py`

**scan_skill_commands** (lines 200-262):
```python
def scan_skill_commands() -> Dict[str, Dict[str, Any]]:
    """Scan ~/.hermes/skills/ and return a mapping of /command -> skill info."""
    global _skill_commands
    _skill_commands = {}
    # Scan SKILLS_DIR and external dirs
    # Parse frontmatter with _parse_frontmatter
    # Check platform compatibility
    # Respect disabled skills config
    # Normalize name to hyphen-separated slug
    return _skill_commands
```

**build_skill_invocation_message** (lines 291-326):
```python
def build_skill_invocation_message(
    cmd_key: str,
    user_instruction: str = "",
    task_id: str | None = None,
    runtime_note: str = "",
) -> Optional[str]:
    """Build the user message content for a skill slash command invocation."""
    # Load skill via skill_view
    # Build activation note: '[SYSTEM: The user has invoked the "X" skill...]'
    # Format with _build_skill_message
    return message
```

**_build_skill_message** (lines 121-197):
```python
def _build_skill_message(
    loaded_skill: dict[str, Any],
    skill_dir: Path | None,
    activation_note: str,
    user_instruction: str = "",
    runtime_note: str = "",
) -> str:
    """Format a loaded skill into a user/system message payload."""
    # Include skill content
    # Inject config values via _inject_skill_config
    # Add setup notes if needed
    # List supporting files
    # Add user instruction if provided
    # Add runtime note if provided
```

**名称规范化** (lines 244-251):
```python
cmd_name = name.lower().replace(' ', '-').replace('_', '-')
cmd_name = _SKILL_INVALID_CHARS.sub('', cmd_name)
cmd_name = _SKILL_MULTI_HYPHEN.sub('-', cmd_name).strip('-')
```

### 3. 实现细节

#### 3.1 commands.rs

```rust
// src-tauri/src/modules/skills/commands.rs

use std::sync::RwLock;
use once_cell::sync::Lazy;

static SKILL_COMMANDS: Lazy<RwLock<HashMap<String, SkillCommandInfo>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct SkillCommandInfo {
    pub name: String,
    pub description: String,
    pub skill_md_path: PathBuf,
    pub skill_dir: PathBuf,
}

pub struct SkillCommands;

impl SkillCommands {
    /// Scan skills directory and build command map
    pub fn scan(workdir: &Path) -> Result<HashMap<String, SkillCommandInfo>, CommandError>;

    /// Get cached command map
    pub fn get() -> HashMap<String, SkillCommandInfo>;

    /// Resolve command alias (space/underscore -> hyphen)
    pub fn resolve(command: &str) -> Option<String>;

    /// Build invocation message for /skill-name
    pub fn build_invocation_message(
        &self,
        cmd_key: &str,
        user_instruction: &str,
        runtime_note: &str,
    ) -> Result<String, CommandError>;

    /// Build preloaded skills prompt
    pub fn build_preloaded_prompt(
        &self,
        identifiers: &[String],
    ) -> Result<(String, Vec<String>, Vec<String>), CommandError>;
}

pub struct SkillInvocationBuilder {
    pub activation_note: String,
    pub skill_content: String,
    pub setup_notes: Vec<String>,
    pub supporting_files: Vec<String>,
    pub config_block: Option<String>,
}

impl SkillInvocationBuilder {
    pub fn from_skill(skill_dir: &Path, skill_content: &str) -> Result<Self, CommandError>;
    pub fn with_config(self, config: HashMap<String, String>) -> Self;
    pub fn with_user_instruction(self, instruction: &str) -> Self;
    pub fn with_runtime_note(self, note: &str) -> Self;
    pub fn build(self) -> String;
}

/// Normalize skill name to command key (spaces/underscores -> hyphens)
pub fn normalize_command_key(name: &str) -> String;
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/commands.rs` | 创建 | SkillCommands |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 commands 子模块 |
| `src-tauri/src/commands/slash.rs` | 修改 | 添加 /skill-name 处理 |
| `src-tauri/src/modules/tools/mod.rs` | 修改 | 注册 skill_manage tool |

### 5. review_checklist

- [ ] scan_skill_commands 动态扫描 skills/ 目录
- [ ] 名称规范化 (space/underscore → hyphen)
- [ ] platform 兼容性检查
- [ ] disabled skills 读取
- [ ] activation_note 格式正确
- [ ] supporting files 列表
- [ ] config_block 注入 (如果需要)
- [ ] **Additional CLI commands**: browse, search, inspect, list, check, update, audit, uninstall
- [ ] **publish command**: to GitHub PR or ClawHub
- [ ] **snapshot export/import** (Hermes hermes_cli/skills_hub.py lines 877-962)

---

## 6F.8 Skill Config Variables

### 1. 概述

实现 Hermes `agent/skill_commands.py` 的 config 变量解析。

### 2. Hermes 参考代码

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/agent/skill_commands.py`

**_inject_skill_config** (lines 82-118):
```python
def _inject_skill_config(loaded_skill: dict[str, Any], parts: list[str]) -> None:
    """Resolve and inject skill-declared config values into the message parts."""
    frontmatter, _ = parse_frontmatter(raw_content)
    config_vars = extract_skill_config_vars(frontmatter)
    resolved = resolve_skill_config_values(config_vars)
    # Format as:
    # [Skill config (from ~/.hermes/config.yaml):
    #   key = value
    # ]
```

**skill_utils.py extract_skill_config_vars** (lines 180-200):
```python
def extract_skill_config_vars(frontmatter: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Extract config variables from frontmatter."""
    # Look for metadata.hermes.config
    # Returns list of {key, description, default}
```

**Frontmatter 格式**:
```yaml
metadata:
  hermes:
    config:
      - key: wiki.path
        description: Path to the LLM Wiki knowledge base
        default: "~/wiki"
```

### 3. 实现细节

#### 3.1 config.rs

```rust
// src-tauri/src/modules/skills/config.rs

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

/// Extract config vars from frontmatter YAML
pub fn extract_config_vars(frontmatter: &serde_json::Value) -> Vec<SkillConfigVar>;

/// Format resolved config as block string
pub fn format_config_block(resolved: &HashMap<String, String>) -> String;
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/config.rs` | 创建 | SkillConfigResolver |
| `src-tauri/src/modules/skills/mod.rs` | 修改 | 注册 config 子模块 |

### 5. review_checklist

- [ ] extract_config_vars 从 metadata.hermes.config 解析
- [ ] resolve_skill_config_values 从 config.yaml 获取值
- [ ] format_config_block 格式: `[Skill config (from ...): key = value]`
- [ ] 默认值处理正确
- [ ] **prompt field 支持** (Hermes skill_utils.py extract_skill_config_vars)
- [ ] **conditional activation**: requires_toolsets, fallback_for_toolsets, requires_tools, fallback_for_tools (Hermes skill_utils.py lines 240-258)
- [ ] **external skills dirs**: skills.external_dirs config (Hermes skill_utils.py lines 173-225)

---

## 6F.9 Frontend UI Extensions

### 1. 概述

实现前端 Skill Hub UI 扩展。

### 2. Hermes 参考

**文件**: `/Users/ryanliu/Documents/IfAI/hermes-agent-main/` (无直接前端，CLI 为主)

### 3. 实现细节

#### 3.1 SkillsHubView.tsx

```tsx
// src/modules/skills/SkillsHubView.tsx

export interface HubSearchResult {
  name: string;
  description: string;
  source: string;
  identifier: string;
  trust_level: 'builtin' | 'trusted' | 'community';
  tags: string[];
}

export function SkillsHubView() {
  // Hub search interface
  // Source filter tabs
  // Install/Preview actions
  // Quarantine management
}
```

#### 3.2 SkillEditor.tsx

```tsx
// src/modules/skills/SkillEditor.tsx

export interface SkillEditorProps {
  skillName?: string;
  mode: 'create' | 'edit';
}

export function SkillEditor({ skillName, mode }: SkillEditorProps) {
  // create/edit/patch UI
  // Frontmatter validation
  // Content size indicator
  // Supporting files manager
  // Security scan status
}
```

#### 3.3 SkillSecurityReport.tsx

```tsx
// src/modules/skills/SkillSecurityReport.tsx

export interface SecurityFinding {
  pattern_id: string;
  severity: 'critical' | 'high' | 'medium' | 'low';
  category: string;
  file: string;
  line: number;
  match: string;
  description: string;
}

export function SkillSecurityReport({ findings }: { findings: SecurityFinding[] }) {
  // Render scan findings
  // Severity indicators
  // Category badges
  // Action buttons (Block/Allow/Warning)
}
```

### 4. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/modules/skills/SkillsHubView.tsx` | 创建 | Hub 市场 UI |
| `src/modules/skills/SkillEditor.tsx` | 创建 | Skill 编辑器 |
| `src/modules/skills/SkillSecurityReport.tsx` | 创建 | 安全报告 UI |
| `src/modules/settings/pages/SkillsSettingsPage.tsx` | 修改 | 集成新组件 |

### 5. review_checklist

- [ ] SkillsHubView 支持 unified_search + source filter
- [ ] SkillEditor 支持 create/edit/patch/preview
- [ ] SkillSecurityReport 显示所有 finding 字段
- [ ] 与现有 SkillsSettingsPage 集成

---

## 6F.10 Integration Tests

### 1. 概述

为所有 Skill Control Plane v2 功能编写集成测试。

### 2. 测试覆盖

| 模块 | 测试用例 |
|------|---------|
| SkillsGuard | 60+ patterns, trust policy (AgentCreated:ask), invisible unicode, structural limits |
| SkillManager | 6 action CRUD + atomic writes + scan rollback |
| Hub Source | 8 sources, LobeHub integration |
| Hub State | quarantine, lock, audit, taps, check_for_updates |
| SkillSync | manifest v1/v2, user modification detection |
| SkillCommands | scan, resolve, invocation + additional CLI |
| SkillConfig | extract, resolve, format + conditional activation |
| SkillExternal | external dirs, remote passthrough |
| SkillSnapshot | export/import JSON |

### 3. impl_targets

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/modules/skills/guard/tests.rs` | 创建 | SkillsGuard tests |
| `src-tauri/src/modules/skills/manager/tests.rs` | 创建 | SkillManager tests |
| `src-tauri/src/modules/skills/hub/tests.rs` | 创建 | Hub tests |
| `src-tauri/src/modules/skills/sync/tests.rs` | 创建 | SkillSync tests |
| `src-tauri/src/modules/skills/commands/tests.rs` | 创建 | SkillCommands tests |
| `src-tauri/src/modules/skills/external/tests.rs` | 创建 | SkillExternal tests |
| `src-tauri/src/modules/skills/snapshot/tests.rs` | 创建 | SkillSnapshot tests |

---

## 附录: Hermes 代码引用索引

### 核心文件引用

| Hermes 文件 | 行数 | Backlog 引用 |
|------------|------|-------------|
| `tools/skills_guard.py` | 56-76 | 6F.1 ScanResult, Finding |
| `tools/skills_guard.py` | 39-47 | 6F.1 INSTALL_POLICY (含 AgentCreated:ask) |
| `tools/skills_guard.py` | 82-484 | 6F.1 THREAT_PATTERNS (60+ patterns) |
| `tools/skills_guard.py` | 487-489 | 6F.1 STRUCTURAL_LIMITS |
| `tools/skills_guard.py` | 505-523 | 6F.1 INVISIBLE_UNICODE |
| `skill_manager_tool.py` | 14-20 | 6F.2 Actions |
| `skill_manager_tool.py` | 83-92 | 6F.2 Validator constants |
| `skill_manager_tool.py` | 99-174 | 6F.2 Validation functions |
| `skill_manager_tool.py` | 48-74 | 6F.2 Security scan |
| `skill_manager_tool.py` | 257 | 6F.2 Atomic writes |
| `skill_manager_tool.py` | 329-333 | 6F.2 Scan rollback |
| `skills_hub.py` | 252-278 | 6F.3 SkillSource ABC |
| `skills_hub.py` | 284-405 | 6F.3 GitHubSource |
| `skills_hub.py` | 129-245 | 6F.3 GitHubAuth |
| `skills_hub.py` | 1947-2104 | 6F.4 LobeHubSource |
| `skills_hub.py` | 1368-1370 | 6F.4 ClawHub community trust |
| `skills_hub.py` | 2397-2435 | 6F.5 TapsManager |
| `skills_hub.py` | 2598 | 6F.5 check_for_updates |
| `skills_hub.py` | 2565 | 6F.5 uninstall_skill |
| `skills_hub.py` | 46-53 | 6F.5 HubPaths |
| `skills_sync.py` | 52-108 | 6F.6 Manifest format |
| `skills_sync.py` | 155-280 | 6F.6 sync_skills logic |
| `skills_sync.py` | 141-152 | 6F.6 _dir_hash |
| `skill_commands.py` | 200-262 | 6F.7 scan_skill_commands |
| `skill_commands.py` | 291-326 | 6F.7 build_skill_invocation_message |
| `skill_commands.py` | 121-197 | 6F.7 _build_skill_message |
| `hermes_cli/skills_hub.py` | 183-306 | 6F.7 CLI browse/search |
| `hermes_cli/skills_hub.py` | 557-632 | 6F.7 CLI check/update |
| `hermes_cli/skills_hub.py` | 877-962 | 6F.7 Snapshot export/import |
| `skill_commands.py` | 82-118 | 6F.8 _inject_skill_config |
| `skill_utils.py` | 180-200 | 6F.8 extract_skill_config_vars |
| `skill_utils.py` | 240-258 | 6F.8 Conditional activation |
| `skill_utils.py` | 173-225 | 6F.9 External skills dirs |
| `skills_tool.py` | 209-273 | 6F.9 Required credential files |
| `skills_tool.py` | 1161-1179 | 6F.9 Remote backend env |

### Additional Hermes CLI Commands

| Hermes 文件 | 行数 | CLI Command | Backlog |
|------------|------|-------------|---------|
| `hermes_cli/skills_hub.py` | 183 | browse | 6F.7 |
| `hermes_cli/skills_hub.py` | 144 | search | 6F.7 |
| `hermes_cli/skills_hub.py` | 449 | inspect | 6F.7 |
| `hermes_cli/skills_hub.py` | 499 | list | 6F.7 |
| `hermes_cli/skills_hub.py` | 557 | check | 6F.7 |
| `hermes_cli/skills_hub.py` | 580 | update | 6F.7 |
| `hermes_cli/skills_hub.py` | 600 | audit | 6F.7 |
| `hermes_cli/skills_hub.py` | 633 | uninstall | 6F.7 |
| `hermes_cli/skills_hub.py` | 711 | publish | 6F.7 |
| `hermes_cli/skills_hub.py` | 877 | snapshot export | 6F.10 |
| `hermes_cli/skills_hub.py` | 917 | snapshot import | 6F.10 |
| `hermes_cli/skills_hub.py` | 668 | tap | 6F.10 |

### Complete Threat Pattern Reference

| Category | Pattern ID | Line | Severity |
|----------|------------|------|----------|
| Exfiltration | env_exfil_curl | 84 | critical |
| Exfiltration | hermes_env_access | 119 | critical |
| Injection | prompt_injection_ignore | 160 | critical |
| Destructive | destructive_root_rm | 198 | critical |
| Persistence | ssh_backdoor | 230 | critical |
| Network | reverse_shell | 253 | critical |
| Obfuscation | echo_pipe_exec | 294 | critical |
| Crypto | crypto_mining | 363 | critical |
| SupplyChain | curl_pipe_shell | 371 | critical |
| PrivilegeEscalation | setuid_setgid | 413 | critical |
| AgentConfig | agent_config_mod | 424 | critical |
| Credential | openai_key_leaked | 444 | critical |
| Jailbreak | jailbreak_dan | 455 | critical |
| ContextExfil | context_exfil | 478 | high |
| InvisibleUnicode | zero_width | 505-523 | high |
