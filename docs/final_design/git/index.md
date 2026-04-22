# Git 集成模块

> If2Ai 的版本控制系统集成——Slash 命令、Prompt 上下文注入与差距分析

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 面向用户的使用指南 |
| [02-implementation.md](./02-implementation.md) | 面向开发者的实现细节与差距分析 |

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **Slash 命令** | `/branch`、`/worktree`、`/commit`、`/commit-push-pr` — Rust 原生 Git 操作 |
| **Prompt 上下文注入** | 每次 Agent 执行前自动捕获 `git status` + `git diff` 注入系统提示 |
| **Worktree 管理** | Git worktree 增删查剪，支持 Tauri IPC 命令创建永久 worktree |
| **默认分支检测** | `origin/HEAD` → `main/master` → 当前分支 三级回退 |
| **commit-push-pr** | 一键 Git→GitHub 工作流：提交→推送→创建 PR |

## 🏗️ 核心源码位置

### cc-haha-main (TypeScript)

| 文件 | 行数 | 核心功能 |
|------|------|--------|
| `src/utils/git.ts` | 927 | Git 基础操作、状态追踪、stash、聚合快照 |
| `src/utils/gitDiff.ts` | 533 | Diff 追踪（两阶段探测、单文件 diff、hunks 解析） |
| `src/utils/git/gitFilesystem.ts` | 700 | 零子进程路径（FS 直读 .git 结构、文件观察器、脏标记缓存） |
| `src/utils/worktree.ts` | 1520 | Worktree 生命周期（创建/恢复/sparse-checkout/.worktreeinclude） |
| `src/tools/shared/gitOperationTracking.ts` | 278 | Git 操作检测引擎（commit/push/merge/rebase/PR 文本挖掘） |
| `src/tools/PowerShellTool/gitSafety.ts` | 177 | Git 安全防护（bare repo 逃逸、NTFS 特例、路径规范化） |
| `src/utils/git/gitignore.ts` | ~50 | .gitignore 集成（isPathGitignored） |

### If2Ai (Rust)

| 文件 | 行数 | 核心功能 |
|------|------|--------|
| `rust/crates/commands/src/lib.rs` | ~300 行 Git 相关 | Slash 命令（`/branch` `/worktree` `/commit` `/commit-push-pr`） |
| `src-tauri/src/modules/runtime/prompt/instruction_files.rs` | ~80 行 Git 相关 | Prompt 快照（read_git_status / read_git_diff） |
| `src-tauri/src/commands/project.rs` | ~50 行 Git 相关 | Tauri 命令（create_permanent_worktree） |
| `rust/crates/claw-cli/src/main.rs` | ~15 行 Git 相关 | Git 仓库根目录发现（find_git_root） |
| `rust/crates/claw-cli/src/init.rs` | ~30 行 Git 相关 | .gitignore 管理（ensure_gitignore_entries） |

## 📊 功能对标矩阵

> 基于 cc-haha-main 源码审查与 If2Ai 源码审查的精确对比。
> 每个维度均附带精确源码位置。cc-haha 源码路径前缀：`/Users/ryanliu/Documents/IfAI/cc-haha-main/`。

| # | 功能维度 | cc-haha 实现 | If2Ai 实现 | 状态 |
|---|---------|-------------|-----------|------|
| 1 | Git Diff 两阶段探测 | `gitDiff.ts` L49-108 fetchGitDiff() shortstat→numstat | 无 | ❌ 缺失 |
| 2 | 单文件 Diff（FileEdit 集成） | `gitDiff.ts` L405-441 fetchSingleFileGitDiff() + getDiffRef merge-base | 无 | ❌ 缺失 |
| 3 | Diff 解析器（hunks） | `gitDiff.ts` L200-298 parseGitDiff() 含内存优化 | 无 | ❌ 缺失 |
| 4 | Git 状态快照 | `git.ts` L389-417 getFileStatus() porcelain 解析 | `instruction_files.rs` L54-70 read_git_status() | ⚠️ 部分 |
| 5 | Git 聚合状态 | `git.ts` L463-502 getGitState() 6 字段并行 Promise.all | 无 | ❌ 缺失 |
| 6 | Stash 防数据丢失 | `git.ts` L429-461 stashToCleanState() 三步法 | 无 | ❌ 缺失 |
| 7 | 零子进程 FS 直读 | `gitFilesystem.ts` 全文件 700 行 — HEAD/refs/packed-refs 直读 | 无（全部通过 shell 命令） | ❌ 缺失 |
| 8 | Git 根目录查找（LRU 缓存） | `gitFilesystem.ts` L27-110 memoizeWithLRU(50) + 性能监测 | `main.rs` L860-872 find_git_root() 无缓存 | ⚠️ 部分 |
| 9 | Worktree 规范化（安全验证） | `gitFilesystem.ts` L123-183 双向验证 + realpath | 无 | ❌ 缺失 |
| 10 | HEAD 解析（分支/分离检测） | `gitFilesystem.ts` L149-183 readGitHead() + isSafeRefName | 无 | ❌ 缺失 |
| 11 | Ref 解析（松散+packed-refs） | `gitFilesystem.ts` L203-266 resolveRef() + commonDir | 无 | ❌ 缺失 |
| 12 | 文件观察器（脏标记缓存） | `gitFilesystem.ts` L333-496 GitFileWatcher 类 | 无 | ❌ 缺失 |
| 13 | Worktree 创建与恢复 | `worktree.ts` L235-375 快速恢复 + sparse-checkout + .worktreeinclude | `commands/lib.rs` L872-925 + `project.rs` L400-447 基础 add/remove | ⚠️ 部分 |
| 14 | .worktreeinclude 文件复制 | `worktree.ts` L391-504 智能展开+折叠优化 | 无 | ❌ 缺失 |
| 15 | Git 操作检测引擎 | `gitOperationTracking.ts` L135-186 commit/push/merge/rebase/PR 文本挖掘 | 无 | ❌ 缺失 |
| 16 | Git 安全防护 | `gitSafety.ts` L75-151 bare repo 逃逸 + NTFS 特例 | 无 | ❌ 缺失 |
| 17 | .gitignore 搜索集成 | `gitignore.ts` L23-37 + Glob 工具 `--no-ignore` 可控 | 搜索工具不尊重 .gitignore | ❌ 缺失 |
| 18 | 分支管理 | 通过 BashTool | `commands/lib.rs` L833-870 /branch list/create/switch | ✅ If2Ai 有 |
| 19 | 提交工作流 | 通过 BashTool | `commands/lib.rs` L927-951 /commit + L953-1058 /commit-push-pr | ✅ If2Ai 有 |
| 20 | 默认分支检测 | `git.ts` getDefaultBranch() | `commands/lib.rs` L1060-1079 detect_default_branch() | ✅ 两者都有 |
| 21 | 前端分支 UI | 动态获取 | `App.tsx` L685 硬编码 `feature/consolidate-codebase` | ⚠️ If2Ai 硬编码 |
| 22 | Prompt Git 上下文注入 | 无独立实现（工具级别） | `instruction_files.rs` L54-128 自动 status+diff 注入 | ✅ If2Ai 优势 |
| 23 | PR/Issue 创建 | SuggestBackgroundPRTool/ReportPRTool | /pr /issue 定义存在但未实现 | ⚠️ 两者都不完整 |

### 状态统计

| 状态 | 数量 | 说明 |
|------|------|------|
| ✅ If2Ai 优势 | 4 | 分支管理、提交工作流、默认分支检测、Prompt 上下文注入 |
| ⚠️ 部分覆盖 | 5 | Git 状态快照、根目录查找（无缓存）、Worktree 基础操作、前端硬编码分支、PR/Issue |
| ❌ 缺失 | 14 | Diff 探测/解析、聚合状态、Stash、FS 直读、Ref/HEAD 解析、文件观察器、安全防护、.gitignore 集成等 |

### 总结

- **✅ If2Ai 优势**（4 项）：结构化 Slash 命令体系（比 cc-haha 通过 BashTool 操作更规范）、Prompt 级别自动 Git 上下文注入、完整的 commit-push-pr 自动化、智能默认分支检测
- **⚠️ 部分覆盖**（5 项）：Git 状态快照（有但简单）、根目录查找（无缓存）、Worktree（基础操作）、前端分支显示（硬编码）、PR/Issue 创建（仅定义）
- **❌ 缺失**（14 项）：Diff 系统（两阶段探测+单文件+解析器）、聚合状态、Stash、FS 直读路径、Ref/HEAD 解析、文件观察器、Worktree 高级功能、操作检测引擎、安全防护、.gitignore 搜索集成

缺失功能的复刻方案与增强路线图详见 [02-implementation.md](./02-implementation.md)。

## 🔗 相关资源

- [cc-haha Git 源码](/Users/ryanliu/Documents/IfAI/cc-haha-main/src/utils/git.ts)
- [If2Ai 命令系统](../../rust/crates/commands/src/lib.rs)
- [编码规范](../../references/coding-style-and-lint-contract.md)
