# 📚 Security 使用指南

> 权限框架、路径验证、安全配置 —— If2Ai 安全系统的用户操作手册。

## 🛡️ 权限框架概述

If2Ai 安全系统采用分层防御架构，覆盖五个核心维度：

```
┌──────────────────────────────────────────────────┐
│                  安全防御层次                       │
├──────────────────────────────────────────────────┤
│  第 1 层：路径验证     防止文件系统遍历攻击          │
│  第 2 层：输入验证     防止注入攻击（XSS/模板/空字节）│
│  第 3 层：访问控制     基于分类的读写权限管理         │
│  第 4 层：原子写入     崩溃安全的文件操作            │
│  第 5 层：威胁扫描     60+ 规则的静态安全分析         │
└──────────────────────────────────────────────────┘
```

安全是一个跨切面关注点，security 模块本身提供基础设施（路径验证、原子写入、输入校验、访问控制），而 ThreatScanner 由 skills 模块实现并共享给其他模块使用。

### 防御范围

| 层次 | 保护对象 | 攻击类型 |
|------|----------|----------|
| 路径验证 | 文件系统 | 路径遍历（`../`）、符号链接逃逸 |
| 输入验证 | 内存/技能内容 | XSS、模板注入、空字节注入 |
| 访问控制 | 内存分类 | 未授权读写 |
| 原子写入 | 所有文件操作 | 中途崩溃导致数据损坏 |
| 威胁扫描 | 外部技能 | 数据外泄、命令注入、持久化等 15 类 |

## 📍 路径验证规则

### 工作原理

所有文件操作必须经过 `validate_safe_path` 验证：

1. 基础路径标准化（canonicalize）
2. 组合基础路径 + 请求路径
3. 如路径存在 → canonicalize 并检查前缀
4. 如路径不存在 → 找最近祖先 → 遍历剩余组件 → 检查每个 `..`

```rust
// 源码参考：src-tauri/src/modules/security/path.rs
pub fn validate_safe_path(base: &Path, requested: &Path) -> Result<PathBuf, PathError>
```

### 保护的路径

| 操作 | 基础路径 | 允许范围 |
|------|----------|----------|
| 内存数据库 | `dirs::data_local_dir()/.if2ai/memory/` | 内存目录内 |
| 技能文件 | `~/.if2ai/skills/` | 技能目录内 |
| 语音文件 | `~/.if2ai/voices/` | 语音目录内 |
| 浏览器配置 | `~/.if2ai/browser-profiles/` | 配置目录内 |

### 拒绝的路径模式

| 模式 | 原因 |
|------|------|
| `../../../etc/passwd` | 路径遍历 |
| `/absolute/path` | 绝对路径逃逸 |
| `~/../../sensitive` | 家目录逃逸 |
| 符号链接指向基础路径外 | 间接逃逸 |

## ⚙️ 安全配置选项

### 输入验证约束

| 约束 | 值 | 说明 |
|------|-----|------|
| Key 最大长度 | 256 字节 | 防止超长键名攻击 |
| Content 最大长度 | 1,000,000 字节 | 约 1MB，防内存溢出 |
| Key 合法字符 | `[a-zA-Z0-9_-.]` | 防止路径/SQL 注入 |

空 Key、超长 Key、含非法字符的 Key 都会被拒绝。

### 注入检测模式

`validate_memory_entry` 检测以下注入模式：

| 模式 | 示例 | 防御 |
|------|------|------|
| XSS | `<script>alert(1)</script>` | 检测 `<script` 标签 |
| JavaScript URI | `javascript:alert(1)` | 检测 `javascript:` 协议 |
| Data URI | `data:text/html,<script>` | 检测 `data:text/html` |
| Go 模板 | `{{.}}` | 检测 `{{.` 和 `{{=` |
| Shell 变量 | `${DANGEROUS}` | 检测 `${` |
| Ruby 表达式 | `#{system('ls')}` | 检测 `#{` |
| 空字节 | `\x00` | 检测 `\\x00` |

所有检测均为**大小写不敏感**。例如 `<SCRIPT>` 和 `<script>` 均会被拦截。

### 路径验证配置

所有文件操作的基础路径配置：

| 数据类型 | 基础路径 | 验证函数 |
|----------|----------|----------|
| 内存数据库 | `data_local_dir()/.if2ai/memory/` | `validate_safe_path` |
| 技能文件 | `~/.if2ai/skills/` | `validate_safe_path` |
| 语音文件 | `~/.if2ai/voices/` | `validate_safe_path` |
| 浏览器配置 | `~/.if2ai/browser-profiles/` | `validate_safe_path` |

## 🔑 工具权限等级说明

### 内存访问权限

内存数据按分类（`MemoryCategory`）组织，不同上下文拥有不同的读写权限：

| 上下文 | 可读分类 | 可写分类 |
|--------|----------|----------|
| 会话（Session） | Conversation, Daily | Conversation |
| 项目（Project） | Core, Daily, Conversation, Custom | Core, Daily, Conversation, Custom |

```rust
// 源码参考：src-tauri/src/modules/security/access.rs
pub struct MemoryAccessContext {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub read_categories: Vec<MemoryCategory>,
    pub write_categories: Vec<MemoryCategory>,
}
```

会话上下文权限较窄（仅能读写对话类数据），项目上下文权限更宽（可读写核心数据）。

### 权限检查流程

```
操作请求 → MemoryAccessContext.can_read(category)?
         ├─ ✅ → 允许读取
         └─ ❌ → 拒绝

操作请求 → MemoryAccessContext.can_write(category)?
         ├─ ✅ → 允许写入
         └─ ❌ → 拒绝
```

### 技能安装权限

技能安装权限由 `TrustLevel` + `InstallPolicy` 控制：

| 来源 | 信任等级 | 扫描结果处理 |
|------|----------|-------------|
| 应用自带 | Builtin | 不扫描，始终信任 |
| 官方仓库 | Trusted | Caution 允许，Dangerous 阻止 |
| 社区来源 | Community | 任何发现即阻止 |
| Agent 生成 | AgentCreated | Dangerous = 询问用户 |

### ThreatScanner 共享

ThreatScanner 由 skills 模块实现，但安全基础设施跨模块共享：
- skills/guard/ — 60+ 威胁模式定义
- security/ — 路径验证 + 输入验证 + 原子写入
- 两者共同构成 If2Ai 的安全防线

## 📝 核心概念速查表

| 安全机制 | 函数 | 保护的攻击 |
|----------|------|------------|
| 路径验证 | `validate_safe_path()` | 路径遍历、符号链接逃逸 |
| 输入验证 | `validate_memory_entry()` | XSS、模板注入、空字节 |
| 访问控制 | `MemoryAccessContext` | 未授权读写 |
| 原子写入 | `atomic_write()` | 崩溃导致数据损坏 |
| 威胁扫描 | `SkillsGuard.scan()` | 15 类 60+ 安全威胁 |
| JSON 原子写 | `atomic_json_write()` | JSON 数据损坏 |

### 安全相关路径

| 路径 | 用途 |
|------|------|
| `~/.if2ai/skills/.hub/audit.log` | 技能安装审计日志 |
| `~/.if2ai/skills/.hub/quarantine/` | 可疑技能隔离区 |
| `~/.if2ai/skills/.hub/lock.json` | 已安装技能清单 |
| `~/.if2ai/memory/memory.db` | 内存数据库 |
| `~/.if2ai/browser-profiles/` | 浏览器配置目录 |

### 安全错误类型

| 错误 | 来源 | 说明 |
|------|------|------|
| `PathTraversalAttempt` | `path.rs` | 路径遍历攻击 |
| `BasePathInvalid` | `path.rs` | 基础路径无效 |
| `InjectionDetected` | `validation.rs` | 注入模式检测 |
| `ContentTooLarge` | `validation.rs` | 内容超限 |
| `EmptyKey` | `validation.rs` | 空 Key |
| `InvalidKeyFormat` | `validation.rs` | Key 格式非法 |
| `WriteError::Io` | `atomic_write.rs` | 写入 I/O 错误 |

## 🔗 相关资源

- [Security 实现文档](./02-implementation.md) — 开发者架构详解
- [Skills 使用指南](../skills/01-usage-guide.md) — 技能安全审查流程
- [Skills 实现文档](../skills/02-implementation.md) — ThreatScanner 详解
- [路径验证源码](../../../src-tauri/src/modules/security/path.rs)
- [输入验证源码](../../../src-tauri/src/modules/security/validation.rs)
- [原子写入源码](../../../src-tauri/src/modules/security/atomic_write.rs)
- [访问控制源码](../../../src-tauri/src/modules/security/access.rs)
