# If2Ai Agent 自主进化架构调研报告与设计文档

## Context

**问题**：If2Ai 当前虽然具备完整的 Agent 循环、分层记忆、多提供商 LLM 集成和浏览器自动化能力，但在 **自我修复（Self-Healing）**、**自我进化（Self-Evolution）**、**Token 极致效率** 三个维度上与前沿的开源 Agent 框架存在显著差距。

**触发**：需要参考 [browser-use/browser-harness](https://github.com/browser-use/browser-harness) 和 [GenericAgent](https://github.com/lsdefine/GenericAgent) 两个项目的核心创新，使 If2Ai 成为 **"Self-healing harness that enables LLMs to complete any task"**。

**预期成果 (2026-04 更新)**：
1. 一份完整的跨项目深度对比调研报告
2. 14 个模块的详细架构设计（12 个 spec 设计 + 2 个新发现已实现模块），覆盖全部 P0~P2 共 15 个差距点
3. Browser-Harness 十二大设计哲学精萃（Part 0，源码追溯）
4. 7 Phase / 24 Pack 的实施路线图 + 前端渐进增强 Pack
5. **全栈 UI/UX 对等设计**：31 个前端新建文件 + 11 个修改文件，基于 298 文件前端基线的渐进增强
6. P0~P2 差距点→后端模块→后端 Pack→前端组件 全栈追溯矩阵（含实现状态）
7. 最终落地为 `docs/design-docs/` 下的正式设计文档
8. **新增**: 2026-04 全栈代码重新扫描报告 (Part 1.3)，标记每个模块的实际代码实现状态

---

## Part 0: Browser-Harness 设计哲学精萃

> 本部分提炼自 [browser-harness](https://github.com/browser-use/browser-harness) 的 [README](https://github.com/browser-use/browser-harness#readme)、[SKILL.md](https://github.com/browser-use/browser-harness/blob/main/SKILL.md)、[daemon.py](https://github.com/browser-use/browser-harness/blob/main/daemon.py)、[helpers.py](https://github.com/browser-use/browser-harness/blob/main/helpers.py)、[admin.py](https://github.com/browser-use/browser-harness/blob/main/admin.py)、[run.py](https://github.com/browser-use/browser-harness/blob/main/run.py) 共计 ~600 行 Python 源码的深度解读。每一条原则都可追溯到源码中的具体实现模式。

### 0.1 总纲：什么是 "Self-healing harness that enables LLMs to complete any task"

Browser-Harness 的定义句来自其 README：

> *"The simplest, thinnest, **self-healing** harness that gives LLM **complete freedom** to complete any browser task. Built directly on CDP."*

核心洞见：**Harness 是控制层，Agent 是执行层。Harness 不写死任何任务路径，它只是提供一个可自修复、可自扩展的浏览器控制系统，让 Agent 自己去发现和创造完成任务的方法。** 类比：Harness 是"马具"——它控制马的走向，但跑多远全靠马自己。

这与 GenericAgent 的"5 层记忆驱动 + auto skill sedimentation"形成了互补的设计哲学：
- Browser-Harness **信任 Agent 的创造力**：Agent 可以在任务中途编辑 harness 本身（helpers.py）
- GenericAgent **信任记忆的积累**：每次成功任务自动沉淀为 skill 卡片
- 两者合一 = **If2Ai 的终极目标**：既有自愈的 harness 底座，又有自进化的 skill 沉淀引擎

### 0.2 十二大 Harness 设计原则

#### 原则 #1: 极简零框架 (Zero Framework, Direct CDP)

```
3 个依赖 (cdp-use, fetch-use, websockets) + ~600 行 Python = 完整的浏览器控制系统
没有任何中间层——Playwright/Puppeteer/Selenium 全部被绕过
CDP WebSocket 直连：一个 websocket 到 Chrome，中间什么都没有
```

**源码证据**：
- `daemon.py:6` — `from cdp_use.client import CDPClient`（唯一的 CDP 依赖）
- `daemon.py:142` — `self.cdp = CDPClient(url)` 直接建立 WebSocket
- `helpers.py:41-43` — `cdp()` 函数通过 Unix Socket 直接转发 CDP 命令，无任何抽象
- `run.py` — 仅 70 行，无 argparse 子命令，无额外控制层

**If2Ai 对标准则**：现有的 3-backend Smart Browser 架构（LocalRustCdp/BrowserUseMcp/BrowserUseCloud）是一个好的策略抽象层，但它不应该变成另一个"框架"。核心原则是：**CDP 直连应该始终是最快、最轻的路径**。

#### 原则 #2: Agent 自编辑 Harness (Agent Edits Its Own Harness)

```
helpers.py 头："""Browser control via CDP. Read, edit, extend -- this file is yours."""
Agent 可以在任务中途编辑 helpers.py，添加缺失的工具函数
修改在 daemon 重启后自动生效（或 reload 命令手动触发）
```

**源码证据**：
- `helpers.py:1` — `"""Read, edit, extend -- this file is yours."""`
- README 示例 — Agent 发现 `upload_file()` 缺失 → 编辑 helpers.py 添加 → 继续任务
- `admin.py:412-433` — `run_update()` 支持 agent 执行 `browser-harness --update -y` 自升级

**If2Ai 对标准则**：这不是"agent 能写代码"那么简单——这是 **harness 本身在运行时是可变的**。If2Ai 需要 Module E (Agent Self-Edit) 不仅让 agent 修改自己的工具实现，还能在 daemon 不重启的情况下热加载新能力。

#### 原则 #3: 进程即状态 (Process as State)

```
PID 文件 → 进程唯一性保证
Unix Socket → 进程间通信（无 HTTP 开销）
deque(maxlen=500) → 内存事件缓冲（FIFO 自动淘汰）
.log 文件 → append-only 诊断日志
无数据库，无配置系统，无状态管理框架
```

**源码证据**：
- `daemon.py:23-27` — `SOCK`, `LOG`, `PID`, `BUF=500`
- `daemon.py:111` — `self.events = deque(maxlen=BUF)` 自动封顶
- `daemon.py:234-239` — `already_running()` 通过 socket 连接检测 daemon 是否存活
- `daemon.py:246-247` — `open(PID, "w").write(str(os.getpid()))` 写入 PID

**If2Ai 对标准则**：Rust 后端天然有更好的状态管理能力，但理念是通用的——**最小化状态存储的复杂度**。If2Ai 当前有完整的 event sourcing + projection 系统，这是正确的方向（事件即状态），但不应在此基础上再加一层 ORM/数据库抽象。

#### 原则 #4: 自愈闭环 (Self-Healing Loop)

```
stale CDP session → 自动检测 → 自动 re-attach → 继续服务
daemon 崩溃 → 下次调用自动检测 → auto-spawn 新 daemon
Chrome 未启动 → 引导用户开启 chrome://inspect → 重试
所有恢复操作对调用者透明——ensure_daemon() 是终极幂等操作
```

**源码证据**：
- `daemon.py:191-197` — session stale 检测和自动 re-attach：
  ```python
  if "Session with given id not found" in msg and sid == self.session:
      log(f"stale session {sid}, re-attaching")
      if await self.attach_first_page():
          return {"result": await self.cdp.send_raw(method, params, session_id=self.session)}
  ```
- `admin.py:78-117` — `ensure_daemon()` 70 行代码处理所有恢复场景：
  - daemon 存活但 CDP 已死 → 重启 daemon
  - daemon 未运行 → subprocess 启动新 daemon
  - Chrome 未授权 → 打开 chrome://inspect 引导用户
  - 最多 2 次尝试 + 60s 超时

**If2Ai 对标准则**：现有的 `self_repair.rs`（仅 memory stuck-flag + broken-tool streak）太弱了。真正的自愈需要 **健康的检测 + 恢复的幂等性 + 透明的用户引导** 三重保障。Module C (Self-Healing Daemon) 就是对标的完整实现。

#### 原则 #5: 坐标优先交互 (Coordinate-First Interaction)

```
交互优先级：坐标点击 > 标签引用 > CSS 选择器
Input.dispatchMouseEvent 在 Chrome compositor 层执行 → 自动穿透 iframe/shadow DOM/cross-origin
截图 → 视觉定位 → 坐标点击 → 再截图验证 — 最可靠的浏览器交互范式
仅在目标无可见几何形状时（hidden input, 0×0 node）才降级到 DOM
```

**源码证据**：
- `helpers.py:72-92` — `click_at_xy(x, y)` 直接发送 CDP `Input.dispatchMouseEvent`
- `SKILL.md:126` — *"Clicking: capture_screenshot() → read the pixel → click_at_xy(x, y) → capture_screenshot() to verify"*
- `SKILL.md:131` — *"Iframe sites: click_at_xy(x, y) passes through; only drop to iframe DOM work when coordinate clicks are the wrong tool"*
- `SKILL.md:138` — *"Coordinate clicks default. Input.dispatchMouseEvent goes through iframes/shadow/cross-origin at the compositor level."*
- `SKILL.md:154` — *"Prefer compositor-level actions over framework hacks."*

**If2Ai 对标准则**：当前的 SmartBrowser 主要依赖 browser-use MCP 的 DOM 选择器。Module J (Coordinate-First Browser Strategy) 是必要升级，不仅是技术上的，更是**哲学上的——截图优先而非选择器优先**。

#### 原则 #6: 连接用户的浏览器 (Connect to User's Browser)

```
不启动自己的浏览器实例——直接 attach 用户正在用的 Chrome/Edge
自动发现 19 个可能的 Chrome profile 路径（macOS/Linux/Windows/Flatpak）
跨平台兼容：Darwin, Windows, Linux, 甚至 Flatpak 沙盒
支持 remote daemon 用于并行子 agent（每个有独立 BU_NAME 和浏览器）
```

**源码证据**：
- `daemon.py:28-50` — `PROFILES` 列表覆盖 19 个可能的浏览器 profile 路径
- `daemon.py:61-85` — `get_ws_url()` 自动发现 DevToolsActivePort
- `SKILL.md:21` — *"new_tab(url), not goto_url(url) — goto runs in the user's active tab"*
- `SKILL.md:138` — *"Connect to the user's running Chrome. Don't launch your own browser."*

**If2Ai 对标准则**：作为桌面应用，If2Ai 天然满足了"连接用户浏览器"的条件。但当前实现在会话断开和恢复方面还不够鲁棒。Module F (Browser Session Self-Healing) 应该借鉴 Browser-Harness 的用户浏览器连接模型。

#### 原则 #7: 技能即文件 (Skills as Files)

```
domain-skills/ — 网站特定的 Markdown 文件，agent 自己写
interaction-skills/ — 可复用的 UI 交互原语（对话/标签/iframe/上传等）
SKILL.md — agent 的"首先读取"入口
技能内容由 agent 贡献——"Skills are written by the harness, not by you"
```

**源码证据**：
- `SKILL.md:1-3` — 技能文件头 `--- name: browser-harness ---`
- `SKILL.md:23-29` — 领域技能和交互技能的组织方式
- `SKILL.md:92-106` — *"If you learned anything non-obvious... open a PR to domain-skills/"* — 贡献回写文化
- `helpers.py:50-53` — `goto_url()` 自动在 domain-skills 中搜索匹配的技能文件
- `README.md:55-58` — *"Skills are written by the harness, not by you"*

**If2Ai 对标准则**：当前 Skills 模块有 guard/hub/manager 等基础设施，但 skill **内容** 是外部/用户管理的。Module B (Skill Sedimentation) 和 Module G (Domain Knowledge Repository) 需要让 **agent 成为 skill 的第一作者**，而不是只让 agent 消费 skill。

#### 原则 #8: 最小上下文哲学 (Minimal Context)

```
Agent 只需要知道 helpers.py (~195 行) 里的函数
不需要注入框架文档、路由表、配置 schema
daemon 进程独立维护浏览器状态，agent 通过极简的 socket 协议通信
典型上下文窗口：~5K tokens
```

**源码证据**：
- `SKILL.md:8` — *"Read helpers.py — that's where the functions live"*
- `run.py:18` — `from helpers import *` 全部预导入
- `daemon.py:168-197` — daemon handler 仅 30 行，5 个 meta 命令
- 无框架文档注入到 agent 上下文

**If2Ai 对标准则**：当前 120K chars 的上下文有巨大的压缩空间。Module A (Context Compression Pipeline) 的目标是把上下文降到 ~30K，但这不是终点——Browser-Harness 证明 ~5K 就够了。**关键是让 agent 信任 harness 而不是阅读 harness。**

#### 原则 #9: 幂等性与宽恕 (Idempotence & Grace)

```
ensure_daemon() — 无条件可重试，无论当前状态如何
already_running() — 防止重复启动（检查 socket 是否存在）
所有 meta 命令都返回明确的 JSON（包括 {"error": "..."}）
失败时提供人类可读的修复指引（如 "click Allow in Chrome"）
```

**源码证据**：
- `admin.py:78` — `def ensure_daemon(wait=60.0, name=None, env=None): """Idempotent. Self-heals stale daemon..."""`
- `daemon.py:234-239` — `already_running()` socket 检测
- `daemon.py:145-152` — CDP 握手失败时的详细错误信息，包含修复建议
- `admin.py:196-199` — 优雅的 shutdown（meta:shutdown → SIGTERM fallback → cleanup）

**If2Ai 对标准则**：这是自愈系统的基石——**所有的修复操作都必须是幂等的**。Module C (Self-Healing Daemon) 的 RecoveryAction 设计必须确保：无论重试多少次，结果都是确定性的。

#### 原则 #10: 无管理层 (No Manager Layer)

```
无重试框架 → CDP 调用失败直接返回错误，由调用者决定如何处理
无会话管理器 → 一次只维护一个 session，stale 则重建
无配置系统 → .env 文件 + 环境变量（BU_NAME, BU_CDP_WS, BROWSER_USE_API_KEY）
无日志框架 → open(LOG, "a").write(f"{msg}\n") 的极简 append-only 日志
```

**源码证据**：
- `SKILL.md:143` — *"Don't add a manager layer. No retries framework, session manager, daemon supervisor, config system, or logging framework."*
- `daemon.py:9-21` — `.env` 加载仅 12 行，直接读文件 split
- `daemon.py:57-58` — `def log(msg): open(LOG, "a").write(f"{msg}\n")` 一行日志
- `daemon.py:191-197` — 错误处理无重试逻辑，stale 检测后直接重建

**If2Ai 对标准则**：这是 Browser-Harness 和 If2Ai 之间最大的**哲学差异**。If2Ai 作为完整桌面应用，管理层的存在是必要的（事件溯源、投影驱动、策略注册表）。但关键问题是：**管理层的复杂度不应泄露到 agent 的执行路径上**——agent 不应该为管理层付费（token/延迟/认知负载）。

#### 原则 #11: 贡献回写 (Always Contribute Back)

```
agent 每次发现网站的非显而易见模式 → 开放 PR 到 domain-skills/
不手写 skill → agent 生成的内容反映"真正在浏览器中可行"的东西
技能可积累 → 下一个 agent 不需要重新发现相同的陷阱
先搜索再行动 → "Search domain-skills/ first before inventing a new approach"
```

**源码证据**：
- `SKILL.md:92-106` — 完整的"贡献回写"指引
- `SKILL.md:105-116` — 域技能应捕获的内容清单
- `SKILL.md:117-121` — 域技能不应写的内容（像素坐标/运行叙述/密钥）
- `README.md:55-58` — *"Skills are written by the harness, not by you"*

**If2Ai 对标准则**：Module B (Skill Sedimentation) 是实现贡献回写的引擎。但关键不是技术实现，而是**文化与系统设计**——agent 应该默认将每次成功执行中的非显而易见发现沉淀为知识，而非仅在被指示时才做。

#### 原则 #12: 完整自由 (Complete Freedom)

```
Harness 不预设任何任务路径 → agent 可以自由探索
helpers.py 是活的 → agent 可以在任务中途扩展 harness 的能力
CDP 直连 → agent 可以发送任何 CDP 命令，不限于预封装函数
"Built directly on CDP. One websocket to Chrome, nothing between."
```

**源码证据**：
- `run.py:66` — `exec(args[1], globals())` agent 的 Python 代码直接 exec，无沙箱限制
- `helpers.py:41-43` — `cdp()` raw CDP 通道允许 agent 直接发送任何 CDP 命令
- `SKILL.md:33-35` — `browser-harness <<'PY' ... PY` 允许任意 Python
- README tagline — *"Complete freedom to complete any browser task"*

**If2Ai 对标准则**：作为桌面应用，If2Ai 需要权限控制（Autonomous/Interactive/Review 模式），这限制了"完整自由"。但这反而不是劣势——**If2Ai 的挑战是在安全约束内最大化 agent 的自由度**。Module E (Agent Self-Edit) 和 Module D (Constitutional Memory) 共同构成了"有护栏的自由"——宪法规则定义边界，自编辑能力提供自由。

### 0.3 设计哲学的融合：If2Ai = Browser-Harness × GenericAgent

| 维度       | Browser-Harness          | GenericAgent                     | If2Ai 理想状态                                       |
| ---------- | ------------------------ | -------------------------------- | ---------------------------------------------------- |
| **底座**   | CDP 直连 + daemon 自愈   | L0-L4 分层记忆 + 自动 skill 沉淀 | SmartBrowser 3 后端 + Daemon 自愈 (Module C)         |
| **自由**   | Agent 编辑 helpers.py    | Agent 遵循 SOP                   | Agent 在宪法边界内自编辑 (Module D+E)                |
| **知识**   | 领域技能 Markdown (静态) | 自动 skill 卡片 (动态)           | 域知识 + 自动沉淀 + 向量搜索 (Module B+G+I)          |
| **上下文** | ~5K tokens (极简)        | <30K tokens (分层注入)           | ~30K tokens 目标 (Module A+H)                        |
| **交互**   | 坐标优先 + 截图驱动      | 无浏览器                         | 坐标优先降级链 + HTML 简化 (Module J+L)              |
| **可靠性** | 进程级自愈               | 3-strike 升级                    | Daemon 自愈 + 分级升级 (Module C+AE-003)             |
| **记忆**   | deque(500) 事件缓冲      | 5 层记忆 (L0-L4)                 | 4 层预算 + 宪法 + checkpoint + 验证门 (Module D+H+K) |

### 0.4 对 If2Ai 架构实施的指导意义

1. **先建底座，再加 intelligence**：自愈 daemon 和浏览器会话恢复 (Phase 3) 应该在 skill 沉淀 (Phase 2) 之前——底座不稳，知识积累会被中断打断。
2. **上下文效率是杠杆支点**：120K→30K token 压缩 (Phase 1) 是 ROI 最高的投资——它不仅省钱，还提高连贯性和减少幻觉。
3. **Agent 必须能写，不只是读**：Skill 沉淀引擎 (Module B) + 域知识仓库 (Module G) 必须让 agent 成为知识的**生产者**，而非仅仅是消费者。
4. **坐标优先是最被低估的能力**：坐标点击穿透 iframe/shadow DOM 的能力在复杂 web app 自动化中是无价的。Module J 虽然标记为 P2，但实际影响可能是 P1 级别的。
5. **管理层不应对 agent 可见**：If2Ai 有优秀的策略注册表、成本守卫、轨迹捕获等管理层——但它们不应该给 agent 的每次请求增加 token 开销。Module A 的分层注入就是为此设计的。

---

## Part 1: 三项目深度对比调研报告

### 1.1 项目概览

| 维度         | Browser-Harness                       | GenericAgent               | If2Ai                      |
| ------------ | ------------------------------------- | -------------------------- | -------------------------- |
| **规模**     | ~1,300 行 Python                      | ~3,600 行 Python           | 数万行 Rust + TypeScript   |
| **架构哲学** | 极简零框架，CDP 直连                  | 极简自进化，分层记忆       | 完整应用框架，投影驱动     |
| **核心卖点** | Agent 可编辑代码自修复                | 每次任务自动沉淀 Skill     | 事件溯源 + 投影驱动桌面 UX |
| **依赖**     | 3 个 (cdp-use, fetch-use, websockets) | 极少 (requests, streamlit) | Tauri 2 + Rust 生态        |
| **目标用户** | 高级开发者/Agent 构建者               | 个人用户/多端接入          | 桌面 Agent 用户            |

### 1.2 八维深度对比矩阵

#### 维度 1: Self-Healing & 容错

| 能力              | Browser-Harness                                             | GenericAgent                                                  | If2Ai 现状                                                                                                 | 差距评级                |
| ----------------- | ----------------------------------------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ----------------------- |
| **进程级恢复**    | Daemon 模式自动重启；PID 锁；心跳探针                       | 3-strike 升级到人工                                           | `self_repair.rs` 仅有 memory-ticker stuck-flag 修复 + broken-tool streak 检测(阈值4)，无进程级 daemon 恢复 | **P0 - 关键差距**       |
| **会话/状态恢复** | Stale session 自动检测重建；Chrome 崩溃→自动重启+恢复标签页 | 每成功一步保存 working checkpoint；重启从最后 checkpoint 恢复 | `resume_cursor` + `CompactionResult` 存在，可中途恢复流，但无崩溃后自动重启                                | **P1 - 高**             |
| **Provider 韧性** | 最小化（daemon 重启重试）                                   | 3 次分级重试+模型降级                                         | `resilience.rs` 有熔断器(5 次失败→30s 冷却)、指数退避、SHA256 缓存、主→备用链路由                          | **已覆盖 (If2Ai 领先)** |
| **工具失败处理**  | Agent 编辑 helpers.py 自修复                                | 3 次重试→模型降级→ask_user                                    | `record_tool_outcome` 追踪 streak；`REPEATED_TOOL_BATCH_LIMIT=3` 触发终结。无自编辑能力                    | **P1 - 高**             |
| **预算/成本守卫** | 无                                                          | Token 预算检查                                                | `cost_guard.rs` 每日/每小时/每会话限制                                                                     | **已覆盖**              |

**结论**：If2Ai 有最佳的 Provider 韧性，但关键缺少 *进程 daemon* 和 *agent 自编辑修复* 模式。

#### 维度 2: Self-Evolution & Skill 沉淀

| 能力                     | Browser-Harness     | GenericAgent                                                | If2Ai 现状                                                                                                 | 差距评级          |
| ------------------------ | ------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ----------------- |
| **任务后自动创建 Skill** | 手动写 skill        | 每次任务自动生成 skill 卡片 (步骤+触发条件)，存入 `skills/` | 无自动 skill 创建。Skill 系统仅支持 *发现+管理*(`skills/manager/`, `skills/hub/`)，不支持生成              | **P0 - 关键差距** |
| **策略注册表**           | 无                  | 无                                                          | 完整的 `strategy_registry.rs`: Draft→Candidate→Compared→Recommended→Active→RolledBack 生命周期             | **If2Ai 领先**    |
| **反射循环**             | 无                  | 定期 working-memory 清理 + L4 归档                          | `reflection_loop.rs` 每 N turn 运行 LLM 反射；`learned_traits.rs` 蒸馏跨会话反复出现的观察                 | **If2Ai 领先**    |
| **Skill 语义搜索**       | Markdown 关键词扫描 | 105K+ 预训练技能卡 + 向量相似度搜索                         | `skill_find`/`skill_search`/`skill_view` 工具存在。`work_loop.rs` 做关键词 token 匹配评分。无向量/嵌入检索 | **P1 - 高**       |
| **轨迹捕获**             | 无                  | 仅任务日志                                                  | `trajectory.rs`: ShareGPT JSONL 导出, `TrajectoryCompressor`, `trajectory_score.rs` 多轴评分               | **If2Ai 领先**    |

**结论**：If2Ai 有最成熟的 *策略治理*（注册表+推广门+轮次），但完全缺少 *生成侧*——将成功任务自动转化为可复用 skill 卡片。这是 GenericAgent 的杀手级特性。

#### 维度 3: Token 效率

| 能力                   | Browser-Harness            | GenericAgent                                                              | If2Ai 现状                                                                                                                                                            | 差距评级          |
| ---------------------- | -------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| **典型上下文窗口使用** | ~5K (最小化上下文)         | <30K (同类的 1/10)                                                        | ~120K (`MAX_REQUEST_CHAR_BUDGET=120_000`)                                                                                                                             | **P0 - 关键差距** |
| **消息修剪**           | Daemon deque(500) 自动封顶 | 消息压缩+修剪管线；分层记忆注入替换原始历史                               | `compact.rs` 在 token 阈值做会话压缩；`stream_preflight.rs` 裁剪到 `MAX_REQUEST_MESSAGE_COUNT=180`。但无消息级压缩——完整消息直到被逐出                                | **P0 - 关键差距** |
| **分层注入**           | 3 层记忆                   | L0(宪法)→L1(迷你索引)→L2(事实)→L3(SOP)→L4(归档)——每 turn 仅注入 ~30 行 L1 | 4 槽预算 (System 10%/Episodic 20%/Semantic 30%/Working 40%，总 4K tokens)。但 working memory 就有 8 turns@1600 tokens，实际请求组装器绕过了预算限制直接用原始消息历史 | **P1 - 高**       |
| **工具结果压缩**       | 仅坐标（极小输出）         | 工具输出截断+压缩后才加入历史                                             | `summarize_tool_result_for_model` 存在但截断很粗糙（基于字符数）；无 LLM 辅助结果摘要                                                                                 | **P1 - 中**       |

**结论**：最大差距。If2Ai 每次请求发送的 token 比必要的多 4-7 倍。GenericAgent 的 L1 "迷你索引 ≤30 行"模式和 Browser-Harness 的最小上下文哲学代表了范式转换。现有 `ContextBudget` 基础设施是正确的抽象——它需要被接入为实际请求组装的 *硬约束*，而不仅是建议。

#### 维度 4: 浏览器自动化

| 能力             | Browser-Harness                                    | GenericAgent                                | If2Ai 现状                                                                                           | 差距评级               |
| ---------------- | -------------------------------------------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------- |
| **后端选项**     | CDP 直连（零框架）                                 | Chrome CDP + Extension bridge（保留登录态） | 3 后端: `LocalRustCdp`/`BrowserUseMcp`/`BrowserUseCloud` + 策略驱动升级                              | **If2Ai 领先（广度）** |
| **坐标优先交互** | 坐标点击→标签引用→CSS 选择器（降级链）             | 无                                          | `SmartBrowserCommandKind` 有 Click/TypeText/Scroll 但无坐标优先策略。依赖 browser-use MCP 的元素选择 | **P2 - 中**            |
| **自修复会话**   | Stale 标签→自动关闭+重建；Chrome 权限对话→自动引导 | 无                                          | 无浏览器会话健康监控或自动恢复。`cold_state.rs` 存在但是被动的                                       | **P1 - 高**            |
| **风险门控**     | 无（信任 agent）                                   | 无                                          | `SmartBrowserRisk` 枚举 + `evaluate_sensitive_action` + `evaluate_cloud_escalation` 策略门           | **If2Ai 领先**         |

#### 维度 5: 记忆系统

| 能力                  | Browser-Harness      | GenericAgent               | If2Ai 现状                                                                                                        | 差距评级                 |
| --------------------- | -------------------- | -------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------ |
| **层级深度**          | 3 (进程/daemon/文件) | 5 (L0-L4)                  | 4 槽 (System/Episodic/Semantic/Working) + pinned + learned_traits + reflection + conversation_recall_vector + HRR | **If2Ai 领先（丰富度）** |
| **宪法层 (L0)**       | 无                   | 不可变 L0 规则始终注入     | 无"宪法级"记忆层。System prompt 有 identity + scenario 但没有在 prompt 重写时幸存的不可变规则                     | **P1 - 高**              |
| **No-exec-no-memory** | 无                   | 仅经验证的执行结果进入记忆 | `memory_store` 工具根据 agent 请求写入，不验证执行                                                                | **P2 - 中**              |
| **衰减/驱逐**         | Deque 封顶           | 阈值后归档                 | Weibull 重要度衰减 (`apply_importance_decay`), 信任评分反馈, 优先级槽驱逐                                         | **If2Ai 领先**           |
| **语义检索**          | 关键词               | 向量相似度                 | `VectorMemoryProvider` (FastEmbed + LanceDB) + `conversation_recall_vector.rs`                                    | **If2Ai 领先**           |

#### 维度 6: 工具系统

| 能力             | Browser-Harness                      | GenericAgent         | If2Ai 现状                                                          | 差距评级               |
| ---------------- | ------------------------------------ | -------------------- | ------------------------------------------------------------------- | ---------------------- |
| **工具数量**     | 24 个交互原语                        | 9 个原子工具         | 40+ 内置 + MCP 可扩展                                               | **If2Ai 领先（广度）** |
| **原子性**       | 高（纯原语）                         | 极高（9 个正交工具） | 中等——许多专用变体 (6 记忆工具, 5 cron 工具, 7 skill 工具)          | **P2 - 中**            |
| **动态工具注册** | Agent 编辑 `helpers.py`→下次自动加载 | 固定集               | `ToolRegistry` 运行时注册 + MCP stdio/SSE 扩展。但无 agent 自创工具 | **P1 - 高**            |

#### 维度 7: Agent 自主性

| 能力           | Browser-Harness                       | GenericAgent                    | If2Ai 现状                                                                                                     | 差距评级     |
| -------------- | ------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------ |
| **任务规划**   | 隐式（skill 引导）                    | SOP 驱动（L3 记忆提供逐步配方） | `WorkLoopKind` 分类: DirectAnswer/DirectExecute/PlanThenConfirm/AutonomousWork/SpecializedSurface              | **基本持平** |
| **失败升级**   | Daemon 重启                           | 3 次→模型降级→ask_user          | `REPEATED_TOOL_BATCH_LIMIT=3`→强制终结；`max_iterations_reached`→带 resume cursor 结束                         | **基本持平** |
| **域知识编码** | 73 个 Markdown 域技能 + 19 个交互原语 | 105K+ 预训练技能卡              | Skills 模块有 guard (60+ 威胁模式), hub (多源市场), sync。但 skill *内容* 是外部/用户管理的，不是 agent 创作的 | **P1 - 高**  |

### 1.3 关键发现总结

**If2Ai 的优势（不需要追加）**：
- Provider 韧性（熔断器 + 指数退避 + 缓存 + 主备链路由）
- 策略治理（Strategy Registry 完整生命周期）
- 记忆丰富度（4 层 + HRR + Weibull 衰减 + 向量检索）
- 浏览器策略广度（3 后端 + 风险门控 + 升级决策）
- 轨迹捕获（ShareGPT 导出 + 压缩 + 多轴评分）
- 投影驱动 UX（事件溯源 → 前端只读投影）
- 权限框架（Autonomous/Interactive/Review 模式）
- 成本守卫（daily/hourly/session 限制）

**If2Ai 的关键差距（需要追加）**：

| 优先级 | 差距                             | 参考源                                 | 对应 Harness 原则                                | 影响                                     |
| ------ | -------------------------------- | -------------------------------------- | ------------------------------------------------ | ---------------------------------------- |
| **P0** | Token 效率: 120K→30K             | GenericAgent L0-L4 分层注入 + 消息压缩 | #8 最小上下文哲学                                | 成本降低 4-7x，幻觉减少，成功率提升      |
| **P0** | Skill 自动沉淀                   | GenericAgent 任务后自动生成 skill 卡片 | #7 技能即文件 / #11 贡献回写                     | Agent 能力随使用持续增长                 |
| **P0** | Self-Healing Daemon              | Browser-Harness daemon 模式            | #3 进程即状态 / #4 自愈闭环 / #9 幂等宽恕        | 进程级自动恢复，真正的无人值守           |
| **P1** | Constitutional Memory (L0)       | GenericAgent 不可变行为宪法            | #12 完整自由（有护栏的自由）                     | Agent 行为一致性的基石                   |
| **P1** | Agent 自编辑工具能力             | Browser-Harness Agent 编辑 helpers.py  | #2 Agent 自编辑 Harness / #12 完整自由           | 工具失败时自修复而非放弃                 |
| **P1** | 浏览器会话自修复                 | Browser-Harness stale session 自动恢复 | #4 自愈闭环 / #6 连接用户浏览器                  | 浏览器崩溃自动恢复                       |
| **P1** | Skill 向量语义搜索               | GenericAgent 105K+ 技能卡 + 向量搜索   | #7 技能即文件（检索升级）                        | Skill 匹配从关键词升级到语义             |
| **P1** | 域知识编码+Agent 创作 Skill 内容 | Both: BH 73 域技能 + GA 105K 技能卡    | #7 技能即文件 / #11 贡献回写                     | Agent 不仅管理 Skill 还能创作域知识      |
| **P1** | 动态工具自创注册                 | Browser-Harness Agent 编辑 helpers.py  | #2 Agent 自编辑 Harness                          | Agent 发现缺失能力时可自创工具           |
| **P1** | Working Checkpoint 短期记忆注入  | GenericAgent update_working_checkpoint | #8 最小上下文哲学（关键信息防丢失）              | 每轮 <200 tokens 关键信息始终在场        |
| **P1** | 分层注入替代原始历史             | GenericAgent L0-L4 分层注入            | #8 最小上下文哲学 / #10 无管理层（不泄露复杂度） | 预算槽作为硬约束而非建议                 |
| **P2** | 执行验证记忆写入                 | GenericAgent No-Exec-No-Memory         | #9 幂等宽恕（只写入确定性的结果）                | 防止幻觉数据污染记忆                     |
| **P2** | 坐标优先浏览器交互               | Browser-Harness coordinate-first       | #5 坐标优先交互                                  | 穿透 iframe/shadow DOM                   |
| **P2** | 浏览器内容 HTML 简化             | GenericAgent simphtml.py               | #1 极简零框架 / #8 最小上下文                    | 网页 token 减少 90%+，浏览器任务成本骤降 |
| **P2** | 工具原子性整合                   | GenericAgent 9 原子工具哲学            | #10 无管理层（工具层极简化）                     | 减少工具选择困惑，提升决策质量           |

---

### 1.3 2026-04-28 全栈代码重新扫描：实现现状对照

> ⚠️ **2026-04-30 校准注**：本节快照早于 commit `aa2deed`（"land complete 41-pack rollout"）。
> 当前真值参见 [`docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md`](../../docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md)
> 与 [`docs/superpowers/plans/2026-04-30-agent-evolution-truth-loop.md`](../../docs/superpowers/plans/2026-04-30-agent-evolution-truth-loop.md)。
> 本节下方的「❌ 待实现」/「🟡 部分」标记仅作为历史；模块 A/B/D/G/J/L 的算法/类型/调用站均已落地，
> 余下断层是 4 个 `landed-stub`（DW-001/DW-002/DW-004/WU-002）等待 iteration 3 的
> `WU-009-deep-utility-handle-threading` Pack 串入真实 `UtilityLlm` / `ProviderManager` / `KnowledgeStore` handle。

> 用户对 codebase 做了大量代码更新后，进行全栈重新扫描。以下对照每个 spec 模块的**设计目标 vs 实际代码实现**。
>
> **真值优先级**：本表为 2026-04-30 二次校准；若与上方历史段落冲突，以本表及 [`docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md`](../../docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md) §3 为准。术语 **`landed-stub`**：调用站 + 类型/算法骨架已落地，但生产依赖（真实 `UtilityLlm`、`ProviderManager`/`McpServerManager` 探针、`KnowledgeStore` 等）仍为占位——对 Exit Gate 计为 **not-done**。

#### 后端扫描摘要（`src-tauri/src/modules` 下约 495 个 `.rs` 文件量级；LOC 未每次重算）

| 模块               | 代码锚点（实际布局）                                                                                                                                                   | 实现状态                                                                                                                                                                                                                                                |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A (上下文压缩)     | `runtime/context_compression/`（`mod.rs`、`digester.rs`、`mini_index.rs`）+ `stream_preflight.rs` / `preflight_hooks.rs` / `budget.rs`                                 | 🟢 **算法与主路径已落地**；⚠️ **DW-002 `landed-stub`**：`stream_task` 生产 digester 仍接 `MockUtilityLlm`，真实消息摘要为 identity，直至 `UtilityLlm` handle 下传                                                                                         |
| B (Skill 沉淀)     | `skills/sedimentation/` + `stream_finalize.rs` 内 `run_sedimentation_pipeline`                                                                                         | 🟢 **done**（WU-003）；设计稿中的单文件 `sedimentation.rs` 已演化为子目录模块                                                                                                                                                                            |
| C (自愈 Daemon)    | `runtime/daemon/{mod,health_check,recovery}.rs` + `self_repair.rs` 兼容层 + `evolution_emitter.rs`；`desktop_host/setup.rs` 内 `spawn_self_healing_daemon_with_extras` | 🟡 **daemon 常驻路径已真化（iter-4/6）**：`BrowserRegistryProbe` + `ProviderApiKeyProbe` 与 SH-001 baseline **同 registry** 轮询；⚠️ **仍剩 stub 线程**：MCP 活性探针、`ProviderManager` 深度 liveness 等见 `setup.rs` 注释与 GAP §9.3 / iteration 7 候选 |
| D (宪法记忆)       | `skills/guard/constitution.rs` + `prompt_planner` Constitution block                                                                                                   | 🟢 **guard/宪法与注入路径已落地**（非附录中的 `runtime/constitution.rs` 单文件）                                                                                                                                                                         |
| E (Agent 自编辑)   | `learning/`、`stream_tool_execution.rs`、self-edit scanner helper；`setup.rs` spawn                                                                                    | 🟡 **helper + tick 已落地**；⚠️ **DW-001 `landed-stub`**：`ConstEmbedder` + `MockUtilityLlm`，扫描输入为空，无真实提案                                                                                                                                    |
| F (浏览器会话自愈) | `smart_browser/runtime.rs` + `browser/session.rs` 等 3-backend                                                                                                         | 🟡 **会话与运行时就绪**；daemon 内浏览器真探针仍为 `StubBrowserProbe`（GAP §4）                                                                                                                                                                          |
| G (域知识仓库)     | `skills/domain_knowledge/` + `stream_finalize.rs` `run_domain_knowledge_contributor`                                                                                   | 🟢 **finalize 钩子 done**；⚠️ **DW-004 `landed-stub`**：`work_loop`  advisory 仍 `MockKnowledgeStore`                                                                                                                                                     |
| H (工作检查点)     | `runtime/working_checkpoint.rs` + `projection.rs` / `recoverability.rs`                                                                                                | 🟢 **持久化与恢复基础已落地**                                                                                                                                                                                                                            |
| I (向量搜索)       | `skills/vector_index.rs` + `memory/embedding/`                                                                                                                         | 🟡 **索引与嵌入基础设施就绪**；与 work_loop 的语义检索深度以代码为准                                                                                                                                                                                     |
| J (坐标优先策略)   | `smart_browser/runtime.rs` `decide_browser_click_strategy`                                                                                                             | 🟢 **done**（WU-006 / DW-005）                                                                                                                                                                                                                           |
| K (验证门)         | `memory_quality_gate.rs`、`memory_write_policy.rs` 等                                                                                                                  | 🟡 **策略分散落地**；独立 `memory/verification_gate.rs` 与否以仓库为准                                                                                                                                                                                   |
| L (HTML 简化器)    | `smart_browser/runtime.rs` `simplify_browser_result_text` / `adaptive_simplify`                                                                                        | 🟢 **done**（WU-006）                                                                                                                                                                                                                                    |

**🆕 新发现的已实现能力（spec 未覆盖）**：

| 能力                         | 代码位置                              | LOC    | 说明                                                                            |
| ---------------------------- | ------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| **Jiaochang Audio 插件沙箱** | `jiaochang_audio/mod.rs`              | 1,596  | Node.js VM isolate 运行游戏/模拟插件，含音频管理                                |
| **MCP Workbench & 工具发现** | `mcp_stdio/manager.rs`                | 765    | MCP server 生命周期管理、tool discovery、workbench activity 记录                |
| **Control Plane（控制面）**  | `control_plane/` (8 files)            | ~1,500 | ingress_classifier、tool_execution_broker、audit、boundary_resolver             |
| **Learning 策略引擎**        | `learning/` (18 files)                | ~3,000 | strategy_registry、reflection、trajectory_score、self_model、failure_clustering |
| **会话标题生成**             | `session/manager.rs`                  | 617+   | emoji 推断 + LLM 驱动的会话标题生成                                             |
| **Git 集成**                 | `git/` (17 files)                     | ~2,000 | branch/commit/PR/issue/worktree + slash commands                                |
| **Agent Loop 委托**          | `agent_loop_delegate.rs`              | 235    | work loop 决策可委托给策略引擎                                                  |
| **Execution Mode Preview**   | `control_plane/ingress_classifier.rs` | ~400   | M2.6 执行模式分类器                                                             |

#### 前端扫描摘要（298 个 TS/TSX 文件）

| 维度                   | Spec 设计                  | 实际代码                                                                       | 差异         |
| ---------------------- | -------------------------- | ------------------------------------------------------------------------------ | ------------ |
| **Store 数量**         | 3 (bootstrap/chat/session) | 7 (bootstrap/chat/session/conversation-slice/runtime-projection/browser-slice) | 多 4 个      |
| **API Facades**        | 未详述                     | 11 个 domain-scoped modules                                                    | 完整架构     |
| **Runtime Projection** | 提及，未量化               | 45 files, ~3000 LOC                                                            | 比预期更复杂 |
| **Jiaochang 前端**     | 未覆盖                     | 60 files (游戏可视化+音频+i18n)                                                | 全新模块     |
| **Voice 组件**         | 未覆盖                     | AgentVoiceIndicator, SttButton, TtsProfilePicker                               | 已实现       |
| **Spec 设计"新"组件**  | 24 个 planned              | 0 个已实现, 但部分有基线(ExecutionModePill, SmartBrowserCockpit 已存在)        | 基线更成熟   |

#### 关键结论（2026-04-30）

1. **后端 A/B/D/G/H/J/L 的算法与主路径已落地**，目录布局与设计附录中的「单文件新建」不完全一致（多为子目录模块）；**C/E/F/I/K** 仍有 **partial** 或 **`landed-stub`** 依赖线程，集中修复 Pack 建议名 **`WU-009-deep-utility-handle-threading`**（见 GAP 报告 §4）。
2. **不再成立（历史）**：「12 模块中 5 个完全待建」——请以本表与 GAP §3 替换该判断。
3. **仍存在 6 个 spec 未覆盖的新能力**（M/N 等）：Module M、N 章节保留。
4. **前端**：`src/components/chat/evolution/` 已落地 6 个 Dev Drawer 子组件并消费 `evolutionEventStore`（GAP §3.3）；Part 7 中「6 类 UI 全缺」的表述已过时，见 §7.1 更新段。

---

## Part 2: 十二个新模块 + 二个新发现模块的详细架构设计

> 覆盖全部 P0~P2 差距点。模块 A~F 为前版已有设计，模块 G~L 为上次补充，模块 M~N 为 2026-04 全栈扫描新发现。
>
> **实现状态图标**: 🟢 大部分已实现 | 🟡 部分已实现 | 🔴 待实现 | 🆕 新发现（spec 新增）
>
> **设计哲学**：每个模块的设计都根植于 Part 0 中提炼的 Browser-Harness 十二大设计原则。下表展示原则到模块的映射关系：

| Harness 原则            | 主要承载模块                                                   | 承载方式                         |
| ----------------------- | -------------------------------------------------------------- | -------------------------------- |
| #1 极简零框架           | Module L (HTML 简化器, 200K→35K)                               | 不做过度抽象，直接简化原始 HTML  |
| #2 Agent 自编辑 Harness | Module E (Self-Edit Engine)                                    | Agent 可在运行时修改工具实现     |
| #3 进程即状态           | Module C (Daemon)                                              | PID 锁 + 状态机 + 事件驱动恢复   |
| #4 自愈闭环             | Module C (Daemon) + Module F (Browser Session)                 | 幂等的健康检测 + 透明恢复        |
| #5 坐标优先交互         | Module J (Coordinate-First Strategy)                           | 降级链: 坐标→标签→CSS 选择器     |
| #6 连接用户浏览器       | Module F (Browser Session Health)                              | 会话健康检测 + 自动 re-attach    |
| #7 技能即文件           | Module B (Sedimentation) + Module G (Domain Knowledge)         | Agent 自动生成 skill/domain 文件 |
| #8 最小上下文哲学       | Module A (Context Compression) + Module H (Working Checkpoint) | 120K→30K + <200 token key_info   |
| #9 幂等宽恕             | Module C (Daemon Recovery)                                     | 所有恢复操作都是幂等的           |
| #10 无管理层            | Module A (Tier Budget)                                         | 管理层不泄露到 agent 上下文      |
| #11 贡献回写            | Module B + G + K (Verification Gate)                           | Agent 默认沉淀发现，经验证后写入 |
| #12 完整自由            | Module D (Constitution) + Module E (Self-Edit)                 | 宪法定义边界，边界内完全自由     |

### Module A: 上下文压缩管线 (Context Compression Pipeline) — P0 🟢 已落地（⚠️ DW-002 `landed-stub`）

**实际代码现状（2026-04-30）**: `runtime/context_compression/` 提供 `ContextTier`、`TierBudgetAllocation`、`compress_for_request`、`MessageDigester`、`build_mini_index` 等；`stream_preflight` / `preflight_hooks` 已接线。**生产路径缺口**：`stream_task` 侧 digester 仍使用占位 `UtilityLlm`（身份变换），真压缩待 **`WU-009-deep-utility-handle-threading`**。下方设计稿中的单文件路径 `context_compression.rs` 已演化为目录模块，类型以仓库为准。

**定位**：将 If2Ai 从"全量发送"转变为"最小可行上下文"架构。目标：将平均请求从 ~120K chars 压缩到 ~30K chars，不丢失任务连贯性。

**Harness 原则锚定**：#8 最小上下文哲学 + #10 无管理层。关键洞察：Agent 应该**信任 harness 而非阅读 harness**——harness 的复杂度不应泄露到 agent 的执行路径上。

**核心数据结构**（实现位于 `src-tauri/src/modules/runtime/context_compression/`，下列片段为设计参考）:

```rust
/// 上下文层级（对应 GenericAgent L0-L4 模式）
pub enum ContextTier {
    Constitution,   // L0: 不可变行为约束，永不压缩
    MiniIndex,      // L1: ≤30 行的会话索引
    ActiveFacts,    // L2: 当前活跃事实（环境/配置/路径）
    TaskSOP,        // L3: 当前任务的 SOP
    RecentHistory,  // L4: 最近的对话历史
}

/// 压缩后的消息表示
pub enum CompressedMessage {
    Full(InputMessage),
    Summarized {
        digest: String,
        original_token_count: usize,
        compressed_token_count: usize,
    },
}

/// 每条消息的压缩决策
pub enum CompressionDecision {
    Keep,
    Compress,
    Drop,
    Merge(Vec<String>),  // message IDs to merge
}

/// 层级预算分配
pub struct TierBudgetAllocation {
    pub constitution: usize,     // ~200 tokens (不可压缩)
    pub mini_index: usize,       // ~500 tokens
    pub active_facts: usize,     // ~1000 tokens
    pub task_sop: usize,         // ~2000 tokens
    pub recent_history: usize,   // 剩余预算
}
```

**关键接口**:

```rust
/// 主入口：在每次 LLM 调用前由 stream_preflight.rs 调用
pub fn compress_for_request(
    messages: &[InputMessage],
    total_budget: usize,
    tier_config: &TierBudgetAllocation,
) -> CompressedContext;

/// LLM 辅助消息摘要（复用现有 UtilityLlm trait）
pub async fn digest_messages(
    messages: &[InputMessage],
    target_tokens: usize,
) -> String;

/// 生成 ≤30 行的会话迷你索引（GenericAgent L1 模式）
pub fn build_mini_index(
    session_history: &[InputMessage],
    current_turn: usize,
) -> String;
```

**集成点**:
- 从 `stream_preflight::build_iteration_request` 调用——替换当前仅基于字符数的裁剪
- 消费现有的 `ContextBudget`（来自 `runtime/budget.rs`）作为 token 分配输入
- 使用 `estimate_tokens`（cl100k_base BPE）做准确计数
- 压缩失败时降级到当前裁剪逻辑（非致命降级）

**关键修改文件**:
- **已建**: `src-tauri/src/modules/runtime/context_compression/`（`mod.rs` 等）
- **修改**: `src-tauri/src/modules/application/turn_service/stream_preflight.rs` (接入压缩器)
- **修改**: `src-tauri/src/modules/runtime/budget.rs` (添加层级预算常量 + `ContextTier` 枚举)

---

### Module B: Skill 沉淀引擎 (Skill Sedimentation Engine) — P0 🟢 已落地

**实际代码现状（2026-04-30）**: `skills/sedimentation/`（含 `dedup.rs`）+ `stream_finalize.rs` 内 `run_sedimentation_pipeline` 已在生产 finalize 路径调用（WU-003）。设计稿中的顶层 `sedimentation.rs` / `deduplication.rs` 已演化为子目录布局。

**定位**：每次成功的自主工作循环完成后，自动将任务执行蒸馏为可复用的 skill 卡片。这是 GenericAgent "No Execution, No Memory" 原则应用于 skill 创建。

**核心数据结构**（实现见 `skills/sedimentation/`；下列片段为设计参考）:

```rust
/// 沉淀触发器——捕获 skill 生成的条件
pub struct SkillSedimentationTrigger {
    pub task_outcome: String,           // "completed" | "partial_success"
    pub tool_sequence: Vec<ToolInvocationRecord>,
    pub user_message: String,
    pub accumulated_text: String,
    pub session_id: String,
    pub run_id: String,
}

/// 工具调用记录
pub struct ToolInvocationRecord {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub result_summary: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// 沉淀出的 skill 卡片
pub struct SedimentedSkill {
    pub name: String,
    pub description: String,
    pub trigger_patterns: Vec<String>,     // 触发此 skill 的关键词/模式
    pub steps: Vec<SkillStep>,
    pub confidence: f64,                    // 基于执行成功率
    pub source_session: String,
    pub source_run: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Skill 步骤
pub struct SkillStep {
    pub tool_name: String,
    pub tool_args_template: String,
    pub expected_outcome: String,
    pub fallback_hint: Option<String>,
}

/// 沉淀决策
pub enum SedimentationDecision {
    Skip { reason: String },
    Generate { skill: SedimentedSkill },
    MergeInto { existing_skill_id: String, delta: SkillDelta },
}
```

**关键接口**:

```rust
/// 在每次成功 turn 结束时从 stream_finalize.rs 调用
pub async fn evaluate_for_sedimentation(
    trigger: &SkillSedimentationTrigger,
) -> SedimentationDecision;

/// LLM 调用：将执行轨迹蒸馏为 skill 卡片
pub async fn synthesize_skill(
    trigger: &SkillSedimentationTrigger,
) -> Result<SedimentedSkill>;

/// 与现有 skill 仓库去重
pub async fn find_similar_skill(
    skill: &SedimentedSkill,
    existing_skills: &[SkillMetadata],
) -> Option<SimilarSkillMatch>;

/// 持久化 skill 卡片
pub async fn persist_skill(skill: &SedimentedSkill) -> Result<String>;
```

**沉淀条件**（仅在以下条件全部满足时触发）:
1. `task_outcome == "completed"` 或 `task_outcome == "partial_success" && has_successful_tool == true`
2. 工具序列中至少有 2 次成功的非 sleep 工具调用
3. 工具序列的去重工具名数量 >= 2（排除单一工具的简单任务）
4. 与现有 skill 的相似度 < 0.85（避免重复生成）

**集成点**:
- 从 `stream_finalize::finalize_stream_task` 调用，在 `TaskOutcomeResolver::resolve` 产出 `completed` 或 `partial_success` 之后
- 读取 `FinalizeStreamInputs` 中的工具调用轨迹
- 写入现有的 `skills/manager/` 目录结构
- 可选地输入 `strategy_registry.rs` 作为 `StrategySource::Reflection` 候选

**关键修改文件**:
- **已建**: `src-tauri/src/modules/skills/sedimentation/`（含去重逻辑）
- **修改**: `src-tauri/src/modules/application/turn_service/stream_finalize.rs` (添加沉淀钩子，约在第 662 行 `dispatch_after_turn` 后)
- **修改**: `src-tauri/src/modules/skills/mod.rs` (注册新子模块)

---

### Module C: Self-Healing Daemon (自修复守护进程) — P0 🟡 框架已落地（⚠️ 残余 stub 见 GAP §9.3）

**实际代码现状（2026-04-30）**: `desktop_host/setup.rs` 在 **truth-loop iter-4+** 已改为 `spawn_self_healing_daemon_with_extras`：同 registry 内运行 **`BrowserRegistryProbe`**（替代孤立 `StubBrowserProbe`）与 **`ProviderApiKeyProbe`**（iter-6，凭据 env 扫描）。**仍待迭代**：MCP 进程探针、`ProviderManager` 工厂式 liveness 等——以 `setup.rs:136-138` 注释与最新 GAP 报告为准，勿复用 rollout 前「仅 legacy 两检查」叙述。

**定位**：一个轻量级 supervisor 层，监控 agent 健康并自动从崩溃、卡死、降级状态恢复。

**Harness 原则锚定**：#3 进程即状态 + #4 自愈闭环 + #9 幂等宽恕。核心约束：**daemon 的所有恢复操作必须是幂等的**——无论重试多少次，结果都是确定性的。参考 Browser-Harness `ensure_daemon()` 的设计：两阶段恢复（socket 探测→subprocess 重启），所有操作对调用者透明。

**核心数据结构** (`src-tauri/src/modules/runtime/daemon/mod.rs`):

```rust
/// Daemon 状态机
pub enum DaemonState {
    Healthy,
    Degraded { reason: String, since: Instant },
    Recovering { attempt: u32, max_attempts: u32 },
    Failed { reason: String },
}

/// 健康检查定义
pub struct HealthCheck {
    pub name: String,
    pub check_fn: Box<dyn Fn() -> HealthStatus + Send + Sync>,
    pub interval: Duration,
    pub failure_threshold: u32,
    pub consecutive_failures: AtomicU32,
}

/// 健康状态
pub enum HealthStatus {
    Healthy,
    Warning(String),
    Critical(String),
}

/// 恢复动作
pub enum RecoveryAction {
    RestartProvider,
    ClearStuckState,
    ResetBrowserSession,
    RestartMcpServer { server_name: String },
    EscalateToUser { message: String },
    DegradeGracefully { dropped_capability: String },
}

/// Daemon 事件（用于 event log + projection）
pub enum DaemonEvent {
    HealthCheckPassed { name: String },
    HealthCheckFailed { name: String, status: HealthStatus },
    RecoveryAttempted { action: RecoveryAction, success: bool },
    StateTransition { from: DaemonState, to: DaemonState },
}
```

**需要注册的健康检查**:
1. **Provider 活性** — 每 60s ping 当前 LLM provider；3 次失败→熔断+切备用
2. **浏览器会话健康** — 验证 CDP 连接活跃；stale→自动关闭+重建
3. **Memory ticker 卡死** — 现有 `repair_clear_stuck_daily` 升级为健康检查
4. **MCP 服务器活性** — 检查所有注册的 MCP stdio 进程是否存活；死进程重启
5. **会话文件完整性** — 读取时验证 session JSON；损坏→从最后已知良好状态恢复

**关键接口**:

```rust
/// 启动后台 Tokio 任务
pub fn start(checks: Vec<HealthCheck>) -> JoinHandle<()>;

/// 动态注册新健康检查
pub fn register_check(&self, check: HealthCheck);

/// 恢复编排器
pub fn attempt_recovery(
    failed_check: &str,
    current_state: &DaemonState,
) -> RecoveryAction;
```

**集成点**:
- 重构现有 `self_repair.rs` 为更通用的框架
- 消费 `MemoryTicker`, `StreamCircuitState`, `BrowserRegistry` 作为健康目标
- 发射 `AgentEvent::DaemonHealthChanged` 到 harness event bus
- 前端 projection 通过现有 `runtime-projection-bridge.ts` 接收 daemon 健康状态

**关键修改文件**:
- **已建**: `src-tauri/src/modules/runtime/daemon/mod.rs` + `health_check.rs` + `recovery.rs`（`self_repair.rs` 保留为兼容层）
- **待深化**: `src-tauri/src/modules/desktop_host/setup.rs` 将完整 probe registry 接为默认常驻路径（WU-002 / WU-009）

---

### Module D: Constitutional Memory Tier (宪法级记忆层) — P1 🟢 已落地

**实际代码现状（2026-04-30）**: 宪法与结构约束位于 `skills/guard/constitution.rs`（及 guard 子模块），`prompt_planner` 已承载 Constitution block。设计附录中的 `runtime/constitution.rs` **未采用**；以 **`skills/guard/`** 为真源。

**定位**：一组不可变的、始终注入的行为约束，不能被 LLM、会话上下文或工具输出覆盖。Agent 的"宪法"。

**核心数据结构**（设计参考；实现以 `skills/guard/constitution.rs` 及 prompt 块为准）:

```rust
/// 宪法规则
pub struct ConstitutionalRule {
    pub id: String,
    pub text: String,
    pub priority: u8,        // 99 = 最高
    pub category: ConstitutionCategory,
    pub immutable: bool,     // true = 不可被 agent 修改
}

/// 规则类别
pub enum ConstitutionCategory {
    Safety,       // 安全约束
    Privacy,      // 隐私保护
    Ethics,       // 伦理规范
    Accuracy,     // 准确性（如 No-Exec-No-Memory）
    UserOverride, // 用户自定义覆盖
}

/// 宪法清单
pub struct ConstitutionManifest {
    pub version: String,
    pub rules: Vec<ConstitutionalRule>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}
```

**默认内置规则（参考 GenericAgent L0）**:
1. **行动验证原则**: 仅将成功的工具执行结果写入记忆，禁止写"固有知识"或"推测"
2. **神圣不可删改性**: 经过验证的数据严禁丢弃，仅可压缩或迁移
3. **禁止存储易变状态**: 时间戳、Session ID、PID 等不进记忆
4. **最小充分指针**: 上层记忆仅留能定位下层的最短标识
5. **安全边界**: 不执行可能造成不可逆损害的操作，除非获得明确用户批准

**集成点**:
- 在 `TurnService::prepare_chat_inputs` 中注入，作为新的 `PromptBlockKind::Constitution`
- Priority 99 确保不被上下文压缩器驱逐
- `PromptCoordinator` 获得新输入: `constitutional_rules: Vec<String>`

**关键修改文件**:
- **已建**: `src-tauri/src/modules/skills/guard/constitution.rs`（及 `prompt_planner` Constitution kind）
- **修改**: `src-tauri/src/modules/application/prompt_planner/` (添加 Constitution block kind)
- **修改**: `src-tauri/src/modules/application/turn_service/mod.rs` (加载+注入宪法)

---

### Module E: Agent 自编辑能力 (Agent Self-Edit) — P1 🟡 已部分落地（⚠️ DW-001 `landed-stub`）

**实际代码现状（2026-04-30）**: `attempt_ledger`、`agent_loop_delegate`、`learning/` 与 self-edit **scanner helper** 已存在；`setup.rs` 间歇 spawn scanner。**生产缺口**：`ConstEmbedder` + `MockUtilityLlm` 导致扫描无真实输入/提案（DW-001）。单文件 `runtime/self_edit.rs` 与否以仓库实际模块拆分为准。

**定位**：当工具持续失败时，允许 agent 检查失败模式并提出修复方案——参数调整、包装脚本、或（对于用户创作的工具）实际代码编辑。

**Harness 原则锚定**：#2 Agent 自编辑 Harness + #12 完整自由（有护栏的自由）。关键设计选择：参考 Browser-Harness 的 `helpers.py`"Read, edit, extend -- this file is yours"哲学，将 live helpers 目录（agent 可写的工具脚本）与核心系统代码（需要编译验证）分离。

**核心数据结构** (`src-tauri/src/modules/runtime/self_edit.rs`):

```rust
/// 工具失败模式
pub struct ToolFailurePattern {
    pub tool_name: String,
    pub consecutive_failures: u32,
    pub error_signatures: Vec<String>,
    pub last_attempted_args: Vec<serde_json::Value>,
}

/// 自编辑提案
pub struct SelfEditProposal {
    pub target: SelfEditTarget,
    pub proposed_change: String,
    pub confidence: f64,
    pub requires_approval: bool,
}

/// 自编辑目标
pub enum SelfEditTarget {
    ToolArgs { tool_name: String },
    WrapperScript { path: PathBuf },
    SkillPatch { skill_id: String },
    Escalate,  // 无法自修复，升级到用户
}
```

**工作流程**:
1. `stream_tool_execution.rs` 检测到 `invalid_tool_args_streak >= LIMIT`
2. 不立即触发终结，先调用 `ToolFailureAnalyzer::analyze()`
3. 产出 `SelfEditProposal`
4. 若 `requires_approval == true`，通过现有 `PermissionPromptDecision` 管线请求用户批准
5. 执行修复 → 重试

**集成点**:
- 从 `stream_tool_execution.rs` 触发，在终结前先尝试自编辑
- 现有 `record_tool_outcome`（`self_repair.rs`）提供失败数据
- 自编辑提案通过现有权限管线获取用户批准

**关键修改文件**:
- **新建**: `src-tauri/src/modules/runtime/self_edit.rs`
- **修改**: `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs` (终结前添加自编辑尝试)
- **修改**: `src-tauri/src/modules/runtime/self_repair.rs` (丰富 `record_tool_outcome`，添加错误签名)

---

### Module F: 浏览器会话自修复 (Browser Session Self-Healing) — P1 🟡 部分已实现

**实际代码现状（2026-04-30）**: `smart_browser/session_health.rs`（`BrowserHealthStatus` 等）+ `runtime.rs` 3-backend 与 **坐标 / 简化** 策略已增强（与 J、L 联动）。`desktop_host/setup.rs` 内 **`BrowserRegistryProbe`** 将注册表心跳接入 daemon（iter-4）。

**定位**：将 Smart Browser 从被动的后端选择器升级为主动的会话管理器，检测并恢复浏览器故障。

**核心数据结构** (`src-tauri/src/modules/smart_browser/session_health.rs`):

```rust
/// 浏览器会话健康状态
pub struct BrowserSessionHealth {
    pub session_id: SmartBrowserSessionId,
    pub status: BrowserHealthStatus,
    pub last_heartbeat: Instant,
    pub failure_count: u32,
}

pub enum BrowserHealthStatus {
    Connected,
    Stale,
    Disconnected,
    Recovering,
    Crashed,
}

/// 恢复计划
pub struct BrowserRecoveryPlan {
    pub action: BrowserRecoveryAction,
    pub preserve_url: Option<String>,
    pub preserve_cookies: bool,
}

pub enum BrowserRecoveryAction {
    Reconnect,
    RestartProcess,
    ClearAndRestart,
    EscalateToCloud,
}
```

**集成点**:
- 注册为 Module C (`SelfHealingDaemon`) 的健康检查
- 与现有 `smart_browser/runtime.rs` 协同
- 恢复时发射 `SmartBrowserEvent`
- 云升级通过现有 `evaluate_cloud_escalation` 策略

**关键修改文件**:
- **已建**: `src-tauri/src/modules/smart_browser/session_health.rs`
- **修改**: `src-tauri/src/modules/smart_browser/runtime.rs` (接入健康监控)
- **修改**: `src-tauri/src/modules/browser/session.rs` (添加心跳+崩溃检测)

---

### Module G: Domain Knowledge Repository (域知识仓库) — P1 🟢 钩子已落地（⚠️ DW-004 `landed-stub`）

**实际代码现状（2026-04-30）**: `skills/domain_knowledge/` + `stream_finalize.rs` 内 `run_domain_knowledge_contributor` 已在生产路径。**生产缺口**：`work_loop` advisory 仍使用 `MockKnowledgeStore`，直至进程级 `KnowledgeStore` 单例下传（GAP §4）。

**定位**：参考 Browser-Harness 的 73+ domain-skills + 19 interaction-skills，以及 GenericAgent 的 105K+ 技能卡体系，构建 Agent 可读写的结构化域知识库。Agent 不仅使用现有 Skill，还能在执行过程中创作和贡献域知识。

**设计理念**（融合两个参考项目）：
- Browser-Harness 模式：Markdown 文件编码 URL 模式、选择器库、JS 代码片段、已知陷阱
- GenericAgent 模式：L3 任务级 SOP（`*_sop.md`）编码前置条件、坑点、执行步骤

**核心数据结构**（实现见 `skills/domain_knowledge/`；下列为设计参考）:

```rust
/// 域知识类别（对应 Browser-Harness 的 domain-skills + interaction-skills）
pub enum DomainKnowledgeKind {
    /// 网站特定知识（URL 模式、选择器、API、陷阱）
    WebsiteDomain {
        domain: String,            // e.g. "amazon.com"
        url_patterns: Vec<String>,
        selectors: Vec<SelectorEntry>,
        js_snippets: Vec<JsSnippet>,
        gotchas: Vec<String>,
    },
    /// UI 交互原语（对话框处理、标签管理、iframe、上传等）
    InteractionPrimitive {
        category: String,          // e.g. "dialogs", "tabs", "uploads"
        problem_statement: String,
        solution_code: String,
        tradeoffs: Vec<String>,
    },
    /// 任务级 SOP（GenericAgent L3 模式）
    TaskSOP {
        task_type: String,         // e.g. "file_batch_rename", "api_integration"
        prerequisites: Vec<String>,
        key_pitfalls: Vec<String>,
        execution_steps: Vec<SOPStep>,
    },
}

/// 选择器条目（带稳定性标记）
pub struct SelectorEntry {
    pub selector: String,
    pub purpose: String,
    pub stability: SelectorStability,  // Stable / Fragile / Untested
    pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
}

pub enum SelectorStability { Stable, Fragile, Untested }

/// SOP 步骤
pub struct SOPStep {
    pub tool_name: String,
    pub description: String,
    pub args_template: Option<String>,
    pub expected_result: String,
}

/// 域知识条目（统一存储格式）
pub struct DomainKnowledgeEntry {
    pub id: String,
    pub kind: DomainKnowledgeKind,
    pub created_by: KnowledgeAuthor,    // Agent | User | Imported
    pub confidence: f64,
    pub access_count: u32,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub enum KnowledgeAuthor { Agent { session_id: String }, User, Imported { source: String } }
```

**关键接口**:

```rust
/// 按域名/任务类型检索域知识
pub async fn lookup_domain_knowledge(
    domain_or_task: &str,
) -> Vec<DomainKnowledgeEntry>;

/// Agent 在执行中发现有用模式后贡献域知识
pub async fn contribute_knowledge(
    entry: DomainKnowledgeEntry,
    verification_evidence: &str,  // 必须有执行证据
) -> Result<String>;

/// 定期清理过期/低置信度的域知识
pub async fn prune_stale_entries(
    max_age_days: u32,
    min_confidence: f64,
) -> Vec<String>;
```

**集成点**:
- 在 `work_loop.rs` 的 skill resolution 阶段调用 `lookup_domain_knowledge`，将匹配的域知识注入 prompt
- 在 `stream_finalize.rs` 的 sedimentation 钩子中，除了生成 skill 卡片外，还提取域知识
- 域知识存储在 `~/.if2ai/domain-knowledge/` 目录（按 domain/task-type 分子目录）
- 前端提供域知识浏览和编辑 UI

**关键修改文件**:
- **已建**: `src-tauri/src/modules/skills/domain_knowledge/`（及 finalize 钩子）
- **修改**: `src-tauri/src/modules/application/turn_service/work_loop.rs` (注入域知识到 prompt)
- **修改**: `src-tauri/src/modules/skills/mod.rs` (注册新子模块)

---

### Module H: Working Checkpoint System (工作检查点系统) — P1 🟢 核心已落地

**实际代码现状（2026-04-30）**: `runtime/working_checkpoint.rs` 与 `projection.rs` / `recoverability.rs` 协同；finalize / preflight 钩子见 WU-003、WU-004 注释。若仍有 `dead_code` 或未启用分支，以仓库与 GAP 报告为准迭代。

**定位**：参考 GenericAgent 的 `update_working_checkpoint` 工具，实现每轮 <200 tokens 的关键信息短期记忆注入机制。确保关键约束、文件路径、失败原因、进度始终在上下文中——即使历史消息被压缩或驱逐。

**设计理念**（GenericAgent 核心模式）：
- 每轮自动从 agent 输出中提取 `<key_info>` 标签内容
- key_info 不进入消息历史（不会被压缩/驱逐），而是作为独立注入
- 任务结束或用户切换任务时自动清除
- 关联 SOP：指向相关的 L3 域知识条目

**核心数据结构** (`src-tauri/src/modules/runtime/working_checkpoint.rs`):

```rust
/// 工作检查点——每个活跃 session 一个
pub struct WorkingCheckpoint {
    pub session_id: String,
    pub key_info: String,              // <200 tokens 的关键信息
    pub related_sop: Option<String>,   // 关联的域知识/SOP 名
    pub turn_created: u64,             // 创建时的 turn 号
    pub turn_updated: u64,             // 最后更新的 turn 号
}

/// 检查点提取结果
pub struct CheckpointExtraction {
    pub key_info: Option<String>,
    pub related_sop: Option<String>,
    pub should_clear: bool,            // 任务完成时清除
}

/// 检查点注入配置
pub struct CheckpointInjectionConfig {
    pub max_tokens: usize,             // 默认 200
    pub injection_position: InjectionPosition,
    pub auto_extract: bool,            // 从 agent 输出自动提取
}

pub enum InjectionPosition {
    BeforeLastUserMessage,   // GenericAgent 风格
    AsSystemBlock,           // If2Ai prompt planner 风格
}
```

**关键接口**:

```rust
/// 从 agent 输出中提取检查点信息
pub fn extract_checkpoint(
    accumulated_text: &str,
    tool_results: &[ToolResult],
) -> CheckpointExtraction;

/// 注入检查点到请求消息中
pub fn inject_checkpoint(
    checkpoint: &WorkingCheckpoint,
    messages: &mut Vec<InputMessage>,
    config: &CheckpointInjectionConfig,
);

/// 更新或清除检查点
pub async fn update_checkpoint(
    session_id: &str,
    extraction: CheckpointExtraction,
) -> Result<()>;
```

**集成点**:
- 在 `stream_preflight::build_iteration_request` 中注入 checkpoint（在压缩之后、发送前）
- 在 `stream_finalize::finalize_stream_task` 中从 accumulated_text 提取并更新 checkpoint
- 存储在内存中（`Arc<RwLock<HashMap<String, WorkingCheckpoint>>>`），非持久化（任务级）
- 可选：暴露 `update_working_checkpoint` 内置工具让 agent 显式写入

**关键修改文件**:
- **已建**: `src-tauri/src/modules/runtime/working_checkpoint.rs`
- **修改**: `src-tauri/src/modules/application/turn_service/stream_preflight.rs` (注入检查点)
- **修改**: `src-tauri/src/modules/application/turn_service/stream_finalize.rs` (提取并更新检查点)

---

### Module I: Skill Vector Semantic Search (Skill 向量语义搜索) — P1 🟡 部分已实现

**实际代码现状（2026-04-30）**: `memory/embedding/` 与 `skills/vector_index.rs` 等已承载索引与嵌入；`work_loop` 侧是否 **完全** 替换关键词路径请对照当前 `score_candidate_skills` 实现。设计稿中的 `skills/vector_search.rs` 单文件可能已合并为 `vector_index` 布局。

**定位**：将现有的关键词 token 匹配升级为嵌入向量相似度搜索，复用 If2Ai 已有的 FastEmbed + LanceDB 基础设施。参考 GenericAgent 的 105K+ 技能卡语义检索能力。

**核心数据结构** (`src-tauri/src/modules/skills/vector_search.rs`):

```rust
/// Skill 嵌入索引
pub struct SkillEmbeddingIndex {
    pub db: Arc<LanceDb>,
    pub table_name: String,
    pub dimension: usize,           // 384 (multilingual-e5-small)
}

/// 搜索结果（带评分）
pub struct SkillSearchResult {
    pub skill_id: String,
    pub skill_name: String,
    pub description: String,
    pub relevance_score: f64,       // 向量相似度
    pub quality_score: f64,         // 基于 access_count + confidence
    pub final_score: f64,           // 综合评分
    pub match_reasons: Vec<String>,
}

/// 索引更新事件
pub enum IndexEvent {
    SkillAdded { skill_id: String },
    SkillUpdated { skill_id: String },
    SkillRemoved { skill_id: String },
    BulkReindex,
}
```

**关键接口**:

```rust
/// 语义搜索 skill（替换现有关键词匹配）
pub async fn search_skills(
    query: &str,
    top_k: usize,
    min_score: f64,
) -> Vec<SkillSearchResult>;

/// 为新 skill 生成嵌入并索引
pub async fn index_skill(
    skill_id: &str,
    name: &str,
    description: &str,
    trigger_patterns: &[String],
) -> Result<()>;

/// 批量重建索引
pub async fn rebuild_index(
    all_skills: &[SkillMetadata],
) -> Result<usize>;
```

**集成点**:
- 替换 `work_loop.rs` 中现有的 `score_candidate_skills()` 关键词匹配逻辑
- 复用 `src-tauri/src/modules/memory/` 中已有的 `FastEmbedProvider` + `LanceDb` 实例
- 新 skill 被 sedimentation engine (Module B) 创建时自动触发索引更新
- 域知识 (Module G) 也纳入索引

**关键修改文件**:
- **已建/合并**: `src-tauri/src/modules/skills/vector_index.rs`（及关联模块）
- **修改**: `src-tauri/src/modules/application/turn_service/work_loop.rs` (替换关键词评分为向量搜索)
- **修改**: `src-tauri/src/modules/skills/sedimentation/`（新 skill 自动索引）
- **修改**: `src-tauri/src/modules/skills/mod.rs` (注册新子模块)

---

### Module J: Coordinate-First Browser Strategy (坐标优先浏览器策略) — P2 🟢 已落地

**实际代码现状（2026-04-30）**: `smart_browser/runtime.rs` 中 `decide_browser_click_strategy` 已在浏览器工具路径真实派发（WU-006 / DW-005）。独立 `coordinate_strategy.rs` 单文件与否以仓库为准（逻辑可能在 `runtime` 内联）。

**定位**：参考 Browser-Harness 的截图优先+坐标点击优先策略，为 Smart Browser 添加新的交互模式降级链：坐标点击 → 标签引用 → CSS 选择器。坐标点击可穿透 iframe/shadow DOM/cross-origin，是最可靠的交互方式。

**Harness 原则锚定**：#5 坐标优先交互。核心范式转换：从"选择器优先"改为"截图优先"——`capture_screenshot()` → 视觉定位 → `click_at_xy(x, y)` → 再截图验证。仅在目标无可见几何形状时才降级到 DOM。`Input.dispatchMouseEvent` 在 Chrome compositor 层执行，天然穿透所有 DOM 边界。

**设计理念**（Browser-Harness 核心策略）：
- 截图优先：先 `capture_screenshot`，从截图中定位元素坐标
- 坐标点击穿透一切：`Input.dispatchMouseEvent` 在合成器级别运行，自动穿透 iframe/shadow DOM
- 选择器作为降级：仅当坐标不可用时才回退到 DOM 查询

**核心数据结构** (`src-tauri/src/modules/smart_browser/coordinate_strategy.rs`):

```rust
/// 浏览器交互策略降级链
pub enum InteractionStrategy {
    /// 优先级 1: 坐标点击（最可靠，穿透一切）
    CoordinateClick {
        x: f64,
        y: f64,
        button: MouseButton,
        verify_screenshot: bool,   // 点击后截图验证
    },
    /// 优先级 2: 可访问性标签引用
    LabelReference {
        label_text: String,
        element_type: Option<String>,
    },
    /// 优先级 3: CSS 选择器（最脆弱）
    CssSelector {
        selector: String,
        action: SelectorAction,
    },
}

pub enum SelectorAction { Click, Type(String), ScrollIntoView }

/// 截图分析结果（用于坐标定位）
pub struct ScreenshotAnalysis {
    pub viewport_size: (u32, u32),
    pub identified_elements: Vec<IdentifiedElement>,
    pub scroll_position: (i32, i32),
}

pub struct IdentifiedElement {
    pub label: String,
    pub bounding_box: (f64, f64, f64, f64),  // x, y, width, height
    pub center: (f64, f64),
    pub element_type: String,
    pub confidence: f64,
}

/// 交互降级决策
pub struct InteractionDecision {
    pub strategy: InteractionStrategy,
    pub fallback_chain: Vec<InteractionStrategy>,
    pub reason: String,
}
```

**关键接口**:

```rust
/// 决定最佳交互策略
pub fn decide_interaction(
    target_description: &str,
    screenshot: Option<&ScreenshotAnalysis>,
    available_selectors: &[String],
) -> InteractionDecision;

/// 执行坐标点击（CDP Input.dispatchMouseEvent）
pub async fn coordinate_click(
    x: f64, y: f64,
    button: MouseButton,
    session: &BrowserSession,
) -> Result<()>;

/// 截图后验证点击效果
pub async fn verify_click_effect(
    before: &[u8],
    after: &[u8],
) -> ClickVerification;
```

**集成点**:
- 作为 `SmartBrowserCommandKind::Click` 的增强策略
- 在 `smart_browser/runtime.rs` 的命令执行中，先尝试坐标策略再降级
- 截图分析可选集成 vision model（利用已有的 multimodal provider）

**关键修改文件**:
- **已建/内联**: 坐标策略逻辑（见 `smart_browser/runtime.rs`）
- **修改**: `src-tauri/src/modules/smart_browser/runtime.rs` (接入坐标优先策略)

---

### Module K: Execution-Verified Memory Gate (执行验证记忆门) — P2 🟡 部分已实现

**实际代码现状**: `memory_quality_gate.rs`(质量门) 和 `memory_write_policy.rs`(写入策略) 已存在，提供记忆写入的验证和策略控制。但 `verification_gate.rs`（独立的验证门模块）不存在——验证逻辑分散在各处，未统一为独立门控。

**定位**：参考 GenericAgent 的核心公理 "No Execution, No Memory"——仅将经过工具执行验证的结果写入长期记忆，防止幻觉数据污染记忆系统。

**设计理念**（GenericAgent L0 公理 #1）：
- 行动验证原则：仅将 **成功的工具执行结果** 写入记忆
- 禁止写入"固有知识"或"推测"
- 禁止存储易变状态（时间戳、Session ID、PID）

**核心数据结构** (`src-tauri/src/modules/memory/verification_gate.rs`):

```rust
/// 记忆写入验证结果
pub enum MemoryVerification {
    /// 已验证：有工具执行证据
    Verified {
        evidence_tool_call_id: String,
        evidence_summary: String,
    },
    /// 未验证：无执行证据，拒绝写入
    Rejected {
        reason: VerificationRejectReason,
    },
    /// 豁免：用户显式写入或宪法规则
    Exempt {
        exemption_reason: String,
    },
}

pub enum VerificationRejectReason {
    NoToolExecution,       // 没有工具调用证据
    ToolFailed,            // 工具调用失败
    VolatileContent,       // 内容包含易变状态（时间戳等）
    SpeculativeContent,    // 内容是推测性的
}

/// 验证门配置
pub struct VerificationGateConfig {
    pub enabled: bool,                    // 默认 true
    pub strict_mode: bool,                // true: 必须有成功工具调用；false: 宽松
    pub volatile_patterns: Vec<String>,   // 易变内容正则模式
    pub exempt_categories: Vec<String>,   // 豁免的记忆类别
}
```

**关键接口**:

```rust
/// 验证记忆写入请求
pub fn verify_memory_write(
    content: &str,
    category: &str,
    recent_tool_results: &[ToolResult],
    config: &VerificationGateConfig,
) -> MemoryVerification;

/// 检测内容是否包含易变状态
pub fn detect_volatile_content(content: &str) -> Vec<String>;
```

**集成点**:
- 在 `memory_store` 内置工具的 `do_memory_store` handler 中，调用验证门
- 验证门拒绝时，返回工具错误消息告知 agent 原因
- 宪法层 (Module D) 的规则 #1 ("行动验证原则") 是此门的策略声明
- 配置通过 `constitution.yaml` 中的 `accuracy` 类别规则控制

**关键修改文件**:
- **新建**: `src-tauri/src/modules/memory/verification_gate.rs`
- **修改**: `src-tauri/src/modules/tools/builtin/` 中的 memory_store 工具 (接入验证门)
- **修改**: `src-tauri/src/modules/memory/mod.rs` (注册新子模块)

---

### Module L: Browser Content Simplifier (浏览器内容简化器) — P2 🟢 已落地

**实际代码现状（2026-04-30）**: `smart_browser/runtime.rs` 中 `simplify_browser_result_text` / `adaptive_simplify` 已在浏览器工具结果路径调用（WU-006）。独立 `content_simplifier.rs` 单文件与否以仓库为准。

**定位**：参考 GenericAgent 的 `simphtml.py`（800 行 HTML 简化器），实现浏览器页面内容的智能简化，将 200K 字符的原始 HTML 压缩到 15K~35K，大幅降低浏览器任务的 token 消耗。

**Harness 原则锚定**：#1 极简零框架 + #8 最小上下文哲学。关键洞察：200K HTML 中只有 15-35K 是对 agent 有用的信息（可交互元素、文本内容、关键属性）。其余的脚本、样式、导航、页脚对 agent 而言都是"噪声"。

**设计理念**（GenericAgent simphtml.py 策略）：
- 过滤元素：script, style, nav, aside, footer, [role=navigation], [role=complementary]
- 保留元素：main, [role=main], article, section, form, input
- 简化属性：仅保留 id, class, name, type, href, src, value 等有意义属性
- 文本提取：递归收集文本内容，计算信息密度
- 输出控制：35K 字符硬限制

**核心数据结构** (`src-tauri/src/modules/smart_browser/content_simplifier.rs`):

```rust
/// 简化配置
pub struct SimplifierConfig {
    /// 最大输出字符数
    pub max_output_chars: usize,         // 默认 35_000
    /// 要过滤的 HTML 标签
    pub filter_tags: Vec<String>,        // ["script", "style", "nav", "aside", ...]
    /// 要保留的 HTML 标签（高优先级）
    pub preserve_tags: Vec<String>,      // ["main", "article", "form", "input", ...]
    /// 要保留的属性
    pub preserve_attrs: Vec<String>,     // ["id", "class", "name", "type", "href", ...]
    /// 启用纯文本模式（仅返回文本内容）
    pub text_only: bool,
    /// 启用标签列表模式（仅返回 tab 信息）
    pub tabs_only: bool,
}

/// 简化结果
pub struct SimplifiedContent {
    pub html: String,                    // 简化后的 HTML
    pub text: String,                    // 纯文本版本
    pub token_estimate: usize,           // 估算 token 数
    pub original_chars: usize,
    pub simplified_chars: usize,
    pub compression_ratio: f64,
    pub key_elements: Vec<KeyElement>,   // 提取的关键交互元素
}

/// 关键交互元素（用于 Agent 定位）
pub struct KeyElement {
    pub tag: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub element_type: Option<String>,
    pub text_content: String,
    pub href: Option<String>,
}
```

**关键接口**:

```rust
/// 简化 HTML 内容
pub fn simplify_html(
    raw_html: &str,
    config: &SimplifierConfig,
) -> SimplifiedContent;

/// 从 URL 获取并简化
pub async fn fetch_and_simplify(
    url: &str,
    session: &BrowserSession,
    config: &SimplifierConfig,
) -> Result<SimplifiedContent>;

/// 动态调整简化级别（根据剩余 token 预算）
pub fn adaptive_simplify(
    raw_html: &str,
    available_tokens: usize,
) -> SimplifiedContent;
```

**集成点**:
- 在 `smart_browser/runtime.rs` 的 `web_scan` 类命令中调用
- 替换当前的原始 HTML 返回，改为返回简化后的内容
- 可选：在 `web_execute_js` 的结果处理中也应用简化
- 与 Module A (上下文压缩管线) 协同：简化后的 HTML 进入工具结果压缩管线

**关键修改文件**:
- **已建/内联**: HTML 简化逻辑（见 `smart_browser/runtime.rs` 与相关 helper）
- **修改**: `src-tauri/src/modules/smart_browser/runtime.rs` (web_scan 返回简化内容)
- **修改**: `src-tauri/src/modules/tools/builtin/browser_tool.rs` (接入简化器)

---

### Module M: Jiaochang Audio Plugin Sandbox (校验教场音频插件沙箱) — 🆕 新发现 (P2)

**定位**: 一个基于 Node.js VM isolate 的游戏/模拟插件运行时，允许 Agent 执行交互式音频游戏和模拟场景。在 If2Ai 桌面应用中提供沉浸式的音频游戏体验。

**实际代码现状**: `jiaochang_audio/mod.rs`(1,596 LOC) 已完整实现，前端 `src/modules/jiaochang/`(60 files) 已实现完整的游戏可视化、音频播放器、策略面板、多语言(i18n: en-US/ja-JP/ko-KR/zh-CN)。这是一个完全已实现但 spec 未覆盖的模块。

**核心能力**:
1. **Node.js VM Isolate**: 在隔离的 Node.js 运行时中执行游戏/模拟插件代码
2. **音频管理**: 音频加载、播放、可视化、track library
3. **策略执行**: runtime adapter + path replay 策略回放
4. **多语言**: i18n 支持 4 种语言
5. **前端可视化**: PixelStage(像素舞台)、StrategyPanel(策略面板)、JiaochangMusicPlayer(音乐播放器)

**前端文件清单** (60 files):
- `src/modules/jiaochang/` — Jiaochang 主模块（游戏入口+路由）
- 组件: `JiaochangMusicPlayer.tsx`, `PixelStage.tsx`, `StrategyPanel.tsx`
- 音频系统: `JiaochangAudioProvider.tsx`, audio visualizer, track library
- 数据适配: Strategy adapter, runtime adapter, path replay
- 国际化: i18n (4 种语言)

**关键代码文件**:
- **后端**: `src-tauri/src/modules/jiaochang_audio/mod.rs` (1,596 LOC, 已实现)
- **前端**: `src/modules/jiaochang/` (60 files, 已实现)

**spec 新增价值**: 该模块已完整实现，spec 中补充文档以标记其存在和架构位置。

---

### Module N: MCP Workbench & Control Plane (MCP 工作台与控制面) — 🆕 新发现 (P1)

**定位**: MCP (Model Context Protocol) 服务器的全生命周期管理 + Agent 执行的控制面（ingress classification、tool execution brokering、audit、boundary resolution）。

**实际代码现状**: 
- `mcp_stdio/manager.rs`(765 LOC): MCP server 生命周期管理（启动/停止/重启）、tool discovery（自动发现 MCP tools）、workbench activity 记录
- `control_plane/`(8 files, ~1,500 LOC): `ingress_classifier.rs`(执行模式分类 M2.6)、`tool_execution_broker.rs`(工具执行代理)、`audit.rs`(审计追踪)、`boundary_resolver.rs`(边界解析)、`session_bridge.rs`、`session_context.rs`、`prepare_step_execution.rs`
- `learning/`(18 files, ~3,000 LOC): `strategy_registry.rs`(策略注册表)、`reflection.rs`(反思引擎)、`trajectory_score.rs`(轨迹评分)、`self_model.rs`(自我模型)、`failure_clustering.rs`(失败聚类)、`failure_taxonomy.rs`(失败分类)、`active_overlay.rs`、`promotion_gate.rs` 等

**核心能力**:
1. **MCP Server 管理**: stdio 进程启动、健康检查、tool 发现、workbench activity 记录
2. **执行模式分类 (M2.6)**: ingress_classifier 分析用户请求→判定最佳执行模式（direct_execute/auto_plan_execute/plan_then_confirm/specialized_surface）
3. **工具执行代理**: tool_execution_broker 集中管理所有工具的权限检查和执行流程
4. **策略注册与反思**: strategy_registry 管理可复用的执行策略，reflection 引擎在每次执行后生成反思笔记
5. **失败分析**: failure_clustering 聚类失败模式，failure_taxonomy 分类失败类型
6. **审计追踪**: audit 记录所有关键操作的审计日志

**关键代码文件**:
- **后端**: `src-tauri/src/modules/runtime/mcp_stdio/manager.rs` (765 LOC, 已实现)
- **后端**: `src-tauri/src/modules/runtime/mcp_stdio/types.rs` (已实现)
- **后端**: `src-tauri/src/modules/control_plane/` (8 files, ~1,500 LOC, 已实现)
- **后端**: `src-tauri/src/modules/learning/` (18 files, ~3,000 LOC, 已实现)
- **前端**: `src/modules/settings/pages/McpWorkbench.tsx` (已实现)
- **前端**: `src/modules/execution-mode/ExecutionModePill.tsx` (已实现)

**spec 新增价值**: MCP Workbench 和 Control Plane 是 spec 未覆盖但代码已完整实现的核心子系统。补充文档标记其存在和与其他模块的交互关系。

---

## Part 3: 实施路线图

> **Harness 哲学优先级的实施指导**：基于 Part 0 的十二大设计原则，实施优先级遵循以下逻辑链——
> 1. **先降低认知负载** (Phase 1, Token 效率)：Agent 的每次请求从 120K→30K tokens，Harness 复杂度不泄露到执行路径 (#8 #10)
> 2. **后建自愈底座** (Phase 3, Self-Healing)：Daemon 进程自愈 + 浏览器会话恢复，确保知识积累不被中断打断 (#3 #4 #9)
> 3. **再启自我进化** (Phase 2, Self-Evolution)：Skill 自动沉淀 + 宪法记忆层，Agent 每次成功任务贡献知识 (#7 #11 #12)
> 4. **然后赋予自主性** (Phase 4, Agent Autonomy)：Agent 自编辑工具 + 验证记忆写入 + 分级升级 (#2 #12)
> 5. **接着深化领域知识** (Phase 6, Domain Knowledge)：结构化域知识仓库 + 工作检查点系统 (#7 #8 #11)
> 6. **最后精炼交互层** (Phase 7, Browser & Tool Refinement)：坐标优先 + HTML 简化 + 工具原子化 (#1 #5 #10)
> 7. **全线打通前端投影** (Phase 5, Integration)：新能力可视化到前端，但排在 Phase 6 之后是因为前端投影依赖域知识事件

### Phase 1: Token 效率基础 (4 Packs)

最高 ROI 阶段。将 token 消耗降低 4x 解锁更快迭代、更低成本、更好连贯性。

| Pack ID       | 名称                     | 范围                                                                                                    | 依赖        |
| ------------- | ------------------------ | ------------------------------------------------------------------------------------------------------- | ----------- |
| `FEAT-TE-001` | 上下文层级预算硬约束     | 在 `stream_preflight.rs` 中接入 `ContextBudget` 作为硬约束；向 `budget.rs` 添加 tier 枚举               | 无          |
| `FEAT-TE-002` | 消息级压缩               | 添加 `CompressedMessage` 类型 + `MessageDigester`（使用 `UtilityLlm`）；集成到会话历史的 preflight 之前 | FEAT-TE-001 |
| `FEAT-TE-003` | 迷你索引构建器 (L1 模式) | 构建 ≤30 行的会话索引，作为 priority-95 prompt block 注入；替换原始 working-memory dump                 | FEAT-TE-001 |
| `FEAT-TE-004` | 工具结果摘要化           | 增强 `summarize_tool_result_for_model`：对 >500 tokens 的结果使用 LLM 辅助压缩                          | FEAT-TE-001 |

### Phase 2: 自我进化核心 (4 Packs)

将每次成功任务转化为可复用知识。

| Pack ID       | 名称               | 范围                                                                                         | 依赖        |
| ------------- | ------------------ | -------------------------------------------------------------------------------------------- | ----------- |
| `FEAT-SE-001` | Skill 沉淀引擎     | 新建 `skills/sedimentation.rs`；在 `stream_finalize.rs` 成功完成时挂钩                       | 无          |
| `FEAT-SE-002` | Skill 去重+合并    | 新建 `skills/deduplication.rs`；关键词 + 可选嵌入匹配现有 skills                             | FEAT-SE-001 |
| `FEAT-SE-003` | 宪法级记忆层       | 新建 `runtime/constitution.rs`；YAML 驱动的不可变规则注入，priority 99                       | 无          |
| `FEAT-SE-004` | Skill 向量语义搜索 | 升级 `work_loop.rs` skill 评分：从关键词 token 匹配到嵌入相似度，复用现有 FastEmbed 基础设施 | FEAT-SE-001 |

### Phase 3: Self-Healing 基础设施 (3 Packs)

从反应式看门狗升级为主动 daemon。

| Pack ID       | 名称                     | 范围                                                                                | 依赖        |
| ------------- | ------------------------ | ----------------------------------------------------------------------------------- | ----------- |
| `FEAT-SH-001` | Self-Healing Daemon 框架 | 重构 `self_repair.rs` → `runtime/daemon/`：健康检查注册表 + 状态机 + 恢复编排器     | 无          |
| `FEAT-SH-002` | Provider + MCP 活性检查  | 注册 provider 心跳 + MCP stdio 进程活性为 daemon 健康检查                           | FEAT-SH-001 |
| `FEAT-SH-003` | 浏览器会话自修复         | 新建 `smart_browser/session_health.rs`；注册为 daemon 检查；自动恢复 stale CDP 会话 | FEAT-SH-001 |

### Phase 4: Agent 自编辑 & 自主性升级 (3 Packs)

弥合工具级自修复和 agent 自主性的差距。

| Pack ID       | 名称                    | 范围                                                                      | 依赖        |
| ------------- | ----------------------- | ------------------------------------------------------------------------- | ----------- |
| `FEAT-AE-001` | 工具失败分析+自编辑提案 | 新建 `runtime/self_edit.rs`；在 `stream_tool_execution.rs` 终结触发前接入 | FEAT-SH-001 |
| `FEAT-AE-002` | 执行验证记忆写入        | 向 `memory_store` 工具添加验证门：仅在工具执行成功时持久化                | 无          |
| `FEAT-AE-003` | 分级失败升级            | 3 次尝试→参数调整→模型降级→用户升级链，在 work loop 中实现                | FEAT-AE-001 |

### Phase 5: 集成 & 强化 (2 Packs)

将所有新能力与现有投影驱动前端打通。

| Pack ID        | 名称                | 范围                                                                                                                                   | 依赖              |
| -------------- | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| `FEAT-INT-001` | 前端投影扩展        | 扩展 `runtime-event-reducer.ts` + `runtime-event-translator.ts`：daemon 健康、沉淀 skill、宪法违规、自编辑提案、域知识贡献、工作检查点 | 所有 Phase 1-4, 6 |
| `FEAT-INT-002` | 端到端 harness 验证 | 新 harness suite：压缩上下文→成功任务→skill 沉淀→daemon 健康→浏览器恢复→域知识→checkpoint                                              | 所有 Phase 1-6    |

### Phase 6: 域知识 & 工作检查点 (3 Packs)

赋予 Agent 结构化域知识创作能力（Browser-Harness 73 域技能 + GenericAgent 105K 技能卡融合设计）和关键信息始终在场的短期记忆机制（GenericAgent working checkpoint 模式）。

| Pack ID       | 名称           | 范围                                                                                                                                                                                                   | 依赖                                  |
| ------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- |
| `FEAT-DK-001` | 域知识仓库     | 新建 `skills/domain_knowledge.rs`；实现 `DomainKnowledgeKind` 三类知识（WebsiteDomain / InteractionPrimitive / TaskSOP）；存储在 `~/.if2ai/domain-knowledge/`；提供 `lookup_domain_knowledge` 检索接口 | 无                                    |
| `FEAT-DK-002` | 工作检查点系统 | 新建 `runtime/working_checkpoint.rs`；`stream_finalize.rs` 中自动提取 `<key_info>` 标签；`stream_preflight.rs` 中注入 checkpoint（<200 tokens）；内存级存储（任务生命周期）                            | FEAT-TE-001                           |
| `FEAT-DK-003` | 域知识自动贡献 | 在 `stream_finalize.rs` 的 sedimentation 钩子中，除 skill 卡片外，同时提取网站域知识（selector/gotcha）和任务 SOP；集成 Module K 执行验证门确保只写入经验证的知识                                      | FEAT-DK-001, FEAT-SE-001, FEAT-AE-002 |

### Phase 7: 浏览器 & 工具精炼 (3 Packs)

浏览器交互能力从 DOM 选择器升级为坐标优先降级链（Browser-Harness 核心策略），HTML 内容从原始 200K 压缩到 15-35K（GenericAgent simphtml 策略），工具集从 40+ 整合为正交原语。

| Pack ID       | 名称               | 范围                                                                                                                                                                                                                         | 依赖        |
| ------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| `FEAT-BR-001` | 浏览器内容简化器   | 新建 `smart_browser/content_simplifier.rs`；实现 `simplify_html`（过滤 script/style/nav，保留 main/form/input，35K 硬限制）+ `adaptive_simplify`（动态预算）；在 `smart_browser/runtime.rs` 的 web_scan 中替换原始 HTML 返回 | FEAT-TE-001 |
| `FEAT-BR-002` | 坐标优先浏览器策略 | 新建 `smart_browser/coordinate_strategy.rs`；实现坐标点击→标签引用→CSS 选择器降级链；集成 `Input.dispatchMouseEvent` CDP 命令；截图验证点击效果                                                                              | FEAT-SH-003 |
| `FEAT-BR-003` | 工具原子性整合     | 审计并整合现有 40+ 工具：将 6 记忆工具合并为 `memory_read`/`memory_write`/`memory_search`；将 5 cron 工具合并为 `schedule_manage`；将 7 skill 工具合并为 `skill_find`/`skill_use`；保留 alias 兼容                           | 无          |

---

## Part 4: 高层架构数据流图

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            User / Frontend                               │
│   ┌─────────────┐   ┌────────────────┐   ┌──────────────────────────┐   │
│   │  ChatUI      │   │ RunInspector    │   │ DaemonHealthIndicator    │   │
│   └──────┬──────┘   └───────┬────────┘   └───────────┬──────────────┘   │
│   ┌──────┴──────────────────┴─────────────────────────┴──────────────┐   │
│   │              runtime-projection-bridge.ts                         │   │
│   │  (+ 新事件: daemon_health, skill_sedimented,                     │   │
│   │    constitution_violation, self_edit_proposal,                    │   │
│   │    domain_knowledge_contributed, checkpoint_updated)              │   │
│   └──────────────────────────┬───────────────────────────────────────┘   │
└──────────────────────────────┼───────────────────────────────────────────┘
                               │ Tauri IPC
┌──────────────────────────────┼───────────────────────────────────────────┐
│                     Rust Backend (Tauri)                                  │
│                              │                                           │
│  ┌───────────────────────────┴────────────────────────────────────────┐  │
│  │                    TurnService.prepare_chat_inputs                  │  │
│  │  ┌──────────────┐  ┌─────────────────┐  ┌──────────────────────┐  │  │
│  │  │ [NEW]        │  │ [NEW]           │  │ Skill Resolution     │  │  │
│  │  │ Constitution │  │ Context         │  │ [ENHANCED]           │  │  │
│  │  │ Injector     │  │ Compressor      │  │ + Vector Search (I)  │  │  │
│  │  │ (P99 block)  │  │ (≤30K target)   │  │ + Domain Know. (G)   │  │  │
│  │  │   (D)        │  │   (A)           │  │                      │  │  │
│  │  └──────────────┘  └─────────────────┘  └──────────────────────┘  │  │
│  │  ┌──────────────────────┐  ┌──────────────────────────────────┐   │  │
│  │  │ [NEW]                │  │ [NEW]                            │   │  │
│  │  │ Working Checkpoint   │  │ Content Simplifier (L)           │   │  │
│  │  │ Injector (H)         │  │ (HTML 200K→15-35K)               │   │  │
│  │  │ (<200 tokens)        │  │                                  │   │  │
│  │  └──────────────────────┘  └──────────────────────────────────┘   │  │
│  └───────────────────────────┬────────────────────────────────────────┘  │
│                              │                                           │
│  ┌───────────────────────────┴────────────────────────────────────────┐  │
│  │                   stream_task → event_loop → tool_execution         │  │
│  │  ┌──────────────────┐  ┌────────────────────┐  ┌───────────────┐  │  │
│  │  │ [NEW]            │  │ [NEW]              │  │ [NEW]         │  │  │
│  │  │ Self-Edit Engine │  │ Graded Escalation  │  │ Coordinate    │  │  │
│  │  │ (on tool fail)   │  │ (3→adjust→degrade) │  │ -First Click  │  │  │
│  │  │   (E)            │  │                    │  │   (J)         │  │  │
│  │  └──────────────────┘  └────────────────────┘  └───────────────┘  │  │
│  └───────────────────────────┬────────────────────────────────────────┘  │
│                              │                                           │
│  ┌───────────────────────────┴────────────────────────────────────────┐  │
│  │                       stream_finalize                              │  │
│  │  ┌──────────────────────┐  ┌──────────────────────────────────┐   │  │
│  │  │ [NEW]                │  │ Trajectory + Reflection + Learn  │   │  │
│  │  │ Skill Sedimentation  │  │ (existing)                       │   │  │
│  │  │ Engine (B)           │  │                                  │   │  │
│  │  ├──────────────────────┤  ├──────────────────────────────────┤   │  │
│  │  │ [NEW]                │  │ [NEW]                            │   │  │
│  │  │ Domain Knowledge     │  │ Checkpoint Extraction (H)        │   │  │
│  │  │ Contribution (G)     │  │                                  │   │  │
│  │  └──────────────────────┘  └──────────────────────────────────┘   │  │
│  │                  │                                                 │  │
│  │  ┌───────────────┴────────────────────────────────────────────┐   │  │
│  │  │ [NEW] Execution-Verified Memory Gate (K)                   │   │  │
│  │  │ (No Execution, No Memory — 验证后才允许写入)               │   │  │
│  │  └────────────────────────────────────────────────────────────┘   │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │             [NEW] Self-Healing Daemon (background Tokio task) (C)   │  │
│  │  ┌────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────┐  │  │
│  │  │ Provider   │  │ MCP Server   │  │ Browser      │  │ Memory │  │  │
│  │  │ Heartbeat  │  │ Liveness     │  │ Session      │  │ Ticker │  │  │
│  │  │ Check      │  │ Check        │  │ Health (F)   │  │ Repair │  │  │
│  │  └────────────┘  └──────────────┘  └──────────────┘  └────────┘  │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Part 5: 验证方案

### 5.1 Token 效率验证
1. 在相同的 5 个不同复杂度的对话场景下，记录压缩前后的 token 数
2. 目标：平均请求 token 从 ~120K chars 降到 ~30K chars（4x 压缩）
3. 验证：任务完成率不下降（使用 trajectory scoring 对比）

### 5.2 Skill 沉淀验证
1. 执行 10 个不同类型的成功任务
2. 验证：至少 6/10 的任务产出有效 skill 卡片
3. 验证：skill 卡片在第二次相似任务中被正确检索和使用
4. 验证：向量语义搜索 (Module I) 的召回率 > 关键词匹配基线

### 5.3 Self-Healing 验证
1. 模拟 Provider 超时（断网 30s）→ 验证自动切换备用 Provider
2. 模拟 MCP 服务器崩溃 → 验证自动重启
3. 模拟浏览器 CDP 断连 → 验证自动恢复
4. 验证：daemon 健康状态在前端 projection 中正确显示

### 5.4 域知识 & 工作检查点验证
1. 执行 3 个网站交互任务 → 验证自动提取域知识（selector、gotcha）
2. 验证：第二次访问相同域名时自动注入域知识
3. 在 10 轮对话中验证 working checkpoint 始终包含关键路径/约束信息
4. 验证：checkpoint 在消息压缩后仍然存在（不被驱逐）
5. 验证：域知识 auto-contribution 受执行验证门 (Module K) 守卫

### 5.5 浏览器增强验证
1. 在包含 iframe/shadow DOM 的页面上验证坐标点击穿透能力
2. 验证：坐标点击→标签引用→CSS 选择器降级链按预期回退
3. 对比 HTML 简化前后的 token 数：目标 200K→15-35K（85%+ 压缩率）
4. 验证：简化后的 HTML 保留所有可交互元素信息

### 5.6 记忆验证门验证
1. 尝试通过 `memory_store` 写入无工具执行证据的内容 → 验证被拒绝
2. 验证：成功工具执行后的内容写入被允许
3. 验证：易变内容（时间戳、PID）被检测并拒绝
4. 验证：宪法规则和用户显式写入享受豁免

### 5.7 工具原子性验证
1. 验证：合并后的 memory_read/memory_write/memory_search 功能覆盖原 6 工具
2. 验证：旧工具名通过 alias 仍可调用（向后兼容）
3. 验证：Agent 在 9 原子工具集上的工具选择准确率 > 40+ 工具集

### 5.8 编译和 lint 验证
```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

### 5.9 端到端 harness suite
```bash
./scripts/pack suite harness/suites/self-healing-e2e.yaml
```

---

## Part 6: P0~P2 差距点 → 模块 → Pack 完整追溯矩阵

> 确保 Part 1 中识别的每一个差距点都在 Part 2 和 Part 3 中被完整覆盖，且贯通 Part 0 的十二大设计原则。
>
> ⚠️ **2026-04-30 校准注**：下表 ✅ = "Pack 落地 + 算法存在"。**实际生产路径深度**以
> [`docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md`](../../docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md) §3 为准。
> **#1 / #5 / #8** 仍对应 **`landed-stub`** Pack：**DW-001**（自编辑扫描）、**DW-002**（preflight digester）、**DW-004**（DK advisory store）。**#3（Self-Healing）** 在 truth-loop iter-4/6 后已为 **browser + provider API-key 真探针**；**MCP / 更深 Provider liveness** 等仍属后续迭代（见 GAP §9.3）。**WU-009-deep-utility-handle-threading** 仍负责把 **`UtilityLlm`** 与 **进程级 `KnowledgeStore`** 串到 DW-001/002/004 的生产路径。

| #   | 差距点                      | 优先级 | Harness 原则                              | 覆盖模块     | 实施 Pack(s)              | 状态                                                                                                                                                 |
| --- | --------------------------- | ------ | ----------------------------------------- | ------------ | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Token 效率: 120K→30K        | P0     | #8 最小上下文 / #10 无管理层              | Module A     | FEAT-TE-001~004           | ⚠️ **`landed-stub`**（DW-002：`UtilityLlm` 占位 → 生产 digester 恒等）                                                                                |
| 2   | Skill 自动沉淀              | P0     | #7 技能即文件 / #11 贡献回写              | Module B     | FEAT-SE-001, 002          | ✅                                                                                                                                                    |
| 3   | Self-Healing Daemon         | P0     | #3 进程即状态 / #4 自愈闭环 / #9 幂等宽恕 | Module C     | FEAT-SH-001, 002          | 🟡 **partial**（daemon：`BrowserRegistryProbe` + `ProviderApiKeyProbe` 已进 `spawn_self_healing_daemon_with_extras`；**MCP 等**仍 stub，见 GAP §9.3） |
| 4   | Constitutional Memory (L0)  | P1     | #12 完整自由（边界）                      | Module D     | FEAT-SE-003               | ✅                                                                                                                                                    |
| 5   | Agent 自编辑工具能力        | P1     | #2 Agent 自编辑 Harness / #12 完整自由    | Module E     | FEAT-AE-001               | ⚠️ **`landed-stub`**（DW-001：`MockUtilityLlm` / 空扫描输入）                                                                                         |
| 6   | 浏览器会话自修复            | P1     | #4 自愈闭环 / #6 连接用户浏览器           | Module F     | FEAT-SH-003               | ✅                                                                                                                                                    |
| 7   | Skill 向量语义搜索          | P1     | #7 技能即文件（检索升级）                 | Module I     | FEAT-SE-004               | ✅                                                                                                                                                    |
| 8   | 域知识编码+Agent 创作       | P1     | #7 技能即文件 / #11 贡献回写              | Module G     | FEAT-DK-001, 003          | ⚠️ **`landed-stub`**（DW-004：`MockKnowledgeStore` advisory）                                                                                         |
| 9   | 动态工具自创注册            | P1     | #2 Agent 自编辑 Harness                   | Module E + B | FEAT-AE-001 + SE-001      | ✅                                                                                                                                                    |
| 10  | Working Checkpoint 短期注入 | P1     | #8 最小上下文（关键信息防丢失）           | Module H     | FEAT-DK-002               | ✅                                                                                                                                                    |
| 11  | 分层注入替代原始历史        | P1     | #8 最小上下文 / #10 无管理层              | Module A + D | FEAT-TE-001, 003 + SE-003 | ✅                                                                                                                                                    |
| 12  | 执行验证记忆写入            | P2     | #9 幂等宽恕（只记录确定结果）             | Module K     | FEAT-AE-002               | ✅                                                                                                                                                    |
| 13  | 坐标优先浏览器交互          | P2     | #5 坐标优先交互                           | Module J     | FEAT-BR-002               | ✅                                                                                                                                                    |
| 14  | 浏览器内容 HTML 简化        | P2     | #1 极简零框架 / #8 最小上下文             | Module L     | FEAT-BR-001               | ✅                                                                                                                                                    |
| 15  | 工具原子性整合              | P2     | #10 无管理层（工具极简）                  | 跨模块       | FEAT-BR-003               | ✅                                                                                                                                                    |

**覆盖统计**: 15/15 差距点 → 14 模块 (12 后端 + 2 新发现) → 22 后端 Packs + 2 新发现 Pack → 31 前端文件 (20 新建 + 11 修改) + 61 已实现文件, 12 原则完整覆盖, 前后端全栈追溯完毕。

**代码实现进度 (2026-04-30 真值)**: 14 模块中 🟢 **完整/产品级闭环** 主要为 **M、N**；**A、B、D、G、H、J、L** 为 **算法 + 主路径已落地**（其中 **#1、#5、#7（域知识）** 关联的 **DW-001 / DW-002 / DW-004 / WU-002** 为 **`landed-stub`**，见表头校准注）；**C、E、F、I、K** 为 **🟡 partial** 或含 stub。**请勿再使用**「🔴5 个待实现 (A/B/D/G/J/L)」——该句为 rollout 前快照，已构成二次误真。

### Principle → Module 正向索引

| 原则                    | 差距#             | 承载模块                         | 实施 Pack                                 |
| ----------------------- | ----------------- | -------------------------------- | ----------------------------------------- |
| #1 极简零框架           | #14               | L (HTML 简化器)                  | FEAT-BR-001                               |
| #2 Agent 自编辑 Harness | #5, #9            | E (Self-Edit)                    | FEAT-AE-001                               |
| #3 进程即状态           | #3                | C (Daemon)                       | FEAT-SH-001                               |
| #4 自愈闭环             | #3, #6            | C + F (Browser Session)          | FEAT-SH-001~003                           |
| #5 坐标优先交互         | #13               | J (Coordinate Strategy)          | FEAT-BR-002                               |
| #6 连接用户浏览器       | #6                | F (Session Health)               | FEAT-SH-003                               |
| #7 技能即文件           | #2, #7, #8        | B + G + I                        | FEAT-SE-001, 002, 004; FEAT-DK-001, 003   |
| #8 最小上下文哲学       | #1, #10, #11, #14 | A + H + L                        | FEAT-TE-001~004; FEAT-DK-002; FEAT-BR-001 |
| #9 幂等宽恕             | #3, #12           | C + K (Verification Gate)        | FEAT-SH-001; FEAT-AE-002                  |
| #10 无管理层            | #1, #11, #15      | A (Tier Budget) + 跨模块         | FEAT-TE-001, 003; FEAT-BR-003             |
| #11 贡献回写            | #2, #8            | B + G                            | FEAT-SE-001, 002; FEAT-DK-001, 003        |
| #12 完整自由            | #4, #5            | D (Constitution) + E (Self-Edit) | FEAT-SE-003; FEAT-AE-001                  |

### Pack → Module → Phase 反向索引

| Phase | Pack ID      | 名称                     | 关联模块 | 关联差距# |
| ----- | ------------ | ------------------------ | -------- | --------- |
| 1     | FEAT-TE-001  | 上下文层级预算硬约束     | A        | #1, #11   |
| 1     | FEAT-TE-002  | 消息级压缩               | A        | #1        |
| 1     | FEAT-TE-003  | 迷你索引构建器           | A        | #1, #11   |
| 1     | FEAT-TE-004  | 工具结果摘要化           | A        | #1        |
| 2     | FEAT-SE-001  | Skill 沉淀引擎           | B        | #2, #9    |
| 2     | FEAT-SE-002  | Skill 去重+合并          | B        | #2        |
| 2     | FEAT-SE-003  | 宪法级记忆层             | D        | #4, #11   |
| 2     | FEAT-SE-004  | Skill 向量语义搜索       | I        | #7        |
| 3     | FEAT-SH-001  | Self-Healing Daemon 框架 | C        | #3        |
| 3     | FEAT-SH-002  | Provider + MCP 活性检查  | C        | #3        |
| 3     | FEAT-SH-003  | 浏览器会话自修复         | F        | #6        |
| 4     | FEAT-AE-001  | 工具失败分析+自编辑提案  | E        | #5, #9    |
| 4     | FEAT-AE-002  | 执行验证记忆写入         | K        | #12       |
| 4     | FEAT-AE-003  | 分级失败升级             | E        | #5        |
| 5     | FEAT-INT-001 | 前端投影扩展             | 全部     | 集成      |
| 5     | FEAT-INT-002 | 端到端 harness 验证      | 全部     | 集成      |
| 6     | FEAT-DK-001  | 域知识仓库               | G        | #8        |
| 6     | FEAT-DK-002  | 工作检查点系统           | H        | #10       |
| 6     | FEAT-DK-003  | 域知识自动贡献           | G + K    | #8, #12   |
| 7     | FEAT-BR-001  | 浏览器内容简化器         | L        | #14       |
| 7     | FEAT-BR-002  | 坐标优先浏览器策略       | J        | #13       |
| 7     | FEAT-BR-003  | 工具原子性整合           | 跨模块   | #15       |
| 🆕     | FEAT-IA-001  | Jiaochang Audio 文档化   | M        | 新发现    |
| 🆕     | FEAT-MW-001  | MCP Workbench 文档化     | N        | 新发现    |

---

## 附录: 关键文件参考

### 新建文件（规划名 → 2026-04-30 实际落点）

> 下列为 rollout 后的**真路径**；括号内为原规划单文件命名。

| 实际路径（或内联位置）                                                                        | 模块     |
| --------------------------------------------------------------------------------------------- | -------- |
| `src-tauri/src/modules/runtime/context_compression/`（原 `context_compression.rs`）           | Module A |
| `src-tauri/src/modules/skills/sedimentation/`（原 `sedimentation.rs` + `deduplication.rs`）   | Module B |
| `src-tauri/src/modules/runtime/daemon/mod.rs`、`health_check.rs`、`recovery.rs`               | Module C |
| `src-tauri/src/modules/skills/guard/constitution.rs`（非 `runtime/constitution.rs`）          | Module D |
| `learning/`、`stream_tool_execution`、self-edit scanner（`runtime/self_edit.rs` 或等价拆分）  | Module E |
| `src-tauri/src/modules/smart_browser/session_health.rs` + `runtime.rs` + `browser/session.rs` | Module F |
| `src-tauri/src/modules/skills/domain_knowledge/`（原 `domain_knowledge.rs`）                  | Module G |
| `src-tauri/src/modules/runtime/working_checkpoint.rs`                                         | Module H |
| `src-tauri/src/modules/skills/vector_index.rs` 等（原 `vector_search.rs`）                    | Module I |
| `smart_browser/runtime.rs` 内坐标策略（原 `coordinate_strategy.rs`）                          | Module J |
| `memory_quality_gate.rs` / `memory_write_policy.rs` 等（原 `verification_gate.rs`）           | Module K |
| `smart_browser/runtime.rs` 内简化管线（原 `content_simplifier.rs`）                           | Module L |

### 需修改的现有文件 (核心)
| 文件路径                                                                  | 修改内容                                                        | 关联模块 |
| ------------------------------------------------------------------------- | --------------------------------------------------------------- | -------- |
| `src-tauri/src/modules/application/turn_service/stream_preflight.rs`      | 接入上下文压缩器 + checkpoint 注入                              | A, H     |
| `src-tauri/src/modules/application/turn_service/stream_finalize.rs`       | Skill 沉淀钩子 + 域知识提取 + checkpoint 提取                   | B, G, H  |
| `src-tauri/src/modules/application/turn_service/stream_tool_execution.rs` | 终结前自编辑尝试 + 分级升级                                     | E        |
| `src-tauri/src/modules/application/turn_service/work_loop.rs`             | 向量搜索替换关键词匹配 + 域知识注入                             | G, I     |
| `src-tauri/src/modules/runtime/budget.rs`                                 | 层级预算常量 + tier 配置                                        | A        |
| `src-tauri/src/modules/runtime/self_repair.rs`                            | 重构为 daemon 框架入口                                          | C        |
| `src-tauri/src/modules/skills/mod.rs`                                     | 注册 sedimentation/deduplication/domain_knowledge/vector_search | B, G, I  |
| `src-tauri/src/modules/memory/mod.rs`                                     | 注册 verification_gate                                          | K        |
| `src-tauri/src/modules/application/prompt_planner/`                       | Constitution block kind                                         | D        |
| `src-tauri/src/modules/smart_browser/runtime.rs`                          | 健康监控 + 坐标策略 + 内容简化                                  | F, J, L  |
| `src-tauri/src/modules/browser/session.rs`                                | 心跳 + 崩溃检测                                                 | F        |
| `src-tauri/src/modules/tools/builtin/` (memory_store)                     | 接入执行验证门                                                  | K        |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs`                     | 接入 HTML 简化器                                                | L        |
| `src/runtime-projection/runtime-event-reducer.ts`                         | 新事件: daemon_health, sedimented, checkpoint 等                | 全部     |
| `src/runtime-projection/runtime-event-translator.ts`                      | 新事件翻译                                                      | 全部     |

---

## Part 7: 前端 UI/UX 对等设计

> 本部分为 Part 2 中 12 个后端新模块设计对应的前端 UI/UX 组件。所有设计基于当前前端架构（React 19 + TypeScript + Tailwind 4 + shadcn/ui + useSyncExternalStore 投影管道），复用现有设计系统和组件库。

### 7.1 前端现状与架构复用 (2026-04 更新)

**现有前端代码规模**: 298 TS/TSX 文件, 7 stores, 11 API facades, 11 domain modules, ~140 components.

**现有前端优势（无需重建）**：
- **投影管道**：`runtime-projection-bridge.ts` → `runtime-event-translator.ts` → `runtime-event-reducer.ts` → `runtime-projection-store.ts`（45 files, ~3000 LOC），支持事件驱动的状态投影
- **七层 Store 体系**：BootstrapStore / SessionStore / ChatStore / ConversationSlice / RuntimeProjectionStore / BrowserSlice / 各模块内 stores (useSyncExternalStore)
- **成熟 UI 组件库**：shadcn/ui + Radix-UI 13 个包 + lucide-react 图标 + sonner toast
- **虚拟化列表**：@tanstack/react-virtual (50+ 消息自动切换)
- **ContextBar 预算可视化**：5 段 token 预算条，已为 Module A 预留扩展点
- **RunInspectorPanel**：工具执行状态追踪，已为 Module C 预留扩展点
- **MemoryWriteCard / MemoryBrowser**：记忆决策展示，已为 Module D/K 预留扩展点
- **SkillsHubView / SkillEditor**：技能管理 UI，已为 Module B/G/I 预留扩展点
- **ExecutionModePill**：执行模式预览 (M2.6)，已实现
- **SmartBrowserCockpit**：Smart Browser 操控台，已实现
- **Voice 组件**：AgentVoiceIndicator, SttButton, TtsProfilePicker，已实现
- **MCP Workbench**：MCP server 管理页面，已实现
- **Jiaochang 前端**：60 files（游戏可视化+音频+i18n），已实现
- **Git 模块**：api.ts (branch detection, repo status)，已实现
- **Memory Debug / Prompt Diagnostics**：调试页面，已实现

**前端差距 (2026-04-30 真值)**: `src/components/chat/evolution/` 已落地 **EvolutionDevDrawer** 及 **6 个子面板**（`DaemonHealthDashboard`、`CompressionHistoryTable`、`SkillSedimentationTimeline`、`SelfEditPanel`、`EvolutionMiscPanel` 等），`App.tsx` 订阅 `runtime_event` 并调用 **`evolutionEventStore.applyEnvelope`**（详见 GAP 报告 §3.3）。因此下列历史表述 **不再成立**：「6 类可视化全不存在」。**仍可能存在的差距**：生产事件密度不足、部分面板仍为 dev 级、与 **landed-stub** 后端状态对不齐时的空数据体验——归 **iteration 3（WU-009）** 与后续 UX 打磨。

**关键结论**: 前端基础远超 spec 原始假设（298 files vs 31 planned changes）。spec Part 7 从"全部新建"调整为"在成熟基线上升级"；Evolution 抽屉已提供 **首版可观测性**，后续迭代聚焦数据真实性与交互深度。

### 7.2 新增投影事件类型

为支持 12 个新模块的状态投影，需要扩展 `RuntimeEventType` 和 `CorrelationIds`：

```typescript
// 扩展 src/transport/contracts.ts
export type RuntimeEventType =
  | 'conversation' | 'tool' | 'permission' | 'memory' | 'activation'
  | 'execution_mode' | 'harness' | 'system'
  // ── 新增 ──
  | 'daemon_health'        // Module C: daemon 状态变更
  | 'skill_sedimented'     // Module B: 新 skill 自动生成
  | 'compression_event'    // Module A: 上下文压缩完成
  | 'constitution_violation'// Module D: 宪法违规警报
  | 'self_edit_proposal'   // Module E: 自编辑提案
  | 'browser_health'       // Module F: 浏览器会话健康
  | 'domain_knowledge'     // Module G: 域知识贡献
  | 'checkpoint_updated'   // Module H: 工作检查点更新
  | 'verification_decision'// Module K: 记忆验证决策
  | 'content_simplified'   // Module L: HTML 简化完成
```

**投影 Store 新字段** (扩展 `RuntimeProjectionStore`):

```typescript
// src/runtime-projection/types.ts
interface RuntimeProjectionState {
  // ... 现有字段
  daemon: {
    status: 'idle' | 'running' | 'degraded' | 'recovering'
    healthChecks: Record<string, HealthCheckResult>  // provider/mcp/browser/memory
    lastIncident?: { timestamp: string; action: string; result: string }
  }
  skills: {
    recentlySedimented: SkillCard[]     // 新沉淀的 skills
    vectorSearchResults?: SkillSearchResult[]
    pendingReview: SedimentedSkill[]    // 待审查的
  }
  domainKnowledge: {
    contributed: DomainKnowledgeEntry[] // 最新贡献
    accessCount: Record<string, number> // 使用热度
  }
  workingCheckpoint: {
    current?: WorkingCheckpoint         // 当前任务 checkpoint
    history: WorkingCheckpoint[]        // checkpoint 历史
  }
  browser: {
    // ... 现有字段
    sessionHealth?: BrowserSessionHealth
    simplifiedContent?: SimplifiedContentMetadata
  }
  verification: {
    recentDecisions: VerificationDecision[]
    rejectedCount: number
  }
}
```

### 7.3 按模块的前端组件设计

---

#### 7.3.A 上下文压缩可视化 (Module A)

**已有基线**: `ContextBar.tsx` — 5 段 token 预算条 + 压缩按钮 + 会话累计行。

**新增强**:

| 组件                       | 位置                                                     | 功能                                                                                                                   |
| -------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `CompressionEventToast`    | `useCompressionToasts.ts` (new hook)                     | 压缩完成后 sonner toast: "上下文已压缩: 120K → 28K (节省 76%)"                                                         |
| `TierBreakdownTooltip`     | `ContextBar.tsx` 增强                                    | 悬停各段时显示 tier 明细: Constitution(200) / MiniIndex(500) / ActiveFacts(1000) / TaskSOP(2000) / RecentHistory(剩余) |
| `CompressionHistoryDrawer` | `src/components/chat/CompressionHistoryDrawer.tsx` (new) | 按时间线展示压缩记录: before/after token 数 + 压缩率 + 触发原因                                                        |

**数据流**:
```
backend compress_for_request() 
  → emit 'compression_event' { before: 120000, after: 28500, tiers: {...} }
  → runtime-projection-bridge translateCompressionEvent()
  → projectionStore.compression.lastEvent
  → CompressionEventToast + ContextBar re-render
```

---

#### 7.3.B 技能沉淀看板 (Module B)

**已有基线**: `SkillsSettingsPage.tsx` — 技能列表 + 审查流程。

**新增强**:

| 组件                         | 位置                                                         | 功能                                                                              |
| ---------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| `SkillSedimentationTimeline` | `src/components/skills/SkillSedimentationTimeline.tsx` (new) | 时间线视图：每个成功任务→生成的 skill 卡片，带触发条件和置信度评分                |
| `SedimentedSkillCard`        | `src/components/skills/SedimentedSkillCard.tsx` (new)        | 新沉淀 skill 的卡片：步骤列表 + 前置条件 + 避免坑点 + "review/approve/reject" CTA |
| `SkillDedupComparison`       | `src/components/skills/SkillDedupComparison.tsx` (new)       | 并排比较：新沉淀 skill vs 已有相似 skill，高亮差异，一键合并                      |

**数据流**:
```
backend sedimentation engine (post stream_finalize)
  → emit 'skill_sedimented' { skill, dedup_score, similar_skills[] }
  → projectionStore.skills.recentlySedimented.push(skill)
  → SkillSedimentationTimeline re-render
  → (if low dedup_score) sonner: "新技能沉淀: 'Amazon 商品搜索' (置信度 0.87)"
```

---

#### 7.3.C Daemon 健康仪表盘 (Module C)

**新增组件**:

| 组件                    | 位置                                                    | 功能                                                           |
| ----------------------- | ------------------------------------------------------- | -------------------------------------------------------------- |
| `DaemonHealthDashboard` | `src/components/daemon/DaemonHealthDashboard.tsx` (new) | 4 象限仪表盘: Provider活性 / MCP进程 / 浏览器会话 / 记忆Ticker |
| `DaemonStatusIndicator` | `src/components/daemon/DaemonStatusIndicator.tsx` (new) | 全局导航栏小指示器: 🟢运行中 / 🟡降级 / 🔴宕机 / 🔵恢复中          |
| `RecoveryActionLog`     | `src/components/daemon/RecoveryActionLog.tsx` (new)     | 最近恢复操作日志: 时间戳 + 触发原因 + 恢复动作 + 结果          |
| `HealthCheckDetail`     | `src/components/daemon/HealthCheckDetail.tsx` (new)     | 单个健康检查的详情: 上次心跳 / 失败次数 / 恢复策略 / 手动触发  |

**UI 布局**:
```
┌────────────────────────────────────────────┐
│  Daemon 健康仪表盘                          │
│  ┌──────────┬──────────┬──────────┬──────┐ │
│  │ Provider  │ MCP      │ Browser  │Memory│ │
│  │  🟢 Live  │ 🟢 Live  │ 🟡 Stale │🟢 OK │ │
│  │  p50: 0.8s│ p50: 1.2s│ 恢复中...│      │ │
│  └──────────┴──────────┴──────────┴──────┘ │
│  ┌──────────────────────────────────────┐   │
│  │ 恢复日志                              │   │
│  │ 14:32 Browser CDP 会话过期 → 自动重建 │   │
│  │ 14:28 MCP 进程超时 → 自动重启        │   │
│  └──────────────────────────────────────┘   │
└────────────────────────────────────────────┘
```

**数据流**:
```
backend daemon heartbeat loop
  → emit 'daemon_health' { status, checks: {provider, mcp, browser, memory} }
  → projectionStore.daemon.status = status
  → DaemonStatusIndicator (navbar) + DaemonHealthDashboard (panel)
```

**集成点**: `DaemonStatusIndicator` 挂在 `GlobalNavbar.tsx` 中，始终可见。点击打开 `DaemonHealthDashboard` 侧面板。

---

#### 7.3.D 宪法层查看与编辑器 (Module D)

**已有基线**: `MemoryBrowser.tsx` + `CompiledMemoryViewer.tsx`。

**新增强**:

| 组件                         | 位置                                                            | 功能                                                                                              |
| ---------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `ConstitutionViewer`         | `src/components/memory/ConstitutionViewer.tsx` (new)            | 只读宪法层展示：按类别分组(操作安全/隐私/准确性/效率)，每条的优先级和来源                         |
| `ConstitutionViolationAlert` | `src/components/chat/ConstitutionViolationAlert.tsx` (new)      | 宪法违规时在消息流中插入红色警报卡: "Agent 试图 写入未经验证的数据 — 操作已被阻止 (宪法规则 #12)" |
| `ConstitutionYamlEditor`     | `src/modules/settings/pages/ConstitutionSettingsPage.tsx` (new) | 设置中的宪法编辑页面：YAML 编辑器 + 语法校验 + 预览注入效果                                       |

**数据流**:
```
backend constitution injection (preflight)
  → emit 'constitution_violation' { rule_id, violation, action_taken }
  → projectionStore.memory.violationLog.push(violation)
  → ConstitutionViolationAlert 插入消息流
```

---

#### 7.3.E Agent 自编辑界面 (Module E)

**新增组件**:

| 组件                   | 位置                                                  | 功能                                                                     |
| ---------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------ |
| `SelfEditProposalCard` | `src/components/chat/SelfEditProposalCard.tsx` (new)  | 自编辑提案卡片：失败模式 + 建议修改 + diff preview + approve/reject 按钮 |
| `ToolFailureAnalysis`  | `src/components/tools/ToolFailureAnalysis.tsx` (new)  | 工具失败分析视图：失败次数 + 错误签名 + 建议修复方案                     |
| `SelfEditHistoryPanel` | `src/components/tools/SelfEditHistoryPanel.tsx` (new) | 自编辑历史：应用的修改 + 效果（改善/恶化）+ 回滚按钮                     |

**UI 布局**:
```
┌───────────────────────────────────────┐
│ ⚠️ upload_file() 已连续失败 4 次      │
│                                       │
│ 失败签名: DOM selector timeout        │
│                                       │
│ 建议修复:                              │
│ ────────────────────────────────────  │
│ - upload_file(selector, path)         │
│ + upload_file_retry(selector, path,   │
│     timeout=15.0)                     │
│                                       │
│ [查看 Diff]  [批准并应用]  [拒绝]      │
└───────────────────────────────────────┘
```

**数据流**:
```
backend self_edit engine detects failure pattern
  → emit 'self_edit_proposal' { failure_pattern, proposal, diff }
  → projectionStore.selfEdit.proposals.push(proposal)
  → SelfEditProposalCard rendered in message stream
```

---

#### 7.3.F 浏览器会话健康监控 (Module F)

**新增强**:

| 组件                      | 位置                                                       | 功能                                                         |
| ------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------ |
| `BrowserSessionHealthBar` | `src/components/browser/BrowserSessionHealthBar.tsx` (new) | 浏览器卡片旁的小健康条: CDP 连接状态 + 上次心跳 + 会话年龄   |
| `BrowserRecoveryToast`    | `useBrowserRecoveryToast.ts` (new hook)                    | 恢复操作 toast: "浏览器 CDP 会话过期 → 自动重建 (耗时 1.2s)" |

**集成点**: 在现有的 `BrowserCard.tsx` 中添加健康指示器。在 Daemon 仪表盘中集成浏览器健康检查详情。

---

#### 7.3.G 域知识浏览器 (Module G)

**新增组件**:

| 组件                                | 位置                                                                | 功能                                                                         |
| ----------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `DomainKnowledgeBrowser`            | `src/components/skills/DomainKnowledgeBrowser.tsx` (new)            | 按域名/任务类型浏览域知识：网站域知识(selector/gotcha) + 交互原语 + 任务SOP  |
| `DomainKnowledgeCard`               | `src/components/skills/DomainKnowledgeCard.tsx` (new)               | 单条域知识卡片：内容摘要 + 作者(Agent/User/Imported) + 置信度 + 最后验证时间 |
| `DomainKnowledgeContributionDialog` | `src/components/skills/DomainKnowledgeContributionDialog.tsx` (new) | Agent 提议贡献域知识时的确认对话框：内容预览 + 证据引用 + 确认/拒绝          |
| `SelectorStabilityIndicator`        | `src/components/skills/SelectorStabilityIndicator.tsx` (new)        | 选择器稳定性标记: 绿色(稳定)/黄色(脆弱)/灰色(未测试) + 最后验证时间          |

**UI 布局**:
```
┌────────────────────────────────────────────────────┐
│  域知识浏览器                           [+ 贡献]    │
│  ┌──────────┬────────────────────────────────────┐ │
│  │ 筛选:    │ amazon.com                          │ │
│  │ amazon   │ ┌────────────────────────────────┐ │ │
│  │ github   │ │ 选择器知识                      │ │ │
│  │ gmail    │ │ #productTitle → 商品标题  🟢稳定│ │ │
│  │ linkedin │ │ #addToCart   → 加入购物车 🟡脆弱│ │ │
│  │ ...      │ │ 陷阱: 搜索结果70%概率被CAPTCHA │ │ │
│  │          │ └────────────────────────────────┘ │ │
│  │          │ ┌────────────────────────────────┐ │ │
│  │          │ │ 任务 SOP: 批量下载订单发票      │ │ │
│  │          │ │ 作者: Agent (session_abc123)   │ │ │
│  │          │ │ 置信度: 0.91 | 上次验证: 2h前  │ │ │
│  │          │ └────────────────────────────────┘ │ │
│  └──────────┴────────────────────────────────────┘ │
└────────────────────────────────────────────────────┘
```

**数据流**:
```
backend domain_knowledge contribution (post stream_finalize)
  → emit 'domain_knowledge' { entry, kind, verification_evidence }
  → projectionStore.domainKnowledge.contributed.push(entry)
  → DomainKnowledgeBrowser re-render (if open)
  → sonner: "域知识贡献: amazon.com 选择器库已更新"
```

---

#### 7.3.H 工作检查点查看器 (Module H)

**新增组件**:

| 组件                        | 位置                                                   | 功能                                                          |
| --------------------------- | ------------------------------------------------------ | ------------------------------------------------------------- |
| `WorkingCheckpointBar`      | `src/components/chat/WorkingCheckpointBar.tsx` (new)   | ContextBar 下方的小条: 当前 checkpoint 关键信息 (<200 tokens) |
| `CheckpointHistoryPanel`    | `src/components/chat/CheckpointHistoryPanel.tsx` (new) | 检查点历史时间线: 每轮 checkpoint 内容 + 关联 SOP + 更新时间  |
| `CheckpointInjectIndicator` | `ContextBar.tsx` 增强                                  | 当 checkpoint 被注入到请求时，显示小标记 "(+cp 187 tokens)"   |

**UI 布局**:
```
┌──────────────────────────────────────────────────────────┐
│  Token 预算 [■■■■■■■■■■■■■■■■■■■■░░░░]  22K / 30K      │
│  ⚡ 工作检查点: 正在编辑 cart-service.ts 第42行, 需要保留  │
│     CORS 配置 (来自 turn 7)                   (+cp 156t)  │
└──────────────────────────────────────────────────────────┘
```

**数据流**:
```
backend checkpoint extraction (post stream_finalize)
  → projectionStore.workingCheckpoint.current = checkpoint
  → WorkingCheckpointBar re-render

backend checkpoint injection (preflight)
  → projectionStore.workingCheckpoint.lastInjected = checkpoint
  → CheckpointInjectIndicator re-render
```

---

#### 7.3.I Skill 语义搜索结果展示 (Module I)

**新增强**: 在现有的 `SkillsHubView.tsx` 搜索结果中增加嵌入相似度评分和匹配原因。

| 组件                    | 位置                                                    | 功能                                                                               |
| ----------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `VectorSearchScore`     | `SkillsHubView.tsx` 增强                                | 搜索结果卡片新增: 向量相似度分数 + 匹配原因 (e.g. "匹配关键词: 文件上传, DOM操作") |
| `SkillSearchResultCard` | `src/components/skills/SkillSearchResultCard.tsx` (new) | 增强版搜索结果卡片：综合评分(向量相似度 × 质量评分) + 匹配片段高亮 + 使用频次      |

**数据流**: 现有的 `skill_search` 工具已能返回结果，前端增加评分展示和处理即可。

---

#### 7.3.J 坐标优先交互预览 (Module J)

**新增组件**:

| 组件                       | 位置                                                        | 功能                                                           |
| -------------------------- | ----------------------------------------------------------- | -------------------------------------------------------------- |
| `CoordinateClickOverlay`   | `src/components/browser/CoordinateClickOverlay.tsx` (new)   | 浏览器截图上叠加点击坐标标记：红圈 + 十字线 + 坐标值           |
| `InteractionStrategyChain` | `src/components/browser/InteractionStrategyChain.tsx` (new) | 展示降级链: 坐标点击 → 标签引用 → CSS 选择器，当前使用策略高亮 |
| `ClickVerificationPreview` | `src/components/browser/ClickVerificationPreview.tsx` (new) | 点击前后截图对比：左 before / 右 after，差异区域高亮           |

**UI 布局**:
```
┌───────────────────────────────────────────┐
│  浏览器截图                                │
│  ┌─────────────────────────────────────┐  │
│  │                                     │  │
│  │              ⊕ (234, 567)           │  │
│  │              ──+──                  │  │
│  │                │                    │  │
│  │                                     │  │
│  └─────────────────────────────────────┘  │
│  策略: 坐标点击 ✓  → 标签引用 → CSS选择器 │
│  点击验证: Before / After  (差异: 菜单已打开)│
└───────────────────────────────────────────┘
```

**集成点**: 在 `BrowserViewerPage.tsx` 的截图显示区域叠加坐标标记层。

---

#### 7.3.K 执行验证门状态 (Module K)

**已有基线**: `MemoryWriteCard.tsx` — 决策展示(allow/deny/prompt)。

**新增强**:

| 组件                     | 位置                                                   | 功能                                                                                                |
| ------------------------ | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `VerificationGateStatus` | `MemoryWriteCard.tsx` 增强                             | 写入请求显示验证结果: ✓ 已验证(证据: tool_call_abc) / ✗ 已拒绝(原因: 无工具证据) / ⚡ 豁免(宪法规则) |
| `VolatileContentWarning` | `MemoryWriteCard.tsx` 增强                             | 检测到易变内容时高亮警告: "内容包含时间戳/会话ID - 拒绝存储"                                        |
| `VerificationAuditLog`   | `src/components/memory/VerificationAuditLog.tsx` (new) | 验证历史日志: 所有写入请求的验证结果 + 拒绝原因统计                                                 |

**数据流**:
```
backend verification gate decision
  → emit 'verification_decision' { decision, evidence, reason }
  → projectionStore.verification.recentDecisions.push(decision)
  → MemoryWriteCard 根据 verification 字段显示状态
```

---

#### 7.3.L HTML 简化效果展示 (Module L)

**新增组件**:

| 组件                     | 位置                                                      | 功能                                                        |
| ------------------------ | --------------------------------------------------------- | ----------------------------------------------------------- |
| `SimplificationDiffView` | `src/components/browser/SimplificationDiffView.tsx` (new) | 并排比较: 原始 HTML 200K vs 简化后 25K，差异字符数 + 压缩率 |
| `KeyElementsList`        | `src/components/browser/KeyElementsList.tsx` (new)        | 提取的关键交互元素列表: tag, id, name, type, text, href     |
| `TokenSavingsBadge`      | `ToolCallMessage.tsx` 增强                                | web_scan 工具结果中显示"Token 节省: 93% (200K→14K)"徽章     |

**UI 布局**:
```
┌──────────────────────────────────────────────┐
│  web_scan 结果                               │
│  ┌────────────────────────────────────────┐  │
│  │ Token 节省: 93% 🏷️ 原始 200K → 14K    │  │
│  │                                        │  │
│  │ 简化后 HTML (可交互元素):              │  │
│  │ ┌─────────────────────────────────┐   │  │
│  │ │ <main><form id="search">...</>  │   │  │
│  │ │ <input name="q" type="text">    │   │  │
│  │ │ <button id="submit">搜索</>     │   │  │
│  │ └─────────────────────────────────┘   │  │
│  │                                        │  │
│  │ 关键元素: 3 forms, 5 inputs, 2 buttons │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**数据流**:
```
backend simplifier (web_scan tool)
  → emit 'content_simplified' { original_chars, simplified_chars, key_elements }
  → projectionStore.browser.simplifiedContent = metadata
  → ToolCallMessage renders TokenSavingsBadge + 简化内容
```

---

### 7.4 前端新增文件清单 (24 个)

| 文件路径                                                      | 对应模块 | 类型                          |
| ------------------------------------------------------------- | -------- | ----------------------------- |
| `src/components/chat/CompressionHistoryDrawer.tsx`            | A        | 新组件                        |
| `src/components/chat/useCompressionToasts.ts`                 | A        | 新 hook                       |
| `src/components/skills/SkillSedimentationTimeline.tsx`        | B        | 新组件                        |
| `src/components/skills/SedimentedSkillCard.tsx`               | B        | 新组件                        |
| `src/components/skills/SkillDedupComparison.tsx`              | B        | 新组件                        |
| `src/components/daemon/DaemonHealthDashboard.tsx`             | C        | 新组件                        |
| `src/components/daemon/DaemonStatusIndicator.tsx`             | C        | 新组件                        |
| `src/components/daemon/RecoveryActionLog.tsx`                 | C        | 新组件                        |
| `src/components/daemon/HealthCheckDetail.tsx`                 | C        | 新组件                        |
| `src/components/memory/ConstitutionViewer.tsx`                | D        | 新组件                        |
| `src/components/chat/ConstitutionViolationAlert.tsx`          | D        | 新组件                        |
| `src/modules/settings/pages/ConstitutionSettingsPage.tsx`     | D        | 新页面                        |
| `src/components/chat/SelfEditProposalCard.tsx`                | E        | 新组件                        |
| `src/components/tools/ToolFailureAnalysis.tsx`                | E        | 新组件                        |
| `src/components/tools/SelfEditHistoryPanel.tsx`               | E        | 新组件                        |
| `src/components/browser/BrowserSessionHealthBar.tsx`          | F        | 新组件                        |
| `src/components/browser/useBrowserRecoveryToast.ts`           | F        | 新 hook                       |
| `src/components/skills/DomainKnowledgeBrowser.tsx`            | G        | 新组件                        |
| `src/components/skills/DomainKnowledgeCard.tsx`               | G        | 新组件                        |
| `src/components/skills/DomainKnowledgeContributionDialog.tsx` | G        | 新组件                        |
| `src/components/skills/SelectorStabilityIndicator.tsx`        | G        | 新组件                        |
| `src/components/chat/WorkingCheckpointBar.tsx`                | H        | 新组件                        |
| `src/components/chat/CheckpointHistoryPanel.tsx`              | H        | 新组件                        |
| `src/components/skills/SkillSearchResultCard.tsx`             | I        | 新组件                        |
| `src/components/browser/CoordinateClickOverlay.tsx`           | J        | 新组件                        |
| `src/components/browser/InteractionStrategyChain.tsx`         | J        | 新组件                        |
| `src/components/browser/ClickVerificationPreview.tsx`         | J        | 新组件                        |
| `src/components/memory/VerificationAuditLog.tsx`              | K        | 新组件                        |
| `src/components/browser/SimplificationDiffView.tsx`           | L        | 新组件                        |
| `src/components/browser/KeyElementsList.tsx`                  | L        | 新组件                        |
| `src/lib/toolDisplay.ts`                                      | E/J/L    | 新工具库(从 chat-ui.tsx 提取) |

### 7.5 前端修改文件清单 (11 个)

| 文件路径                                                  | 修改内容                                                                             | 关联模块 |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------ | -------- |
| `src/transport/contracts.ts`                              | 新增 7 种 `RuntimeEventType`                                                         | 全部     |
| `src/runtime-projection/types.ts`                         | 新增 `daemon`, `skills`, `domainKnowledge`, `workingCheckpoint`, `verification` 字段 | 全部     |
| `src/runtime-projection/runtime-event-translator.ts`      | 新增 7 个 translator 函数                                                            | 全部     |
| `src/runtime-projection/runtime-event-reducer.ts`         | 新增 7 个 case handler                                                               | 全部     |
| `src/runtime-projection/runtime-projection-bridge.ts`     | 新增 7 个 listen() 订阅                                                              | 全部     |
| `src/components/chat/ContextBar.tsx`                      | Tier 明细 tooltip + Checkpoint 注入指示 + 会话累计增强                               | A, H     |
| `src/components/chat/ToolCallMessage.tsx` / `chat-ui.tsx` | Token 节省徽章 + 验证门状态显示                                                      | K, L     |
| `src/components/memory/MemoryWriteCard.tsx`               | 验证门状态 + 易变内容警告                                                            | K        |
| `src/components/browser/BrowserCard.tsx`                  | 集成 `BrowserSessionHealthBar`                                                       | F        |
| `src/modules/skills/SkillsHubView.tsx`                    | 向量搜索评分展示                                                                     | I        |
| `src/modules/app-shell/components/GlobalNavbar.tsx`       | 集成 `DaemonStatusIndicator`                                                         | C        |

### 7.6 前端实施优先级 (2026-04 更新)

与 Part 3 后端路线图对齐，但考虑现有前端基线更成熟：

| 后端 Phase             | 前端对应工作                                                                  | 当前前端基线                                         | Pack 关联    |
| ---------------------- | ----------------------------------------------------------------------------- | ---------------------------------------------------- | ------------ |
| Phase 1 (Token 效率)   | `CompressionHistoryDrawer` + ContextBar 增强                                  | ContextBar 已存在（5-segment 可视化）                | FEAT-INT-001 |
| Phase 2 (自我进化)     | `SkillSedimentationTimeline` + `ConstitutionViewer`                           | SkillsHubView 已存在, MemoryBrowser 已存在           | FEAT-INT-001 |
| Phase 3 (Self-Healing) | `DaemonHealthDashboard` + `DaemonStatusIndicator` + `BrowserSessionHealthBar` | SmartBrowserCockpit 已存在, RunInspectorPanel 已存在 | FEAT-INT-001 |
| Phase 4 (自编辑)       | `SelfEditProposalCard` + `VerificationAuditLog`                               | —                                                    | FEAT-INT-001 |
| Phase 6 (域知识)       | `DomainKnowledgeBrowser` + `WorkingCheckpointBar`                             | —                                                    | FEAT-INT-001 |
| Phase 7 (浏览器精炼)   | `CoordinateClickOverlay` + `SimplificationDiffView`                           | BrowserViewerPage 已存在                             | FEAT-INT-001 |
| 🆕 Jiaochang            | ✅ 已完整实现 (60 files)                                                       | Jiaochang frontend 已完成                            | —            |
| 🆕 MCP Workbench        | ✅ 已完整实现                                                                  | McpWorkbench settings page 已完成                    | —            |

**核心原则**: 所有前端增强整合在 `FEAT-INT-001` (前端投影扩展) 一张 Pack 中，与后端各 Phase 同步交付。前端采用 **渐进增强**策略——每个后端 Phase 完成后扩展对应的投影管道和 UI，不阻塞后端开发。新发现的已实现模块 (Jiaochang, MCP Workbench, Control Plane) 标记为 ✅ 已完成，无需新增前端工作。

---

## Part 8: 全栈追溯矩阵 (后端→前端, 含实现状态)

| 后端 Module           | 实现状态 | 后端 Pack       | 前端主组件                                                                                                 | 前端修改文件                                 | 前端新增文件                                                                               |
| --------------------- | -------- | --------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------ |
| A (上下文压缩)        | 🔴 待实现 | FEAT-TE-001~004 | CompressionHistoryDrawer, ContextBar增强                                                                   | `ContextBar.tsx`                             | `CompressionHistoryDrawer.tsx`, `useCompressionToasts.ts`                                  |
| B (技能沉淀)          | 🔴 待实现 | FEAT-SE-001,002 | SkillSedimentationTimeline, SedimentedSkillCard, SkillDedupComparison                                      | —                                            | 3 files                                                                                    |
| C (自愈Daemon)        | 🟡 部分   | FEAT-SH-001,002 | DaemonHealthDashboard, DaemonStatusIndicator, RecoveryActionLog, HealthCheckDetail                         | `GlobalNavbar.tsx`                           | 4 files                                                                                    |
| D (宪法层)            | 🔴 待实现 | FEAT-SE-003     | ConstitutionViewer, ConstitutionViolationAlert                                                             | —                                            | `ConstitutionViewer.tsx`, `ConstitutionViolationAlert.tsx`, `ConstitutionSettingsPage.tsx` |
| E (自编辑)            | 🟡 部分   | FEAT-AE-001,003 | SelfEditProposalCard, ToolFailureAnalysis, SelfEditHistoryPanel                                            | —                                            | 3 files                                                                                    |
| F (浏览器会话)        | 🟡 部分   | FEAT-SH-003     | BrowserSessionHealthBar, useBrowserRecoveryToast                                                           | `BrowserCard.tsx`                            | 2 files                                                                                    |
| G (域知识)            | 🔴 待实现 | FEAT-DK-001,003 | DomainKnowledgeBrowser, DomainKnowledgeCard, DomainKnowledgeContributionDialog, SelectorStabilityIndicator | —                                            | 4 files                                                                                    |
| H (工作检查点)        | 🟡 部分   | FEAT-DK-002     | WorkingCheckpointBar, CheckpointHistoryPanel                                                               | `ContextBar.tsx`                             | 2 files                                                                                    |
| I (向量搜索)          | 🟡 部分   | FEAT-SE-004     | SkillSearchResultCard                                                                                      | `SkillsHubView.tsx`                          | 1 file                                                                                     |
| J (坐标优先)          | 🔴 待实现 | FEAT-BR-002     | CoordinateClickOverlay, InteractionStrategyChain, ClickVerificationPreview                                 | —                                            | 3 files                                                                                    |
| K (验证门)            | 🟡 部分   | FEAT-AE-002     | VerificationAuditLog, MemoryWriteCard增强                                                                  | `MemoryWriteCard.tsx`, `ToolCallMessage.tsx` | 1 file                                                                                     |
| L (HTML简化)          | 🔴 待实现 | FEAT-BR-001     | SimplificationDiffView, KeyElementsList                                                                    | `ToolCallMessage.tsx`                        | 2 files                                                                                    |
| **M (Jiaochang)**     | 🟢 已实现 | —               | JiaochangMusicPlayer, PixelStage, StrategyPanel                                                            | —                                            | 60 files (已存在, 无需新建)                                                                |
| **N (MCP Workbench)** | 🟢 已实现 | —               | McpWorkbenchPage, ExecutionModePill                                                                        | —                                            | ~10 files (已存在, 无需新建)                                                               |

**全栈统计 (2026-04 更新)**: 14 模块 → 22 实现 Packs + 2 新发现模块 → 31 前端新建文件 + 61 已实现前端文件

**实现进度**: 🟢 2/14 已完整实现 | 🟡 7/14 部分实现 | 🔴 5/14 待实现

**新增 Pack 建议** (新发现模块):
| Pack ID       | 名称                   | 范围                                                     | 依赖            |
| ------------- | ---------------------- | -------------------------------------------------------- | --------------- |
| `FEAT-IA-001` | Jiaochang Audio 文档化 | 为 `jiaochang_audio/mod.rs`(1,596 LOC) 补充架构文档+spec | 无 (代码已实现) |
| `FEAT-MW-001` | MCP Workbench 文档化   | 为 `mcp_stdio/` + `control_plane/` 补充架构文档+spec     | 无 (代码已实现) |
