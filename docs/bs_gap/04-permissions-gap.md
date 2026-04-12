# 四、权限系统偏差

> 本章节对比权限系统在 CLAW-CLI Baseline 和 If2Ai 桌面端的实现差异。

---

## 4.1 权限模型架构对比

### CLAW-CLI Baseline

```rust
// runtime/permissions.rs

// 5 级权限模式（有序枚举）
pub enum PermissionMode {
    ReadOnly = 0,        // 只读
    WorkspaceWrite = 1,  // 可写 workdir
    DangerFullAccess = 2, // 危险全访问
    Prompt = 3,          // 每次提示
    Allow = 4,          // 无限制
}

// 权限策略
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_requirements: BTreeMap<String, PermissionMode>, // 工具 → 所需权限
}

// 授权结果
pub enum PermissionOutcome {
    Allow,
    Deny { reason: String },
    Prompt { request: PermissionRequest },
}

// 权限检查（核心算法）
pub fn authorize(&self, tool_name: &str, input: &str,
                 prompter: Option<&mut dyn PermissionPrompter>) -> PermissionOutcome {
    let required = self.tool_requirements.get(tool_name)
        .copied()
        .unwrap_or(DangerFullAccess);  // 默认需要 DangerFullAccess

    match () {
        _ if self.active_mode >= required => Allow,           // 放行
        _ if self.active_mode == Prompt => Prompt { ... },    // 提示
        _ if self.active_mode == WorkspaceWrite && required == DangerFullAccess => Prompt { ... }, // 边界
        _ => Deny { reason: ... },                          // 拒绝
    }
}
```

**关键特性**：
- 权限是**有序比较**：`ReadOnly < WorkspaceWrite < DangerFullAccess < Prompt < Allow`
- 每个工具有**独立需求级别**
- `Prompt` 模式会**真正调用 prompter** 让用户确认

### If2Ai 桌面端

```rust
// modules/tools/context.rs
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,   // 存在
    Allow,    // 存在
}

// modules/runtime/permissions.rs — 同样的 PermissionPolicy 结构
pub struct PermissionPolicy { ... }
```

**表面上一致，但关键差异在于使用方式**：

```rust
// commands/agent.rs:358 — 🔴 问题
let permission_policy = PermissionPolicy::new(PermissionMode::DangerFullAccess);
```

**所有工具调用都硬编码为 `DangerFullAccess`，没有任何权限检查。**

---

## 4.2 Prompter 集成对比

### CLAW-CLI Baseline

```rust
// main.rs — run_turn() 调用处
let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
let result = runtime.run_turn(input, Some(&mut permission_prompter));
//                                           ↑ prompter 真实传入

// conversation.rs — run_turn() 内部
let permission_outcome = permission_policy.authorize(
    &tool_name, &input, Some(*prompt)  // ← 真实传入并调用
);
```

当 `PermissionOutcome::Prompt` 时，`CliPermissionPrompter` 会：
1. 在终端显示 `[Tool permission required: bash]`
2. 等待用户输入 `y/n`
3. 返回用户的决定

### If2Ai 桌面端

```rust
// commands/agent.rs:382
let result = runtime.run_turn(user_message.clone(), None);
//                                                 ↑ None！prompter 从未传入

// conversation.rs — run_turn() 内部
let permission_outcome = permission_policy.authorize(
    &tool_name, &input, None  // ← None，永远不提示
);
```

由于 `prompter` 是 `None`，即使 `PermissionPolicy` 的算法要求 `Prompt`，也会静默降级为 `Deny`（代码逻辑：没有 prompter 不能 prompt，直接拒绝）。

但因为 `permission_policy` 本身就是 `DangerFullAccess`，所以实际执行时是直接 Allow。

---

## 4.3 PermissionMode 配置来源对比

### CLAW-CLI Baseline

```rust
// config.rs — 多层配置
RuntimeConfig.feature_config.permission_mode
    // 支持以下配置值：
    // "default", "plan", "read-only"          → ReadOnly
    // "acceptEdits", "auto", "workspace-write" → WorkspaceWrite
    // "dontAsk", "danger-full-access"          → DangerFullAccess
```

用户在运行 `claw --permission-mode workspace-write` 时，会传入对应的 permission mode。

### If2Ai 桌面端

**完全缺失**：
- 前端没有权限模式选择器
- `run_agent_turn` 硬编码 `DangerFullAccess`
- `start_agent_stream` 完全没有权限检查

---

## 4.4 Sandbox 沙箱对比

### CLAW-CLI Baseline

```rust
// runtime/sandbox.rs
pub struct SandboxConfig {
    pub enabled: Option<bool>,
    pub namespace_restrictions: Option<bool>,  // Linux namespace 隔离
    pub network_isolation: Option<bool>,
    pub filesystem_mode: Option<FilesystemIsolationMode>,
    pub allowed_mounts: Vec<String>,
}

// Linux namespace 实现
Command::new("unshare")
    .args(["--user", "--map-root-user", "--mount", "--ipc", "--pid", "--uts", "--fork", "--net"])
    .current_dir(cwd)
    .env("HOME", cwd.join(".sandbox-home"))
    .env("TMPDIR", cwd.join(".sandbox-tmp"))
```

**实际隔离层次**：
- Linux + namespace active：`unshare --mount` 提供真正的文件系统隔离
- Linux + namespace inactive：仅 `HOME`/`TMPDIR` 重定向
- 非 Linux：沙箱配置无效

### If2Ai 桌面端

**完全缺失**：无沙箱模块。

```rust
// modules/tools/builtin/bash.rs — 仅有 cd 包装
pub async fn run_bash(input: BashInput, ctx: SharedToolContext) -> Result<String, ToolError> {
    let workdir = ctx.lock().unwrap().workdir.clone();
    let cmd = format!("cd {} && {}", workdir.display(), input.command);
    // 仅此而已，没有 namespace 包装
}
```

**实际限制**：仅通过 `cd $workdir` 限制访问，不是强制隔离。

---

## 4.5 权限与工具的对应关系

### CLAW-CLI Baseline

```rust
// 工具权限注册（tools/src/lib.rs 的 mvp_tool_specs）
("bash",         DangerFullAccess)
("read_file",    ReadOnly)
("write_file",   WorkspaceWrite)
("edit_file",    WorkspaceWrite)
("glob_search",  ReadOnly)
("grep_search",  ReadOnly)
("WebFetch",     ReadOnly)
("WebSearch",    ReadOnly)
("TodoWrite",    WorkspaceWrite)
("Agent",        DangerFullAccess)
```

### If2Ai 桌面端

工具权限信息存在于 `ToolEntry.input_schema` 附近，但**实际 dispatch 时不检查权限**。

---

## 4.6 小结

| 问题 | 位置 | 严重度 |
|------|------|--------|
| PermissionPolicy 硬编码 DangerFullAccess | `agent.rs:358` | 🟠 严重 |
| Prompter 参数传 None | `agent.rs:382` | 🟠 严重 |
| 前端无权限模式选择 UI | `App.tsx` | 🟡 中 |
| Sandbox 模块完全缺失 | — | 🟡 中 |
| bash 工具仅靠 cd 限制，无 namespace | `bash.rs` | 🟡 中 |
