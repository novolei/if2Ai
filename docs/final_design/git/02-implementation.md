# Git 模块 — 实现细节与差距分析

> If2Ai Git 实现的精确源码分析，以及与 cc-haha-main 的逐项差距对比

## 1. 当前 If2Ai 实现

### 1.1 Slash 命令系统

If2Ai 的 Git 功能核心在 `rust/crates/commands/src/lib.rs`，通过结构化的 Slash 命令提供。

**命令注册**（L73-233 `SLASH_COMMAND_SPECS` 数组）：

| 命令 | 类别 | 行号 | 状态 |
|------|------|------|------|
| `/branch` | `SlashCommandCategory::Git` | L187-193 | ✅ 已实现 |
| `/worktree` | `SlashCommandCategory::Git` | L195-200 | ✅ 已实现 |
| `/commit` | `SlashCommandCategory::Git` | L202-208 | ✅ 已实现 |
| `/commit-push-pr` | `SlashCommandCategory::Git` | L210-216 | ✅ 已实现 |
| `/diff` | `SlashCommandCategory::Workspace` | L162-169 | ❌ 仅定义 |
| `/pr` | `SlashCommandCategory::Git` | L218-224 | ❌ 仅定义 |
| `/issue` | `SlashCommandCategory::Git` | L226-232 | ❌ 仅定义 |

**Git 命令基础设施**（L1081-1100）：

```rust
// L1081-1083: 获取 git 命令 stdout
fn git_stdout(cwd: &Path, args: &[&str]) -> io::Result<String>
// → 内部调用 run_command_stdout("git", args, cwd)

// L1085-1087: 检查 git 命令成功
fn git_status_ok(cwd: &Path, args: &[&str]) -> io::Result<()>
// → 内部调用 run_command_success("git", args, cwd)
```

**辅助函数**：

- `branch_exists(cwd, branch)` — 通过 `git branch --list <name>` 检查分支存在性
- `current_branch(cwd)` — 通过 `git branch --show-current` 获取当前分支
- `build_branch_name(hint)` — 生成合规的分支名：slugify hint 为小写并将空格/特殊字符归一为 `-`；若 `SAFEUSER` 或 `USER` 环境变量存在且非空则格式为 `<用户>/<slug>`，否则直接使用 slug（无额外长度裁剪）
- `write_temp_text_file(prefix, ext, content)` — 写入临时文件用于 commit message 和 PR body
- `command_exists(program)` — 检查外部命令可用性
- `detect_default_branch(cwd)` — 智能默认分支检测

### 1.2 Prompt Git 上下文注入

**源码**：`src-tauri/src/modules/runtime/prompt/instruction_files.rs` L54-128

#### read_git_status()（L54-70）

```rust
pub(super) fn read_git_status(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["--no-optional-locks", "status", "--short", "--branch"])
        .current_dir(cwd)
        .output()
        .ok()?;
    // ... 成功检查 + 空值过滤
}
```

- 使用 `--no-optional-locks` 避免与并发 git 进程冲突
- 使用 `--short --branch` 获取简洁状态 + 当前分支名
- 返回 `Option<String>`，空时自动跳过注入

#### read_git_diff()（L72-90）

```rust
pub(super) fn read_git_diff(cwd: &Path) -> Option<String> {
    let staged = read_git_output(cwd, &["diff", "--cached"])?;
    let unstaged = read_git_output(cwd, &["diff"])?;
    // 合并 Staged changes + Unstaged changes 两个 section
}
```

- 分别获取暂存和未暂存变更
- 无大小限制、无文件数限制、无超时
- 无未追踪文件 diff（`git diff` 默认不包含未追踪文件）

#### render_project_context()（L104-128）

将 status 和 diff 快照注入 `# Project context` 部分：

```rust
pub(super) fn render_project_context(project_context: &ProjectContext) -> String {
    let mut lines = vec!["# Project context".to_string()];
    // ... 日期 + 工作目录 + 指令文件数
    if let Some(status) = &project_context.git_status {
        lines.push("Git status snapshot:".to_string());
        lines.push(status.clone());
    }
    if let Some(diff) = &project_context.git_diff {
        lines.push("Git diff snapshot:".to_string());
        lines.push(diff.clone());
    }
    lines.join("\n")
}
```

### 1.3 Worktree 管理

If2Ai 有两个 Worktree 入口：

#### Slash 命令版（`commands/lib.rs` L872-925）

- `/worktree list` → `git worktree list`
- `/worktree add <path> [branch]` → `git worktree add`，支持分支存在性检查
- `/worktree remove <path>` → `git worktree remove`
- `/worktree prune` → `git worktree prune`
- 约 54 行实现

#### Tauri 命令版（`src-tauri/src/commands/project.rs` L400-447）

```rust
#[tauri::command]
pub async fn create_permanent_worktree(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let project = state.project_manager.get_project(&id).await...;
    let branch_name = format!("if2ai/{slug}");
    // git worktree add <target> -b <branch>
}
```

- 分支命名规范：`if2ai/{project-slug}`
- 目标路径：`{parent-dir}/{project-slug}-worktree`
- 使用 `tokio::task::spawn_blocking` 异步执行
- 约 48 行实现

### 1.4 Git 仓库检测与 .gitignore

#### find_git_root()（`rust/crates/claw-cli/src/main.rs` L860-872）

```rust
fn find_git_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(env::current_dir()?)
        .output()?;
    // ... 成功检查 + 空值检查
    Ok(PathBuf::from(path))
}
```

- 通过子进程调用 `git rev-parse --show-toplevel`
- 无缓存机制（每次调用都启动新子进程）
- 仅在 CLI 入口使用

#### ensure_gitignore_entries()（`rust/crates/claw-cli/src/init.rs` L130-160）

```rust
fn ensure_gitignore_entries(path: &Path) -> Result<InitStatus, std::io::Error> {
    if !path.exists() {
        // 创建新 .gitignore，添加 .claw 相关条目
    }
    // 检查已有条目，缺失则追加
    // 三种状态：Created / Updated / Skipped
}
```

- 管理 If2Ai（Claw）相关的 .gitignore 条目
- 仅在 `/init` 命令时执行
- 无运行时 .gitignore 查询能力

---

## 2. cc-haha 差距分析与增强计划

### A 类：cc-haha 有但 If2Ai 缺失（需复刻）

#### A.1 Git Diff 两阶段探测系统

- **cc-haha 源码**：`src/utils/gitDiff.ts` L49-108
- **实现**：`fetchGitDiff()` 采用两阶段探测
  - 第一阶段：`git diff --shortstat` O(1) 获取变更文件数和行数
  - 第二阶段：若文件数 ≤ `MAX_FILES`(50)，执行 `git diff --numstat` 获取详细统计
  - 若文件数 ≤ `MAX_FILES_FOR_DETAILS`(500)，获取逐文件 diff
  - 合并未追踪文件列表（`git ls-files --others --exclude-standard`）
- **关键参数**：
  - `MAX_FILES = 50` — 触发详细 diff 的文件数阈值
  - `MAX_FILES_FOR_DETAILS = 500` — 触发逐文件 diff 的阈值
  - `MAX_DIFF_SIZE_BYTES = 1MB` — 单个 diff 大小限制
  - `GIT_TIMEOUT_MS = 5000` — git 命令超时
- **If2Ai 现状**：`read_git_diff()`（`instruction_files.rs` L72-90）获取完整 diff 文本，无大小/文件数/超时限制
- **风险**：大型仓库的 diff 可能达到数 MB，严重消耗 Prompt 预算
- **建议方案**：
  1. 在 `instruction_files.rs` 中为 `read_git_diff()` 添加两阶段探测
  2. 第一阶段用 `git diff --shortstat` 快速判断
  3. 超过阈值时截断或降级为 numstat 摘要
  4. 添加 5s 超时（`tokio::time::timeout`）

#### A.2 单文件 Diff（FileEdit 集成）

- **cc-haha 源码**：`src/utils/gitDiff.ts` L405-441
- **实现**：`fetchSingleFileGitDiff()` 对编辑后的文件获取 PR 视角 diff
  - `getDiffRef` 优先级：`CLAUDE_CODE_BASE_REF` 环境变量 → `merge-base` → `HEAD`
  - 未追踪文件：生成全量新增 diff
- **If2Ai 现状**：无对应功能
- **建议方案**：
  1. 在 `commands` crate 中新增 `single_file_diff(cwd, file_path)` 函数
  2. 优先使用 merge-base 作为 diff ref
  3. 供 `/diff` 命令和 file_edit 工具调用

#### A.3 Diff 解析器与内存优化

- **cc-haha 源码**：`src/utils/gitDiff.ts` L200-298
- **实现**：`parseGitDiff()` 解析 hunks
  - 内存优化：`'' + line` 强制分配（V8 切片字符串防泄漏）
  - `MAX_LINES_PER_FILE = 400` — 每文件最大行数限制
- **If2Ai 现状**：无对应功能
- **建议方案**：
  1. 在 Rust 中实现 `parse_diff_hunks()` 函数
  2. Rust 的 `String` 无切片字符串泄漏问题，无需 V8 式优化
  3. 保留每文件行数限制（400 行）避免单文件爆炸

#### A.4 Git 聚合状态快照

- **cc-haha 源码**：`src/utils/git.ts` L463-502
- **实现**：`getGitState()` 返回 6 字段
  - `commitHash` — 当前 HEAD commit
  - `branchName` — 当前分支名
  - `remoteUrl` — 远程仓库 URL
  - `isHeadOnRemote` — HEAD 是否在远程存在
  - `isClean` — 工作区是否干净
  - `worktreeCount` — worktree 数量
  - 并行 `Promise.all` 获取
- **If2Ai 现状**：仅有 `read_git_status()` 和 `read_git_diff()` 两个独立函数，无聚合快照
- **建议方案**：
  1. 在 `src-tauri/src/modules/` 下新增 `git/` 模块
  2. 定义 `GitState` struct 含上述 6 字段
  3. 使用 `tokio::join!` 并行获取各字段
  4. 通过 Tauri IPC 暴露给前端

#### A.5 Stash 防数据丢失

- **cc-haha 源码**：`src/utils/git.ts` L429-461
- **实现**：`stashToCleanState()` 三步法
  1. 检查是否有未追踪文件（`git ls-files --others`）
  2. `git add -A` 暂存所有变更
  3. `git stash push` 存入 stash
- **If2Ai 现状**：无 stash 功能
- **建议方案**：
  1. 在 `commands` crate 中新增 `stash_to_clean_state()` 函数
  2. 在 `/commit` 和 `/commit-push-pr` 执行前可选调用
  3. 提供 `/stash list` / `/stash pop` Slash 命令

#### A.6 零子进程 FS 直读

- **cc-haha 源码**：`src/utils/git/gitFilesystem.ts` 全文件 700 行
- **实现**：直读 `.git/HEAD`、`refs/`、`packed-refs`
  - 性能：~10ms 子进程 → ~1ms FS 读取
  - 安全：`isSafeRefName()` allowlist 验证 + `isValidGitSha()` 40/64 字符校验
  - 功能：`readGitHead()`、`resolveRef()`、`getGitRoot()`、`GitFileWatcher`
- **If2Ai 现状**：全部通过 `std::process::Command` 调用 git CLI
- **建议方案**：
  1. 在 `src-tauri/src/modules/git/` 中新增 `filesystem.rs`
  2. 使用 `std::fs::read_to_string` 直读 `.git/HEAD` 和 `refs/`
  3. 解析 `packed-refs` 文件（格式：`<sha> <ref-name>`）
  4. Rust 的 FS 读取性能更优（无需 V8 序列化开销）
  5. 添加 `is_safe_ref_name()` 和 `is_valid_git_sha()` 验证函数

#### A.7 文件观察器（脏标记缓存）

- **cc-haha 源码**：`src/utils/git/gitFilesystem.ts` L333-496
- **实现**：`GitFileWatcher` 类
  - 观察 `.git/HEAD`、`config`、当前分支 ref 文件
  - 脏标记设计：计算前清除 → 计算中如有变更重设 → 下次 `get()` 重算
  - 使用 `fs.watch()` 实现
- **If2Ai 现状**：无对应功能
- **建议方案**：
  1. 使用 `notify` crate（Tauri 项目已有依赖）监听 `.git/` 目录变更
  2. 实现 `GitStateCache` struct，带 `AtomicBool` 脏标记
  3. 缓存 `GitState` 结果，观察器触发时设脏
  4. 在 Agent 循环开始时检查脏标记，按需刷新

#### A.8 Worktree 高级功能

- **cc-haha 源码**：`src/utils/worktree.ts` L235-375（1520 行总量）
- **实现**：
  - 快速恢复路径：FS 直读跳过 `git fetch`（节省 6-8s）
  - sparse-checkout 支持 + 失败回滚
  - `.worktreeinclude` 文件复制（L391-504 智能展开+折叠优化）
- **If2Ai 现状**：基础 `add/remove/list/prune`（~54 行）+ Tauri 命令（~48 行）
- **建议方案**：
  1. 在 `commands` crate 中扩展 `/worktree` 命令
  2. 新增 `/worktree restore <path>` 恢复已存在的 worktree
  3. 添加 `.worktreeinclude` 支持（读取文件列表并在 worktree 创建时复制）
  4. sparse-checkout 可延后到 P4

#### A.9 Git 操作检测引擎

- **cc-haha 源码**：`src/tools/shared/gitOperationTracking.ts` L135-186
- **实现**：`detectGitOperation()` 解析 LLM 输出中的 Git 操作
  - 检测 commit SHA（`/\b[0-9a-f]{7,40}\b/`）
  - 检测 push 分支
  - 检测 merge/rebase 结果
  - 检测 PR 号（`#\\d+`）
- **If2Ai 现状**：无对应功能
- **建议方案**：
  1. 在 `src-tauri/src/modules/git/` 中新增 `operation_tracking.rs`
  2. 使用 `regex` crate 实现 Git 操作正则匹配
  3. 在 Agent 循环的工具执行回调中调用
  4. 用于触发后续操作（如 push 后自动检查 CI 状态）

#### A.10 Git 安全防护

- **cc-haha 源码**：`src/tools/PowerShellTool/gitSafety.ts` L75-151
- **实现**：
  - bare repo 沙箱逃逸防护
  - NTFS 路径规范化（8.3 短名、PS provider 限定、驱动器相对路径）
- **If2Ai 现状**：无 Git 特定安全验证
- **建议方案**：
  1. 在 `src-tauri/src/modules/git/` 中新增 `safety.rs`
  2. 实现 `is_safe_ref_name()` — allowlist 字符集验证
  3. 实现 `is_valid_git_sha()` — 40/64 字符十六进制校验
  4. 实现 `is_bare_repo()` — 检测 `.git/config` 中的 `bare = true`
  5. NTFS 特例在 Rust 中不适用（macOS/Linux 为主平台）
  6. 在所有 Git 操作入口调用安全检查

#### A.11 .gitignore 搜索集成

- **cc-haha 源码**：`src/utils/git/gitignore.ts` L23-37
- **实现**：`isPathGitignored()` + Glob 工具 `--no-ignore` 环境变量可控
- **If2Ai 现状**：`glob_search` 和 `content_search` 工具不尊重 .gitignore
- **建议方案**：
  1. 在文件搜索工具中添加 `respect_gitignore: bool` 参数（默认 true）
  2. 使用 `ignore` crate（ripgrep 的 .gitignore 解析库）进行过滤
  3. 在 `glob_search` 实现中调用 `.gitignore` 过滤
  4. 在 `content_search` 实现中传入 `--ignore` 相关参数

#### A.12 前端动态分支显示

- **cc-haha**：动态获取当前分支
- **If2Ai 现状**：`src/App.tsx` L685 硬编码为 `'feature/consolidate-codebase'`
  - 硬编码值通过 props 传递到 `ChatWorkspace` → `HomeScreen` → UI 显示
  - 涉及文件：`App.tsx` L685, `chat-ui.tsx` L127/L233, `ChatWorkspace.tsx` L98/L353/L391, `HomeScreen.tsx` L33/L55/L440
- **建议方案**：
  1. 在 Tauri 后端新增 `get_current_branch()` 命令
  2. 前端在项目切换/Agent 循环开始时调用获取当前分支
  3. 替换 `App.tsx` L685 硬编码为动态值

### B 类：If2Ai 已有且优于 cc-haha（需保持和强化）

#### B.1 Prompt 自动 Git 上下文注入

- **If2Ai 源码**：`src-tauri/src/modules/runtime/prompt/instruction_files.rs` L54-128
- **优势**：每次 Agent 执行前自动捕获 `git status` + `git diff` 注入系统提示
- **cc-haha 对比**：无独立的 Prompt 级别 Git 上下文自动注入，仅在工具调用时获取
- **保持和强化**：
  1. 保持 `--no-optional-locks` 避免锁冲突
  2. 添加两阶段探测（A.1）后，注入的 diff 将更智能
  3. 考虑注入 `GitState` 聚合快照（A.4）替代纯 status 文本

#### B.2 完整的 Slash 命令体系

- **If2Ai 源码**：`rust/crates/commands/src/lib.rs`
- **优势**：结构化的 Rust 命令，比 cc-haha 通过 BashTool 更规范
  - `/branch list/create/switch` — L833-870
  - `/worktree list/add/remove/prune` — L872-925
  - `/commit` — L927-951
  - `/commit-push-pr` — L953-1058（完整 Git→GitHub 工作流自动化）
- **保持和强化**：
  1. 保持现有命令的稳定性
  2. 实现 `/diff`、`/pr`、`/issue` 的执行逻辑
  3. 添加 `/stash list/pop` 命令

#### B.3 项目级 Worktree 创建

- **If2Ai 源码**：`src-tauri/src/commands/project.rs` L400-447
- **优势**：前端 Tauri 命令，规范分支名 `if2ai/{project}`
- **cc-haha 对比**：无前端 GUI 触发 worktree 创建的能力
- **保持和强化**：
  1. 保持 Tauri IPC 接口
  2. 添加 sparse-checkout 支持（P4）

#### B.4 智能默认分支检测

- **If2Ai 源码**：`rust/crates/commands/src/lib.rs` L1060-1079
- **实现**：`detect_default_branch()` 三级回退
  1. `git symbolic-ref refs/remotes/origin/HEAD` — 获取远程默认分支
  2. `main` / `master` — 常见默认分支名
  3. `current_branch()` — 兜底返回当前分支
- **cc-haha 对比**：`git.ts` getDefaultBranch() 仅检查 `main/master`
- **保持和强化**：
  1. 现有三级回退已优于 cc-haha
  2. 可考虑缓存结果避免重复子进程调用

---

## 3. 增强路线图

### P0 — 基础能力（影响日常使用）

| # | 功能 | 优先级原因 | 预估工作量 |
|---|------|-----------|-----------|
| P0.1 | .gitignore 搜索集成 | 搜索结果包含大量无关文件，影响 Agent 效率 | 2 天 |
| P0.2 | 前端动态分支显示 | 硬编码分支名对用户造成困惑 | 0.5 天 |
| P0.3 | Git 聚合状态快照 | 前端和 Agent 都需要统一的 Git 状态视图 | 2 天 |

### P1 — 性能优化（高频操作加速）

| # | 功能 | 优先级原因 | 预估工作量 |
|---|------|-----------|-----------|
| P1.1 | FS 直读路径（HEAD/refs） | 减少 ~90% 的 git 子进程调用 | 3 天 |
| P1.2 | LRU 缓存 | 避免重复的 `git rev-parse` 和状态查询 | 1 天 |
| P1.3 | 文件观察器（脏标记缓存） | Agent 循环中避免重复计算 Git 状态 | 2 天 |

### P2 — 工具集成（Agent 能力增强）

| # | 功能 | 优先级原因 | 预估工作量 |
|---|------|-----------|-----------|
| P2.1 | Diff 两阶段探测 | 防止大仓库 diff 爆炸消耗 Prompt 预算 | 2 天 |
| P2.2 | 单文件 Diff | `/diff` 命令和 file_edit 工具需要 | 1 天 |
| P2.3 | Diff 解析器 | 结构化 diff 数据供 Agent 理解 | 2 天 |
| P2.4 | 操作检测引擎 | 自动检测 Agent 的 Git 操作结果 | 2 天 |

### P3 — 安全加固（防护性功能）

| # | 功能 | 优先级原因 | 预估工作量 |
|---|------|-----------|-----------|
| P3.1 | Ref 名验证 | 防止命令注入 | 1 天 |
| P3.2 | Bare repo 防护 | 防止沙箱逃逸 | 0.5 天 |
| P3.3 | Worktree 安全规范化 | 防止路径遍历 | 1 天 |

### P4 — 高级功能（完善体验）

| # | 功能 | 优先级原因 | 预估工作量 |
|---|------|-----------|-----------|
| P4.1 | Stash 管理 | 防数据丢失 | 1 天 |
| P4.2 | sparse-checkout | 大仓库 worktree 性能 | 2 天 |
| P4.3 | .worktreeinclude | worktree 文件同步 | 1 天 |
| P4.4 | 高级 Worktree 恢复 | 快速恢复节省 6-8s | 2 天 |

---

## 4. 模块化建议

建议在 `src-tauri/src/modules/` 下新增 `git/` bounded context 模块：

```
src-tauri/src/modules/git/
├── mod.rs              # 模块入口，公开 GitState、GitProvider
├── state.rs            # GitState struct + 聚合获取
├── filesystem.rs       # FS 直读（HEAD/refs/packed-refs）
├── diff.rs             # Diff 两阶段探测 + 解析器
├── safety.rs           # 安全验证（ref 名、SHA、bare repo）
├── operation.rs        # 操作检测引擎
├── ignore.rs           # .gitignore 解析与过滤
├── watcher.rs          # 文件观察器 + 脏标记缓存
└── worktree.rs         # Worktree 高级功能
```

这遵循 If2Ai 的 bounded context 模块化原则，将分散在 `commands/`、`instruction_files.rs`、`project.rs` 中的 Git 逻辑统一收敛。
