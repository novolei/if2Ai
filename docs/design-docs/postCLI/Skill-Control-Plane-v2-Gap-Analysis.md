# Skills 系统 Gap Analysis：hermes-agent vs if2Ai

> **文档角色**：系统架构师视角的功能对比与差距分析报告  
> **对比基准**：hermes-agent-main（Python CLI）vs if2Ai（Rust Desktop）  
> **分析日期**：2026-04-14  
> **分析范围**：技能框架设计、数据模型、调用链路、管理机制、Hub、Guard、前端集成

---

## 执行摘要

经过对两个项目全量代码的深度扫描与架构对比，**if2Ai 在 Skills 框架的基础设施层面（模块组织、类型系统、Hub 多源、Guard 策略）已达到或超过 hermes-agent**，但在 **核心运行时集成**（系统提示注入、自动激活、上下文传递）存在重大架构缺口，导致现有的 Skills 基础设施形同虚设——代码存在，但 Agent 根本不知道有哪些 Skill 可用，也没有任何自动激活路径。

**严重程度分级**

| 等级                | 描述                                      | 数量 |
| ------------------- | ----------------------------------------- | ---- |
| 🔴 **P0 - 阻塞型**   | 导致核心功能完全缺失，Skills 框架无法运转 | 3    |
| 🟠 **P1 - 严重缺口** | 关键能力缺失，用户体验受损                | 5    |
| 🟡 **P2 - 功能漂移** | 已实现但行为与参考实现不符                | 4    |
| 🟢 **P3 - 增强机会** | if2Ai 独有特性或可优化项                  | 3    |

---

## 一、架构全景对比

### 1.1 hermes-agent Skills 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                     Agent Loop (run_agent.py)                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  System Prompt                                            │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │  ## Skills (mandatory)                          │    │   │
│  │  │  <available_skills>                             │    │   │
│  │  │    name: dogfood | desc: QA testing             │    │   │
│  │  │    name: git-ops  | desc: Git workflow          │    │   │
│  │  │    ...（按条件过滤的全量索引）                    │    │   │
│  │  │  </available_skills>                            │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  │  静态指令：「先扫索引，匹配则调 skill_view()」          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ToolRegistry                                                     │
│  ├── skills_list    ← 列出所有技能（toolset: "skills"）          │
│  ├── skill_view     ← 加载技能全文                               │
│  └── skill_manage  ← 创建/编辑/删除                             │
└─────────────────────────────────────────────────────────────────┘
        │ skill_view 调用
        ▼
┌──────────────────────────────┐
│  ~/.hermes/skills/           │
│  ├── dogfood/SKILL.md        │
│  ├── git-ops/SKILL.md        │
│  └── .hub/（安装元数据）     │
└──────────────────────────────┘
        │ Hub 安装
        ▼
┌─────────────────────────────────────────────┐
│  多源适配器（7个）                           │
│  OptionalSkill → SkillsSh → WellKnown →     │
│  GitHub → ClawHub → Marketplace → LobeHub   │
└─────────────────────────────────────────────┘
```

**关键设计原则**：模型在每次对话开始时即知晓所有可用技能的目录索引，无需用户主动触发。

---

### 1.2 if2Ai Skills 架构图（当前状态）

```
┌──────────────────────────────────────────────────────────────────┐
│                  Agent Loop (agent.rs / conversation.rs)          │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │  System Prompt (prompt.rs)                                 │   │
│  │  ┌──────────────────────────────────────────────────┐    │   │
│  │  │  CLAUDE.md + 项目上下文 + LSP 信息              │    │   │
│  │  │                                                  │    │   │
│  │  │  ❌ 无 Skills 索引注入！                         │    │   │
│  │  │  ❌ 模型不知道有任何 Skill 存在！               │    │   │
│  │  └──────────────────────────────────────────────────┘    │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                    │
│  ToolRegistry                                                      │
│  ├── skill        ← 按名称加载（模型不知要叫什么名字）            │
│  └── skill_search ← 本地搜索（但未触发自动激活）                 │
│                                                                    │
│  SkillCommands（已实现但完全未接入）                              │
│  ├── scan()                                                        │
│  ├── build_invocation_message()                                    │
│  └── build_preloaded_prompt() ← ❌ 从未被调用                    │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│  Skills 基础设施（健全，但孤立）                                  │
│  ├── modules/skills/hub/        ← Hub + 多源适配                  │
│  ├── modules/skills/guard/      ← Guard + 策略矩阵               │
│  ├── modules/skills/manager/    ← CRUD + 原子写                   │
│  ├── modules/skills/commands.rs ← Slash + 预加载（未接入）        │
│  ├── modules/skills/config.rs   ← 配置解析（未接入调用链）       │
│  └── commands/skills_hub.rs     ← Tauri IPC（部分为占位符）      │
└──────────────────────────────────────────────────────────────────┘
```

**核心问题**：Skills 基础设施完备，但与 Agent Runtime 之间 **缺乏所有运行时集成桥梁**。

---

## 二、详细 Gap 分析

### 🔴 GAP-01：系统提示中无 Skills 索引注入（P0 - 阻塞型）

**Hermes 实现**

```python
# run_agent.py
has_skills_tools = any(name in self.valid_tool_names 
                       for name in ['skills_list', 'skill_view', 'skill_manage'])
if has_skills_tools:
    skills_prompt = build_skills_system_prompt(
        available_tools=self.valid_tool_names,
        available_toolsets=avail_toolsets,
    )
    prompt_parts.append(skills_prompt)
```

```python
# agent/prompt_builder.py — build_skills_system_prompt()
result = (
    "## Skills (mandatory)\n"
    "Before replying, scan the skills below. If one clearly matches your task, "
    "load it with skill_view(name) and follow its instructions.\n"
    "<available_skills>\n"
    + "\n".join(index_lines) + "\n"   # 全量 name:desc 索引
    "</available_skills>\n"
)
```

生成的索引内容示例：
```
## Skills (mandatory)
Before replying, scan the skills below. If one clearly matches your task,
load it with skill_view(name) and follow its instructions.
<available_skills>
dogfood: Systematic exploratory QA testing of web applications
git-ops: Git workflow automation and branching strategy
code-review: Code review checklist and best practices
</available_skills>
```

**if2Ai 现状**

`src-tauri/src/modules/runtime/prompt.rs` 完全没有 Skills 相关内容。系统提示仅由 CLAUDE.md、项目上下文（git status / LSP 信息）组成。

**影响**：Agent 永远不会主动使用任何 Skill。即使用户安装了 20 个技能，对话 AI 完全不知道它们的存在，除非用户手动输入 `/skill-name` 触发 slash 命令。

**修复方向**：
1. 在 `prompt.rs` 中增加 `build_skills_system_prompt(workdir, available_tool_names)` 函数
2. 在 `agent.rs` 的系统提示构建阶段调用并拼接
3. 实现 LRU + 磁盘快照缓存（hermes 的磁盘缓存键为文件 mtime 哈希）

---

### 🔴 GAP-02：无 `skill_view` 工具——Skill 激活入口设计漂移（P0 - 阻塞型）

**Hermes 模型**

三工具设计：
| 工具           | 职责                         | 触发方式                 |
| -------------- | ---------------------------- | ------------------------ |
| `skills_list`  | 列出可用技能目录             | 用户查询/Agent 主动      |
| `skill_view`   | **加载技能全文到对话上下文** | Agent 匹配索引后主动调用 |
| `skill_manage` | 创建/编辑/删除技能           | 用户或 Agent 请求        |

`skill_view` 是核心：模型看到系统提示中的索引后，**主动决策**调用 `skill_view(name="dogfood")` 将完整 SKILL.md 内容注入到当前对话回合中。

**if2Ai 现状**

两工具设计：
| 工具           | 职责                                            |
| -------------- | ----------------------------------------------- |
| `skill`        | 按名称加载（等同于 `skill_view`，但命名不直观） |
| `skill_search` | 本地搜索                                        |

缺少 `skills_list` 工具（只有 `skill_search` 作为替代，但语义不同），缺少 `skill_manage` 工具。

**更深层问题**：即使 `skill` 工具（等效于 `skill_view`）存在，没有系统提示索引，模型不知道有哪些技能可以加载。`skill_search` 的存在也无法弥补——模型需要知道"要搜什么"才会去搜。

**修复方向**：
1. 新增 `skills_list` 工具（或确保系统提示索引足够完整让 `skill_search` 作为发现工具）
2. 将 `skill` 工具更名或添加别名为 `skill_view`（或在系统提示中明确说明工具名）
3. 新增 `skill_manage` 工具（对接 `SkillManager` trait）

---

### 🔴 GAP-03：Skill 配置变量（config vars）未接入调用链（P0 - 阻塞型）

**Hermes 实现**

```python
# skills_tool.py — skill_view 加载后自动执行
for env_var in required_environment_variables:
    register_env_passthrough(env_var)
for cred_file in required_credential_files:
    register_credential_files([cred_file])
```

`skill_view` 返回技能内容的同时，自动将 SKILL.md frontmatter 中声明的：
- `required_environment_variables` → 注册为环境变量透传
- `required_credential_files` → 注册为凭证文件透传
- `config` 声明 → 从 `~/.hermes/config.yaml` 的 `skills.config.<key>` 读取并注入

**if2Ai 现状**

`modules/skills/config.rs` 实现了完整的 `SkillConfigVar` 和 `SkillConfigResolver`，但：
- `builtin/skill.rs` 的 `skill_tool_entry()` 调用链中 **从未调用** `SkillConfigResolver`
- `remote_passthrough.rs` 模块存在但未接入 `skill` 工具
- 配置变量的发现和注入完全脱节

**影响**：需要凭证（API Key、Token）的 Skills 无法正常工作，用户安装后看不到配置入口。

**修复方向**：
1. 在 `skill_tool_entry()` 的 handler 中调用 `SkillConfigResolver::from_yaml(workdir)`
2. 解析 SKILL.md frontmatter 中的 `config:` 声明
3. 将解析后的配置值注入 skill 内容（模板替换或追加 config block）

---

### 🟠 GAP-04：`SkillCommands::build_preloaded_prompt()` 未接入 Session 初始化（P1）

**Hermes 实现**

```python
# cli.py — session 启动时
preloaded_prompt, skill_names, supporting_files = build_preloaded_skills_prompt(
    identifiers=args.preloaded_skills  # 通过 --preloaded-skills 传入
)
if preloaded_prompt:
    initial_messages.append({
        "role": "user",
        "content": preloaded_prompt
    })
```

支持 `--preloaded-skills` 参数将指定技能的完整内容作为 user 消息预加载到会话开头。

**if2Ai 现状**

`SkillCommands::build_preloaded_prompt()` 已完整实现，构建逻辑与 hermes 一致，但：
- 在 `agent.rs`、`session.rs`、`conversation.rs` 中没有任何调用点
- 前端 `SkillsSettingsPage.tsx` 没有"预加载技能到会话"的 UI 入口
- Tauri 命令层没有对应的 IPC 接口

**修复方向**：
1. 添加 Tauri 命令 `start_session_with_skills(skill_names: Vec<String>)`
2. 在会话初始化时调用 `build_preloaded_prompt()` 并注入消息
3. 前端添加"对此会话激活技能"的快捷入口

---

### 🟠 GAP-05：Slash 命令 `/skill-name` 未接入 Agent 消息发送（P1）

**Hermes 实现**

```python
# agent/skill_commands.py
def build_skill_invocation_message(name, user_instruction):
    return {
        "role": "user",
        "content": f"[SYSTEM: The user has invoked skill '{name}' with instruction: {user_instruction}]\n\n{skill_content}\n\n{config_block}"
    }
```

当用户发送 `/dogfood run tests` 时，系统：
1. 识别 `/dogfood` 为已安装的技能名
2. 加载 `dogfood/SKILL.md` 内容
3. 构建包含 `[SYSTEM: ...]` 激活标记的特殊消息
4. 将消息插入对话上下文发给模型

**if2Ai 现状**

`SkillCommands::build_invocation_message()` 逻辑完整，但：
- `commands/slash.rs` 中 `/skills` 命令仅执行 `list_skills`，不解析 `/具体技能名`
- 没有将 slash 命令解析结果接入 `start_agent_stream` 的预处理管道
- 前端 `chat-ui.tsx` 对 `/` 开头消息没有技能激活的前处理逻辑

**修复方向**：
1. 在消息发送前处理 `/skill-name [instruction]` 格式
2. 调用 `SkillCommands::build_invocation_message()` 替换原始消息
3. 或在 Agent 侧作为系统消息前置注入

---

### 🟠 GAP-06：条件激活逻辑（`requires_toolsets` / `metadata.hermes`）未评估（P1）

**Hermes 实现**

```python
# agent/prompt_builder.py — _skill_should_show()
def _skill_should_show(conditions, available_tools, available_toolsets):
    for ts in conditions.get("fallback_for_toolsets", []):
        if ts in ats: return False  # 有原生工具时隐藏回退技能
    for ts in conditions.get("requires_toolsets", []):
        if ts not in ats: return False  # 缺少必需工具集时隐藏
    for t in conditions.get("requires_tools", []):
        if t not in at: return False
    return True
```

系统提示中的技能索引会根据当前会话可用的工具集动态过滤：
- `requires_toolsets: [browser]` → 没有 browser 工具时不显示
- `fallback_for_toolsets: [computer_use]` → 有 computer_use 原生工具时不显示回退技能

**if2Ai 现状**

`SkillCommandInfo` 结构体有 `requires_toolsets` 和 `fallback_for_toolsets` 字段，`check_toolsets_availability()` 方法也已实现，但：
- 在 `skills_search.rs` 中完全未使用这些字段
- `prompt.rs` 没有技能索引，更谈不上条件过滤
- `slash.rs` 的 `list_skills_impl` 不过滤 requires_toolsets

---

### 🟠 GAP-07：Hub `hub_update` 和 `hub_publish` 为占位符（P1）

**Hermes 实现**

完整实现了：
- `do_install(source, identifier)` → quarantine → scan → confirm → install
- `do_uninstall(name)` → 清理文件 + 锁文件 + 提示缓存失效
- `do_update(name)` → 检查新版本 + 重新安装
- `do_publish(skill_dir)` → 发布到 Hub

**if2Ai 现状**

```rust
// src-tauri/src/commands/skills_hub.rs
#[tauri::command]
pub async fn hub_update(_name: String) -> Result<String, String> {
    // TODO: implement update
    Ok("hub_update: not yet implemented".to_string())
}

#[tauri::command]
pub async fn hub_publish(_skill_dir: String) -> Result<String, String> {
    // TODO: implement publish
    Ok("hub_publish: not yet implemented".to_string())
}
```

安装流程也不完整：`hub_check` 调用 `HubState::fetch`，但缺少 quarantine → scan → user_confirm → install 完整流水线，安装后也没有失效 Skills 索引缓存。

---

### 🟠 GAP-08：`skill_manage` 工具（Agent 自主创建/编辑技能）缺失（P1）

**Hermes 实现**

`skill_manage` 是核心工具，允许 Agent 自主创建、编辑、修补技能：
```python
# tools/skill_manager_tool.py
def skill_manage(action, name, content=None, file_path=None, patch=None):
    # action: "create" | "edit" | "delete" | "patch"
    # 严格校验 frontmatter 格式
    # 写入 ~/.hermes/skills/<name>/SKILL.md
```

这是 Agent 自我进化能力的关键：模型可以在对话中发现需要重复使用的模式，然后创建新技能持久化它。

**if2Ai 现状**

`DefaultSkillManager::manage()` 完整实现了 CRUD，`SkillManageAction` 枚举定义了所有操作，但没有对应的 `ToolEntry` 将其暴露给 Agent。

Agent 侧只有 `extract_skill_proposal_name()` + `create_agent_skill_proposal_draft()` — 这是一个更保守的「提案」机制，需要人工审批。

---

### 🟡 GAP-09：`skill_search` 工具语义与 Hermes `skills_list` 不对等（P2 - 功能漂移）

**Hermes 设计**

`skills_list` 返回全量分类目录（`name`, `description`, `category`），是 Agent 的"地图"工具，配合系统提示中的静态索引使用。

**if2Ai 现状**

`skill_search` 主要功能是**过滤搜索**（按 query 子串匹配），不支持空查询返回全量列表（空 query 返回全量，但返回格式和信息量不同）。没有 `category` 字段。

**影响**：工具语义不一致，会造成 Agent 行为预期差异。

---

### 🟡 GAP-10：`skill.json` 必须字段与 hermes frontmatter 语义不对等（P2 - 设计漂移）

**Hermes 数据模型**（frontmatter only）

```yaml
---
name: dogfood
description: QA testing tool
version: 1.0.0
metadata:
  hermes:
    tags: [qa, testing, browser]
    related_skills: []
    requires_tools: [browser_action]
    config:
      - key: target_url
        description: Target URL to test
        default: http://localhost:3000
---
```

**if2Ai 数据模型**（SKILL.md frontmatter + skill.json）

```json
{
  "id": "demo-skill",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["custom"],
  "review": {
    "status": "active",
    "riskLevel": "low",
    "lastReviewedAt": "2026-04-14T00:00:00Z"
  }
}
```

**分析**：if2Ai 的双文件设计（SKILL.md + skill.json）是有意识的架构升级，增加了 `review` 字段的 **强制治理**（比 hermes 严格）。但存在字段漂移：
- hermes 的 `metadata.hermes.config` 声明 → if2Ai 在 `SkillConfigVar` 中实现但未接入
- hermes 的 `metadata.hermes.tags` → if2Ai 的 `skill.json` 中无 tags 字段（Hub 搜索会影响质量）
- hermes 的 `readiness_status`（运行前环境检查）→ if2Ai 无对应概念

---

### 🟡 GAP-11：SkillReadinessStatus（运行前就绪检查）缺失（P2）

**Hermes 实现**

```python
class SkillReadinessStatus(str, Enum):
    AVAILABLE = "available"
    SETUP_NEEDED = "setup_needed"
    UNSUPPORTED = "unsupported"
```

`skills_list` 对每个技能检查 `required_environment_variables` 是否已设置，返回 `setup_needed` 状态，让 Agent 知道该技能需要先配置才能使用。

**if2Ai 现状**

`SkillInfo.status` 字段存在（在 `slash.rs` 中），但值来源于 `skill.json` 的 `review.status`（审查状态），而非运行时就绪状态。没有环境变量检查逻辑。

---

### 🟡 GAP-12：Skills 索引缓存机制缺失（P2 - 性能）

**Hermes 实现**

```python
# agent/prompt_builder.py
# LRU 内存缓存 + 磁盘快照（key = 文件 mtime 哈希）
@lru_cache(maxsize=1)
def build_skills_system_prompt_cached(mtime_hash: str) -> str:
    return _build_skills_system_prompt_impl()
```

每次构建系统提示时，计算技能目录的文件修改时间哈希，命中缓存则直接返回，避免每次对话都全量扫盘。

**if2Ai 现状**

由于完全没有实现 Skills 索引注入，这个缓存机制也不存在。一旦实现了注入，需要同步实现缓存，否则每次对话都扫描文件系统会有性能问题。

---

### 🟢 GAP-13：if2Ai 独有特性——Skill Proposal + 审批工作流（优势）

**hermes 无此机制**

if2Ai 实现了独特的 **Agent 技能提案机制**：
```rust
// skill.rs
fn extract_skill_proposal_name(text: &str) -> Option<String>
fn create_agent_skill_proposal_draft(workdir, name, content) -> Result<PathBuf>
fn approve_skill_proposal(skill_dir) -> Result<()>
fn rollback_skill_proposal(skill_dir) -> Result<()>
```

Agent 输出 `skill_proposal: <name>` 触发提案创建，用户在前端审批后激活。这比 hermes 的直接创建更安全，适合桌面应用的信任模型。

**建议**：这是 if2Ai 的架构优势，应保留并完善（当前前端 UI 仅有 `review_status` 显示，缺少一键审批/拒绝操作）。

---

### 🟢 GAP-14：if2Ai 独有特性——`skill.json` 强治理（优势）

hermes 没有独立的 `skill.json`，全依赖 frontmatter。if2Ai 引入的 `skill.json` + `SkillReviewStatus` 状态机是更强的治理框架：
- `Draft` → `Quarantine` → `ReviewPassed` → `Active` → `Disabled`
- 强制 `lastReviewedAt` 字段
- `riskLevel` 标注

这是桌面应用比 CLI 工具更需要的安全保障，应继续沿用。

---

### 🟢 GAP-15：if2Ai 独有特性——完善的前端 Skills Hub UI（优势）

hermes 主要通过 CLI (`hermes skills search/install`) 管理技能，if2Ai 有：
- `SkillsSettingsPage.tsx`：双 Tab（已安装/市场）
- `SkillsHubView.tsx`：搜索结果分组 + 信任标签
- `SkillEditor.tsx`：前端编辑器
- `SkillSecurityReport.tsx`：安全扫描结果可视化

这些是 Desktop 产品的明显优势。

---

## 三、功能矩阵对比

| 功能                                    | Hermes                            | if2Ai 当前                 | 优先级 |
| --------------------------------------- | --------------------------------- | -------------------------- | ------ |
| 系统提示中自动注入 Skills 索引          | ✅ 全量+条件过滤                   | ❌ 完全缺失                 | P0     |
| `skill_view` / `skill` 工具（加载全文） | ✅                                 | ✅（命名不同）              | —      |
| `skills_list` 工具                      | ✅                                 | ❌（只有 skill_search）     | P1     |
| `skill_manage` 工具                     | ✅                                 | ❌（后端有 Manager 未暴露） | P1     |
| Slash `/skill-name` 激活                | ✅ 接入消息管道                    | ⚠️ 实现了但未接入           | P1     |
| 预加载技能到 Session                    | ✅ `--preloaded-skills`            | ⚠️ 实现了但未接入           | P1     |
| 配置变量自动注入（config vars）         | ✅                                 | ❌ 模块存在但断路           | P0     |
| 环境变量/凭证透传（remote_passthrough） | ✅ skill_view 后自动               | ❌ 未接入                   | P0     |
| 条件激活（requires_toolsets 过滤）      | ✅                                 | ⚠️ 字段存在但未评估         | P1     |
| SkillReadinessStatus（就绪检查）        | ✅                                 | ❌                          | P2     |
| Skills 索引 LRU+磁盘缓存                | ✅                                 | ❌                          | P2     |
| Hub 多源搜索（7源）                     | ✅ 7源                             | ✅ 7源（部分占位）          | —      |
| Hub 安装完整流水线                      | ✅ quarantine→scan→confirm→install | ⚠️ 部分实现                 | P1     |
| Hub update / publish                    | ✅                                 | ❌ 占位符                   | P1     |
| Guard 安装前扫描                        | ✅                                 | ✅ 实现更完善               | —      |
| skill.json 强治理                       | ❌ 无                              | ✅ if2Ai 领先               | —      |
| Agent 技能提案机制                      | ❌ 无                              | ✅ if2Ai 独有优势           | —      |
| 前端 Hub UI                             | ❌（CLI only）                     | ✅ 完整 UI                  | —      |
| 技能标签（tags）                        | ✅                                 | ❌ skill.json 无 tags       | P2     |
| 平台过滤（platforms）                   | ✅                                 | ✅                          | —      |
| related_skills                          | ✅                                 | ❌                          | P3     |

---

## 四、根因分析

### 为什么会产生这些 Gap？

**1. 基础设施优先，集成滞后**

if2Ai 按照合理的分层顺序建设：先实现 Skills 基础模块（Hub/Guard/Manager），再接入运行时。但运行时集成阶段的工作被推迟了，导致"基础设施孤岛"现象。

**2. hermes 的隐式设计契约未被显式记录**

hermes 最关键的设计是：**系统提示中的技能索引 + Agent 自主调用 skill_view** 这个隐式契约。这在 hermes 代码里分散在 `run_agent.py`（注入点）和 `prompt_builder.py`（构建逻辑），没有明确的设计文档说明这是"强制性"的设计决策，导致迁移时被遗漏。

**3. 工具数量哲学差异**

hermes 用 3 个专用工具（list/view/manage）覆盖所有场景；if2Ai 试图用 2 个工具（skill/skill_search）覆盖，导致 skills_list 和 skill_manage 的功能空缺，同时工具名称与 hermes 约定不一致，影响 prompt engineering 的有效性。

---

## 五、修复路线图

### Phase A：P0 修复（核心运行时集成，1-2周）

```
A1. 实现 build_skills_index() in prompt.rs
    - 扫描 discover_skill_roots_with_metadata(workdir)
    - 按 precedence 排序，shadow 去重
    - 生成 <available_skills>...</available_skills> 文本
    - LRU 缓存（key = 文件 mtime 哈希）
    
A2. 在 agent.rs 系统提示构建阶段注入
    - 在 load_system_prompt() 之后追加 build_skills_index()
    - 包含指令：「先扫索引，匹配则调 skill(name=...)」
    
A3. 接入 SkillConfigResolver 到 skill 工具
    - skill_tool_entry() handler 中读取 frontmatter config 声明
    - 从 ~/.if2ai/config.yaml 解析 skills.config.* 
    - 注入配置到 skill 内容返回
    
A4. 接入 remote_passthrough 到 skill 工具
    - 读取 frontmatter required_environment_variables
    - 验证环境变量存在
```

### Phase B：P1 修复（完整调用链，2-3周）

```
B1. 新增 skills_list ToolEntry
    - 返回全量技能列表（name, description, source, status）
    - 支持空 query 返回全量、有 query 时过滤
    
B2. 新增 skill_manage ToolEntry
    - 暴露 DefaultSkillManager::manage() 给 Agent
    - action: create | edit | delete | patch
    
B3. 接入 SkillCommands slash 激活
    - 在消息预处理阶段识别 /skill-name 格式
    - 调用 build_invocation_message() 构建激活消息
    
B4. 接入 build_preloaded_prompt
    - 新增 Tauri 命令支持 preloaded_skills 参数
    - 前端 Session 启动时支持选择预加载技能
    
B5. 完善 Hub 安装流水线
    - 实现 hub_install: fetch → quarantine → scan → confirm → install
    - 安装完成后失效 skills 索引缓存
    - 实现 hub_update
```

### Phase C：P2 完善（质量提升，持续）

```
C1. 条件激活评估
    - 在 build_skills_index() 中过滤 requires_toolsets
    - 接入当前会话的 available_toolsets
    
C2. SkillReadinessStatus
    - skill_search / skills_list 返回就绪状态
    - 系统提示中标注 setup_needed 的技能
    
C3. skill.json tags 字段
    - 增加 tags 字段支持 Hub 搜索分类
    
C4. 前端完善 Proposal 审批 UI
    - 一键 Approve/Reject 提案
    - 提案列表视图
```

---

## 六、关键设计决策建议

### 决策 1：保留 `skill.json` 双文件设计

**建议**：保留。if2Ai 的双文件设计（SKILL.md + skill.json）是桌面产品合理的升级，提供更强的治理能力。不需要回退到 hermes 的 frontmatter-only 方案。

但需要将 skill.json 的 `tags`、`config` 等字段补全，确保 Hub 搜索质量和配置注入能力。

### 决策 2：工具命名策略

**建议**：在 `skill_tool_entry()` 的 description 中明确说明这是 `skill_view` 的等效工具，同时补充 `skills_list` 和 `skill_manage`。在系统提示中使用和工具实际名称一致的指令（避免让 Agent 调用不存在的 `skill_view`）。

### 决策 3：Agent 自主创建 vs 提案机制

**建议**：保留 if2Ai 的提案机制作为安全默认，但可以添加一个"信任模式"让高级用户允许 Agent 直接创建技能（类似 hermes 的直接写入）。

### 决策 4：系统提示索引的粒度

**建议**：实现**分层披露**：
- 系统提示中只放 `name: description` 的压缩索引（避免 token 浪费）
- Agent 调用 `skill(name=...)` 时才加载全文
- 提供 `skills_list(category="...")` 支持按类别精细发现

---

## 附录：文件映射对照表

| hermes 文件                                           | if2Ai 等效文件                                   | 状态             |
| ----------------------------------------------------- | ------------------------------------------------ | ---------------- |
| `agent/prompt_builder.py::build_skills_system_prompt` | ❌ 不存在                                         | **缺失**         |
| `run_agent.py::skills_prompt 注入`                    | `commands/agent.rs` 系统提示构建                 | **缺失**         |
| `tools/skills_tool.py::skills_list`                   | ❌ 不存在                                         | **缺失**         |
| `tools/skills_tool.py::skill_view`                    | `tools/builtin/skill.rs::skill_tool_entry`       | ✅（命名不同）    |
| `tools/skill_manager_tool.py::skill_manage`           | `modules/skills/manager/` (后端)                 | ⚠️ 未暴露         |
| `agent/skill_commands.py`                             | `modules/skills/commands.rs`                     | ⚠️ 未接入         |
| `tools/skills_hub.py`                                 | `modules/skills/hub/` + `commands/skills_hub.rs` | ✅（部分占位）    |
| `tools/skills_guard.py`                               | `modules/skills/guard/`                          | ✅ 实现更完善     |
| `agent/skill_utils.py::config`                        | `modules/skills/config.rs`                       | ⚠️ 未接入调用链   |
| `agent/skill_utils.py::remote_passthrough`            | `modules/skills/remote_passthrough.rs`           | ⚠️ 未接入         |
| `hermes_cli/skills_hub.py`                            | `src/modules/skills/SkillsHubView.tsx`           | ✅（前端更完善）  |
| N/A（无此机制）                                       | `skill.rs::create_agent_skill_proposal_draft`    | ✅ if2Ai 独有优势 |
| N/A（无此机制）                                       | `skill.rs::SkillManifest + review 状态机`        | ✅ if2Ai 独有优势 |
