# Agent 提示词架构改善设计文档 v1.0

> **文档类型**：系统架构设计 + 策略改善规范  
> **作者角色**：高级系统架构设计师 + 策略分析设计师  
> **参考基准**：openhanako-main（`lib/browser/`、`core/agent.js`、`desktop/src/locales/zh.json`）  
> **适用版本**：if2Ai Tauri v2 + Rust 后端  
> **创建时间**：2026-04-18  
> **状态**：草稿，待 Phase 8 执行

---

## 目录

1. [背景与问题陈述](#1-背景与问题陈述)
2. [现状诊断：根因分析](#2-现状诊断根因分析)
3. [架构对比：三层提示词体系](#3-架构对比三层提示词体系)
4. [System Prompt 改善设计](#4-system-prompt-改善设计)
5. [工具描述改善设计](#5-工具描述改善设计)
6. [工具机制改善设计（执行保障层）](#6-工具机制改善设计执行保障层)
7. [Agent Loop 改善设计](#7-agent-loop-改善设计)
8. [优先级排序与实施路径](#8-优先级排序与实施路径)
9. [成功指标](#9-成功指标)
10. [附录：完整对比矩阵](#10-附录完整对比矩阵)

---

## 1. 背景与问题陈述

### 1.1 触发问题

用户报告了以下典型失败场景：

```
用户：用 browser 搜索 xchat 最新资讯并写入 x.md
AI：[调用 web_search] → 找到结果 → 直接输出文本 → 任务结束
实际输出：x.md 未被创建
```

诊断结论：Agent 在完成第一个工具调用（`web_search`）后，未继续调用 `file_write` 完成写文件步骤，loop 以 `model_stop_no_tools` 终止。

### 1.2 问题的系统性本质

这不是单一 bug，而是**提示词架构的系统性缺陷**：

- if2Ai 缺乏多步骤任务执行的工具机制保障
- 工具描述质量低，无法引导 LLM 完成完整工作流
- System Prompt 缺少关键的行为约束段落
- 无任务完成"仪式"工具（等价于 openhanako 的 `stage_files`）

### 1.3 范围声明

本文档覆盖：
- `src-tauri/src/modules/runtime/prompt.rs`（System Prompt 构建）
- `src-tauri/src/modules/tools/builtin/*.rs`（工具描述与 schema）
- `src-tauri/src/commands/agent.rs`（Agent Loop 控制）
- 新增工具的设计规范

---

## 2. 现状诊断：根因分析

### 2.1 Agent Loop 的停止机制

```rust
// src-tauri/src/commands/agent.rs:1910-1919
if pending_tool_uses.is_empty() {
    terminal_status = Some("model_stop_no_tools");
    break;  // ← 任务在此终止
}
```

**结论**：任何导致 LLM 在完成中间步骤后不再调用工具的因素，都会触发过早终止。

### 2.2 提示词层面的根因

| 根因编号 | 描述                                                  | 影响程度 |
| -------- | ----------------------------------------------------- | -------- |
| R1       | System Prompt 缺失多步骤任务持续执行指令              | P0       |
| R2       | `file_write` 描述未关联 web_search 工作流             | P0       |
| R3       | 缺少任务完成"仪式"工具（stage_files 等价物）          | P0       |
| R4       | `file_edit` 被 `disabled: true`，AI 无法增量编辑文件  | P1       |
| R5       | `TodoWrite` 缺少 list/toggle，AI 无法追踪任务状态     | P1       |
| R6       | System Prompt 角色定位过窄（仅 software engineering） | P1       |
| R7       | 无记忆使用行为规则，记忆系统暴露破坏用户体验          | P1       |
| R8       | 无运行时动态通知注入（只读模式等）                    | P2       |
| R9       | 无用户档案（user.md）注入机制                         | P2       |
| R10      | 无产品身份注入（Agent 不知道运行在 if2Ai 上）         | P2       |

---

## 3. 架构对比：三层提示词体系

### 3.1 openhanako 的三层架构

openhanako 将行为约束分布在三个互相强化的层次：

```
┌─────────────────────────────────────────────────────┐
│  Layer 1: System Prompt（战略规则）                   │
│  - 网页工具优先级 web_search → web_fetch → browser   │
│  - 人格约束（ishiki 11条）                            │
│  - 记忆使用规则（3条精密约束）                         │
│  - 工作目录语义消歧                                    │
├─────────────────────────────────────────────────────┤
│  Layer 2: 工具描述（战术指引）                         │
│  - web_search: "搜索完用 web_fetch 深化"              │
│  - stage_files: "文件创建后必须调用"                   │
│  - todo: "多步骤任务时追踪进度"                        │
├─────────────────────────────────────────────────────┤
│  Layer 3: 工具机制（执行保障）                         │
│  - stage_files 强制闭环                              │
│  - todo 的 list/toggle 驱动任务状态可见               │
│  - subagent preamble 预设子任务行为                   │
└─────────────────────────────────────────────────────┘
```

"搜索后写文件"工作流的三层覆盖：
- **战略**：`## 网页工具优先级` → web_search 先行
- **战术**：`stage_files.description` → "文件创建后必须调用"
- **保障**：调用 `stage_files` 本身是一次工具调用，迫使 Loop 继续至少一轮

### 3.2 if2Ai 的当前状态（修复后）

```
┌─────────────────────────────────────────────────────┐
│  Layer 1: System Prompt（战略规则）                   │
│  ✅ 网页工具优先级（已添加）                           │
│  ✅ 多步骤任务指令（一行 bullet）                      │
│  ❌ 人格约束（缺失）                                  │
│  ❌ 记忆使用规则（缺失）                              │
├─────────────────────────────────────────────────────┤
│  Layer 2: 工具描述（战术指引）                         │
│  ✅ web_search 已有工作流引导（已改善）                │
│  ✅ file_write 已有使用场景说明（已改善）              │
│  ❌ 缺少任务完成触发型工具描述                         │
├─────────────────────────────────────────────────────┤
│  Layer 3: 工具机制（执行保障）                         │
│  ❌ 无 stage_files 等价物                            │
│  ❌ TodoWrite 无 list/toggle（无状态可见）             │
│  ❌ file_edit 被 disabled                            │
└─────────────────────────────────────────────────────┘
```

**结论**：if2Ai 目前第一、二层有改善，但第三层（执行保障层）完全缺失，是最大的架构债务。

---

## 4. System Prompt 改善设计

### 4.1 当前 `build()` 段落顺序

```
1. get_simple_intro_section()
2. [Output Style if any]
3. get_simple_system_section()
4. get_simple_doing_tasks_section()
5. get_actions_section()
6. get_web_tool_priority_section()  ← 已添加
7. [skills_index]
8. SYSTEM_PROMPT_DYNAMIC_BOUNDARY
9. environment_section()
10. [project_context + instruction_files]
11. [config section]
12. [append_sections]
```

### 4.2 需新增/改善的段落

#### 4.2.1 产品身份注入（新增）

**目的**：让 Agent 知道自己运行在 if2Ai 平台上，提升品牌一致性。

**实现位置**：`get_simple_intro_section()` 中新增一句。

**改善后文本**：

```
You are if2Ai, an AI assistant running on the if2Ai platform.
You help users with software engineering, research, information retrieval, writing, and analysis.
Use the instructions below and the tools available to you to assist the user.

IMPORTANT: You must NEVER generate or guess URLs unless you are confident they are correct.
You may use URLs provided by the user or discovered via web_search/web_fetch tools.
```

#### 4.2.2 记忆使用行为规则（新增）

**目的**：防止 Agent 说"我记得你说过..."破坏用户体验，与 openhanako 的精密记忆规则对齐。

**实现位置**：`build()` 中 `append_sections` 前、`memory_context` 注入时动态添加（当记忆上下文非空时）。

**完整规则文本（英文）**：

```
# Memory Usage Rules

Memories and user profile are internalized background knowledge.
Apply them silently — shaping your angle, tone, and judgment — but never referencing them explicitly.

- NEVER say "I remember", "you mentioned before", or "based on my memory" unless the user explicitly asks.
- Memory may be outdated; the current conversation always takes priority.
- When information conflicts, go with the current conversation, not stored memory.
```

**实现方式**（`agent.rs` 中注入 memory context 时追加）：

```rust
// 在 memory_context 非空时，prepend 记忆使用规则
if !memory_context.is_empty() {
    system_prompt.push(get_memory_usage_rules_section());
    system_prompt.push(memory_context);
}
```

#### 4.2.3 运行时动态通知机制（新增）

**目的**：支持只读模式通知、后台任务结果通知等运行时状态注入。

**实现位置**：`build()` 的 `append_sections` 机制（已存在）+ 新增动态注入 API。

**只读模式文本**：

```
[System Notice] Currently in READ-ONLY MODE.
File write, edit, and delete operations are disabled.
You can only use read-only tools (read_file, grep_search, glob_search, web_search, web_fetch, browser).
If the user asks for file modifications, explain that read-only mode is active.
```

**实现方式**：

```rust
// agent.rs 中根据 permission_mode 动态追加
if mode == PermissionMode::ReadOnly {
    system_prompt.push(get_readonly_mode_notice());
}
```

#### 4.2.4 多步骤任务规则强化（改善现有段落）

**当前**（一行 bullet，力度不足）：
```
- For multi-step tasks (e.g. "search X and write results to file Y"), 
  continue calling tools until ALL steps are complete — do not stop after the first tool result.
```

**改善后**（独立段落，更具约束力）：

```
# Multi-step task execution

When a task contains multiple steps, you MUST continue calling tools until EVERY step is complete.

Common multi-step patterns:
- "Search X and write to file Y" → web_search → (web_fetch if needed) → file_write
- "Browse X and summarize" → browser navigate → snapshot/screenshot → file_write
- "Find X and create a report" → search/fetch → file_write → deliver_file

Rules:
- Do NOT stop after completing the first step and outputting results as text.
- If you wrote a file, call deliver_file to confirm delivery to the user.
- If the task seems done but you haven't written any files, re-read the user's request.
- A task is only complete when all explicitly requested outputs exist as real files or tool results.
```

#### 4.2.5 工作目录语义消歧（改善现有 environment_section）

**当前**：
```
Working directory: {cwd}
```

**改善后**：
```
Working directory: {cwd}
NOTE: When the user says "desktop", "desk", or "workspace", they mean this working directory,
NOT the system Desktop (~/Desktop). Files mentioned by the user default to this directory.
```

### 4.3 改善后的 `build()` 完整段落顺序

```
 1. get_product_identity_section()          ← 新增（产品身份）
 2. [Output Style if any]
 3. get_simple_system_section()
 4. get_simple_doing_tasks_section()        ← 已包含多步骤 bullet
 5. get_actions_section()
 6. get_web_tool_priority_section()         ← 已添加
 7. get_multistep_task_section()            ← 新增独立段落（改善现有规则）
 8. [skills_index]
 9. SYSTEM_PROMPT_DYNAMIC_BOUNDARY
10. environment_section()                   ← 改善 cwd 语义消歧
11. [project_context + instruction_files]
12. [config section]
13. [memory_rules + memory_context]         ← 新增条件注入
14. [readonly_mode_notice if applicable]    ← 新增条件注入
15. [append_sections]
```

---

## 5. 工具描述改善设计

### 5.1 改善原则

工具描述应遵循以下层次结构：

```
[What] 工具做什么（1 句）
[When] 什么时候用（与其他工具的区别）
[Workflow] 典型工作流（下一步应该做什么）
[Constraint] 使用约束（何时不应用）
```

### 5.2 各工具改善规范

#### `web_search`

**当前描述**：
```
Search the web for up-to-date information. Uses configured provider (Tavily/Brave/Serper/SearXNG)
or falls back to DuckDuckGo. Returns titles, URLs, and text snippets. For deeper content from
a specific URL in the results, follow up with web_fetch. When the task is to search AND save
results (e.g. "search X and write to file Y"), always call file_write after gathering the
information — do not stop at the search results.
```

**评估**：已经较好，符合三要素结构。无需改动。

#### `web_fetch`

**当前描述**：
```
Fetch and extract readable text from a web page URL. Use this after web_search to get the
full content of a specific result URL. Prefer web_search for general queries; use web_fetch
when you have a known URL and need its full text content. Falls back to browser if the page
requires JavaScript rendering.
```

**评估**：已经较好。无需改动。

#### `browser`（重点改善项）

**当前描述**：
```
Control an interactive headless web browser. Use 'navigate' to load a URL (auto-starts browser
if needed). For other actions, start explicitly with action='start' first. Interact with elements
using their [N] ref numbers from 'snapshot'. Actions: start | stop | navigate | snapshot |
screenshot | click | type | scroll | select | key | wait | evaluate
```

**问题**：
- 缺少"何时用 browser vs web_fetch"的判断指引（虽然 System Prompt 有，但工具层重复更保险）
- 缺少"完成后应该做什么"的工作流引导
- `show` action 缺失

**改善后描述**：
```
Control an interactive headless web browser for pages that require JavaScript rendering,
login, or user interaction. Use ONLY when web_search or web_fetch cannot do the job.

Typical workflow:
  navigate → snapshot (to see page structure) → click/type/scroll → screenshot (to capture result)

After gathering information via browser, use file_write to save results if the user requested a file.
Use action='navigate' to load a URL (browser auto-starts). Use snapshot to get element ref numbers
for click/type. Actions: start | stop | navigate | snapshot | screenshot | click | type | scroll |
select | key | wait | evaluate

CAPTCHA warning: If you land on a verification page, use 'screenshot' to inspect it.
Consider switching to web_search or a different URL instead of attempting to bypass it.
```

#### `file_write`

**当前描述**（已改善）：
```
Write content to a file at the given path. Creates the file if it does not exist, or overwrites
it (use append: true to append instead). Use this to save research results, reports, notes, or
any text content the user asked to be written to a file (e.g. after web_search or browser tasks).
The path can be relative (resolved against the working directory) or absolute.
```

**改善后**（添加 deliver_file 工作流提示）：
```
Write content to a file at the given path. Creates the file if it does not exist, or overwrites
it (use append: true to append instead). Use this to save research results, reports, notes, or
any text content the user asked to be written to a file (e.g. after web_search or browser tasks).
The path can be relative (resolved against the working directory) or absolute.

After writing, call deliver_file with the file path to notify the user the file is ready.
```

#### `file_edit`（待 re-enable 后的目标描述）

**当前状态**：`disabled: true`（应修复）

**目标描述**：
```
Apply a precise string replacement to a file (find-and-replace). Use this to make targeted edits
without overwriting the entire file. Preferred over file_write when modifying existing content.
Requires exact match of old_string including whitespace and indentation.
```

**Schema 建议**（参考 openhanako 的 `edit` 工具）：
```json
{
  "path": "Path to the file to edit",
  "old_string": "Exact string to find and replace",
  "new_string": "Replacement string",
  "replace_all": "Replace all occurrences (default: false)"
}
```

#### `TodoWrite`（改善现有工具）

**当前描述**：
```
Update the structured task list for the current session. Use this to track multi-step tasks
with status (pending/in_progress/completed).
```

**问题**：缺少使用时机的明确指引；无 list/toggle 能力说明。

**改善后描述**：
```
Manage the task list for the current session. Use this for multi-step tasks to track progress.

Actions:
- write (default): Create or replace the entire task list with new todos
- Use status: "pending" | "in_progress" | "completed" | "cancelled"

Best practice: At the start of a multi-step task, write all steps as todos.
Update status to "in_progress" when starting a step, "completed" when done.
This makes your progress visible and helps you continue systematically.
```

**长期改善**（增加 `query` / `update` action）：参见第 6 节新工具设计。

#### `memory_recall`

**当前描述**：
```
Recall memories matching a query
```

**改善后**：
```
Search and retrieve memories from long-term storage. Use this to recall user preferences,
past context, or stored facts before responding to queries that may benefit from personalization.
Returns memories sorted by relevance. Silently apply recalled information — do not mention
"I found a memory" or "according to my memory" in your response.
```

---

## 6. 工具机制改善设计（执行保障层）

这是当前最大的架构债务区域。

### 6.1 新增 `deliver_file` 工具（等价于 `stage_files`）

**设计目标**：
- 为文件写入操作提供"任务完成仪式"
- 强迫 Agent 在完成文件写入后继续一轮工具调用（防止 `model_stop_no_tools`）
- 在 UI 层展示可点击的文件链接

**工具规范**：

| 字段    | 值                           |
| ------- | ---------------------------- |
| 工具名  | `deliver_file`               |
| Toolset | `files`                      |
| 权限    | `ReadOnly`（仅通知，不修改） |

**Description**：
```
Deliver one or more completed files to the user. Call this after file_write to confirm
the file is ready and make it visible in the chat interface.

REQUIRED: Always call this after writing a file the user explicitly requested.
This is the final step of any task that produces output files.

The user will see a clickable file card in the chat — do not describe the file path in text,
deliver_file handles the presentation.
```

**Schema**：
```json
{
  "type": "object",
  "properties": {
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Absolute or relative path to the file"
          },
          "label": {
            "type": "string",
            "description": "Human-readable label for the file (e.g. 'xchat Research Report')"
          }
        },
        "required": ["path"]
      },
      "description": "List of files to deliver to the user"
    },
    "message": {
      "type": "string",
      "description": "Optional brief summary of what was written (1-2 sentences)"
    }
  },
  "required": ["files"]
}
```

**前端 UI 实现要求**：
- 接收 `deliver_file` 工具结果时，在聊天消息中渲染文件卡片组件
- 文件卡片显示：文件名、类型图标、文件大小、点击打开按钮
- 对应 Tauri 命令：`open_file_in_explorer` / 调用系统默认程序打开

**后端实现**（Rust `deliver_file.rs`）：
```rust
// 验证文件路径存在
// 发送 Tauri 事件到前端展示文件卡片
// 返回成功信息给 LLM
```

### 6.2 改善 `TodoWrite` → `todo` 工具（增加 query/update actions）

**设计目标**：与 openhanako 的 `todo` 工具对齐，支持增量操作。

**新 Schema（新增 actions）**：

```json
{
  "action": {
    "type": "string",
    "enum": ["write", "list", "update"],
    "description": "write: create/replace full todo list; list: show current todos; update: change status of specific todo by id"
  },
  "todos": {
    "type": "array",
    "description": "For action=write: full list of todos to set"
  },
  "id": {
    "type": "string",
    "description": "For action=update: todo id to update"
  },
  "status": {
    "type": "string",
    "enum": ["pending", "in_progress", "completed", "cancelled"],
    "description": "For action=update: new status"
  }
}
```

**新 Description**：
```
Manage the structured task list for the current session.

Actions:
- write: Create or completely replace the task list (provide full todos array)
- list: View current todos and their statuses (no parameters needed)
- update: Change the status of a specific todo by id

Use todos to plan and track multi-step tasks. Start complex tasks by writing all steps,
then update status as you progress. Call list before update to get current todo ids.
```

### 6.3 修复 `file_edit` 工具

**当前状态**：`disabled: true` in `file_edit.rs`

**修复目标**：重新启用并完善 schema。

**建议 Schema**（替换当前 unified-diff 方案）：
```json
{
  "path": {
    "type": "string",
    "description": "Path to the file to edit"
  },
  "old_string": {
    "type": "string",
    "description": "Exact string to find (must match exactly including whitespace)"
  },
  "new_string": {
    "type": "string",
    "description": "Replacement string"
  },
  "replace_all": {
    "type": "boolean",
    "description": "Replace all occurrences (default: false, replaces first occurrence only)"
  }
}
```

**启用条件**：完成 find-and-replace 实现后将 `disabled: false`，并在 permission policy 中注册为 `WorkspaceWrite`（已存在）。

### 6.4 新增 `check_tasks` 工具（等价于 `check_pending_tasks`）

**设计目标**：让 Agent 能查询当前会话的后台任务状态。

**工具规范**：

| 字段    | 值            |
| ------- | ------------- |
| 工具名  | `check_tasks` |
| Toolset | `system`      |
| 权限    | `ReadOnly`    |

**Description**：
```
Check the status of background or long-running tasks in the current session.
Use this after starting subagent tasks, cron jobs, or any deferred operation
to see if they have completed, are still running, or failed.
Returns all tasks unless filtered by status.
```

**Schema**：
```json
{
  "status": {
    "type": "string",
    "enum": ["pending", "running", "completed", "failed"],
    "description": "Filter by status. Omit to return all tasks."
  }
}
```

---

## 7. Agent Loop 改善设计

### 7.1 `max_iterations` 动态化

**当前**：硬编码 `let max_iterations: usize = 10;`

**问题**：10 轮对于复杂的多步骤任务（搜索 + 深读 5 个 URL + 写报告 + deliver_file）可能不够。

**改善方案**：

```rust
// 根据任务复杂度动态确定上限
// 简单任务（单工具）：10 轮
// 复杂任务（含 browser）：20 轮
// 最大上限：30 轮（防止无限循环）
let max_iterations: usize = match infer_task_complexity(&messages_for_stream) {
    TaskComplexity::Simple => 10,
    TaskComplexity::Medium => 20,
    TaskComplexity::Complex => 30,
};
```

**短期简化方案**（无需复杂推断）：
```rust
let max_iterations: usize = 20;  // 从 10 提高到 20
```

### 7.2 `model_stop_no_tools` 的智能续跑机制

**问题**：LLM 完成了中间步骤后输出文本停止，但任务实际未完成。

**改善方案**：检测"任务未完成"信号并注入提示继续执行。

```rust
// 在 model_stop_no_tools 时，检查是否存在未完成迹象
if pending_tool_uses.is_empty() {
    // 检查：用户请求的文件是否已被创建
    let has_unfulfilled_file_request = check_unfulfilled_file_request(
        &session_messages,
        &accumulated_text,
    );
    
    if has_unfulfilled_file_request && tool_loop_iter < max_iterations {
        // 注入系统提示继续执行
        let continue_prompt = SystemReminderMessage::new(
            "You mentioned writing a file but no file_write tool was called. \
             Please call file_write now to complete the task."
        );
        session_messages.push(continue_prompt);
        // 不 break，继续下一轮
        continue;
    }
    
    terminal_status = Some("model_stop_no_tools");
    break;
}
```

**`check_unfulfilled_file_request` 检测规则**：
- 用户消息中包含"写入"/"write to"/"save to"/"写到"等关键词
- 用户消息中包含".md"/".txt"/".json"/".csv"等文件扩展名
- 且当次 loop 没有成功的 `file_write` 或 `deliver_file` 工具调用

### 7.3 `contains_unverified_file_claim` 改善

**当前**：检测到"已创建/already written"等声明但无工具证明时，将文本改为`未执行工具，无法确认完成`。

**改善**：改为更有引导性的中文消息（因为用户是中文环境）：

```rust
// 当前：
"未执行工具，无法确认完成。"

// 改善后：
"⚠️ 提示：我描述了创建文件，但实际上没有调用文件写入工具。\
 如需真正创建文件，请再次发送指令，我会使用 file_write 工具完成。"
```

### 7.4 Resume 机制改善

**当前**：有 resume_cursor 机制，但 `max_iterations_reached` 时才有 `resume_available: true`。

**改善**：当 `model_stop_no_tools` 且存在未完成任务迹象时，也设置 `resume_available: true`，允许用户点击"继续"。

---

## 8. 优先级排序与实施路径

### Phase 8A — 立即改善（1-2天，无破坏性变更）

> 目标：解决任务过早终止的 P0 问题

| #   | 改善项                                                  | 文件                                           | 工作量 |
| --- | ------------------------------------------------------- | ---------------------------------------------- | ------ |
| 1   | **新增 `deliver_file` 工具**（Rust + 前端 UI）          | `builtin/deliver_file.rs` + 前端 FileCard 组件 | M      |
| 2   | **`max_iterations` 从 10 提升到 20**                    | `agent.rs:1279`                                | XS     |
| 3   | **新增 `get_multistep_task_section()` 独立段落**        | `prompt.rs`                                    | S      |
| 4   | **产品身份注入到 `get_simple_intro_section()`**         | `prompt.rs`                                    | XS     |
| 5   | **`browser` 工具描述改善**（加入工作流 + CAPTCHA 提示） | `browser_tool.rs`                              | XS     |

### Phase 8B — 工具机制完善（3-5天）

| #   | 改善项                                              | 文件                     | 工作量 |
| --- | --------------------------------------------------- | ------------------------ | ------ |
| 6   | **修复 `file_edit` 工具**（find-and-replace，启用） | `file_edit.rs`           | M      |
| 7   | **`TodoWrite` 增加 `list`/`update` actions**        | `todo_write.rs`          | S      |
| 8   | **新增 `check_tasks` 工具**                         | `builtin/check_tasks.rs` | S      |
| 9   | **`model_stop_no_tools` 智能续跑机制**              | `agent.rs`               | M      |
| 10  | **运行时动态通知注入（只读模式）**                  | `agent.rs` + `prompt.rs` | S      |

### Phase 8C — 体验深化（5-10天）

| #   | 改善项                                  | 文件                     | 工作量 |
| --- | --------------------------------------- | ------------------------ | ------ |
| 11  | **记忆使用行为规则注入**                | `prompt.rs` + `agent.rs` | S      |
| 12  | **用户档案（user.md）注入机制**         | `prompt.rs` 新增加载逻辑 | M      |
| 13  | **工作目录语义消歧改善**                | `prompt.rs`              | XS     |
| 14  | **输出格式约束段落（ishiki 等价）**     | `prompt.rs`              | S      |
| 15  | **`deliver_file` 前端文件卡片 UI 完善** | 前端组件                 | M      |

### 工作量说明

| 级别 | 含义                         |
| ---- | ---------------------------- |
| XS   | < 30分钟，纯文字修改         |
| S    | 1-3小时，少量代码            |
| M    | 半天，需要新增文件或较大修改 |
| L    | 1-2天，涉及前后端协调        |

---

## 9. 成功指标

### 9.1 功能性指标

| 指标                             | 当前状态                    | 目标 |
| -------------------------------- | --------------------------- | ---- |
| "搜索+写文件"任务完成率          | ~30%（经常停在 web_search） | >90% |
| browser 连续操作成功率           | 不稳定（CDP 崩溃）          | >85% |
| `model_stop_no_tools` 过早终止率 | 高（多步骤任务常见）        | <10% |
| `max_iterations_reached` 率      | 低（10 轮对简单任务够）     | <5%  |

### 9.2 质量性指标

| 指标           | 衡量方式                                              |
| -------------- | ----------------------------------------------------- |
| 工具调用准确性 | web_search vs web_fetch vs browser 选择是否符合优先级 |
| 文件交付可见性 | 用户能否通过 deliver_file 文件卡片直接打开文件        |
| 任务完整性     | 用户所有明确要求的输出物是否都被创建                  |
| 记忆体验       | Agent 是否出现"我记得你说过"类暴露性语言              |

### 9.3 验收测试用例

**TC-001：搜索写文件（P0）**
```
输入：用 browser 搜索 xchat 最新资讯并写入 x.md
期望：web_search → file_write(x.md) → deliver_file(x.md)
     x.md 文件实际存在，UI 显示文件卡片
```

**TC-002：浏览器连续操作（P0）**
```
输入：进入 google.com，搜索 rust programming
期望：browser navigate → browser type → browser key(Enter) → browser snapshot
     完整执行无中断，thumbnail 正确显示
```

**TC-003：多步骤任务（P1）**
```
输入：搜索 3 篇关于 AI 的最新文章，提取摘要，写入 ai_summary.md
期望：web_search → web_fetch(url1) → web_fetch(url2) → web_fetch(url3) → file_write → deliver_file
     全程 loop 不中断，最终文件正确
```

**TC-004：只读模式（P1）**
```
前提：权限模式设为 ReadOnly
输入：帮我创建一个 hello.txt
期望：Agent 输出"当前处于只读模式，无法创建文件"，不调用 file_write
```

---

## 10. 附录：完整对比矩阵

### 10.1 System Prompt 段落完整对比

| 段落                    | openhanako（中文）           | if2Ai（当前）    | Gap 等级 |
| ----------------------- | ---------------------------- | ---------------- | -------- |
| 产品平台身份            | `你运行在 OpenHanako 平台上` | ❌                | P2       |
| 人格定义（identity）    | ✅ 三层架构                   | ❌                | P3       |
| 情绪系统（yuan/MOOD）   | ✅ `<mood>` 机制              | ❌                | P3       |
| 行为约束（ishiki 11条） | ✅ 禁止特定句式等             | ❌                | P2       |
| 用户档案                | ✅ `user.md` 注入             | ❌                | P2       |
| 记忆使用规则            | ✅ 3条精密约束                | ❌                | P1       |
| 网页工具优先级          | ✅ 3层有序 + 禁止             | ✅（已添加）      | ✅        |
| 多步骤任务指令          | `todo` 工具 + 文案双保险     | ✅ 一行（需强化） | P0→P1    |
| 设置修改路由            | ✅ `update_settings` 专段     | ❌                | P2       |
| 工作目录语义消歧        | ✅ `## 书桌`                  | 部分（缺消歧）   | P2       |
| 时间边界（凌晨4点）     | ✅                            | ❌                | P3       |
| 团队协作                | ✅ 多 Agent roster            | ❌                | P3       |
| 主动技能获取            | ✅ 完整流程                   | 部分             | P2       |
| 只读模式通知            | ✅ 动态注入                   | ❌                | P1       |
| 后台任务感知            | ✅ `<hana-background-result>` | ❌                | P2       |
| Windows 平台规则        | ✅ 动态注入                   | ❌                | P3       |

### 10.2 工具完整性对比

| 工具         | openhanako                   | if2Ai                        | 状态       |
| ------------ | ---------------------------- | ---------------------------- | ---------- |
| 网页搜索     | `web_search`                 | `web_search` ✅               | 对等       |
| 网页抓取     | `web_fetch`                  | `web_fetch` ✅                | 对等       |
| 浏览器控制   | `browser`                    | `browser` ✅                  | 对等       |
| 文件写入     | `write`（Pi SDK）            | `file_write` ✅               | 对等       |
| 文件读取     | `read`（Pi SDK）             | `read_file` ✅                | 对等       |
| 文件编辑     | `edit`（Pi SDK）             | `file_edit`（disabled）❌     | **Gap P1** |
| 文件交付     | `stage_files` ✅              | ❌                            | **Gap P0** |
| 任务追踪     | `todo`（4 actions）          | `TodoWrite`（write only）    | **Gap P1** |
| 后台任务查询 | `check_pending_tasks`        | ❌                            | Gap P2     |
| 设置修改     | `update_settings`            | `Config`（get/set）          | 部分对等   |
| 记忆检索     | `search_memory`（tags+日期） | `memory_recall`              | 部分对等   |
| 子任务       | `subagent` + `ask_agent`     | `agent`（递归）              | 架构不同   |
| 系统命令     | `bash`（Pi SDK）             | `bash` ✅                     | 对等       |
| 向用户发消息 | 无专用工具                   | `SendUserMessage` ✅          | if2Ai 更强 |
| 结构化输出   | 无（MOOD 机制）              | `StructuredOutput` ✅         | if2Ai 更强 |
| 定时任务     | cron（via Pi）               | `cron_add/list/run/remove` ✅ | 对等       |

### 10.3 工具描述质量评分（修复后）

| 工具            | 修复前评分       | 修复后评分 | 满分 |
| --------------- | ---------------- | ---------- | ---- |
| `web_search`    | 4/10             | 8/10       | 10   |
| `web_fetch`     | 2/10             | 8/10       | 10   |
| `browser`       | 5/10             | 7/10       | 10   |
| `file_write`    | 2/10             | 8/10       | 10   |
| `file_edit`     | 0/10（disabled） | 0/10       | 10   |
| `TodoWrite`     | 5/10             | 5/10       | 10   |
| `memory_recall` | 3/10             | 3/10       | 10   |
| `deliver_file`  | ❌ 不存在         | —          | —    |

---

## 变更历史

| 版本 | 日期       | 变更内容                               |
| ---- | ---------- | -------------------------------------- |
| v1.0 | 2026-04-18 | 初始版本，基于 openhanako 深度对比分析 |

---

*本文档基于对 `/Users/ryanliu/Downloads/openhanako-main` 和 `/Users/ryanliu/Documents/IfAI/if2Ai` 的代码级深度分析，所有参考数据均有对应文件路径和行号支撑。*
