# 一、整体架构对比

## 1.1 应用形态对比

| 维度 | CLAW-CLI (Baseline) | If2Ai 桌面端 (现状) |
|------|---------------------|-------------------|
| 入口形态 | 终端 REPL（`claw` 二进制） | Tauri 桌面应用 |
| 前端 | 无（纯 CLI 输出） | React + shadcn/ui |
| 状态管理 | Session 文件（`~/.claw/sessions/`） | Session 文件（`~/.if2ai/`） |
| LLM 调用 | `DefaultRuntimeClient → ClawApiClient` | `RealApiClient → ClawApiClient`（真实） |
| 工具执行 | `CliToolExecutor → GlobalToolRegistry` | `ToolRegistryExecutor → ToolRegistry` |
| 权限系统 | `PermissionPolicy.authorize()`（真实生效） | `PermissionPolicy`（硬编码 DangerFullAccess） |
| Agent Loop | `ConversationRuntime::run_turn()`（唯一路径） | 两条路径（见 2.1） |
| SlashCommand | 28 个命令 + parse/dispatch/suggest | ⚠️ 代码副本存在但未暴露 |
| Skill 系统 | Skill 工具 + discover_skill_roots() | ❌ 完全缺失 |
| 配置管理 | `~/.claw/settings.json` + `.claw/` | `~/.claude/settings.json` |

## 1.2 代码模块对应关系

```
CLAW-CLI (rust/crates/)          If2Ai 桌面端 (src-tauri/src/)
─────────────────────────────────────────────────────────────────────
claw-cli/src/main.rs              commands/agent.rs
  ├─ LiveCli                     ├─ run_agent_turn()
  │   ├─ ConversationRuntime     │   └─ ConversationRuntime::run_turn()
  │   ├─ DefaultRuntimeClient    │       ├─ RealApiClient
  │   └─ CliToolExecutor         │       └─ ToolRegistryExecutor
  │                                ├─ start_agent_stream()
  │                                │   └─ ⚠️ 独立实现，不走 ConversationRuntime
  ├─ render.rs                   └─ (无对等物，前端渲染)
  ├─ input.rs                    └─ (前端 ChatUI)
  └─ (slash command 处理)          └─ ⚠️ modules/commands/lib.rs 有副本但未暴露
runtime/src/conversation.rs       modules/runtime/conversation.rs ✅ 复用
runtime/src/session.rs            modules/runtime/session.rs ✅ 复用
runtime/src/permissions.rs        modules/runtime/permissions.rs ✅ 复用
runtime/src/bash.rs              modules/tools/builtin/bash.rs ⚠️ 重写
runtime/src/file_ops.rs          modules/tools/builtin/*.rs ⚠️ 重写
runtime/src/permissions.rs       modules/tools/context.rs ⚠️ 重写
tools/src/lib.rs                 modules/tools/mod.rs ⚠️ 重写
  └─ Skill 工具                   └─ ❌ 未实现
api/src/providers/claw_provider  modules/api/providers/ ✅ 复用
runtime/src/compact.rs           modules/runtime/compact.rs ✅ 复用
runtime/src/prompt.rs            modules/runtime/prompt.rs ✅ 复用
runtime/src/hooks.rs             modules/runtime/hooks.rs ✅ 复用
rust/crates/commands/src/lib.rs  modules/commands/lib.rs ✅ 副本存在（未暴露）
runtime/src/sandbox.rs           (缺失)
runtime/src/mcp_stdio.rs         (缺失)
plugins/src/lib.rs               (缺失)
```

## 1.3 关键架构差异

### 两条 Agent 路径问题（最严重）

```
CLAW-CLI:
  run_repl()
    └─ LiveCli::run_turn()
          └─ ConversationRuntime::run_turn()  ← 唯一路径，工具循环完整

If2Ai 桌面端:
  ├─ run_agent_turn()  ──────────────────┐
  │     └─ ConversationRuntime::run_turn()   ← 路径 A（工具有完整循环）
  │           tools: None  ← 🔴 致命缺陷
  │
  └─ start_agent_stream() ───────────────┐
        └─ 独立流式处理                    ← 路径 B（工具定义发给 LLM）
              ├─ tools: Some(...)  ← ✅ 正确
              └─ InputJsonDelta 事件被忽略  ← 🔴 致命缺陷
```

### Tauri Commands vs CLI

CLAW-CLI 是一个单进程 CLI，通过 `ConversationRuntime` 直接驱动 Agent Loop。

If2Ai 桌面端将同样逻辑拆成了两个 Tauri commands：
- `run_agent_turn` — 同步阻塞，返回完整结果（走 ConversationRuntime 但 tools=None）
- `start_agent_stream` — 异步流式，实时推送 SSE 事件（绕过 ConversationRuntime）

前端通过 `listenToStream` 监听 SSE 事件，在前端渲染流式文本。

## 1.4 工具注册架构对比

```
CLAW-CLI:
  tools/src/lib.rs
    └─ GlobalToolRegistry
          ├─ mvp_tool_specs()     — 17 个内置工具规格
          └─ with_plugin_tools()  — 插件工具聚合
  runtime/src/
    └─ file_ops.rs, bash.rs      — 工具实际执行逻辑

If2Ai 桌面端:
  modules/tools/
    ├─ registry.rs               — ToolRegistry（DashMap）
    ├─ context.rs                — ToolContext（workdir + permission_mode）
    ├─ toolset.rs               — ToolSet 分类（额外新增）
    └─ builtin/*.rs             — 每个工具独立文件
  commands/tools.rs              — execute_tool Tauri command
```

**评价**：Phase 4 实现了更多工具（19 个），并新增了 ToolSet 分类，但权限系统未真实生效。

**新增发现**：SlashCommand 系统（28 命令）在 `modules/commands/lib.rs` 中有代码级副本，但从未被 Tauri command 暴露。Skill 系统完全缺失（无 Skill 工具注册、无 discover_skill_roots() 实现、无 SKILL.md 格式）。

## 1.5 SlashCommand / Skills 系统架构对比

### 1.5.1 SlashCommand 系统

```
CLAW-CLI:
  claw-cli/src/main.rs
    └─ SlashCommand::parse(input)  ──→ 28 个命令路由
          ├─ /help, /status, /compact
          ├─ /model, /permissions
          ├─ /branch, /commit, /diff, /pr, /issue
          ├─ /skills, /agents
          └─ ... (共 28 个命令)
    └─ handle_slash_command()  ──→ 命令执行

  rust/crates/commands/src/lib.rs
    ├─ SlashCommand enum + SlashCommandSpec
    ├─ SlashCommand::parse()
    ├─ suggest_slash_commands()  ── Levenshtein 模糊匹配
    ├─ render_slash_command_help()
    └─ discover_skill_roots() / load_skills_from_roots()

If2Ai 桌面端:
  src-tauri/src/modules/commands/lib.rs
    └─ ⚠️ 完全相同的代码副本（28 命令）
          ├─ SlashCommand enum ✅
          ├─ SlashCommandSpec ✅
          ├─ SlashCommand::parse() ✅
          ├─ suggest_slash_commands() ✅
          └─ render_slash_command_help() ✅
          └─ ❌ 但从未通过 Tauri command 暴露给前端
          └─ ❌ 前端 ChatUI 完全绕过此系统

  src/App.tsx + ChatUI.tsx
    └─ ❌ 不检查 `/` 前缀，所有输入直接发给 LLM
```

### 1.5.2 Skill 系统

```
CLAW-CLI:
  rust/crates/tools/src/lib.rs
    └─ Skill 工具定义 + execute_skill()
          ├─ Skill 工具：加载本地 SKILL.md
          ├─ Skill 发现路径：$CODEX_HOME/skills, ~/.claw/skills, ~/.codex/skills
          └─ SKILL.md 格式：frontmatter (name/description) + 指令内容

  rust/crates/commands/src/lib.rs
    └─ discover_skill_roots() ── 多路径优先级搜索

If2Ai 桌面端:
  src/modules/settings/pages/SkillsSettingsPage.tsx
    └─ ⚠️ 硬编码占位符 UI（SKILL_ITEMS 数组，onCheckedChange 空实现）

  modules/tools/mod.rs
    └─ ❌ Skill 工具未注册
    └─ ❌ discover_skill_roots() 未实现
    └─ ❌ SKILL.md 格式未定义
```

## 1.6 核心问题总览

| # | 问题 | 严重度 | 影响范围 |
|---|------|--------|---------|
| A1 | `run_agent_turn` 的 `tools: None` | 🔴 阻塞 | 非流式 Agent 路径 |
| A2 | `start_agent_stream` 忽略 tool_use 事件 | 🔴 阻塞 | 流式 Agent 路径 |
| A3 | 两条路径能力不一致（工具循环仅路径 A 有） | 🟠 严重 | 整体 Agent 功能 |
| P1 | PermissionPolicy 硬编码 DangerFullAccess | 🟠 严重 | 所有工具调用 |
| P2 | PermissionPrompter 从未调用 | 🟠 严重 | 交互式权限确认 |
| C1 | SlashCommand 代码已复制但未暴露 | 🟠 严重 | 28 个 CLI 命令无法使用 |
| C2 | 前端无 Slash 命令解析/分发 | 🟡 中 | `/` 输入直接发给 LLM |
| C3 | 前端无 Tab 命令补全 | 🟡 中 | CLI 体验未还原 |
| C4 | `Skill` 工具完全缺失 | 🟠 严重 | 无法使用自定义技能 |
| C5 | Skill 发现路径未实现 | 🟠 严重 | 无 Skill 加载能力 |
| C6 | SkillsSettingsPage 是占位符 | 🟡 中 | 无真实后端连接 |
| F1 | 前端无工具调用结果展示面板 | 🟡 中 | 工具可见性 |
| F2 | 流式场景下工具结果无法回传 LLM | 🟡 中 | 流式工具循环 |
| F3 | 模型选择 UI 无效 | 🟡 中 | 用户体验 |
| S1 | Context Compaction 未接入 | 🟡 中 | 长对话 token 溢出 |
| M1 | MCP 协议完全缺失 | 🟢 低 | 外部工具集成 |
| M2 | Plugin 系统完全缺失 | 🟢 低 | 第三方扩展 |
| M3 | Sandbox 完全缺失 | 🟢 低 | 高级安全隔离 |
