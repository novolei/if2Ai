# 📚 Skills 使用指南

> 从发现到执行 —— 用户视角的技能系统完整操作手册。

## 📍 技能发现与安装

### 技能来源

If2Ai 技能系统支持三种技能来源：

| 来源 | 路径 | 信任等级 |
|------|------|----------|
| **内置技能** | 应用自带，随版本更新 | `Builtin`（永不扫描） |
| **市场安装** | 从 GitHub / ClawHub / skills.sh 下载 | `Trusted` 或 `Community` |
| **用户创建** | 本地手动创建或 Agent 生成 | `AgentCreated` |

### 浏览技能市场

技能市场支持多源搜索，适配器包括：

- **GitHub**：从 `openai/skills`、`anthropics/skills` 等仓库获取
- **ClawHub**：If2Ai 官方技能注册中心
- **skills.sh**：社区技能聚合平台

搜索流程：

```
用户输入关键词 → Hub 统一搜索 → 返回 SkillMeta 列表
                  ├─ GitHub 适配器
                  ├─ ClawHub 适配器
                  └─ skills.sh 适配器
```

### 安装技能

安装流程自动执行安全扫描：

1. 从源下载技能包（`SkillBundle`）
2. `SkillsGuard` 执行 15 类 60+ 模式扫描
3. 根据 `TrustLevel` + 扫描结果判断是否允许安装
4. 允许 → 写入 `~/.if2ai/skills/`；可疑 → 隔离到 `quarantine/`

源码参考：[`src-tauri/src/modules/skills/hub/state.rs`](../../../src-tauri/src/modules/skills/hub/state.rs)

## 🔄 技能执行与参数配置

### 斜杠命令调用

技能通过 `/skill-name` 斜杠命令调用。系统扫描 `~/.if2ai/skills/` 下的所有技能目录，自动注册可用的斜杠命令。

```rust
// 源码参考：src-tauri/src/modules/skills/commands.rs
pub struct SkillCommandInfo {
    pub name: String,            // 命令名（如 "/my-skill"）
    pub description: String,     // 技能描述
    pub platforms: Vec<String>,  // 支持的平台
    pub enabled: bool,           // 是否启用
    pub requires_toolsets: Vec<String>,  // 依赖的工具集
}
```

### 参数配置

技能可通过 `SKILL.md` 中的 config block 定义参数：

- 参数以 YAML frontmatter 格式嵌入 `SKILL.md`
- 运行时由 `commands.rs` 解析并传递给技能
- 支持平台过滤（macOS / Linux / Windows）

### 执行上下文

每次技能执行在独立的 `SkillContext` 中运行：

| 属性 | 说明 |
|------|------|
| `skills_dir` | 技能根目录 `~/.if2ai/skills/` |
| `guard` | `SkillsGuard` 实例，用于安全检查 |

## 🛡️ 技能安全审查流程

### 四级信任模型

| 信任等级 | 来源 | 扫描策略 | 发现威胁时 |
|----------|------|----------|-----------|
| `Builtin` | 应用自带 | 不扫描 | — |
| `Trusted` | openai/skills 等官方源 | 扫描但允许 Caution 级别 | 仅 Critical/High 阻止 |
| `Community` | 社区来源 | 严格扫描 | 任何发现 = 阻止 |
| `AgentCreated` | AI 代理创建 | 扫描 | Dangerous = 询问而非阻止 |

### 扫描流程

```
技能文件 → SkillsGuard.scan()
             ├─ threat_patterns: 60+ 正则匹配
             ├─ invisible_unicode: 隐形字符检测
             ├─ structural_limits: 结构性限制
             └─ InstallPolicy.should_allow_install()
                  ├─ Verdict::Clean → 允许
                  ├─ Verdict::Caution → 视信任等级
                  └─ Verdict::Dangerous → 阻止/询问
```

### 威胁分类（15 类）

| 分类 | 说明 | 典型模式 |
|------|------|----------|
| Exfiltration | 数据外泄 | `curl $ENV`、上传到外部服务器 |
| Injection | 注入攻击 | prompt injection、命令注入 |
| Destructive | 破坏性操作 | `rm -rf /`、格式化磁盘 |
| Persistence | 持久化 | crontab、launchd、注册表 |
| Network | 网络攻击 | 端口扫描、反向 shell |
| Obfuscation | 混淆 | base64 编码执行、十六进制转义 |
| Mining | 挖矿 | 加密货币挖掘代码 |
| SupplyChain | 供应链 | 依赖篡改、typosquatting |
| PrivilegeEscalation | 提权 | sudo 滥用、setuid |
| CredentialExposure | 凭证泄露 | 硬编码密钥、token 暴露 |
| AgentConfigPersistence | 代理配置持久化 | 修改 AI 代理系统提示 |
| ContextExfiltration | 上下文外泄 | 窃取对话历史 |
| Jailbreak | 越狱 | 绕过安全限制的指令 |
| InvisibleUnicode | 不可见 Unicode | 零宽字符、同形字 |
| StructuralLimits | 结构限制 | 超大文件、嵌套过深 |

源码参考：[`src-tauri/src/modules/skills/guard/threat_patterns.rs`](../../../src-tauri/src/modules/skills/guard/threat_patterns.rs)

## ✏️ 技能编辑与自定义

### 创建技能

使用 `SkillManager` 的 CRUD 操作创建技能：

1. 验证技能名称（`^[a-z0-9][a-z0-9._-]*$`，最长 64 字符）
2. 创建技能目录
3. 写入 `SKILL.md`（最大 100,000 字符）
4. 可选添加辅助文件（单个最大 1MB）

```rust
// 源码参考：src-tauri/src/modules/skills/manager/validator.rs
pub struct SkillValidator;
impl SkillValidator {
    pub const MAX_NAME_LENGTH: usize = 64;
    pub const MAX_DESCRIPTION_LENGTH: usize = 1024;
    pub const MAX_SKILL_CONTENT_CHARS: usize = 100_000;
    pub const MAX_SUPPORTING_FILE_BYTES: usize = 1_048_576;
}
```

### 编辑技能

所有写操作使用原子写入（`atomic_write`），确保中途崩溃不会损坏文件：

- 写入 `.tmp` 临时文件 → 同步到磁盘 → 原子重命名
- 编辑前自动触发安全扫描

### 外部技能目录

支持通过 `external_dirs` 挂载外部技能目录，便于开发期热加载。

## 🛒 技能市场浏览

### Hub 状态管理

技能市场状态保存在 `~/.if2ai/skills/.hub/` 目录：

| 文件 | 用途 |
|------|------|
| `lock.json` | 已安装技能清单 |
| `audit.log` | 安装/扫描审计日志 |
| `quarantine/` | 可疑技能隔离区 |
| `taps.json` | 可信代理包源（TAPS）配置 |
| `index-cache/` | 搜索索引缓存 |

### 安装后管理

- **更新**：重新下载并扫描
- **卸载**：从 `lock.json` 移除并删除目录
- **导出/导入**：通过 `snapshot` 模块打包迁移

### 搜索与筛选

技能市场支持关键词搜索和分类筛选：

```rust
// 源码参考：src-tauri/src/modules/skills/hub/types.rs
pub struct SkillMeta {
    pub name: String,           // 技能名称
    pub description: String,    // 简要描述
    pub source: String,         // 来源标识（"github" / "clawhub"）
    pub trust_level: TrustLevel,// 信任等级
    pub tags: Vec<String>,      // 分类标签
    pub extra: HashMap<String, serde_json::Value>, // 额外元数据
}
```

搜索结果按信任等级和相关性排序，`Builtin` 和 `Trusted` 来源优先展示。

### 技能包结构

下载的 `SkillBundle` 包含完整的技能文件：

```rust
// 源码参考：src-tauri/src/modules/skills/hub/types.rs
pub struct SkillBundle {
    pub name: String,                           // 技能名称
    pub files: HashMap<String, Vec<u8>>,        // 相对路径 → 文件内容
    pub source: String,                         // 来源标识
    pub trust_level: TrustLevel,                // 信任等级
    pub metadata: HashMap<String, serde_json::Value>, // 额外元数据
}
```

### 平台兼容性

每个技能可声明支持的平台：

- `macos` → 映射到内部 `darwin`
- `linux` → 映射到内部 `linux`
- `windows` → 映射到内部 `win32`

源码参考：[`src-tauri/src/modules/skills/commands.rs`](../../../src-tauri/src/modules/skills/commands.rs)

## 📝 核心概念速查表

| 操作 | 命令/接口 | 关键文件 |
|------|-----------|----------|
| 搜索技能 | `SkillHub.search()` | `hub/marketplace.rs` |
| 安装技能 | `SkillHub.install()` | `hub/state.rs` |
| 扫描技能 | `SkillsGuard.scan()` | `guard/mod.rs` |
| 创建技能 | `SkillManager.create()` | `manager/actions.rs` |
| 编辑技能 | `SkillManager.edit()` | `manager/actions.rs` |
| 删除技能 | `SkillManager.delete()` | `manager/actions.rs` |
| 导出技能 | `Snapshot.export()` | `snapshot/mod.rs` |

## 🔗 相关资源

- [Skills 实现文档](./02-implementation.md) — 开发者架构详解
- [Security 模块](../security/01-usage-guide.md) — 安全与权限
- [技能安全扫描规范](../../../src-tauri/src/modules/skills/guard/policy.rs)
