# 九、Skills 与 Commands 系统差距审计

> 本章节深度对比 CLAW-CLI 的 SlashCommand 系统、Skill 加载机制与 If2Ai 桌面端的实现差距。

---

## 目录

| 节 | 内容 |
|----|------|
| [9.1](#91-架构总览) | CLAW-CLI vs If2Ai 整体架构流程图 |
| [9.2](#92-slashcommand-系统详细对比) | SlashCommand 枚举、parse、suggest、help 等完整对比 |
| [9.3](#93-skill-系统详细对比) | Skill 工具、发现路径、SKILL.md 格式对比 |
| [9.4](#94-commandsskills-与其他系统的关联) | 与 Agent Loop、Session、Permission 的关联 |
| [9.5](#95-skillscommands-gap-总结) | 4 项关键发现总结 |
| [9.6](#96-详细-gap-清单) | 19 个 Gap 清单表 |
| [9.7](#97-修复方案) | 修复方案（SlashCommand 暴露、Skill 工具注册） |

---

## 9.1 架构总览

### CLAW-CLI Baseline

```
用户输入（REPL）
    │
    ├─  SlashCommand::parse(input)  ──→ /command 路由
    │      │
    │      ├─ /help      → render_slash_command_help()
    │      ├─ /status    → Session 状态报告
    │      ├─ /compact   → compact_session()
    │      ├─ /model     → 切换模型
    │      ├─ /branch    → Git branch 操作
    │      ├─ /commit    → Git commit
    │      ├─ /diff      → Git diff
    │      ├─ /skills    → handle_skills_slash_command()
    │      ├─ /agents    → handle_agents_slash_command()
    │      └─ ... (28+ 命令)
    │
    └─  普通消息 → ConversationRuntime::run_turn()
                   └─ LLM 处理
```

### If2Ai 桌面端现状

```
用户输入（ChatUI textarea）
    │
    └─  startAgentStream() → LLM
           └─ 完全绕过 SlashCommand 层！
           └─ SlashCommand 系统存在于 src-tauri/src/modules/commands/lib.rs
              但从未被 Tauri command 暴露
              前端从未调用
```

---

## 9.2 SlashCommand 系统详细对比

### 9.2.1 CLAW-CLI 的 SlashCommand 定义

**位置**：`rust/crates/commands/src/lib.rs`

```rust
// 28 个命令，5 大类别
enum SlashCommand {
    Help,
    Status,
    Compact,
    Branch { action, target },
    Bughunter { scope },
    Worktree { action, path, branch },
    Commit,
    CommitPushPr { context },
    Pr { context },
    Issue { context },
    Ultraplan { task },
    Teleport { target },
    DebugToolCall,
    Model { model },
    Permissions { mode },
    Clear { confirm },
    Cost,
    Resume { session_path },
    Config { section },
    Memory,
    Init,
    Diff,
    Version,
    Export { path },
    Session { action, target },
    Plugins { action, target },
    Agents { args },
    Skills { args },
    Unknown(String),
}
```

**关键特性**：
- `SlashCommand::parse(input)` — 将 `/command args` 字符串解析为枚举
- `SlashCommandSpec` — 每个命令的元数据（名称、别名、摘要、参数提示、resume 支持度、类别）
- `suggest_slash_commands()` — Levenshtein 距离模糊匹配建议
- `render_slash_command_help()` — 按类别格式化帮助文本
- Tab 自动补全（`input.rs complete_slash_command()`）

### 9.2.2 If2Ai 桌面端的 SlashCommand

**位置**：`src-tauri/src/modules/commands/lib.rs`（完全相同的副本）

```rust
// ⚠️ 代码存在，但从未被使用！
enum SlashCommand { Help, Status, Compact, ... }  // 28 个变体
const SLASH_COMMAND_SPECS: &[SlashCommandSpec] = &[
    // 完整的命令规格表
];
```

**问题**：这套代码存在于 if2ai 后端，但：

| 缺失环节 | 说明 |
|---------|------|
| 无 Tauri command 暴露 | 前端无法调用 slash command 解析 |
| 前端无解析逻辑 | `ChatUI` 的 `handleKeyDown` 不检查 `/` 前缀 |
| 无命令分发 | `run_agent_turn` / `start_agent_stream` 直接发给 LLM |
| 无 Tab 补全 | 前端 textarea 无命令补全功能 |

### 9.2.3 对比矩阵

| 维度 | CLAW-CLI Baseline | If2Ai 桌面端 |
|------|-------------------|--------------|
| SlashCommand 枚举定义 | ✅ `lib.rs` | ✅ 副本存在 |
| SlashCommandSpec 表 | ✅ 28 个命令 | ✅ 副本存在 |
| 命令解析 `parse()` | ✅ 完整实现 | ✅ 副本存在 |
| 命令分发 `dispatch()` | ✅ 完整实现 | ❌ 未实现 |
| 帮助渲染 | ✅ `render_slash_command_help()` | ❌ 未实现 |
| 模糊建议 `suggest()` | ✅ Levenshtein 距离 | ❌ 未实现 |
| Tab 自动补全 | ✅ `complete_slash_command()` | ❌ 未实现 |
| 前端命令输入解析 | N/A | ❌ 未实现 |
| Tauri command 暴露 | N/A | ❌ 未实现 |

---

## 9.3 Skill 系统详细对比

### 9.3.1 CLAW-CLI 的 Skill 定义

**位置**：`rust/crates/tools/src/lib.rs` — `Skill` 工具

```rust
// Skill 作为一个 LLM 可调用的工具
ToolSpec {
    name: "Skill",
    description: "Load a local skill definition and its instructions.",
    input_schema: {
        "properties": {
            "skill": { "type": "string" },
            "args": { "type": "string" }
        },
        "required": ["skill"],
    },
    required_permission: ReadOnly,
}
```

**执行逻辑**：
```rust
fn execute_skill(input: SkillInput) -> Result<SkillOutput, String> {
    let skill_path = resolve_skill_path(&input.skill)?;
    let prompt = fs::read_to_string(&skill_path)?;  // 读取 SKILL.md
    let description = parse_skill_description(&prompt);
    Ok(SkillOutput { skill, path, args, description, prompt })
}

fn resolve_skill_path(skill: &str) -> Result<PathBuf, String> {
    // 搜索路径优先级:
    // 1. $CODEX_HOME/skills/{skill}/SKILL.md
    // 2. ~/.agents/skills/{skill}/SKILL.md
    // 3. ~/.config/opencode/skills/{skill}/SKILL.md
    // 4. ~/.codex/skills/{skill}/SKILL.md
}
```

### 9.3.2 CLAW-CLI 的 Skill 发现机制

**位置**：`rust/crates/commands/src/lib.rs` — `discover_skill_roots()`

```rust
// 技能文件发现路径
SkillRoot 发现优先级:
  项目级 (cwd 祖先回溯):
    .codex/skills/          → SkillOrigin::SkillsDir
    .claw/skills/           → SkillOrigin::SkillsDir
    .codex/commands/        → SkillOrigin::LegacyCommandsDir（兼容）
    .claw/commands/         → SkillOrigin::LegacyCommandsDir（兼容）
  用户级:
    $CODEX_HOME/skills/
    ~/.codex/skills/
    ~/.claw/skills/
```

**Skill 文件格式**（SKILL.md）：
```markdown
---
name: my-skill
description: 这是一个自定义技能
---

# 技能指令

这里可以包含详细的技能描述和执行指令...
```

### 9.3.3 If2Ai 桌面端的 Skill 系统

**位置**：`src/modules/settings/pages/SkillsSettingsPage.tsx`

```typescript
// ⚠️ 硬编码 UI，完全没有后端集成
const SKILL_ITEMS = [
  { title: '代码助手', description: '生成、解释和优化代码片段。', enabled: true },
  { title: '文件管理', description: '读取、写入和整理本地工作区文件。', enabled: true },
  { title: '浏览器任务', description: '进行网页访问、抓取和验证。', enabled: false },
  { title: '终端执行', description: '通过命令行完成受控自动化任务。', enabled: true },
]

// ⚠️ SkillsSettingsPage 只是一个设置页面
// 点击"配置指南"按钮没有任何实际功能
// 开关切换也没有任何效果
```

**问题**：

| 问题 | 说明 |
|------|------|
| Skill 工具未注册 | if2ai 的 `mvp_tool_specs()` 中没有 `Skill` 工具 |
| Skill 发现系统缺失 | 无 `discover_skill_roots()` / `load_skills_from_roots()` |
| Skill 文件格式 | 未定义 SKILL.md 格式规范 |
| UI 是占位符 | `SkillsSettingsPage` 的开关切换不触发任何后端调用 |

### 9.3.4 Skill 工具缺失对比

| 工具 | CLAW-CLI Baseline | If2Ai 桌面端 |
|------|------------------|--------------|
| `Skill` 工具定义 | ✅ | ❌ 缺失 |
| Skill 工具执行 | ✅ `execute_skill()` | ❌ 缺失 |
| Skill 发现路径 | ✅ 6 个路径 | ❌ 缺失 |
| Skill 加载 | ✅ 解析 SKILL.md | ❌ 缺失 |
| `SkillSearch` 工具 | ✅ | ❌ 缺失 |

---

## 9.4 Commands/Skills 与其他系统的关联

### 9.4.1 与 Agent Loop 的关系

**CLAW-CLI**：
```
用户输入
    │
    ├─ /slash command → handle_slash_command() → 直接返回结果（不走 LLM）
    └─ 普通消息 → run_turn() → LLM
                    ├─ LLM 可调用 Skill 工具
                    └─ Skill 执行后 → 返回 prompt 给 LLM
```

**If2Ai 桌面端**：
```
用户输入 → startAgentStream() → LLM
              │
              └─ ⚠️ 完全绕过 SlashCommand
              └─ ⚠️ Skill 工具未注册，LLM 无法调用
```

### 9.4.2 与 Session 的关系

| 命令 | 对 Session 的影响 | if2ai 状态 |
|------|-----------------|-----------|
| `/compact` | 调用 `compact_session()` | ❌ 未实现 |
| `/clear` | 清空 `session.messages` | ❌ 未实现 |
| `/session` | 列出/切换会话 | ⚠️ 部分实现（通过 ProjectRail） |
| `/resume` | 加载会话文件 | ❌ 未实现 |

### 9.4.3 与 Permission System 的关系

| 命令 | 权限需求 | if2ai 状态 |
|------|---------|-----------|
| `/permissions` | 查看/切换权限模式 | ❌ 未实现 |
| `/init` | 写文件（创建 CLAW.md） | ❌ 未实现 |
| `/commit` | Git write 操作 | ❌ 未实现 |

---

## 9.5 Skills/Commands Gap 总结

### 9.5.1 关键发现

**发现 1：SlashCommand 代码已复制但未连接**

if2ai 桌面端的 `src-tauri/src/modules/commands/lib.rs` 包含与 CLAW-CLI `rust/crates/commands/src/lib.rs` **完全相同的 SlashCommand 实现**（28 个命令、SlashCommandSpec、parse、suggest 等），但：

1. 没有 Tauri command 暴露这些功能
2. 前端完全不知道这套系统的存在
3. 所有的 `/` 输入都被当作普通消息发给 LLM

**发现 2：SkillsSettingsPage 是占位符 UI**

```typescript
// SkillsSettingsPage.tsx — 硬编码数据，无后端调用
const SKILL_ITEMS = [
  { title: '代码助手', enabled: true, ... },
  { title: '文件管理', enabled: true, ... },
  ...
]
// onCheckedChange={() => {}}  ← 空回调！
```

**发现 3：Skill 工具本身未注册**

Phase 4 的 `mvp_tool_specs()` 中没有 `Skill` 工具（CLAW-CLI baseline 有）。

**发现 4：缺少 3 个关键 LLM 工具**

| 缺失工具 | Baseline 功能 | 影响 |
|---------|-------------|------|
| `Skill` | 加载本地 SKILL.md 作为 LLM 上下文 | Agent 无法使用自定义技能 |
| `ToolSearch` | 搜索可用工具 | Agent 无法做工具选择 |
| `Agent` | 嵌套 Agent 调用 | 无法实现多 Agent 协作 |

---

## 9.6 详细 Gap 清单

| # | Gap | 严重度 | 涉及文件 |
|---|-----|--------|---------|
| C1 | SlashCommand 系统已复制但未暴露 | 🟡 中 | `modules/commands/lib.rs`（未暴露） |
| C2 | 前端无 SlashCommand 解析 | 🟡 中 | `ChatUI.tsx` |
| C3 | 前端无 Tab 命令补全 | 🟡 中 | `ChatUI.tsx` |
| C4 | `/help` 命令未实现 | 🟡 中 | 无 |
| C5 | `/status` 命令未实现 | 🟡 中 | 无 |
| C6 | `/compact` 命令未实现 | 🟡 中 | 无 |
| C7 | `/model` 命令未实现 | 🟡 中 | 无 |
| C8 | `/permissions` 命令未实现 | 🟡 中 | 无 |
| C9 | `/branch` 命令未实现 | 🟡 中 | 无 |
| C10 | `/commit` 命令未实现 | 🟡 中 | 无 |
| C11 | `/diff` 命令未实现 | 🟡 中 | 无 |
| C12 | `/session` 命令部分实现 | 🟡 中 | ProjectRail 替代 |
| C13 | `Skill` 工具未注册 | 🟠 严重 | `modules/tools/mod.rs` |
| C14 | `Skill` 执行逻辑未实现 | 🟠 严重 | 无 |
| C15 | Skill 发现路径未实现 | 🟠 严重 | 无 |
| C16 | SKILL.md 格式未定义 | 🟡 中 | 无 |
| C17 | `ToolSearch` 工具未注册 | 🟡 中 | 无 |
| C18 | `Agent` 工具未注册 | 🟡 中 | 无 |
| C19 | SkillsSettingsPage 是占位符 | 🟡 中 | `SkillsSettingsPage.tsx` |

---

## 9.7 修复方案

### 方案 A：前端实现 SlashCommand 层（推荐）

```typescript
// App.tsx 或 ChatUI.tsx
const handleInput = (input: string) => {
  if (input.startsWith('/')) {
    // 调用 Tauri command 解析 slash command
    invoke('parse_slash_command', { input })
      .then((result) => {
        if (result.is_slash_command) {
          // 调用对应的 Tauri command
          invoke('dispatch_slash_command', { command: result.command, args: result.args })
          return
        }
      })
  }
  // 普通消息 → startAgentStream()
}
```

新增 Tauri commands：
- `parse_slash_command(input: string) → SlashCommandResult`
- `dispatch_slash_command(command, args) → String`
- `list_slash_commands() → Vec<SlashCommandSpec>`
- `suggest_slash_commands(input: string) → Vec<String>`

### 方案 B：Skill 工具注册

```rust
// modules/tools/mod.rs
if let Err(e) = registry.register(skill_tool_entry()) {
    eprintln!("Failed to register skill tool: {}", e);
}
```

### 方案 C：Skill 发现后端

```rust
// 新增 Tauri commands
#[tauri::command]
pub fn list_skills(cwd: String) -> Vec<SkillSummary>

#[tauri::command]
pub fn load_skill(skill_name: String, cwd: String) -> Result<SkillOutput, String>
```

---

## 9.8 小结

### 核心差距

| 维度 | CLAW-CLI Baseline | If2Ai 桌面端 |
|------|-------------------|--------------|
| SlashCommand 代码 | 28 个命令完整实现 | ✅ 代码副本存在于 `modules/commands/lib.rs` |
| SlashCommand 暴露 | parse/dispatch/suggest 均可调用 | ❌ 无 Tauri command 暴露 |
| 前端命令解析 | Tab 补全 + `/` 前缀解析 | ❌ 直接发给 LLM |
| Skill 工具 | ✅ `execute_skill()` + 6 路径发现 | ❌ 未注册、未实现 |
| SKILL.md 格式 | 定义清晰，支持 frontmatter | ❌ 未定义 |
| SkillsSettingsPage | 有配置界面 | ⚠️ 占位符 UI，开关无效 |

### 关键数据

| 数据 | 值 |
|------|-----|
| SlashCommand 总命令数 | **28** |
| Gap 清单条目数 | **19**（C1-C19） |
| Skill 发现路径优先级 | **6**（项目 .codex/.claw → 用户 ~/.codex/.claw） |
| 关键发现数 | **4** |
| 缺失的关键 LLM 工具 | **3**（Skill、ToolSearch、Agent） |

### 修复优先级

| 优先级 | 行动 | 涉及文件 |
|--------|------|---------|
| 🟠 高 | 新建 `commands/slash.rs` 暴露 Tauri commands | `commands/slash.rs`（新建） |
| 🟠 高 | 注册 `Skill` 工具 + 实现 `discover_skill_roots()` | `modules/tools/mod.rs` |
| 🟡 中 | 前端 ChatUI 接入 slash 解析 | `ChatUI.tsx` |
| 🟡 中 | SkillsSettingsPage 接入真实数据 | `SkillsSettingsPage.tsx` |
| 🟡 中 | 实现 `/help`, `/status`, `/compact` 等命令 | `commands/slash.rs` |
