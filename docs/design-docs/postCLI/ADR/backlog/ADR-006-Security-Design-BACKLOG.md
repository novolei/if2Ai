# ADR-006 Security Design — 详细实施 backlog

## 概述

**目标**: 在 `src-tauri/src/modules/security/` 中实现多层安全防护 (NEW MODULE)
**ADR**: [ADR-006](./ADR-006-Security-Design.md)
**优先级**: P0 (安全相关)
**预估工时**: 3-4 天
**现状**: `src-tauri/src/modules/security/` 目录不存在，需要创建

---

## 子任务清单

### TASK-006-01: 输入验证实现

**目标**: 实现记忆条目的安全验证

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/security/validation.rs`
- [ ] 实现 `pub fn validate_memory_entry(key: &str, content: &str) -> Result<(), SecurityError>`
  - Key 长度: 1-256 字符
  - Key 格式: 仅 alphanumeric + `_.-`
  - Content 长度: ≤ 1,000,000 字符
  - 检测注入模式
- [ ] 实现 `fn contains_injection_pattern(content: &str) -> bool`
  - XSS: `<script`, `javascript:`, `data:text/html`
  - Template injection: `{{.`, `{{=`, `${`, `#{`
  - Null byte: `\x00`
- [ ] 定义 `SecurityError` 枚举
- [ ] 编写测试覆盖所有验证规则

**验收标准**:
- [ ] 空 key 被拒绝
- [ ] 超长 key 被拒绝
- [ ] 非法字符被拒绝
- [ ] XSS 模式被检测
- [ ] 注入模式被检测

**测试标准**:
```rust
#[test]
fn rejects_empty_key() {
    assert!(validate_memory_entry("", "content").is_err());
}

#[test]
fn rejects_too_long_key() {
    let key = "a".repeat(257);
    assert!(validate_memory_entry(&key, "content").is_err());
}

#[test]
fn detects_xss_pattern() {
    assert!(contains_injection_pattern("<script>alert(1)</script>"));
    assert!(contains_injection_pattern("javascript:void(0)"));
}

#[test]
fn accepts_valid_input() {
    assert!(validate_memory_entry("valid_key", "valid content").is_ok());
}
```

---

### TASK-006-02: 路径验证实现

**目标**: 实现安全的路径管理

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/security/path.rs`
- [ ] 实现 `pub fn validate_safe_path(base: &Path, requested: &Path) -> Result<PathBuf, SecurityError>`
  - 规范化两路径
  - 验证 requested 在 base 下
- [ ] 实现 `pub fn get_memory_db_path() -> Result<PathBuf, SecurityError>`
  - 返回 `~/.if2ai/memory/memory.db`
  - 创建目录（如不存在）
- [ ] 实现 `pub fn get_session_path(project_id: &str) -> Result<PathBuf, SecurityError>`
- [ ] 编写测试

**验收标准**:
- [ ] 路径遍历攻击被阻止
- [ ] 规范化路径正确
- [ ] 目录自动创建

**测试标准**:
```rust
#[test]
fn prevents_path_traversal() {
    let base = Path::new("/home/user/.if2ai");
    let malicious = Path::new("/home/user/.if2ai/../../../etc/passwd");

    let result = validate_safe_path(base, malicious);
    assert!(result.is_err());
}

#[test]
fn allows_valid_subpath() {
    let base = Path::new("/home/user/.if2ai");
    let valid = Path::new("/home/user/.if2ai/memory/test.db");

    let result = validate_safe_path(base, valid);
    assert!(result.is_ok());
}
```

---

### TASK-006-03: 原子写入实现

**目标**: 实现安全的原子文件写入

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/security/atomic_write.rs`
- [ ] 实现 `pub async fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), WriteError>`
  - 写入临时文件 (`.tmp`)
  - `sync_all()` 刷新到磁盘
  - `rename()` 原子替换
- [ ] 实现 `pub async fn atomic_json_write<P: AsRef<Path>, T: Serialize>(path: P, data: &T) -> Result<(), WriteError>`
- [ ] 定义 `WriteError` 枚举
- [ ] 编写测试（模拟进程崩溃场景）

**验收标准**:
- [ ] 临时文件正确创建
- [ ] `sync_all()` 被调用
- [ ] `rename()` 是原子操作
- [ ] 崩溃后无遗留临时文件

---

### TASK-006-04: MemoryAccessContext 实现

**目标**: 实现记忆访问控制

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/security/access.rs`
- [ ] 定义 `MemoryAccessContext` 结构体:
  ```rust
  pub struct MemoryAccessContext {
      pub session_id: Option<String>,
      pub project_id: Option<String>,
      pub read_categories: Vec<MemoryCategory>,
      pub write_categories: Vec<MemoryCategory>,
  }
  ```
- [ ] 实现工厂方法:
  - `MemoryAccessContext::for_session(session_id: &str) -> Self`
  - `MemoryAccessContext::for_project(project_id: &str) -> Self`
- [ ] 实现权限检查:
  - `can_read(&self, category: &MemoryCategory) -> bool`
  - `can_write(&self, category: &MemoryCategory) -> bool`
- [ ] 编写测试

**验收标准**:
- [ ] Session 上下文只能访问 Conversation/Daily
- [ ] Project 上下文可访问 Core/Daily/Conversation
- [ ] 权限检查正确

---

### TASK-006-05: ThreatScanner 实现

**目标**: 实现威胁扫描

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/security/scanner.rs`
- [ ] 定义 `ThreatScanner` 结构体:
  ```rust
  pub struct ThreatScanner {
      blocked_patterns: Vec<Regex>,
      max_content_size: usize,
  }
  ```
- [ ] 实现 `Default`:
  - XSS patterns: `<script[^>]*>`, `javascript:`, `on\w+\s*=`
  - Path traversal: `\.\./`, `C:\\`, `/etc/passwd`
  - Shell injection: `[;&|``$]`
  - max_size = 1,000,000
- [ ] 实现 `pub fn scan(&self, content: &str) -> Result<(), ThreatDetected>`
- [ ] 定义 `ThreatDetected` 错误类型
- [ ] 编写测试

**验收标准**:
- [ ] XSS 模式被检测
- [ ] 路径遍历被检测
- [ ] Shell 注入被检测
- [ ] 超大内容被拒绝

---

### TASK-006-06: FrozenSnapshot 安全验证

**目标**: 实现快照完整性验证

**具体任务**:
- [ ] 在 `src-tauri/src/modules/runtime/snapshot.rs` 添加验证逻辑
- [ ] 实现 `pub fn verify_frozen_snapshot(snapshot: &FrozenSnapshot, current_prompt: &str) -> bool`
  - 计算当前 prompt 的哈希
  - 与快照中的哈希比较
  - 不匹配时记录警告日志
- [ ] 在 SessionManager 加载时调用验证
- [ ] 编写测试

**验收标准**:
- [ ] 未修改的 prompt 通过验证
- [ ] 修改的 prompt 验证失败
- [ ] 失败时记录警告

---

### TASK-006-07: 安全集成测试

**目标**: 端到端安全测试

**具体任务**:
- [ ] 创建 `tests/security_integration.rs`
- [ ] 测试场景:
  - 恶意 key 注入被阻止
  - 恶意 content 注入被阻止
  - 路径遍历被阻止
  - 原子写入在崩溃后正确恢复
  - 权限检查正确执行
- [ ] 运行 `cargo test --package if2ai security`

**验收标准**:
- [ ] 所有安全测试通过
- [ ] 无 panic 或 unwrap
- [ ] 错误消息不泄露敏感信息

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-006-01 | 输入验证基础 |
| P0 | TASK-006-02 | 路径安全基础 |
| P0 | TASK-006-03 | 原子写入基础 |
| P1 | TASK-006-04 | 访问控制 |
| P1 | TASK-006-05 | 威胁扫描 |
| P1 | TASK-006-06 | 快照验证 |
| P2 | TASK-006-07 | 集成测试 |

---

## 验收总览

- [ ] 输入验证阻止恶意 key/content
- [ ] 路径验证阻止遍历攻击
- [ ] 原子写入防止数据损坏
- [ ] AccessContext 正确执行权限检查
- [ ] ThreatScanner 检测 XSS/注入/路径遍历
- [ ] FrozenSnapshot 验证 prompt 完整性
- [ ] 所有安全测试通过
