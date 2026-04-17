# ADR-013 Medium Gaps — 详细实施 backlog

## 概述

**目标**: 修复 Phase 6B/6BW 中的 7 个 Medium severity gaps — 质量改进、缺失工具和安全基础设施。
**ADR**: [ADR-013](./ADR-013-Phase-Remediation-Design.md)
**优先级**: P2 (在 Critical 和 High gaps 之后修复)
**预估工时**: 2-3 天
**基于审计**: [phase-6b-6bw-6e-gap-audit-report.md](../../../../generated/phase-6b-6bw-6e-gap-audit-report.md) v2

**依赖**: M7（安全集成测试）依赖 M5（ThreatScanner）先创建。其他 Medium gaps 相互独立。

---

## 子任务清单

### TASK-013-M1: No JSON-to-SQLite Migration Tool

**对应 TASK**: 001-08
**对应 ADR**: ADR-001 (SQLite P0 Persistence)

#### 目的

早期版本的 legacy `memory.json` 文件无法导入 SQLite。升级时拥有现有数据的用户会丢失记忆。

#### 实施计划

在 `SqliteMemoryProvider` 中添加 `migrate_from_json()`：

File: `src-tauri/src/modules/memory/providers/sqlite_provider.rs`

```rust
use std::path::Path;

impl SqliteMemoryProvider {
    /// Migrate entries from a legacy JSON file into SQLite.
    ///
    /// Returns the number of entries successfully migrated.
    /// Failed entries are skipped with a warning, migration continues.
    pub async fn migrate_from_json(&self, json_path: &Path) -> Result<usize, MemoryError> {
        let content = tokio::fs::read_to_string(json_path).await
            .map_err(|e| MemoryError::Generic(format!("read json: {e}")))?;

        let entries: Vec<MemoryEntry> = serde_json::from_str(&content)
            .map_err(|e| MemoryError::Generic(format!("parse json: {e}")))?;

        let mut count = 0;
        for entry in entries {
            // Use store() which handles INSERT OR REPLACE
            match self.store(entry).await {
                Ok(()) => count += 1,
                Err(e) => {
                    tracing::warn!(
                        "[migrate] Failed to migrate entry: {e}, skipping"
                    );
                }
            }
        }

        tracing::info!("[migrate] Migrated {count}/{total} entries from {path:?}",
            path = json_path, total = entries.len());

        Ok(count)
    }
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/providers/sqlite_provider.rs` | 添加 `migrate_from_json()` 方法 |

#### 验收标准

- [ ] `migrate_from_json()` 读取 JSON 文件并解析为 `Vec<MemoryEntry>`
- [ ] 每个条目通过 `store()` 写入 SQLite
- [ ] 失败条目跳过并记录 warning，不中断迁移
- [ ] 返回成功迁移的条目数量

---

### TASK-013-M2: Token Counting Uses 4:1 Heuristic Instead of tiktoken-rs

**对应 TASK**: 004-03
**对应 ADR**: ADR-004 (Token Budget Allocation)

#### 目的

`char_count / 4 + 1` 启发式对非英文文本和工具消息不准确。这导致预算强制执行不可靠。

#### 实施计划

**Step 1**: 在 `Cargo.toml` 中添加 `tiktoken-rs` 依赖

File: `src-tauri/Cargo.toml`

```toml
tiktoken-rs = "0.6"
```

**Step 2**: 用基于 tiktoken 的估算替换 `estimate_token_count_from_chars`

File: `src-tauri/src/modules/runtime/compact.rs`

```rust
use tiktoken_rs::cl100k_base;

pub fn estimate_token_count_from_chars(text: &str) -> usize {
    let bpe = cl100k_base().expect("tiktoken model unavailable");
    bpe.encode_with_special_tokens(text).len()
}
```

File: `src-tauri/src/modules/runtime/budget.rs` — 同样替换启发式估算

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/Cargo.toml` | 添加 `tiktoken-rs` 依赖 |
| `src-tauri/src/modules/runtime/compact.rs` | 用 tiktoken 替换启发式 |
| `src-tauri/src/modules/runtime/budget.rs` | 用 tiktoken 替换启发式 |

#### 验收标准

- [ ] `tiktoken-rs` 依赖已添加到 `Cargo.toml`
- [ ] `estimate_token_count_from_chars` 使用 cl100k_base 编码器
- [ ] 现有测试通过（token 计数更准确，不影响测试逻辑）

#### 风险

- **初始化成本**: tiktoken 模型首次加载 ~1ms。可接受。
- **依赖大小**: `tiktoken-rs` 增加约 1MB 二进制大小。可接受。

---

### TASK-013-M3: No YAML Budget Configuration Loading

**对应 TASK**: 004-08
**对应 ADR**: ADR-004 (Token Budget Allocation)

#### 目的

ContextBudget 使用硬编码默认值（4000 tokens, 10/20/30/40%）。用户无法在不修改代码的情况下自定义。

#### 实施计划

添加 `BudgetConfig` struct 带 YAML 反序列化：

File: `src-tauri/src/modules/runtime/budget.rs`

```rust
use serde::Deserialize;
use std::path::Path;

/// Configurable budget parameters loaded from YAML.
///
/// Default: total=4000, system=10%, episodic=20%, semantic=30%, working=40%.
#[derive(Debug, Clone, Deserialize)]
pub struct BudgetConfig {
    pub total_tokens: usize,
    pub system_pct: Option<f32>,
    pub episodic_pct: Option<f32>,
    pub semantic_pct: Option<f32>,
    pub working_pct: Option<f32>,
}

impl BudgetConfig {
    /// Load configuration from a YAML file.
    ///
    /// Returns defaults if the file cannot be read or parsed.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(e.to_string()))?;
        serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::Yaml(e.to_string()))
    }

    /// Build a ContextBudget from this config.
    pub fn into_budget(self) -> ContextBudget {
        ContextBudget::new(
            self.total_tokens,
            self.system_pct.unwrap_or(10.0),
            self.episodic_pct.unwrap_or(20.0),
            self.semantic_pct.unwrap_or(30.0),
            self.working_pct.unwrap_or(40.0),
        )
    }
}

/// Error type for configuration loading
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("YAML parse error: {0}")]
    Yaml(String),
}
```

示例 YAML 配置：

```yaml
total_tokens: 8000
system_pct: 10
episodic_pct: 20
semantic_pct: 30
working_pct: 40
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/runtime/budget.rs` | 添加 `BudgetConfig` 带 YAML 加载 |
| `src-tauri/Cargo.toml` | 添加 `serde_yaml` 依赖（如果尚未存在） |

#### 验收标准

- [ ] `BudgetConfig` struct 可从 YAML 文件加载
- [ ] 缺失字段使用默认值
- [ ] `into_budget()` 生成有效的 `ContextBudget`
- [ ] 文件不存在时使用硬编码默认值

---

### TASK-013-M4: Session Schema Partially Incompatible with claw-cli

**对应 TASK**: 005-02
**对应 ADR**: ADR-005 (Upstream Claw-CLI Relationship)

#### 目的

Session schema 缺少 `project_id`、`title`、`created_at`、`updated_at`、`token_count`、`pinned` 字段，这些是 claw-cli 兼容性所需的。

#### 实施计划

在 Session struct 中添加缺失字段：

File: `src-tauri/src/modules/session/mod.rs`

```rust
pub struct Session {
    pub id: String,
    pub messages: Vec<ConversationMessage>,
    pub version: u32,
    // NEW: claw-cli compatibility fields
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub token_count: usize,
    pub pinned: bool,
}
```

在构造函数中初始化这些字段：

```rust
impl Session {
    pub fn new() -> Self {
        Self {
            id: generate_id(),
            messages: Vec::new(),
            version: 1,
            project_id: None,
            title: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            token_count: 0,
            pinned: false,
        }
    }
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/session/mod.rs` | 添加缺失的 schema 字段 |

#### 验收标准

- [ ] Session struct 拥有全部 6 个新字段
- [ ] `Session::new()` 正确初始化所有字段
- [ ] 序列化/反序列化包含新字段
- [ ] 现有测试通过

---

### TASK-013-M5: ThreatScanner Module Doesn't Exist

**对应 TASK**: 006-05
**对应 ADR**: ADR-006 (Security Design)

#### 目的

没有基于正则表达式的扫描来检测记忆内容存储前的密钥、API key 或敏感模式。

#### 实施计划

创建 `security/scanner.rs`：

File: `src-tauri/src/modules/memory/security/mod.rs`

```rust
pub mod scanner;
pub use scanner::{Threat, ThreatScanner, ThreatType};
```

File: `src-tauri/src/modules/memory/security/scanner.rs`

```rust
use regex::Regex;

/// Type of detected threat in memory content
#[derive(Debug, Clone)]
pub enum ThreatType {
    ApiKey,
    PrivateKey,
    Password,
    SecretToken,
}

/// A detected threat with its location
#[derive(Debug, Clone)]
pub struct Threat {
    pub threat_type: ThreatType,
    pub matched_text: String,
    pub position: usize,
}

/// Regex-based scanner for sensitive patterns in memory content
pub struct ThreatScanner {
    patterns: Vec<(ThreatType, Regex)>,
}

impl ThreatScanner {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                (ThreatType::ApiKey, Regex::new(r"(?i)(api[_-]?key|apikey)\s*[:=]\s*[A-Za-z0-9]{20,}").unwrap()),
                (ThreatType::PrivateKey, Regex::new(r"-----BEGIN (RSA |EC )?PRIVATE KEY-----").unwrap()),
                (ThreatType::Password, Regex::new(r"(?i)(password|passwd|pwd)\s*[:=]\s*\S+").unwrap()),
                (ThreatType::SecretToken, Regex::new(r"(?i)(secret|token)\s*[:=]\s*[A-Za-z0-9+/]{20,}={0,2}").unwrap()),
            ],
        }
    }

    /// Scan content for sensitive patterns. Returns all matches.
    pub fn scan(&self, content: &str) -> Vec<Threat> {
        let mut threats = Vec::new();
        for (threat_type, pattern) in &self.patterns {
            for mat in pattern.find_iter(content) {
                threats.push(Threat {
                    threat_type: threat_type.clone(),
                    matched_text: mat.as_str().to_string(),
                    position: mat.start(),
                });
            }
        }
        threats
    }

    /// Check if content contains any threats. Returns true if threats found.
    pub fn has_threats(&self, content: &str) -> bool {
        !self.scan(content).is_empty()
    }
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/security/scanner.rs` | **创建** — ThreatScanner |
| `src-tauri/src/modules/memory/security/mod.rs` | **创建** — 模块根 |
| `src-tauri/Cargo.toml` | 添加 `regex` 依赖（如果尚未存在） |

#### 验收标准

- [ ] ThreatScanner 可检测 API key、私钥、密码、secret token
- [ ] `scan()` 返回所有匹配项及位置
- [ ] `has_threats()` 正确检测是否存在威胁
- [ ] 正则表达式不产生误报（测试验证）

---

### TASK-013-M6: FrozenSnapshot Verification Failure Has No Warning Log Details

**对应 TASK**: 006-06
**对应 ADR**: ADR-012 (Wiring Layer 2)

#### 目的

`FrozenSnapshot::verify()` 返回 `bool` 但验证失败路径（agent.rs:987-991）仅记录 `warn!` 而没有关于变更内容的额外上下文。

#### 实施计划

增强 `FrozenSnapshot::verify` 返回 `VerifyResult` 带详细信息：

File: `src-tauri/src/modules/runtime/snapshot.rs`

```rust
/// Result of a frozen snapshot verification
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub valid: bool,
    pub expected_hash: String,
    pub actual_hash: Option<String>,
}

impl FrozenSnapshot {
    /// Verify the current system prompt against the captured snapshot.
    ///
    /// Returns a detailed result showing expected vs actual hash.
    pub fn verify_detailed(&self, current_prompt: &str) -> VerifyResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        current_prompt.hash(&mut hasher);
        let actual_hash = format!("{:x}", hasher.finish());

        VerifyResult {
            valid: self.prompt_hash == actual_hash,
            expected_hash: self.prompt_hash.clone(),
            actual_hash: Some(actual_hash),
        }
    }
}
```

在 agent.rs 中使用详细验证：

```rust
let verify_result = frozen_snapshot.verify_detailed(&system_prompt_text);
if !verify_result.valid {
    tracing::warn!(
        "[run_agent_turn] System prompt integrity check FAILED: expected={}, actual={}",
        verify_result.expected_hash,
        verify_result.actual_hash.unwrap_or_else(|| "unknown".into())
    );
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/runtime/snapshot.rs` | 增强 verify 返回详细结果 |
| `src-tauri/src/commands/agent.rs` | 使用 `verify_detailed()` 获取详细日志 |

#### 验收标准

- [ ] `verify_detailed()` 返回 `VerifyResult` 包含 expected 和 actual hash
- [ ] 失败日志显示具体的 hash 对比
- [ ] 现有 `verify()` bool 方法保留（向后兼容）

---

### TASK-013-M7: Security Integration Tests Missing

**对应 TASK**: 006-07
**对应 ADR**: ADR-006 (Security Design)

#### 目的

没有集成测试验证安全层（输入验证、路径验证、原子写入、访问控制）端到端工作。

#### 实施计划

创建 `src-tauri/tests/security_integration.rs`：

File: `src-tauri/tests/security_integration.rs`

```rust
use if2ai::modules::memory::security::scanner::{ThreatScanner, ThreatType};

#[test]
fn test_api_key_detected() {
    let scanner = ThreatScanner::new();
    let content = "my api_key=abcdef1234567890abcdef1234567890abcdef";
    let threats = scanner.scan(content);
    assert!(!threats.is_empty());
    assert!(matches!(threats[0].threat_type, ThreatType::ApiKey));
}

#[test]
fn test_private_key_detected() {
    let scanner = ThreatScanner::new();
    let content = "-----BEGIN RSA PRIVATE KEY-----";
    let threats = scanner.scan(content);
    assert!(!threats.is_empty());
    assert!(matches!(threats[0].threat_type, ThreatType::PrivateKey));
}

#[test]
fn test_clean_content_no_threats() {
    let scanner = ThreatScanner::new();
    let content = "This is a clean memory entry with no secrets.";
    assert!(!scanner.has_threats(content));
}

#[test]
fn test_path_traversal_blocked() {
    // Use validate_safe_path from memory module
    use if2ai::modules::memory::validate_safe_path;
    assert!(validate_safe_path("/safe/path/memory.db").is_ok());
    assert!(validate_safe_path("../../etc/passwd").is_err());
}

#[test]
fn test_xss_input_rejected() {
    // Use validate_memory_entry from memory module
    use if2ai::modules::memory::validate_memory_entry;
    assert!(validate_memory_entry("clean content").is_ok());
    assert!(validate_memory_entry("<script>alert(1)</script>").is_err());
}

#[test]
fn test_atomic_write_rollback() {
    // Verify atomic_write uses temp file + rename pattern
    use if2ai::modules::memory::atomic_write;
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_atomic_write.txt");
    atomic_write(&path, "test content".as_bytes()).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "test content");
    let _ = std::fs::remove_file(&path);
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/tests/security_integration.rs` | **创建** — 安全集成测试 |

#### 验收标准

- [ ] `test_api_key_detected` 检测 API key 模式
- [ ] `test_private_key_detected` 检测私钥
- [ ] `test_clean_content_no_threats` 对清洁内容无误报
- [ ] `test_path_traversal_blocked` 阻止路径遍历
- [ ] `test_xss_input_rejected` 拒绝 XSS 输入
- [ ] `test_atomic_write_rollback` 验证原子写入模式
- [ ] 所有测试通过

---

## 验收总览

- [ ] M1: `migrate_from_json()` 在 SqliteMemoryProvider 中可用
- [ ] M2: tiktoken-rs 用于 token 计数
- [ ] M3: BudgetConfig 可从 YAML 文件加载
- [ ] M4: Session schema 拥有全部 claw-cli 兼容字段
- [ ] M5: ThreatScanner 存在并检测敏感模式
- [ ] M6: FrozenSnapshot verify 返回详细验证结果
- [ ] M7: 安全集成测试全部通过
- [ ] cargo fmt + clippy + test 全部通过
