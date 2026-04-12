# Project System — If2Ai 多项目支持

**版本**: 1.0
**最后更新**: 2026-04-12
**状态**: Implementation-ready
**依赖**: Phase 1 slices 1.6 (SessionManager), 1.7 (Tauri Commands)

---

## ⚠️ Gap Analysis & Activation Status (2026-04-12)

### 现状

`project-system.md` 定义了 Project/Session 管理和存储结构，**但存在以下关键 Gap**：

| Gap | 当前状态 | 需要改动的文件 |
|-----|----------|----------------|
| **Agent 不知道 workdir** | `run_agent_turn` 只接收 `session_id`，不传 `project_id/workdir` | `src-tauri/src/commands/agent.rs` |
| **Filesystem 无 allowlist** | `file_read` 用 denylist，可绕过 | `modules/tools/builtin/file_read.rs` |
| **Bash 无 workdir 限制** | 直接在 host 环境执行 | `modules/tools/builtin/bash.rs` |
| **PermissionMode 未生效** | 定义了但始终用 `DangerFullAccess` | `modules/runtime/permissions.rs` |
| **Sandbox 未激活** | 定义了 `sandbox.rs` 但从未使用 | 后续迭代 |

### 激活需要的改动

1. **`commands/agent.rs`** — 传递 `project_id` 并查询 `workdir` 注入 context
2. **`modules/tools/builtin/file_read.rs`** — 实现 workdir allowlist
3. **`modules/tools/builtin/bash.rs`** — 实现 workdir 限制
4. **`modules/projects/mod.rs`** — 新增 `PermissionMode` 持久化

### 设计文档

完整的边界隔离设计见 [project-workdir-boundary.md](./project-workdir-boundary.md)。

---

## 1. 系统概览

### 1.1 为什么需要 Project？

用户需要用 If2Ai 处理多个独立的工作区（工作目录 / 代码仓库 / 文档项目）。
每个 Project 对应一个本地工作目录，Project 下的每个 Session 是独立的对话线程。

```
Project "web-app"         Project "data-pipeline"
├── Session "auth bug"    ├── Session "etl refactor"
├── Session "api design"  └── Session "deployment"
└── Session "readme"     └── Session "metrics"
```

### 1.2 Project ↔ Session 关系

- **1:N** — 一个 Project 包含多个 Session
- **Session 不可跨 Project 移动**（保持数据完整性）
- Project 是纯元数据，Session 存储在 Project 子目录下

### 1.3 存储结构

```
~/.if2ai/
├── projects/
│   └── <project_id>/
│       └── project.json      # Project 元数据
│       └── sessions/
│           ├── <session_id>.json
│           └── <session_id>.json
```

---

## 2. 数据模型

### 2.1 Project struct

```rust
// src-tauri/src/modules/projects/mod.rs  (新建)

/// Project metadata — represents a workspace/project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    /// Unique project ID (UUID v4).
    pub id: String,
    /// Human-readable project name (editable).
    pub name: String,
    /// Absolute path to the project working directory.
    pub workdir: PathBuf,
    /// Creation timestamp (RFC3339).
    pub created_at: String,
    /// Last accessed timestamp (RFC3339).
    pub updated_at: String,
}
```

### 2.2 ProjectMeta (for listing)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub workdir: String,         // display only (PathBuf → String)
    pub created_at: String,
    pub session_count: usize,    // number of sessions in this project
}
```

### 2.3 ProjectError

```rust
#[derive(Debug, Clone)]
pub enum ProjectError {
    NotFound(String),
    AlreadyExists(String),
    WorkdirNotFound(String),
    ReadError(String),
    WriteError(String),
    InvalidData(String),
}
```

---

## 3. ProjectManager

```rust
// src-tauri/src/modules/projects/mod.rs

pub struct ProjectManager {
    projects_dir: PathBuf,
}

impl ProjectManager {
    /// Create a new project.
    pub async fn create_project(
        &self,
        name: String,
        workdir: PathBuf,
    ) -> Result<Project, ProjectError>;

    /// Get a project by ID.
    pub async fn get_project(&self, id: &str) -> Result<Project, ProjectError>;

    /// List all projects (sorted by updated_at descending).
    pub async fn list_projects(&self) -> Result<Vec<ProjectMeta>, ProjectError>;

    /// Update project name.
    pub async fn rename_project(
        &self,
        id: &str,
        new_name: String,
    ) -> Result<Project, ProjectError>;

    /// Delete a project and all its sessions.
    pub async fn delete_project(&self, id: &str) -> Result<(), ProjectError>;

    /// Get the sessions directory for a project.
    pub fn sessions_dir(&self, project_id: &str) -> PathBuf;
}
```

### 3.1 存储格式

每个 Project 存储为 `~/.if2ai/projects/<id>/project.json`：

```json
{
  "id": "uuid-v4",
  "name": "my-project",
  "workdir": "/Users/ryan/Dev/my-project",
  "created_at": "2026-04-12T10:00:00Z",
  "updated_at": "2026-04-12T12:30:00Z"
}
```

---

## 4. Session ↔ Project 关联

Session 数据新增 `project_id` 字段。SessionManager 支持**双路径存储**：

```
旧路径（仅兼容旧数据，仅读取）: ~/.if2ai/sessions/<session_id>.json
新路径（新建 session）        : ~/.if2ai/projects/<project_id>/sessions/<session_id>.json
```

> **向后兼容策略**：存量无 `project_id` 的旧 session 文件仍在旧路径，通过 `sessions_dir_old` 只读访问。
> 新建 session 全部写入新路径。旧 session 不会迁移（保持数据完整性）。

### 4.1 Session struct 修改

**现有 `Session`（src-tauri/src/modules/session/manager.rs）无 `project_id` 字段，必须新增：**

```rust
// 修改 src-tauri/src/modules/session/manager.rs

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct Session {
    pub id: String,
    pub project_id: String,           // NEW: owning project（兼容旧值为空字符串）
    pub title: String,
    pub messages: Vec<ConversationMessage>,
    pub created_at: String,
    pub updated_at: String,
    pub token_count: u64,
}
```

> **旧 session 文件**（无 `project_id` 字段）：读取时 `serde_json` 会将缺失字段默认为空字符串 `""`，不影响功能。

### 4.2 Session::new() 修改

```rust
// 修改现有 Session::new(title) 签名，支持传入 project_id
impl Session {
    /// Create a new session with a title and optional project_id.
    /// project_id="" 表示无关联 project（旧路径兼容）。
    pub fn new(title: String, project_id: String) -> Self { ... }
}
```

### 4.3 SessionManager 修改

```rust
// 修改 src-tauri/src/modules/session/manager.rs
// SessionManager 新增 projects_base_dir 字段，支持双路径

pub struct SessionManager {
    sessions_dir: PathBuf,         // 旧路径 ~/.if2ai/sessions/（仅读）
    projects_base_dir: PathBuf,  // 新路径 ~/.if2ai/projects/
}

impl SessionManager {
    /// Create a session within a specific project (新路径).
    pub async fn create_session_for_project(
        &self,
        project_id: &str,
        title: String,
    ) -> Result<Session, SessionError>;

    /// List sessions within a specific project (新路径).
    pub async fn list_project_sessions(
        &self,
        project_id: &str,
    ) -> Result<Vec<SessionMeta>, SessionError>;

    /// 旧方法保留：创建无 project 的 session（旧路径，仅向后兼容）。
    pub async fn create_session(&self, title: impl Into<String>) -> Result<Session, SessionError> {
        self._create_session_impl(title, String::new()).await
    }

    /// 内部实现：统一创建逻辑。
    async fn _create_session_impl(
        &self,
        title: impl Into<String>,
        project_id: String,
    ) -> Result<Session, SessionError>;
}
```

> **路径计算规则**：
> - `project_id == ""` → 存储到 `~/.if2ai/sessions/<id>.json`（旧路径，只读）
> - `project_id != ""` → 存储到 `~/.if2ai/projects/<project_id>/sessions/<id>.json`（新路径）

---

## 5. Tauri Commands

### 5.1 Project Commands

```rust
// src-tauri/src/commands/project.rs  (新建)

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    name: String,
    workdir: String,
) -> Result<Project, String>

#[tauri::command]
pub async fn list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<ProjectMeta>, String>

#[tauri::command]
pub async fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Project, String>

#[tauri::command]
pub async fn rename_project(
    state: State<'_, AppState>,
    id: String,
    new_name: String,
) -> Result<Project, String>

#[tauri::command]
pub async fn delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String>
```

### 5.2 New Session Commands (project-scoped)

```rust
// src-tauri/src/commands/session.rs  (新增 — project-scoped session commands)

#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    project_id: String,   // "" = 旧路径（无 project），非空 = 新路径
    title: String,
) -> Result<Session, String>

#[tauri::command]
pub async fn list_project_sessions(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<SessionMeta>, String>

// 现有的 list_sessions() / delete_session() 保留不动
```

### 5.3 AppState 扩展

```rust
// src-tauri/src/commands/mod.rs

pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,  // NEW
}
```

---

## 6. 前端 TypeScript 接口

```typescript
// src/lib/tauri.ts  (新增接口)

export interface Project {
  id: string;
  name: string;
  workdir: string;
  created_at: string;
  updated_at: string;
}

export interface ProjectMeta {
  id: string;
  name: string;
  workdir: string;
  created_at: string;
  session_count: number;
}

// Project operations
export async function createProject(name: string, workdir: string): Promise<Project>
export async function listProjects(): Promise<ProjectMeta[]>
export async function getProject(id: string): Promise<Project>
export async function renameProject(id: string, newName: string): Promise<Project>
export async function deleteProject(id: string): Promise<void>

// Session operations (project-scoped)
export async function createSession(projectId: string, title: string): Promise<Session>
export async function listProjectSessions(projectId: string): Promise<SessionMeta[]>
```

---

## 7. UI 设计规范

### 7.1 Left Rail 结构

```
┌─────────────────────────┐
│ [Logo] If2Ai            │
├─────────────────────────┤
│ ▼ Project 1             │  ← 可展开的项目（展开显示 sessions）
│   · if2ai               │  ← Session（选中态：白色背景）
│   · auth bug             │
│   · api design           │
├─────────────────────────┤
│ [+] 新建对话             │  ← 在当前 project 下创建新 session
├─────────────────────────┤
│ ▼ if2ai                 │  ← 另一个 project
│   · readme               │
├─────────────────────────┤
│ [设置]                   │
└─────────────────────────┘
```

- 项目名称旁边有展开/折叠箭头 (ChevronRight / ChevronDown)
- 选中 project 时高亮整行
- 选中 session 时高亮并显示右侧箭头
- "新建对话" 在当前 project 的 session 列表下方

### 7.2 欢迎界面（Welcome Screen）

当无活跃 session 或用户点击项目切换时显示：

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│        ✨  有什么我可以帮助你的？                   │
│                                                     │
│  ┌─────────────────────┐  ┌──────────────────────┐  │
│  │  📁 Project 1      │  │  📁 if2ai            │  │
│  │  📁 data-pipeline  │  │                      │  │
│  │  ─────────────────│  │                      │  │
│  │  ➕ 新建项目        │  │                      │  │
│  └─────────────────────┘  └──────────────────────┘  │
│                                                     │
│         [在 "if2ai" 中开始新对话 →]                │
└─────────────────────────────────────────────────────┘
```

- 左下角 Project 快捷选择区（当前 project 高亮）
- 右上角当前 project 名称 + 可点击切换
- "新建项目" 按钮打开系统目录选择对话框
- 右下角 "在 [当前 project] 中开始新对话 →"

### 7.3 Session 状态指示器

在 Left Rail 的 session 行右侧显示状态图标：

| 状态 | 图标 | 颜色 |
|------|------|------|
| Idle | `○` | 灰色 |
| Running | `◐` | 蓝色 + spin |
| Working | `●` | 绿色 |
| Error | `✕` | 红色 |

状态存储在 React 前端 state 中（不需要持久化）。

### 7.4 底部状态栏

始终显示在聊天窗口底部：

```
┌────────────────────────────────────────────────────┐
│ Session: if2ai · Project: my-project    [Idle]    │
└────────────────────────────────────────────────────┘
```

---

## 8. 实现顺序

1. **ProjectManager (Rust)** — Project CRUD，数据模型，持久化
2. **Project Commands** — Tauri IPC for projects
3. **Session ↔ Project 关联** — 修改 SessionManager session 路径
4. **Enhanced Frontend** — Left Rail 项目树 + 欢迎界面
5. **Session Status** — 前端状态管理 + 状态指示器
6. **真实 Agent 集成** — `run_agent_turn` 调用 ConversationRuntime

---

## 9. 与现有模块的兼容性

- `SessionManager` 的 `create_session(title)` 仍可用（创建无 project 的 session 用于向后兼容）
- `list_sessions()` 返回所有 session（不考虑 project）
- 新命令全部是增量添加，不修改已稳定的接口
