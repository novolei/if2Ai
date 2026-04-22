# Git 模块 — 使用指南

> If2Ai 当前可用的 Git 命令、自动上下文机制，以及与 cc-haha 的深度对比

## 1. 当前可用的 Git 命令

### /branch — 分支管理

- **定义**：`rust/crates/commands/src/lib.rs` L187-193
- **实现**：`rust/crates/commands/src/lib.rs` L833-870

| 子命令 | 用法 | 说明 |
|--------|------|------|
| `list` | `/branch` 或 `/branch list` | 列出本地分支（含最新提交信息） |
| `create` | `/branch create <name>` | 创建新分支并切换 |
| `switch` | `/branch switch <name>` | 切换到已有分支 |

**实现细节**：

```
/branch list   → git branch --list --verbose
/branch create → git switch -c <name>
/branch switch → git switch <name>
```

所有操作通过 `git_stdout()` / `git_status_ok()` 封装 `std::process::Command` 调用 git CLI。

### /worktree — Worktree 管理

- **定义**：`rust/crates/commands/src/lib.rs` L195-200
- **实现**：`rust/crates/commands/src/lib.rs` L872-925

| 子命令 | 用法 | 说明 |
|--------|------|------|
| `list` | `/worktree` 或 `/worktree list` | 列出所有 worktree |
| `add` | `/worktree add <path> [branch]` | 添加 worktree（可选指定分支） |
| `remove` | `/worktree remove <path>` | 移除 worktree |
| `prune` | `/worktree prune` | 清理已删除的 worktree 引用 |

**实现细节**：

```
/worktree list           → git worktree list
/worktree add <path>     → git worktree add <path>
/worktree add <path> -b  → git worktree add <path> -b <branch>  （分支不存在时）
/worktree add <path> <b> → git worktree add <path> <branch>     （分支已存在时）
/worktree remove <path>  → git worktree remove <path>
/worktree prune          → git worktree prune
```

分支存在性检查通过 `branch_exists()` 实现（`git branch --list <name>`）。

### /commit — 提交

- **定义**：`rust/crates/commands/src/lib.rs` L202-208
- **实现**：`rust/crates/commands/src/lib.rs` L927-951

**流程**：

1. 检查工作区是否有变更（`git status --short`）
2. 若无变更，返回 "skipped — no workspace changes"
3. 将提交消息写入临时文件（`claw-commit-message-*.txt`）
4. 执行 `git add -A` 暂存所有变更
5. 执行 `git commit --file <temp-file>` 提交

> 注意：`git add -A` 会暂存所有变更，包括未追踪文件。这与 cc-haha 的行为一致，但可能需要在权限模型中考虑。

### /commit-push-pr — 提交推送 PR 工作流

- **定义**：`rust/crates/commands/src/lib.rs` L210-216
- **实现**：`rust/crates/commands/src/lib.rs` L953-1058

**流程**：

1. 检查 `gh` CLI 是否可用（必需依赖）
2. 检测默认分支（`detect_default_branch()`）
3. 若当前在默认分支上，自动创建新分支（分支名由 `branch_name_hint` 或 `pr_title` 生成）
4. 如有工作区变更，执行 `/commit` 流程
5. 检查分支是否有 diff（`git diff --stat <default>...HEAD`）
6. 推送到远程（`git push --set-upstream origin <branch>`）
7. 创建 PR（`gh pr create --title ... --body-file ... --base <default>`）
8. 若 PR 已存在，通过 `gh pr view --json url` 获取已有 PR 链接

**分支名生成规则**（`build_branch_name()` 当前实现）：

- 取 `pr_title` 或 `branch_name_hint` 作为 hint
- 通过 slugify 转为小写并将空格/特殊字符归一为 `-`
- 若环境变量 `SAFEUSER` 或 `USER` 存在且非空，最终分支名为 `<用户>/<slug>`
- 否则直接使用 slug（无额外长度裁剪）

### /diff — Diff 显示（规划中）

- **定义**：`rust/crates/commands/src/lib.rs` L162-169
- **状态**：Slash 命令定义存在，但执行逻辑尚未实现（当前未在 `handle_slash_command` 中实现任何逻辑，执行 `/diff` 不会产生输出（完全无效果））

### /pr — PR 创建（规划中）

- **定义**：`rust/crates/commands/src/lib.rs` L218-224
- **状态**：Slash 命令定义存在，但执行逻辑尚未独立实现（功能已由 `/commit-push-pr` 覆盖部分场景）

### /issue — Issue 创建（规划中）

- **定义**：`rust/crates/commands/src/lib.rs` L226-232
- **状态**：Slash 命令定义存在，但执行逻辑尚未实现

## 2. 自动 Git 上下文

If2Ai 在每次 Agent 执行前自动捕获 Git 状态并注入系统提示词，这是 If2Ai 相对于 cc-haha 的独有优势。

### 工作原理

**源码**：`src-tauri/src/modules/runtime/prompt/instruction_files.rs` L54-128

1. **read_git_status()**（L54-70）：执行 `git --no-optional-locks status --short --branch`，获取当前分支和文件变更状态
2. **read_git_diff()**（L72-90）：分别执行 `git diff --cached`（暂存变更）和 `git diff`（未暂存变更），合并输出
3. **render_project_context()**（L104-128）：将 status 和 diff 快照注入系统提示的 `# Project context` 部分

### 注入格式

```
# Project context
- Today's date is 2026-04-21.
- Working directory: /path/to/project
- Claw instruction files discovered: 2.

Git status snapshot:
## feature/my-branch
 M src/main.rs
?? new-file.txt

Git diff snapshot:
Staged changes:
diff --git a/src/main.rs ...
Unstaged changes:
diff --git a/new-file.txt ...
```

### 关键特性

- **`--no-optional-locks`**：避免与并发的 git 进程冲突
- **自动空值过滤**：status/diff 为空时不注入
- **无大小限制**：与 cc-haha 的两阶段探测不同，If2Ai 获取完整 diff 文本，无文件数/大小限制

## 3. Tauri IPC Worktree 命令

**源码**：`src-tauri/src/commands/project.rs` L400-447

`create_permanent_worktree` 是前端可调用的 Tauri 命令，用于为项目创建持久化的 worktree：

- 分支命名规范：`if2ai/{project-slug}`
- 目标路径：`{parent-dir}/{project-slug}-worktree`
- 幂等性检查：目标路径已存在时返回错误
- 异步执行：使用 `tokio::task::spawn_blocking` 避免阻塞

## 4. 深度对比：cc-haha Git 操作方式 vs If2Ai

### 4.1 Diff 获取方式对比

| 维度 | cc-haha | If2Ai |
|------|---------|-------|
| **策略** | 两阶段探测 shortstat→numstat | 全量 `git diff` + `git diff --cached` |
| **源码** | `gitDiff.ts` L49-108 fetchGitDiff() | `instruction_files.rs` L72-90 read_git_diff() |
| **大小限制** | MAX_FILES=50, MAX_FILES_FOR_DETAILS=500, MAX_DIFF_SIZE_BYTES=1MB | 无限制 |
| **超时** | GIT_TIMEOUT_MS=5000 | 无超时 |
| **单文件 Diff** | fetchSingleFileGitDiff() merge-base 视角 | 无 |
| **性能** | O(1) shortstat 快速判断是否需要详细 diff | 始终获取完整 diff |

### 4.2 Git 根目录查找对比

| 维度 | cc-haha | If2Ai |
|------|---------|-------|
| **方法** | FS 直读 `.git/` 目录 + LRU 缓存 | `git rev-parse --show-toplevel` 子进程 |
| **源码** | `gitFilesystem.ts` L27-110 memoizeWithLRU(50) | `main.rs` L860-872 find_git_root() |
| **缓存** | LRU 50 条 + 性能监测计时 | 无缓存 |
| **性能** | ~1ms FS 读取 | ~10ms 子进程调用 |
| **安全性** | isSafeRefName() + isValidGitSha() | 无 Git 特定安全验证 |

### 4.3 Worktree 管理对比

| 维度 | cc-haha | If2Ai |
|------|---------|-------|
| **代码量** | `worktree.ts` 1520 行 | `commands/lib.rs` L872-925 (~54 行) + `project.rs` L400-447 (~48 行) |
| **快速恢复** | FS 直读跳过 fetch（节省 6-8s） | 无恢复机制 |
| **sparse-checkout** | 支持 + 失败回滚 | 无 |
| **.worktreeinclude** | 智能展开+折叠优化（L391-504） | 无 |
| **安全验证** | 双向验证 + realpath 规范化 | 无 |

### 4.4 安全性对比

| 维度 | cc-haha | If2Ai |
|------|---------|-------|
| **Ref 名验证** | isSafeRefName() allowlist | 无 |
| **SHA 校验** | isValidGitSha() 40/64 字符 | 无 |
| **Bare repo 防护** | 沙箱逃逸检测 | 无 |
| **NTFS 特例** | 8.3 短名、PS provider 限定、驱动器相对路径 | 不适用（非 Windows） |
| **路径规范化** | realpath 双向验证 | 无 |
