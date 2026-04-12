# Project Workdir Boundary — 项目工作目录隔离与访问控制

**版本**: 1.0
**最后更新**: 2026-04-12
**状态**: Implementation-ready
**依赖**: Phase 1 slices 1.6 (ProjectManager), 1.7 (Tauri Commands), BL-104 (ToolRegistry)

---

## 1. 背景与问题

### 1.1 现状

`project-system.md` 定义了 Project/Session 管理，存储结构为：

```
~/.if2ai/
├── projects/
│   └── <project_id>/
│       ├── project.json    # 含 workdir 字段
│       └── sessions/
└── sessions/               # 旧版 session（无 project_id）
```

**`workdir` 字段仅作为元数据存储，从未用于访问控制。**

### 1.2 关键 Gap

| Gap | 当前状态 | 风险 |
|-----|----------|------|
| **Agent 不知道 project context** | `run_agent_turn` 只接收 `session_id`，不传 `project_id/workdir` | Agent 可能操作任意路径 |
| **Filesystem 无 allowlist** | `file_read` 用 denylist，可绕过 | 可以读取 `/etc/passwd` 等 |
| **Bash 无 workdir 限制** | 直接在 host 环境执行任意命令 | 可以 `rm -rf /` 等危险操作 |
| **PermissionMode 未生效** | `permissions.rs` 定义了 `ReadOnly`/`WorkspaceWrite`，但始终用 `DangerFullAccess` | 无差异化保护 |

### 1.3 需要做什么

| 优先级 | 任务 | 改动文件 |
|--------|------|----------|
| P0 | Agent 命令接收 `project_id`，查询 workdir 注入 context | `commands/agent.rs` |
| P0 | `file_read` 实现 workdir allowlist | `modules/tools/builtin/file_read.rs` |
| P0 | `bash` 工具实现 workdir 限制（`cd $workdir && $cmd`） | `modules/tools/builtin/bash.rs` |
| P1 | Per-project `PermissionMode` 持久化 | `modules/projects/mod.rs` |
| P2 | 激活 `sandbox.rs` 配置（namespace 隔离） | `modules/runtime/sandbox.rs` |

---

## 2. 系统设计

### 2.1 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                         Frontend (React)                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ ProjectRail — 显示项目 workdir，创建/切换项目             │   │
│  │ ChatUI — 在 project context 下与 Agent 对话              │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              │ createProject(name, workdir)      │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Tauri Commands                                            │   │
│  │ createProject → ProjectManager.create_project            │   │
│  │ run_agent_turn(session_id) → 查询 project.workdir        │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         AppState                                 │
│  ┌──────────────────┐    ┌─────────────────────────────────┐   │
│  │ ProjectManager   │    │ ToolRegistry                    │   │
│  │ projects_dir     │    │ (带 workdir 上下文)              │   │
│  │ get_project(id)  │    │                                  │   │
│  └──────────────────┘    └─────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
   │ file_read   │    │    bash     │    │ write_file  │
   │ (allowlist) │    │ (workdir)   │    │ (allowlist) │
   └─────────────┘    └─────────────┘    └─────────────┘
```

### 2.2 核心改动点

#### 2.2.1 ProjectManager — 新增 workdir 验证

```rust
// src-tauri/src/modules/projects/mod.rs

impl ProjectManager {
    /// Get project with workdir validation.
    /// Returns error if workdir does not exist or is not accessible.
    pub async fn get_project_with_workdir(
        &self,
        id: &str,
    ) -> Result<Project, ProjectError> {
        let project = self.get_project(id).await?;
        if !project.workdir.exists() {
            return Err(ProjectError::WorkdirNotFound(
                project.workdir.display().to_string()
            ));
        }
        Ok(project)
    }

    /// Get the effective workdir for a session.
    /// Falls back to home directory if project has no workdir.
    pub async fn get_workdir_for_session(
        &self,
        session: &Session,
    ) -> PathBuf {
        if session.project_id.is_empty() {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            self.get_project(&session.project_id)
                .await
                .map(|p| p.workdir)
                .unwrap_or_else(|_| PathBuf::from("."))
        }
    }
}
```

#### 2.2.2 agent.rs — 传递 project context

```rust
// src-tauri/src/commands/agent.rs

#[tauri::command]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    session_id: String,
    user_message: String,
) -> Result<RunAgentTurnResponse, String> {
    // 获取 session 和 project workdir
    let session = state.session_manager.restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    let workdir = state.project_manager
        .get_workdir_for_session(&session)
        .await;

    // 将 workdir 传给 RuntimeConfig
    let runtime_config = RuntimeConfig {
        workdir: Some(workdir),  // ✅ 新增
        ..Default::default()
    };

    let mut runtime = ConversationRuntime::new(
        state.api_client.clone(),
        system_prompt,
        state.tool_registry.clone(),
        session,
        runtime_config,
        max_turns,
    );

    runtime.run_turn(user_message).await
}
```

#### 2.2.3 RuntimeConfig — 新增 workdir 字段

```rust
// src-tauri/src/modules/runtime/config.rs

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub workdir: Option<PathBuf>,      // ✅ Agent 执行的工作目录
    pub permission_mode: PermissionMode,
    pub max_turns: usize,
    pub max_tokens: u32,
    pub model: Option<String>,
    pub temperature: Option<f32>,
}
```

---

## 3. Filesystem 边界控制

### 3.1 Workdir Allowlist 策略

**原则**：工具只能访问 project workdir 内的文件。

```
✅ ALLOW:  project workdir 下的所有文件
✅ ALLOW:  临时目录（/tmp）
❌ DENY:   /etc/, /usr/, /bin/, /sbin/
❌ DENY:   ~/.ssh/, ~/.aws/, ~/.config/
❌ DENY:   其他用户的 home 目录
```

### 3.2 read_file — Workdir 限制

```rust
// src-tauri/src/modules/tools/builtin/file_read.rs

pub async fn execute_read_file(args: &Value) -> Result<String, ToolError> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::Handler("Missing 'path' argument".into()))?;

    let offset = args.get("offset").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64());

    // 获取 workdir from context（从 RuntimeConfig 传入）
    let workdir = get_current_workdir()?;

    let requested_path = PathBuf::from(path);
    let canonical_path = requested_path.canonicalize()
        .map_err(|e| ToolError::Handler(format!("Invalid path: {}", e)))?;

    // Allowlist 检查：路径必须在 workdir 内
    let canonical_workdir = workdir.canonicalize()
        .map_err(|e| ToolError::Handler(format!("Invalid workdir: {}", e)))?;

    if !canonical_path.starts_with(&canonical_workdir) {
        return Err(ToolError::Handler(
            format!("Path '{}' is outside allowed workdir '{}'",
                path, workdir.display())
        ));
    }

    // 敏感路径额外检查（保留原有 denylist）
    let sensitive_patterns = ["/etc/passwd", "/etc/shadow", "/.ssh/", "/.aws/"];
    let path_str = path.to_string();
    for pattern in sensitive_patterns {
        if path_str.contains(pattern) {
            return Err(ToolError::Handler(
                format!("Access to '{}' is forbidden", pattern)
            ));
        }
    }

    // 执行读取
    execute_read_file_internal(&canonical_path, offset, limit).await
}
```

### 3.3 bash — Workdir 限制

```rust
// src-tauri/src/modules/tools/builtin/bash.rs

pub async fn execute_bash(args: &Value) -> Result<String, ToolError> {
    let command = args.get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::Handler("Missing 'command' argument".into()))?;

    let timeout = args.get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    // 获取 workdir
    let workdir = get_current_workdir()?;

    // 危险命令检查（保留原有 denylist）
    let dangerous = ["rm -rf /", "rm -rf /*", "sudo su", "mkfs", ":(){:|:&};"];
    for pat in dangerous {
        if command.contains(pat) {
            return Err(ToolError::Handler(
                format!("Command '{}' is forbidden", pat)
            ));
        }
    }

    // 强制在 workdir 内执行
    // 方法 1: cd + command（简单但有 race condition 风险）
    // 方法 2: 使用 bash -c "cd ... && ..."（更好）
    let wrapped_command = format!("cd {} && {}", workdir.display(), command);

    execute_bash_internal(&wrapped_command, timeout).await
}
```

### 3.4 Context 获取机制

需要一种方式让工具函数访问当前的 `workdir` context。两种方案：

**方案 A：thread_local 变量**（简单但不够优雅）

```rust
// src-tauri/src/modules/tools/context.rs

thread_local! {
    static WORKDIR: OnceCell<PathBuf> = OnceCell::new();
}

pub fn set_workdir(path: PathBuf) {
    WORKDIR.with(|w| { w.set(path); });
}

pub fn get_workdir() -> Option<PathBuf> {
    WORKDIR.with(|w| w.get().cloned())
}
```

**方案 B：通过 Arc<Mutex> 传递**（更可控，推荐）

```rust
// 在 ToolExecutor 或 ToolRegistry 中存储 context

pub struct ToolContext {
    pub workdir: PathBuf,
    pub permission_mode: PermissionMode,
}

pub struct ToolRegistry {
    tools: Arc<DashMap<String, ToolEntry>>,
    context: Arc<Mutex<ToolContext>>,  // ✅
}
```

**本设计采用方案 B**，在 `run_agent_turn` 时设置 context。

---

## 4. Permission Mode 系统

### 4.1 PermissionMode 定义（已存在）

```rust
// src-tauri/src/modules/runtime/permissions.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// 只读模式：只能读取 workdir 内的文件
    ReadOnly,
    /// 工作目录写模式：可以读写 workdir 内的文件
    WorkspaceWrite,
    /// 完全访问模式：无限制（当前默认）
    DangerFullAccess,
    /// 提示模式：写入前需要用户确认
    Prompt,
    /// 允许模式：明确允许的操作
    Allow,
}
```

### 4.2 Per-Project PermissionMode

```rust
// src-tauri/src/modules/projects/mod.rs

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub workdir: PathBuf,
    pub permission_mode: PermissionMode,  // ✅ 新增
    pub created_at: String,
    pub updated_at: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            permission_mode: PermissionMode::WorkspaceWrite,  // 合理默认值
            ..
        }
    }
}
```

### 4.3 Permission 检查

```rust
// src-tauri/src/modules/tools/builtin/file_operations.rs

pub fn check_permission(
    mode: PermissionMode,
    operation: &str,  // "read" | "write" | "execute"
    path: &Path,
) -> Result<(), ToolError> {
    match (mode, operation) {
        (PermissionMode::ReadOnly, "write") | (PermissionMode::ReadOnly, "execute") => {
            Err(ToolError::Handler(
                format!("{} operation not allowed in ReadOnly mode", operation)
            ))
        }
        _ => Ok(()),
    }
}
```

---

## 5. Sandbox 容器化（未来迭代）

### 5.1 SandboxConfig（已定义，未激活）

```rust
// src-tauri/src/modules/runtime/sandbox.rs

pub struct SandboxConfig {
    pub enabled: Option<bool>,
    pub namespace_restrictions: Option<bool>,  // Linux namespaces
    pub network_isolation: Option<bool>,
    pub filesystem_mode: Option<FilesystemIsolationMode>,  // Default: WorkspaceOnly
    pub allowed_mounts: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum FilesystemIsolationMode {
    WorkspaceOnly,   // 只允许访问 workdir
    Host,            // 无限制
}
```

### 5.2 激活条件

当 `project.permission_mode == PermissionMode::DangerFullAccess` 且 `sandbox.enabled == true` 时：
- 使用 `unshare` 创建 Linux namespace
- 将 workdir 挂载为 rootfs
- 禁用网络或限制网络访问

**这不是本 Phase 的目标**，属于 Phase 4+ 未来迭代。

---

## 6. 前端集成

### 6.1 Project 创建时指定 workdir

```typescript
// src/components/ProjectRail.tsx

async function handleCreateProject() {
  const selected = await open();  // 系统目录选择对话框
  if (selected) {
    await createProject({
      name: selected.name,
      workdir: selected.path,
      permission_mode: 'workspace_write',  // 默认
    });
  }
}
```

### 6.2 运行时显示 project context

```typescript
// src/components/SessionStatus.tsx

// 显示当前 session 所属的 project 和 workdir
function SessionStatus({ session }) {
  const project = useProject(session.project_id);
  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground">
      <FolderOpen className="h-4 w-4" />
      <span>{project?.name || 'No Project'}</span>
      {project && (
        <code className="text-xs bg-muted px-1 rounded">
          {project.workdir}
        </code>
      )}
    </div>
  );
}
```

---

## 7. 实现顺序

| 阶段 | 任务 | 优先级 |
|------|------|--------|
| **Phase 4.1** | Agent 命令接收 `project_id` 并查询 workdir | P0 |
| **Phase 4.2** | `file_read` 实现 workdir allowlist | P0 |
| **Phase 4.3** | `bash` 工具实现 workdir 限制 | P0 |
| **Phase 4.4** | ToolContext Arc<Mutex> 传递机制 | P0 |
| **Phase 4.5** | Per-project `PermissionMode` 持久化 | P1 |
| **Phase 4.6** | 集成测试 + 前端 UI 验证 | P1 |

---

## 8. 与 project-system.md 的差异

| 方面 | Phase 1 (project-system.md) | 本 Phase |
|------|----------------------------|----------|
| `workdir` 字段 | 仅作为元数据存储 | 用于访问控制 |
| Agent context | 不传 workdir | 传递 workdir 给 runtime |
| Filesystem 访问 | 无限制 | workdir allowlist |
| Bash 执行 | 无限制 | 限制在 workdir 内 |
| PermissionMode | 未实现 | Per-project 持久化 + 生效 |

---

## 9. 测试计划

### 9.1 单元测试

```rust
#[tokio::test]
async fn test_file_read_outside_workdir() {
    let workdir = PathBuf::from("/tmp/test_project");
    let context = ToolContext { workdir, permission_mode: PermissionMode::WorkspaceWrite };
    let registry = ToolRegistry::new(context);

    // 尝试读取 /etc/passwd（不在 workdir 内）
    let result = registry.dispatch("read_file", json!({ "path": "/etc/passwd" })).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_bash_restricted_to_workdir() {
    let workdir = PathBuf::from("/tmp/test_project");
    let context = ToolContext { workdir, permission_mode: PermissionMode::WorkspaceWrite };
    let registry = ToolRegistry::new(context);

    // 执行 ls /（应该在 workdir 内执行，实际是 ls /tmp/test_project/）
    let result = registry.dispatch("bash", json!({ "command": "ls /" })).await;
    // 应该成功，但路径解析在 workdir 内
}
```

### 9.2 集成测试

- 创建 project with workdir
- 在 project 内创建 session
- 调用 `run_agent_turn`，验证 workdir 传递
- 执行 `read_file` 访问 workdir 内/外文件，验证 allowlist

---

**版本**: 1.0 | **最后更新**: 2026-04-12
**参考**: [project-system.md](./project-system.md) — Project 基本定义
