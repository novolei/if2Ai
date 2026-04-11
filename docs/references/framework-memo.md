# If2Ai 框架设计备忘录

> **受众**：系统架构师 Agent、新接手的 AI executor、人类 review 者  
> **目的**：一份文档掌握整个 If2Ai 工程体系——从架构设计到 executor 自动循环的完整链路  
> **最后更新**：2026-04-11 | **当前 Phase**：Phase 1 已完成，Phase 2/3 为 draft

---

## 目录

1. [项目概述](#1-项目概述)
2. [设计文档体系](#2-设计文档体系)
3. [Exec-Plan YAML 完整规范](#3-exec-plan-yaml-完整规范)
4. [Harness 框架详解](#4-harness-框架详解)
5. [Executor 17 步自动循环](#5-executor-17-步自动循环)
6. [防止 Executor 作弊的三重机制](#6-防止-executor-作弊的三重机制)
7. [Agent 系统（sub-agents）](#7-agent-系统sub-agents)
8. [代码规范合约](#8-代码规范合约)
9. [当前项目状态](#9-当前项目状态)
10. [常用命令速查](#10-常用命令速查)

---

## 1. 项目概述

### 技术栈
- **后端**：Rust + Tauri 2（桌面应用框架）
- **前端**：React + shadcn/ui（TypeScript）
- **AI 集成**：OpenAI / Claude / Ollama（多 provider 支持）
- **持久化**：SQLite（通过 rusqlite）+ 文件系统（会话 JSON）

### 核心模块结构
```
src-tauri/src/modules/
├── runtime/           # 对话循环（ConversationRuntime）+ 提示词构建 + 压缩
│   ├── conversation.rs      # Phase 1: 核心 agent 循环
│   ├── prompt.rs            # Phase 1/2: SystemPromptBuilder → PromptBuilder
│   ├── compression.rs       # Phase 2: ContextWindowManager（新建）
│   ├── error.rs             # Phase 2: AgentError enum（新建）
│   ├── http_server.rs       # Phase 3: axum HTTP API Server（新建）
│   └── lsp_bridge.rs        # Phase 3: stdio JSON-RPC Bridge（新建）
├── api/               # LLM provider 客户端（OpenAI/Claude/Ollama）
│   ├── client.rs
│   ├── provider.rs
│   └── fallback.rs          # Phase 2: FallbackChain（新建）
├── tools/             # 工具注册表（read_file, write_file, execute_command）
│   └── registry.rs
├── session/           # 会话管理（SessionManager）
│   └── manager.rs
├── memory/            # Phase 2: 记忆系统（整个目录新建）
│   ├── mod.rs
│   ├── provider.rs          # MemoryProvider trait
│   ├── builtin.rs           # BuiltInMemory（读写 MEMORY.md + USER.md）
│   └── manager.rs           # MemoryManager
└── plugins/           # 扩展插件
    └── gateway/             # Phase 2: 消息网关
        ├── adapter.rs       # MessagingAdapter trait（新建）
        ├── handler.rs       # MessageHandler + IncomingMessage（新建）
        ├── telegram.rs      # TelegramAdapter（新建）
        └── discord.rs       # Phase 2b: DiscordAdapter（新建）
```

### 入口点（3 个，分 Phase 实现）
| 入口 | Phase | 实现文件 |
|------|-------|----------|
| Tauri 桌面 UI（IPC） | Phase 1 | `src-tauri/src/commands/` |
| HTTP JSON-RPC API | Phase 3 | `runtime/http_server.rs` |
| IDE stdout/stdin JSON-RPC | Phase 3 | `runtime/lsp_bridge.rs` + bin |

---

## 2. 设计文档体系

### 阅读顺序（新 executor 必读路径）
```
1. docs/design-docs/system-architecture-framework.md  15min  全局 9 子系统视图
2. docs/design-docs/module-boundaries-and-integration.md  10min  模块边界
3. CLAUDE.md                                           5min   executor 行为规范
4. docs/references/coding-style-and-lint-contract.md   5min   代码规范（全局强制）
5. 当前 slice 的 design_ref 文档                       —      具体接口定义
```

### 所有设计文档（15 份）

#### 第一层：架构基础（3 份，Phase 1 实现参考）
| 文档 | 核心内容 |
|------|---------|
| `system-architecture-framework.md` | 9 子系统全景图，Phase 路线图，数据流 |
| `module-boundaries-and-integration.md` | 6 个 Rust 模块的边界和 AppState 共享状态 |
| `entry-points-design.md` | Tauri / API Server / IDE Bridge 三入口设计 |

#### 第二层：Phase 1 核心子系统（10 份）
| 文档 | 实现模块 | Phase |
|------|---------|-------|
| `agent-loop.md` | `runtime/conversation.rs` | 1 |
| `agent-orchestrator.md` | `runtime/orchestrator.rs` | 1 |
| `provider-resolution.md` | `api/provider.rs` | 1 |
| `llm-routing.md` | `api/routing.rs` | 1 |
| `tool-system.md` | `tools/registry.rs` | 1 |
| `prompt-builder.md` | `runtime/prompt.rs` | 1/2 |
| `session-persistence.md` | `session/manager.rs` | 1 |
| `data-schema.md` | 数据库 schema | 1 |
| `context-compression.md` | `runtime/compression.rs` | 2 |
| `error-handling.md` | `runtime/error.rs` + `api/fallback.rs` | 2 |

#### 第三层：Phase 2/3 高级特性（4 份）
| 文档 | 实现模块 | Phase |
|------|---------|-------|
| `messaging-gateway.md` | `plugins/gateway/` | 2 |
| `memory-system.md` | `modules/memory/` | 2 |
| `agent-self-improvement.md` | `runtime/trajectory.rs` + `reward.rs` | 3 |
| `testing-strategy.md` | `harness/` | 1-3 |

### 关联文档
| 文档 | 位置 | 用途 |
|------|------|------|
| `coding-style-and-lint-contract.md` | `docs/references/` | 全局 Rust 代码规范 |
| `harness-testing.md` | `docs/design-docs/` | Harness 测试策略 |
| `QUALITY_SCORE.md` | `docs/generated/` | Phase 完成状态自动记录 |

---

## 3. Exec-Plan YAML 完整规范

### 文件位置
```
docs/exec-plans/
├── index.md                          # 标注 [当前] Phase
├── active/
│   ├── phase-1-foundation.yaml       # Phase 1（已完成）
│   ├── phase-2-advanced-features.yaml   # Phase 2（draft）
│   └── phase-3-ecosystem.yaml        # Phase 3（draft）
└── completed/                        # 已归档的完成计划
```

### YAML 完整字段说明

```yaml
meta:
  phase: 1                  # 阶段编号
  phase_status: draft       # draft | active | completed（人类手动改 active）
  title: "..."
  owner: "@architects"
  depends_on_phase: 0       # 前置 Phase（0 = 无前置）
  design_docs:              # 本 Phase 参考的设计文档列表
    - docs/design-docs/xxx.md
  human_checkpoints:
    - after_slice: "1.5"    # 到达此 slice 后 executor 必须停下等待人工 review

slices:
  - id: "1.1"               # 格式：Phase.序号（如 1.1, 2.3b）
    title: "..."            # 简短动词+名词
    status: pending         # pending | done | skip | blocked
    design_ref: "docs/design-docs/xxx.md"  # executor 必读的设计文档
    backlog_ref: "BL-101"   # 可选：产品 backlog 引用
    impl_targets:           # 必须修改的文件列表（每个都要有实际代码变更）
      - path/to/file.rs     # 新建注明 # 必须新建，修改注明 # MODIFY: 原因

    # ── 防作弊三字段（以下三个字段缺一不可）──────────────────
    current_state: |
      # 描述当前文件状态，明确标注 ❌ MISSING
      # 让 executor 必须做 gap analysis 而不能直接跳过

    must_implement:
      - |
        # 来自 design_ref 的 Rust 类型签名（精确到方法级别）
        # executor 必须一字不差实现这些接口

    acceptance:
      - type: compile
        cmd: "cargo check -p if2ai-backend"
      - type: symbol             # ← 防跳过核心：grep 检查
        file: "src-tauri/src/modules/xxx.rs"
        grep: "pub struct FooBar"
        label: "FooBar — design_ref §章节"
      - type: test
        cmd: "cargo test -p if2ai-backend -- module::path"
        expect: "all tests pass"
      - type: harness            # 可选：Layer 3 行为测试
        suite: "harness/suites/xxx.yaml"
    # ──────────────────────────────────────────────────────────

    review_checklist:           # code-reviewer agent 用的逐条检查项
      - "..."

dashboard:
  last_updated: 'YYYY-MM-DD'
  completed_slices: ['1.1', '1.2']
  current_slice: "1.3"
  blocked: []                   # 遇到阻塞时在此记录
  notes: "..."
```

### check-slice 验证命令
```bash
python -m harness.runner check-slice --file docs/exec-plans/active/<file>.yaml
# ✅ N slices validated OK 表示格式合法
```

### 激活新 Phase 的步骤
1. 打开 `docs/exec-plans/index.md`，把 `[当前]` 指向新 Phase
2. 打开新 Phase YAML，将 `phase_status: draft` 改为 `active`
3. 启动 executor（或通知 executor 读取 index.md）

---

## 4. Harness 框架详解

### 文件结构
```
harness/
├── runner.py              # CLI 入口（6 个子命令）
├── gate.py                # 三层门控逻辑
├── __init__.py
├── evaluators/            # 评估器（behavior/correctness/performance）
├── fixtures/              # 测试夹具（LLM mock、tool mock）
├── runners/               # 运行器（local/distributed）
└── suites/                # 测试套件 YAML
    ├── tool_registry_basic.yaml
    ├── e2e_conversation.yaml
    ├── context_compression.yaml
    ├── phase1_integration.yaml    # Phase 1 集成
    ├── phase2_integration.yaml    # Phase 2 集成（骨架）
    ├── self_improvement_baseline.yaml  # Phase 3 自我进化
    └── phase3_integration.yaml    # Phase 3 集成（骨架）
```

### 6 个 CLI 子命令

| 命令 | 用途 | 输出 |
|------|------|------|
| `run --slice <id>` | 运行单个 slice 的全部 gates | JSON 报告 + PASS/FAIL |
| `check-slice --file <yaml>` | 验证 YAML 格式完整性 | ✅ N slices OK 或错误列表 |
| `review --slice <id>` | 静态代码 review（lint + 规范检查） | REVIEW_PASS / REVIEW_FAIL |
| `status` | 查看所有 slice 的完成状态 | 进度表 |
| `promote` | Phase 切换（draft → active） | 切换确认 |
| `diff-gate` | 验证确实有源代码变更（防空 commit） | DIFF_GATE PASS/FAIL |

### 三层门控（gate.py）

```
Layer 1  compile_gate    cargo check          < 10s   快速失败
Layer 1b symbol_gate     grep 检查关键符号    < 5s    防止接口未实现
Layer 2  test_gate       cargo test           < 60s   单元覆盖
Layer 3  behavior_gate   harness suite YAML   < 120s  行为验证（可选）
```

### symbol_gate 工作原理
从 exec-plan YAML 的 acceptance 字段中提取 `type: symbol` 条目，然后 `grep` 对应文件。
这是"防作弊"的核心机制——executor 必须写出具体类型才能通过此 gate。

### Harness Suite YAML 格式
```yaml
suite_id: "tool_registry_basic"
runner: "cargo_test"
target_pass_rate: 1.0
test_cases:
  - id: "tr-001"
    name: "..."
    type: "unit"
    test_fn: "module::tests::fn_name"  # 必须指向真实存在的 Rust 测试函数
    expect:
      - "..."
```

---

## 5. Executor 17 步自动循环

Executor（Claude Code）启动后按以下 17 步循环执行，直到所有 slice done 或遇到 `human_checkpoint`：

```
步骤 1  READ      → 读取 exec-plan，找第一个 status: pending 的 slice
步骤 2  READ      → 读取该 slice 的 design_ref 文档（必须，不能跳过）
步骤 3  READ      → 读取 impl_targets 中现有文件（理解当前状态）
步骤 4  GAP       → gap analysis：对照 current_state 和 must_implement，
                    列出 TO-DO LIST（此列表必须非空才继续）
步骤 5  IMPL      → 按 TO-DO LIST 实现代码，遵守 design_ref 接口定义
步骤 6  LINT      → cargo fmt + cargo clippy -D warnings + cargo test（全部通过）
步骤 7  GATE      → python -m harness.runner run --slice <id> --workspace .
步骤 8  FIX       → gate 失败则修复，回到步骤 5
步骤 9  REVIEW    → code-reviewer sub-agent 或 harness review 命令
步骤 10 FIX       → review FAIL 则修复，回到步骤 5
步骤 11 DIFF-GATE → python -m harness.runner diff-gate --workspace .
                    FAIL = 没有写代码 → 回到步骤 4
步骤 12 COMMIT    → git commit（格式：feat(slice-N.M): <描述>）
步骤 13 UPDATE    → 更新 exec-plan YAML 中该 slice 的 status 为 done
步骤 14 UPDATE    → 更新 dashboard 区域
步骤 15 REPORT    → 追加 docs/generated/QUALITY_SCORE.md 变更记录
步骤 16 STATUS    → python -m harness.runner status --workspace .
步骤 17 NEXT      → 立即开始下一个 pending slice（不等待用户确认）
```

**停止条件**：所有 slice done，或遇到 `human_checkpoint`。

### 自动继续原则
Executor 在步骤 17 后**立即自动开始下一个 slice**，不停下询问。
唯一允许停止的场合：
- 遇到 `meta.human_checkpoints.after_slice` 中指定的 slice
- gate 连续失败 3 次进入 blocked 状态

---

## 6. 防止 Executor 作弊的三重机制

> **背景**：早期版本中 executor 会仅更新 YAML/文档就标记 slice done，不写代码。以下三重机制正是为了杜绝这种行为。

### 机制 1：current_state + must_implement（GAP Analysis 强制）

每个 slice 包含：
- `current_state`：明确描述文件现状和 `❌ MISSING` 的接口
- `must_implement`：从 design_ref 粘贴的精确接口签名

executor 必须做 GAP analysis（步骤 4），`TO-DO LIST 必须非空`，否则说明 current_state 有误。

### 机制 2：symbol_gate（接口存在性检查）

acceptance 中的 `type: symbol` 条目通过 grep 检查具体 symbol 是否在文件中：
```yaml
- type: symbol
  file: "src-tauri/src/modules/runtime/error.rs"
  grep: "pub enum AgentError"
  label: "AgentError — error-handling.md"
```
executor 不写出 `pub enum AgentError`，gate 就不会通过。

### 机制 3：DIFF-GATE（代码变更验证）

步骤 11 强制运行：
```bash
python -m harness.runner diff-gate --workspace .
```
DIFF-GATE 检查 impl_targets 中是否有非 YAML/docs 文件的 git diff。
`只有文档变更 = DIFF_GATE FAIL = 不能 commit，必须回到步骤 4。`

### 阻塞处理
连续失败 3 次后，executor 在 `dashboard.blocked` 记录原因并停止。
人类通过读取该字段了解卡点，提供保守实现方案或修正 design_ref。

---

## 7. Agent 系统（sub-agents）

### code-reviewer（`.claude/agents/code-reviewer.md`）

**触发方式**：`Use the code-reviewer subagent to review slice <id>`  
**工具权限**：Read, Grep, Glob, Bash  
**工作流程**：
1. 运行 `git diff HEAD`
2. 读取 `docs/references/coding-style-and-lint-contract.md`
3. 从对应 slice 读取 `review_checklist`
4. 运行 cargo fmt check + clippy + 无 unwrap 检查
5. 输出 `REVIEW_PASS` 或 `REVIEW_FAIL`

### system-architect（`.claude/agents/system-architect.md`）

**触发方式**：`Use the system-architect subagent to generate Phase 4 exec-plan`  
**工具权限**：Read, Write, Edit, Grep, Glob, Bash, WebFetch  
**职责**：生成新 Phase YAML、audit 现有 exec-plans、监控 executor 进度、评估 Phase 切换时机

### 调用规则
- Executor 在步骤 9 调用 code-reviewer
- 人类（或 planner）通过 "Use the system-architect subagent to..." 触发规划
- Sub-agent 是无状态的，每次调用都重新读取文件

---

## 8. 代码规范合约

文件：`docs/references/coding-style-and-lint-contract.md`

### 核心规则（截要）

| 规则 | 来源 | 违反处理 |
|------|------|---------|
| 无 `unwrap()` / `expect()`（测试除外） | Code Shape | REVIEW_FAIL |
| 无硬编码 API key / secret | Security | REVIEW_FAIL |
| 无 `todo!()` / `unimplemented!()` | Completion | REVIEW_FAIL |
| 所有 `pub fn` 有 `///` doc 注释 | Doc | REVIEW_FAIL |
| 异步代码用 tokio（不用 std::thread） | Async | REVIEW_FAIL |
| 跨模块用 `crate::modules::*` | Boundary | compile 检查 |
| 默认 `pub(crate)`（只在边界用 `pub`） | Visibility | REVIEW_FAIL |
| 一文件一有界职责 | Modularization | REVIEW_FAIL |
| `Result<T, E>` 传播用 `?` | Error | REVIEW_FAIL |
| 新增 `allow(...)` 必须附理由 | Lint | REVIEW_FAIL |

### Git Commit 格式
```
<type>(slice-<id>): <简短描述>

Slice: <id>
Design-ref: <design_ref 文件名>
Gate: PASS (compile ✅ test ✅ behavior ✅/⏭️)
Review: PASS
```
`type`：`feat` / `fix` / `refactor` / `test` / `chore`

---

## 9. 当前项目状态

### Phase 1（已完成 ✅）
| Slice | 标题 | 状态 |
|-------|------|------|
| 1.1 | 建立 Rust 模块骨架 | ✅ done |
| 1.2 | 修复编译错误 | ✅ done |
| 1.3 | ConversationRuntime 核心 | ✅ done |
| 1.4 | ProviderManager | ✅ done |
| 1.5 | ToolRegistry + 基础工具 | ✅ done |
| 1.6 | SessionManager | ✅ done |
| 1.7 | Tauri Commands 网关 | ✅ done |
| 1.8 | 前端接线 React | ✅ done |
| 1.9 | Phase 1 集成测试 | ✅ done |

### Phase 2（draft，等待激活）
7 个 slice：2.1(PromptBuilder) → 2.2(Compression) → 2.3(ErrorHandling) → 2.4(Memory) → 2.5(Telegram) → 2.5b(Discord) → 2.6(Integration)

### Phase 3（draft，等待 Phase 2 完成）
4 个 slice：3.1(HTTP API Server) → 3.2(LSP Bridge) → 3.3(Self-Improvement) → 3.4(Integration)

### 已知的关键 Bug（exec-plans 中已修正）
1. **compression.rs 路径**：harness suite `context_compression.yaml` 引用 `runtime::compression::tests::*`，但现有实现在 `compact.rs`（`runtime::compact`）→ slice 2.2 要求新建 `compression.rs`
2. **axum 缺失**：`src-tauri/Cargo.toml` 没有 axum → slice 3.1 有 symbol check 强制先添加
3. **lsp.rs 是客户端**：`lsp.rs` 是 LSP 客户端（读诊断信息），slice 3.2 需要新建 `lsp_bridge.rs`（stdio JSON-RPC 服务端）

### 测试覆盖
- Phase 1：124 个测试全部通过
- Phase 2/3：测试代码尚未写（等待 executor 执行对应 slice）

---

## 10. 常用命令速查

### 开发环境
```bash
# Python venv（harness 依赖 pyyaml）
source .venv/bin/activate
# 初次安装：python3 -m venv .venv && .venv/bin/pip install pyyaml

# Rust 开发
cargo check -p if2ai-backend       # 快速编译检查
cargo test -p if2ai-backend        # 运行测试
cargo clippy --workspace -D warnings  # lint
cargo fmt --all                    # 格式化
```

### Harness 操作
```bash
# 运行单个 slice 的全部 gates
python -m harness.runner run --slice <id> --workspace .

# 带 harness suite
python -m harness.runner run --slice 1.9 --workspace . --suite harness/suites/phase1_integration.yaml

# 验证 exec-plan YAML 格式
python -m harness.runner check-slice --file docs/exec-plans/active/<phase>.yaml

# 静态代码 review
python -m harness.runner review --slice <id> --workspace .

# 查看进度
python -m harness.runner status --workspace .

# 预览 Phase 切换
python -m harness.runner promote --workspace . --dry-run

# 执行 Phase 切换
python -m harness.runner promote --workspace .

# 验证有真实代码变更
python -m harness.runner diff-gate --workspace .
```

### Git 工作流
```bash
# 查看 executor 的历史工作
git log --oneline -20

# 查看当前变更
git diff --staged

# executor 的 commit 格式
git commit -m "feat(slice-1.3): <描述>

Slice: 1.3
Design-ref: agent-loop.md
Gate: PASS (compile ✅ test ✅ behavior ✅)
Review: PASS"
```

### 诊断命令
```bash
# 查看所有 pending slices
grep -n "status: pending" docs/exec-plans/active/*.yaml

# 查看所有 blocked
grep -n "blocked" docs/exec-plans/active/*.yaml

# 检查文件是否存在（audit 用）
find src-tauri/src -name "*.rs" | sort

# 检查 struct/trait 是否实现
grep -rn "pub struct\|pub trait\|pub enum" src-tauri/src/modules/<module>/

# 检查 Cargo.toml 依赖
grep -A30 "^\[dependencies\]" src-tauri/Cargo.toml | head -35
```
