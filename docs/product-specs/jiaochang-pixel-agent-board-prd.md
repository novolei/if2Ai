# PRD: 校场 Pixel Agent 运行看板

> Status: draft
> Owner: product / agent-runtime / frontend
> Created: 2026-04-25
> Target repo: `/Users/ryanliu/Documents/IfAI/if2Ai`
> Reference: `ringhyacinth/Star-Office-UI` feature model + `timeshiftsauce/CeruMusic` music-source/playback strategy + local visual reference `/Users/ryanliu/Downloads/platform.png`
> Live reference links: [Star Office UI](https://github.com/ringhyacinth/Star-Office-UI) / [CeruMusic](https://github.com/timeshiftsauce/CeruMusic)

## 1. 背景

If2Ai 当前已经有 Chat、记忆等一级工作区，但 Agent 的真实运行状态仍主要隐藏在聊天流、工具结果、日志和调试面板里。用户需要一个低认知成本的可视化看板，能一眼看出：

- 当前 Agent 是否在线、在做什么、卡在哪里。
- 当前任务处于思考、检索、执行、写入、同步、错误还是完成。
- 多 Agent / 子任务协作时，每个角色的分工与状态。
- 昨日或最近一次运行留下了什么摘要、产物和风险。

参考 [Star Office UI](https://github.com/ringhyacinth/Star-Office-UI) 的像素办公室方向，本功能在 If2Ai 内新增一级入口 **校场**，以像素化东方庭院 / 武侠校场风格呈现 Agent 运行状态。视觉风格必须严格参考本地 `/Users/ryanliu/Downloads/platform.png`：等距像素、白墙黑瓦、水渠庭院、红木栏杆、旗幡、石板路、江湖人物与明亮清爽色彩。所有校场相关像素图片资产都必须与该附件的构图语言、色彩温度、像素颗粒度、屋顶 / 墙面 / 水体 / 植被表现保持同一美术体系。

## 2. 产品定位

**校场** 是 If2Ai 的 Agent 运行态可视化 cockpit，不是装饰页。它把 If2Ai 已有运行数据、事件流、记忆摘要和多 Agent 协作投射成可巡视的像素场景。

一句话：让用户像巡查门派校场一样，看见 Agent 正在练什么功、卡在哪一式、谁在协作、下一步该不该介入。

## 3. 目标

1. 在左侧全局 sidebar 增加一级入口 `校场`，点击进入独立页面。
2. 新增像素化可视化 Agent 运行看板页面，首屏即为可交互等距校场场景。
3. 复刻并本地化 Star Office UI 的核心能力：状态可视化、多人 Agent、昨日小记、资产定制、AI 生图装修、移动适配、安全边界。
4. 在上述基础上增加音乐播放器，让校场具备独立氛围控制。
5. 使用 If2Ai 现有 Tauri + Rust + React/TypeScript codebase 原生实现，不引入独立 Python/Flask 后端，不引入 Electron desktop-pet。
6. 首版允许 fixture/demo 数据兜底，但最终数据源必须可接入 If2Ai runtime projection / run event log / session summary。
7. 音乐系统需要具备 CeruMusic 式插件音源能力，支持自动获取音乐资源、解析真实播放 URL、缓存、歌词和服务歌单导入。
8. 校场需要支持中英日韩四种语言：`zh-CN`、`en-US`、`ja-JP`、`ko-KR`。
9. 校场未来需要支持独立窗口与 mini mode。
10. AI 生图装修既作为开发期资产生成路径，也作为 app 内用户功能提供。

## 4. 非目标

- 不直接复制 Star Office UI 的美术资源。其 README 标注代码 MIT、美术资产仅学习 / 演示 / 交流用途；If2Ai 应使用 Codex 生成或自有原创资源。
- 不把校场实现塞入 ChatWorkspace 内部。
- 不新建独立服务进程作为主数据源。
- 不在首版实现公网分享、Cloudflare Tunnel 或访客公网 join 流程。
- 不把音乐播放器做成系统级播放器；它仅服务校场页面氛围。

## 5. 用户画像

| 用户 | 需求 |
| --- | --- |
| If2Ai 日常用户 | 想快速知道 Agent 是否在跑、跑到哪、有没有出错。 |
| 多 Agent / subagent 使用者 | 想看到多个 Agent 的在线、分工、协作和卡点。 |
| 开发者 / reviewer | 想从可视化状态快速定位运行事件、工具调用、错误和产物。 |
| 产品体验调试者 | 想用一个高辨识度页面验证 If2Ai 的 runtime projection 是否真的成为 UI 真相源。 |

## 5.1 双视角评审结论

### Staff 系统架构师 / 策略师评审

校场不应被实现成孤立的视觉玩具，而应成为 If2Ai runtime truth 的观察窗。核心策略：

- **数据真相优先于动画**：所有状态必须来自 typed view model，fixture 只能兜底，不能成为第二套真相。
- **先读后控**：MVP 先做观测型 cockpit；控制型动作必须等状态、权限、回滚语义稳定后再加。
- **把复杂性关进 adapter**：runtime 数据、音乐 URL 解析、资产配置、主题持久化都要有 facade，不进入 UI 组件。
- **按 Pack 分期收敛风险**：视觉 Shell、运行数据、多人 Agent、音乐系统、插件音源必须分阶段交付，避免一次性大爆炸。
- **校场可作为 projection adoption testbed**：如果校场展示的 run state 与 Chat / event log 冲突，优先修 projection，而不是在校场本地补丁。

### 前端资深 UI/UX 设计师评审

校场的核心价值不是“看起来可爱”，而是降低运行态焦虑。体验策略：

- **3 秒状态识别**：用户进入页面后，第一眼应知道 Agent 是待命、运行、卡住还是完成。
- **像素场景承载情绪，浮层承载信息**：地图只表现状态和氛围，具体事件、文件、错误放在结构化面板。
- **渐进披露**：默认展示“谁在干什么”；点击 Agent / 区域后再展开时间线、工具、文件和错误细节。
- **让运行过程有节奏感**：音乐、灯笼、水面、旗幡、角色动作与 Agent 状态联动，但不能影响可读性。
- **中式武侠主题要产品化**：不只换皮，应把状态命名、区域、反馈、空态、完成仪式都统一成“校场 / 战报 / 出招 / 收招”的语言系统。

## 5.2 已定产品决策

本轮 PRD 决策如下，后续实现不再作为开放问题处理：

| 决策 | 结论 |
| --- | --- |
| 音乐插件音源 | 需要 CeruMusic 式插件音源能力，作为正式后续 Pack，不再 optional |
| 音乐资源获取 | 按 CeruMusic 策略自动获取：metadata -> adapter/plugin -> resolved URL -> cache -> audio |
| 本地音乐导入 | 需要持久化文件访问权限，支持重启后继续播放授权文件 |
| 校场窗口形态 | 需要未来支持独立窗口和 mini mode |
| AI 生图装修 | 同时用于开发期生成资产和 app 内用户功能 |
| 多语言 | 支持中文、英文、日文、韩文四语 |
| 像素资产风格 | 所有校场图片资产必须严格按 `/Users/ryanliu/Downloads/platform.png` 的样式、色彩、等距视角生成 |
| UI 字体 | 校场标题、按钮、状态气泡、像素标签必须使用对应像素字体；长正文使用可读 CJK 字体兜底 |
| Runtime projection | 后续会稳定提供 subagent identity、tool ledger、run progress；校场现在必须预留 typed 接口 |
| 插件 sandbox | 音源插件 sandbox 采用 Rust side isolate / worker，不采用 renderer 直接执行插件代码 |
| AI 生图 provider | 首版接入 ChatGPT Image 2，并在模型设置页新增对应 provider 配置入口 |

参考 Star Office UI：其当前主线已支持 CN/EN/JP 三语切换，并在字体资产中包含韩文字体；If2Ai 校场需要在此基础上补齐 `ko-KR`，并以结构化 i18n catalog 实现，而不是把翻译散落在组件里。

## 6. 当前代码集成点

必须基于当前 AppShell 架构接入：

- `src/modules/app-shell/types.ts`：`AppSection` 增加 `'jiaochang'`。
- `src/modules/app-shell/components/GlobalNavbar.tsx`：新增 tooltip label `校场` 的导航按钮，建议图标 `Swords` / `Landmark` / `Map` from `lucide-react`。
- `src/app/AppShell.tsx` 与 `src/app/ContentRouter.tsx`：把 `jiaochang` 作为一级 section 路由到新页面。
- `src/modules/app-shell/components/SectionWorkspace.tsx`：不得只保留占位空态，校场必须有真实页面组件。
- 新模块建议：`src/modules/jiaochang/**`。
- 新资产建议：`src/assets/jiaochang/**`。

硬约束：

- UI 不直接订阅 raw Tauri events；优先消费 projection/store selector。
- 若 runtime projection 尚未完整提供所需字段，先做 typed facade + fixture adapter，不把事件解析散落在组件里。
- 所有新增 Tauri IPC 走 `src/lib/tauri.ts` 或模块化 API facade，不在组件内裸写 `invoke(...)`。

### 6.1 Staff 架构策略

推荐模块边界：

```text
src/modules/jiaochang/
  JiaochangPage.tsx
  components/
    PixelStage.tsx
    AgentSprite.tsx
    RunInspector.tsx
    TimelinePanel.tsx
    WarReportCard.tsx
    DecorDrawer.tsx
  data/
    jiaochang-types.ts
    runtime-adapter.ts
    fixture-adapter.ts
    selectors.ts
  audio/
    JiaochangAudioProvider.tsx
    useJiaochangAudio.ts
    audio-state.ts
    audio-events.ts
    source-adapters.ts
    plugin-adapters.ts
    resolve-track-url.ts
    audio-cache.ts
    audio-visualizer.ts
  theme/
    theme-registry.ts
    pixel-stage-layout.ts
  i18n/
    locales.ts
    zh-CN.ts
    en-US.ts
    ja-JP.ts
    ko-KR.ts
  windows/
    JiaochangWindowBridge.ts
    mini-mode.ts
```

数据流必须保持单向：

```text
runtime projection / event log / session summary
  -> jiaochang data adapter
  -> typed view model
  -> React components
  -> user action callbacks
  -> typed facade / explicit command
```

禁止路径：

- React component 直接解析 raw stream event。
- React component 直接调用 Tauri `invoke(...)` 获取 runtime truth。
- 音乐 source adapter 在 UI 组件里拼 URL。
- 同一状态同时由 fixture、projection、localStorage 多处写入。
- 翻译字符串散落在组件内，绕过 i18n catalog。
- 独立窗口 / mini mode 直接复制主页面逻辑，形成第二套运行状态。

### 6.2 数据真相优先级

| 数据 | 主真相 | 兜底 | UI 降级 |
| --- | --- | --- | --- |
| Agent 当前状态 | runtime projection | fixture adapter | 显示 `演示` badge |
| run event timeline | canonical run event log | recent session events | 空态 `暂无出招记录` |
| 多 Agent roster | team/subagent projection | current run participants | 仅显示主 Agent |
| 昨日战报 | session summary / memory narrative | no data | `昨日无战报` |
| 音乐队列 | audio provider state | bundled track registry | `未配置曲目` |
| 主题 | theme registry + localStorage | default theme | `神雕校场` |
| 语言 | app locale setting | OS locale | `zh-CN` |
| 独立窗口状态 | main window projection bridge | last snapshot | 只读 mini 状态 |

## 7. 信息架构

### 7.1 全局入口

左侧 sidebar / GlobalNavbar：

- Label: `校场`
- Tooltip: `校场`
- Active state: 与 Chat、记忆一致。
- 点击后保留当前 chat/session 状态，不打断正在运行的 Agent。

### 7.2 校场页面布局

首屏建议三层结构：

1. **中央像素场景**
   等距校场地图，展示 Agent 小人、区域、状态气泡、路径动画。

2. **右侧运行面板**
   展示当前任务、状态、工具调用、错误、产物、用时、最近事件。

3. **底部 / 左下音乐播放器**
   播放 / 暂停、上一首 / 下一首、音量、曲名、循环模式。

顶部可选轻量工具栏：

- Agent 筛选
- 场景缩放
- 资产 / 装修入口
- 显示模式：`实时` / `演示` / `昨日`

### 7.3 UX 信息层级

校场页面需要同时服务“瞄一眼”和“深排查”两个模式：

1. **巡视层**：默认首屏。展示地图、Agent 位置、状态气泡、音乐播放器、总体状态。
2. **点选层**：点击 Agent / 区域后。展示当前任务、最近工具、进度、预计下一步。
3. **排查层**：点击错误 / 时间线事件后。展示错误详情、关联文件、可回 Chat 的上下文入口。
4. **复盘层**：切换到 `昨日` 或 `战报`。展示完成事项、卡点、产物、下一步。

每层都要能单独关闭或返回上层，避免用户被面板困住。

### 7.4 创新交互建议

这些建议按优先级分为 MVP 可做与后续增强：

| 建议 | 价值 | 阶段 |
| --- | --- | --- |
| Agent 路径回放 | 把一次 run 的状态迁移变成可回放路线，适合复盘 | FEAT-JC-003 |
| 区域热力 | 哪些区域最近最常卡住，如医庐高亮代表错误多 | FEAT-JC-003 |
| 战报折扇 | 昨日战报以可展开折扇 / 卷轴形式展示，但内容仍结构化 | FEAT-JC-004 |
| 音乐随状态变奏 | running 用 focus track，blocked 降低音量或切到 tense track，done 播放短 victory sting | FEAT-JC-005 |
| 点击“回到现场” | 从校场事件一键跳回对应 Chat 消息 / tool result | FEAT-JC-002 |
| Mini 校场 | 未来作为悬浮小窗，只显示状态和音乐 | later |

## 8. 核心功能需求

### FR-001 一级入口：校场

**需求**

- `AppSection` 支持 `jiaochang`。
- GlobalNavbar 显示校场图标。
- 点击后进入 `JiaochangPage`，不影响当前 ChatWorkspace 的 session 状态。

**验收**

- Chat、记忆、校场三者可互相切换。
- 切回 Chat 后原会话、项目、输入状态仍在。

### FR-002 像素校场场景

**需求**

- 页面中央渲染一张等距像素校场背景。
- 背景需要包含：演武台、书案 / 策略区、藏经阁 / 资料区、工坊 / 执行区、信使驿站 / 同步区、医庐 / 错误修复区、休息廊。
- 每个区域对应一个 Agent 状态。
- Agent sprite 根据状态移动或定位到对应区域。

**状态映射**

| If2Ai 状态 | Star Office 类比 | 校场区域 | 视觉表现 |
| --- | --- | --- | --- |
| `idle` | idle | 休息廊 | 站立 / 喝茶 / 待命 |
| `planning` | writing / researching | 策略书案 | 展开卷轴、思考气泡 |
| `researching` | researching | 藏经阁 | 翻书、检索光点 |
| `executing` | executing | 演武台 / 工坊 | 挥剑、锤炼、命令火花 |
| `writing` | writing | 书案 | 写代码 / 文档卷轴 |
| `syncing` | syncing | 信使驿站 | 飞鸽 / 旗幡 / 进度线 |
| `blocked` | error | 医庐 / 诊断区 | 红色感叹号、停步 |
| `done` | idle | 正殿前 | 完成徽记、归位 |

**验收**

- 至少 1 个主 Agent 可根据状态变化切换位置和动画。
- 无真实数据时展示 demo/fixture 状态，并清楚标注 `演示`。

### FR-003 Agent 状态数据

**需求**

定义前端领域模型：

```ts
export type JiaochangAgentStatus =
  | 'idle'
  | 'planning'
  | 'researching'
  | 'executing'
  | 'writing'
  | 'syncing'
  | 'blocked'
  | 'done'

export interface JiaochangAgent {
  id: string
  name: string
  role: 'main' | 'subagent' | 'reviewer' | 'tool'
  projectionIdentity?: JiaochangSubagentIdentity
  avatarKey: string
  status: JiaochangAgentStatus
  statusText: string
  currentTask?: string
  lastEventAt?: string
  progress?: number
  runProgress?: JiaochangRunProgress
  lane?: string
}

export interface JiaochangSubagentIdentity {
  agentId: string
  displayName: string
  role: 'main' | 'subagent' | 'reviewer' | 'tool'
  parentAgentId?: string
  model?: string
  sessionId?: string
  runId?: string
  capabilities?: string[]
}

export interface JiaochangRunProgress {
  runId: string
  phase: JiaochangAgentStatus
  percent?: number
  startedAt?: string
  updatedAt?: string
  etaSeconds?: number
  currentStep?: string
  totalSteps?: number
  completedSteps?: number
}

export interface JiaochangRunEvent {
  id: string
  ts: string
  agentId: string
  type: 'thought' | 'tool' | 'file' | 'error' | 'review' | 'done'
  title: string
  detail?: string
  severity?: 'info' | 'warning' | 'error'
}

export interface JiaochangToolLedgerEntry {
  id: string
  runId: string
  agentId: string
  toolName: string
  status: 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled'
  startedAt?: string
  completedAt?: string
  durationMs?: number
  summary?: string
  evidenceRef?: {
    sessionId?: string
    messageId?: string
    toolCallId?: string
    filePath?: string
  }
  errorCode?: string
}
```

数据来源优先级：

1. If2Ai runtime projection / canonical run event log。
2. 稳定 projection 字段：subagent identity、tool ledger、run progress。
3. session summary / memory compiled read model。
4. fixture adapter。

预留接口要求：

- `runtime-adapter.ts` 必须提供 `getSubagentIdentities()`、`getToolLedger()`、`getRunProgress()` 三个 typed selector 或等价聚合 selector。
- projection 字段暂未接入时，fixture adapter 只能生成同形数据并标注 `source: 'fixture'`，不得改变组件模型。
- UI 面板读取 `JiaochangToolLedgerEntry` 展示工具调用证据，策略面板基于 tool ledger 和 run progress 给出建议。
- run progress 的 percent 不是必填；缺失时 UI 显示阶段式进度，不伪造百分比。

**验收**

- 数据适配集中在 `src/modules/jiaochang/data/**`，组件只消费 typed view model。
- 没有运行中任务时显示空态：`校场暂静，等待下一次出招。`

### FR-004 多 Agent 协作

**需求**

- 支持展示多个 Agent：主 Agent、subagent、reviewer、工具 worker。
- 每个 Agent 有名称、状态、角色、头像 / sprite。
- 右侧 roster 可筛选当前 Agent。
- 场景中最多同时展示 8 个 Agent；更多折叠为队列计数。

**验收**

- fixture 模式至少展示 3 个 Agent：主控、检索、审查。
- 点击场景中的 Agent 后右侧面板切换为该 Agent 的详情。

### FR-005 运行时间线

**需求**

- 右侧面板展示最近运行事件。
- 事件类型包括：思考、工具调用、文件变更、错误、review、完成。
- 工具调用可显示工具名、耗时、状态。
- 错误事件突出显示，并提供 `返回 Chat 查看上下文` 操作。

**验收**

- 最近 20 条事件可滚动查看。
- 错误状态会让对应 Agent 进入 `blocked` 区域。

### FR-006 昨日小记 / 最近战报

**需求**

参考 Star Office UI 的 `昨日小记`，在 If2Ai 中命名为 `昨日战报`：

- 优先读取最近 24 小时 session summary / memory narrative。
- 展示完成事项、遗留风险、关键文件、下一步建议。
- 必须脱敏：不显示 API key、token、完整 home path 下的敏感文件内容。

**验收**

- 无摘要时显示空态，不报错。
- 有摘要时最多展示 5 条 bullet，避免占满页面。

### FR-007 资产定制 / 装修

**需求**

- 提供 `装点校场` 抽屉。
- 支持切换背景、Agent sprite、旗帜、灯笼、桥、水面、树、训练器具等素材。
- 首版资产内置，不要求用户上传。
- 所有资源从 `src/assets/jiaochang/**` 引用，打包进前端。
- 支持 AI 生图装修作为 app 内用户功能，不只作为开发期资产生成手段。
- AI 生图支持参考图：默认可引用 `/Users/ryanliu/Downloads/platform.png` 或用户选择的本地参考图。
- AI 生图任务必须异步执行：开始任务 -> 返回 task id -> UI 轮询 / 订阅状态 -> 完成后进入候选背景列表。
- 生图结果进入 `候选` 状态，用户确认后才应用为当前校场主题。
- 支持恢复默认背景、恢复上一个生成背景、收藏生成背景。

**验收**

- 至少 1 套默认主题：`神雕校场`。
- 至少 1 套备用主题：`夜巡校场` 或 `雨后校场`。
- 无 API key 时，装点抽屉仍可使用内置主题，并显示生图功能未配置。
- 生图失败不影响当前主题。
- 用户确认前不覆盖当前背景。

### FR-008 Codex 图片资源生成

**需求**

Codex 在实现 PRD 时需生成原创图片资源，不使用 Star Office UI 的现成美术资产。所有校场相关像素图片资产必须严格锚定附件 `/Users/ryanliu/Downloads/platform.png` 的视觉风格，不允许生成风格漂移的泛像素图。建议资产清单：

| 资源 | 路径建议 | 规格 | 说明 |
| --- | --- | --- | --- |
| 主背景 | `src/assets/jiaochang/backgrounds/shendiao-courtyard.webp` | 1920x1080 或 2048x1152 | 等距像素校场全景 |
| 小背景 | `src/assets/jiaochang/backgrounds/shendiao-courtyard-mobile.webp` | 1080x1440 | 移动端裁切 |
| Agent spritesheet | `src/assets/jiaochang/sprites/agent-scholar.png` | 4x4 frames, transparent | 主 Agent |
| Reviewer spritesheet | `src/assets/jiaochang/sprites/agent-swordsman.png` | 4x4 frames, transparent | reviewer |
| Tool worker spritesheet | `src/assets/jiaochang/sprites/agent-craftsman.png` | 4x4 frames, transparent | tool worker |
| 状态图标 | `src/assets/jiaochang/icons/status-*.png` | 64x64 transparent | idle / executing / blocked 等 |
| 装饰物 | `src/assets/jiaochang/decor/*.png` | transparent | 灯笼、旗幡、石狮、木桩、卷轴 |
| 封面缩略 | `src/assets/jiaochang/cover.webp` | 1024x576 | 装修抽屉预览 |

图片生成风格提示词：

```text
Isometric pixel art martial arts training courtyard, strictly matching the attached reference image /Users/ryanliu/Downloads/platform.png in style, color palette, camera angle, tile scale, roof texture, white plaster walls, black tiled roofs, red wooden railings, stone paths, clear blue water channels, lotus leaves, lanterns, banners, small bridge, training platform, scroll desk, library pavilion, workshop corner, bright clean daylight, cozy agent workspace, Chinese wuxia courtyard mood, 32-bit pixel art, crisp edges, no text, no watermark, original composition.
```

角色 spritesheet 提示词：

```text
Transparent background pixel art spritesheet, 4 columns by 4 rows, small wuxia-inspired agent character, matching the attached reference image /Users/ryanliu/Downloads/platform.png character scale, outline weight, color saturation and daylight palette, scholar robe and subtle tech accessory, idle walk work error poses, consistent scale, crisp 32-bit pixel art, no text, no watermark, original character design.
```

严格风格约束：

- 视角必须为等距俯视像素视角，不能变成平面侧视、Q 版头像、写实插画、赛博风或厚涂风。
- 色彩必须贴近附件：清亮天光、白墙、黑灰瓦、朱红木构、湖蓝水面、嫩绿草木、金黄点缀。
- 建筑语言必须贴近附件：江南 / 武侠庭院、白墙黑瓦、红柱灯笼、石板路、水渠、小桥、庭院房间切面。
- 像素密度、边缘锐度、阴影方式需要与附件一致；禁止混入高频噪点、模糊光效、大面积渐变、3D 渲染质感。
- 生成资产不得出现文字、水印、真实影视人物肖像、片名、商标或 Star Office UI 原始素材。
- 所有 sprite、icon、decor 必须能和主背景自然叠放，不能出现不同分辨率 / 不同光源 / 不同透视的拼贴感。
- 资产验收需要附带缩略预览或截图比对，确认与 `/Users/ryanliu/Downloads/platform.png` 处在同一视觉体系。

**验收**

- 资源文件名稳定、无空格。
- 背景无文字、无水印。
- 所有默认图片资产均通过 `/Users/ryanliu/Downloads/platform.png` 风格一致性检查；不符合的资产必须重生成。
- 所有图片在打包后能正常加载。
- 如生成工具产出非 spritesheet，允许首版使用静态 PNG + CSS bobbing animation。

### FR-009 音乐播放系统

**需求**

校场页面内置氛围播放器。实现参考 `timeshiftsauce/CeruMusic` 的播放架构，但必须改写为 If2Ai 原生 React + Tauri 版本：

- CeruMusic 是 Electron + Vue + TypeScript + Pinia 项目，定位为合规插件音乐播放器框架，不直接存储或提供音乐源文件。
- If2Ai 不复制 CeruMusic 代码，不引入 Electron/Vue/Pinia，也不接入默认外部音乐源。
- 可借鉴其架构思想：全局音频状态、双 audio 槽、事件订阅、音量渐变、播放队列、WebAudio 分析器、可视化、播放设置与插件式音源边界。

首版必须实现 `JiaochangAudioProvider` / `useJiaochangAudio`，由校场页面挂载一个全局音频控制器：

- 播放 / 暂停。
- 上一首 / 下一首。
- 进度条 seek。
- 音量 slider。
- 静音。
- 循环模式：单曲 / 列表 / 随机 / 关闭。
- 当前曲名展示。
- 当前播放时间 / 总时长。
- 播放队列抽屉。
- 音频错误提示。
- 用户选择保存在 localStorage。

#### FR-009A 音源与合规边界

参考 CeruMusic 的合规定位，校场播放器只提供播放框架，不内置第三方曲库：

- 可使用项目内原创 / 免版权音频资源。
- 可支持用户本地导入音频文件，但必须由用户显式选择。
- 必须支持 CeruMusic 式插件音源 adapter；插件默认关闭，用户安装并启用后才可解析远程音乐资源。
- 不绕过 DRM，不解析未授权平台音乐，不缓存第三方受限音频。
- 若无音频文件，播放器显示 `未配置曲目`，但控件不崩溃。
- 不自动播放；必须用户点击播放，遵守浏览器音频策略。
- 本地导入需要持久化文件访问权限：用户授权后，重启 app 仍能访问被授权的本地曲目；若授权失效或文件被删除，显示 typed error。

#### FR-009A.1 CeruMusic 式音源获取与播放策略

必须复刻 CeruMusic 的“歌曲元数据 → 真实播放 URL → audio element 播放”的策略，但改写为 If2Ai 原生实现。

CeruMusic 可参考链路：

1. 用户先有一个播放队列条目，条目保存 `songmid/hash/name/singer/source/types/typeUrl/url` 等元数据。
2. 播放时不直接假设 `src` 可用，而是调用 `getSongRealUrl(song)`。
3. `source === 'local'` 时通过本地音乐索引拿 `file://` URL。
4. 服务插件歌曲如果已有 `song.url`，直接使用该 URL，例如 Navidrome/Subsonic 插件用服务端 `stream&id=...` 构造播放 URL。
5. 普通在线音源歌曲通过 `pluginId + source + songInfo + quality` 调用插件 `musicUrl(source, musicInfo, quality)` 获取真实 URL。
6. 获取 URL 前先按 `name-singer-source-quality` 生成缓存 key，命中缓存则播放本地缓存文件。
7. 未命中缓存时请求插件，拿到 URL 后异步缓存，不阻塞播放。
8. 真实 URL 写入 audio slot 后，需要等待 audio `canplay` / ready，再调用 `play()`。
9. 如果 URL 获取失败、URL 加载失败或 audio error，则尝试候选歌曲 / 候选源，全部失败后进入错误态或自动下一首。
10. 快速连续切歌必须用 request id / abort token 防竞态，旧请求返回后不得覆盖新歌。

If2Ai 目标实现模型：

```ts
export interface JiaochangMusicSourceAdapter {
  id: string
  label: string
  kind: 'bundled' | 'local' | 'service' | 'plugin'
  enabled: boolean
  listTracks?(): Promise<JiaochangMusicTrack[]>
  resolveTrackUrl(track: JiaochangMusicTrack, quality: JiaochangAudioQuality): Promise<string>
  getLyric?(track: JiaochangMusicTrack): Promise<string | null>
  testConnection?(): Promise<{ ok: boolean; message: string }>
}

export type JiaochangAudioQuality =
  | 'ambient'
  | 'standard'
  | 'high'
  | 'lossless'

export interface JiaochangResolvedTrackUrl {
  trackId: string
  url: string
  sourceAdapterId: string
  quality: JiaochangAudioQuality
  fromCache: boolean
  expiresAt?: string
}
```

解析顺序：

1. `bundled`：直接返回打包资源 URL。
2. `local`：通过用户选择的本地文件句柄 / Tauri 授权路径返回 object URL 或 `asset://` / `file://` 受控 URL。
3. `service`：类似 CeruMusic Navidrome 插件，远程服务返回 playlist + track metadata，track 自带合法 stream URL。
4. `plugin`：使用用户安装并启用的合规 adapter，根据 `track metadata + quality` 解析播放 URL。
5. `cache`：任何远程 URL 解析前先查缓存索引；解析成功后可异步缓存。

落地范围：

- FEAT-JC-005 必须实现 `bundled` 与 `local` adapter。
- FEAT-JC-006 必须实现 CeruMusic 式 `service` / `plugin` adapter 能力：插件安装、配置、连接测试、歌单获取、歌曲 URL 解析、歌词获取、缓存。
- 插件音源能力不是首屏 MVP 阻塞项，但属于校场音乐系统正式范围。

安全与合规约束：

- 不内置任何第三方平台解析插件。
- 不把插件代码放进 renderer 直接执行；插件 sandbox 明确采用 Rust side isolate / worker。
- Rust side isolate / worker 必须通过受限 RPC 与主进程通信，只暴露 `pluginInfo`、`configSchema`、`testConnection`、`getPlaylists`、`getPlaylistSongs`、`resolveTrackUrl`、`getLyric` 等白名单能力。
- 插件运行必须具备超时、取消、并发上限、内存 / 输出大小限制和错误隔离；插件崩溃不得影响 If2Ai 主进程、Agent runtime 或 renderer。
- 插件必须显式安装、显式启用、可查看权限、可删除。
- 插件只能返回用户有权访问的 URL；不得绕过 DRM、付费限制或平台访问控制。
- cache 默认只缓存 bundled/local metadata；远程音频缓存必须单独开关，并显示来源、大小、失效策略与清理入口。

#### FR-009A.2 持久化本地音乐授权

**需求**

本地音乐导入不是一次性播放，必须支持持久化访问：

- 用户通过 Tauri open dialog 选择音频文件或目录。
- 后端保存授权记录：track id、display name、artist/album metadata、duration、file fingerprint、authorized path token / bookmark、last verified timestamp。
- 重启后从授权记录恢复本地曲库。
- 播放前验证文件仍存在且权限有效。
- 文件移动、删除、权限失效时显示 `本地曲目不可访问`，并提供 `重新定位` / `从列表移除`。
- 不把绝对路径显示在普通 UI；详情层可显示脱敏路径。

建议持久化位置：

- metadata: `~/.if2ai/jiaochang/music-library.json` 或 SQLite table。
- permissions: Tauri fs scope / OS security-scoped bookmark 等平台适配机制。
- cache: `~/.if2ai/jiaochang/music-cache/`。

**验收**

- 导入本地音频后，重启 app 仍能在授权有效时播放。
- 删除源文件后，播放器进入 typed error，不无限重试。
- 普通 telemetry 不记录本地绝对路径。

播放失败策略：

- `resolveTrackUrl` 抛错：显示 `无法获取播放链接`，可尝试下一个候选 adapter。
- `audio canplay` 超时：释放当前 URL，尝试候选源。
- `audio error`：记录 typed error，不记录完整 URL，尝试下一首或进入 stopped。
- 如果连续失败超过 3 首，停止自动跳过并显示需要用户处理的错误。

#### FR-009B 双槽播放与无感切歌

参考 CeruMusic `ControlAudio` 的双槽架构，If2Ai 首版应设计为：

- 内部维护两个 `HTMLAudioElement`：`audioA` 与 `audioB`。
- `primarySlot` 表示当前活跃 audio。
- 下一首可先写入 secondary slot 并 preload。
- 切歌时进行短 crossfade；如果实现成本过高，首版可先做音量渐隐 / 渐入，但类型和状态命名需预留双槽。
- 切歌完成后 swap primary slot，并清理 secondary src。

建议状态模型：

```ts
export type JiaochangAudioSlot = 'A' | 'B'

export interface JiaochangAudioState {
  primarySlot: JiaochangAudioSlot
  srcA: string
  srcB: string
  activeTrackId?: string
  isPlaying: boolean
  currentTime: number
  duration: number
  volume: number
  muted: boolean
  repeatMode: 'off' | 'one' | 'all' | 'shuffle'
  queue: JiaochangMusicTrack[]
  error?: string
}
```

#### FR-009C 事件订阅

参考 CeruMusic 的音频事件发布 / 订阅机制，If2Ai 需要为播放器提供轻量事件总线：

```ts
export type JiaochangAudioEvent =
  | 'ended'
  | 'seeked'
  | 'timeupdate'
  | 'play'
  | 'pause'
  | 'error'
  | 'canplay'
  | 'slotSwap'
```

要求：

- `useJiaochangAudio` 提供 `subscribe(event, callback)`。
- 订阅者卸载时必须 unsubscribe，避免内存泄漏。
- 可视化、进度条、队列面板只能通过状态或订阅接口感知 audio 事件，不直接抢占 audio element。

#### FR-009D WebAudio 可视化与音效

参考 CeruMusic `AudioManager` 的 WebAudio 管线，但 If2Ai 首版只取低风险子集：

- `audio -> MediaElementAudioSourceNode -> analyser -> destination`。
- 提供频谱 / 波形可视化，嵌入校场播放器或水面 / 灯笼氛围动效。
- `AudioContext` 需要在用户点击播放后创建或 resume。
- 跨域音频默认不支持可视化；如果未来支持远程音源，必须处理 `crossOrigin` 和错误降级。
- 首版可不实现完整 EQ、Bass Boost、Surround、输出设备选择；这些作为二期增强。

#### FR-009E 播放设置

参考 CeruMusic 的播放设置拆分，校场播放器需要可持久化：

- 音量。
- repeat mode。
- 最近播放曲目。
- 是否启用淡入淡出。
- 是否启用可视化。

存储建议：

- 首版用 localStorage key `if2ai.jiaochang.audio.v1`。
- 若未来支持用户导入曲库，再迁移到 Tauri app data store。

建议 track model：

```ts
export interface JiaochangMusicTrack {
  id: string
  title: string
  artist?: string
  /** Optional only for bundled/local tracks that already have a known playable reference.
   * Remote/service/plugin tracks must resolve playable URL through resolveTrackUrl(). */
  src?: string
  source: 'bundled' | 'local' | 'service' | 'plugin'
  sourceAdapterId?: string
  sourceTrackId?: string
  hash?: string
  album?: string
  mood: 'day' | 'night' | 'focus' | 'victory'
  coverSrc?: string
  duration?: number
  availableQualities?: JiaochangAudioQuality[]
  resolvedUrl?: JiaochangResolvedTrackUrl
  license?: string
}

export interface JiaochangLocalMusicGrant {
  trackId: string
  displayName: string
  fingerprint: string
  grantedAt: string
  lastVerifiedAt?: string
  pathDisplay?: string
  permissionRef: string
  status: 'active' | 'missing' | 'permission_lost'
}
```

**验收**

- 页面内存在且仅存在一个全局音频控制器实例。
- 播放、暂停、上一首、下一首、seek、音量、静音、循环模式可用。
- 切歌时不会同时听到两个未受控音频源；若启用 crossfade，则 swap 后 secondary slot 被清理。
- `ended` 后按 repeat mode 正确进入下一首 / 单曲重播 / 停止。
- WebAudio 可视化失败时只隐藏可视化，不影响播放。
- 切换离开校场再返回后，播放器状态与音量偏好可恢复。
- 没有音频资源时页面仍完全可用。

### FR-010 响应式与性能

**需求**

- 桌面：中央场景 + 右侧面板 + 底部播放器。
- 窄屏：场景在上，面板 tab 化，播放器固定底部。
- 场景图片必须懒加载或预加载可控。
- 动画使用 CSS transform / requestAnimationFrame，不造成 layout thrash。

**验收**

- 1366x768、1440x900、390x844 三档无文字重叠。
- 页面首屏不因图片加载失败变白屏。

### FR-011 Agent 路径回放

**需求**

把一次 Agent run 的状态迁移转译成校场路径回放：

- 从 event timeline 中提取状态片段：planning -> researching -> executing -> writing -> syncing -> done / blocked。
- 地图上用淡色足迹 / 轨迹线展示 Agent 曾经经过的区域。
- 提供 `实时` 与 `回放` 两种模式。
- 回放支持暂停、快进、跳到错误点。

**验收**

- 至少 fixture run 可回放完整路径。
- 真实数据缺失时间戳时，回放降级为状态序列，不伪造精确时间。

### FR-012 情绪化状态反馈

**需求**

校场需要把运行状态转化成低干扰的情绪反馈：

- `idle`：环境平稳，水面 / 灯笼缓动。
- `planning`：书案出现卷轴光标或思考符。
- `researching`：藏经阁有书页翻动。
- `executing`：演武台 / 工坊有短促动作。
- `blocked`：医庐区域红色提示，但不闪烁刺激。
- `done`：短暂完成仪式，例如旗幡亮起、角色归位。

**验收**

- 动效不遮挡文本。
- reduced motion 系统设置开启时，状态动效降到最小。

### FR-013 从校场回到上下文

**需求**

校场不是孤立复盘页，必须能回到真实工作上下文：

- timeline 事件可提供 `sessionId`、`messageId`、`toolCallId`、`filePath`。
- 点击事件可跳回 Chat 对应消息，或打开关联文件预览 / Git Workbench。
- 若无法定位原上下文，显示 `无法定位原始上下文`，不静默失败。

**验收**

- fixture 中至少一个 tool event 可触发 `返回 Chat 查看上下文`。
- 真实上下文缺失时不会破坏当前页面状态。

### FR-014 校场策略面板

**需求**

Staff 策略视角建议增加一个轻量策略面板，用于把“状态”转成“下一步建议”：

- 当 `blocked`：列出可能原因、最近错误、建议操作。
- 当长时间 `executing`：提示运行时长、最近事件、是否需要查看日志。
- 当多 Agent 并行：显示谁阻塞了整体进度。
- 当任务完成：显示产物、风险、建议 commit / review。

**验收**

- 策略面板只能基于已有 event / summary 推断，并标注 `推断`。
- 不自动执行破坏性操作。

### FR-015 视觉与声音联动

**需求**

音乐播放器不只是独立控件，应成为校场氛围系统的一部分：

- track mood 可与校场状态联动：`focus`、`night`、`victory`。
- blocked 状态可降低音乐音量或显示静音建议，但不得自动惊吓式切歌。
- done 状态可播放短音效或切换 victory track，但必须可关闭。
- 可视化可影响水面 / 灯笼亮度，不影响运行信息可读性。

**验收**

- 所有自动音乐联动默认温和，可在设置中关闭。
- 浏览器禁止自动播放时不报错，只提示用户点击播放。

### FR-016 四语国际化

**需求**

参考 Star Office UI 的 CN/EN/JP 多语言切换，If2Ai 校场必须支持：

- 简体中文：`zh-CN`
- English：`en-US`
- 日本語：`ja-JP`
- 한국어：`ko-KR`

i18n 范围：

- sidebar label / tooltip。
- 校场页面所有按钮、tab、空态、错误、状态、战报、策略建议标题。
- Agent 气泡文本。
- loading 文案。
- 音乐播放器控件。
- 装点校场 / AI 生图装修。
- 独立窗口 / mini mode 文案。

实现要求：

- 新增 `src/modules/jiaochang/i18n/**`，集中管理翻译。
- 不允许组件内硬编码四语分支。
- 默认语言跟随 If2Ai app locale；若无 app locale，则按 OS locale；仍无法识别则使用 `zh-CN`。
- 字体策略需覆盖 CJK：中文、日文、韩文都不能出现 tofu 方块。
- 校场 UI 的短文本界面元素必须使用像素字体：标题、按钮、tab、badge、状态气泡、地图标签、播放器短标签。
- 长正文、复杂面板、战报段落、错误详情、文件路径与代码片段使用系统字体 / monospace 兜底，优先保证可读性。

建议 locale model：

```ts
export type JiaochangLocale = 'zh-CN' | 'en-US' | 'ja-JP' | 'ko-KR'

export interface JiaochangI18nCatalog {
  nav: Record<string, string>
  status: Record<JiaochangAgentStatus, string>
  empty: Record<string, string>
  actions: Record<string, string>
  audio: Record<string, string>
  decor: Record<string, string>
  mini: Record<string, string>
}
```

**验收**

- 四种语言可切换，且切换后 loading、气泡、状态、播放器、装点抽屉同步更新。
- 任一 key 缺失时 fallback 到 `en-US`，并在 dev 环境提示 missing key。
- 韩文 locale 不挤爆按钮或状态 pill。

### FR-017 独立窗口与 Mini Mode

**需求**

校场未来需要支持从主窗口分离为独立窗口，并支持 mini mode：

1. **独立校场窗口**
   - 从校场页面点击 `独立窗口` 打开。
   - 独立窗口展示完整 PixelStage、运行面板、音乐播放器。
   - 状态来自主 app 的同一 typed projection bridge，不重新拉一套数据。
   - 关闭独立窗口不影响主 Chat / Agent run。

2. **Mini Mode**
   - 小窗只显示当前 Agent sprite、状态 pill、当前音乐状态。
   - 支持拖动、置顶、点击恢复完整校场。
   - 只读状态，不提供破坏性控制。
   - 支持四语状态文案。

实现建议：

- 新增 Tauri window label：`jiaochang` / `jiaochang-mini`。
- 用 event bridge 同步 snapshot：`jiaochang://snapshot` 或 Tauri event `jiaochang-sync-state`。
- mini mode 不直接访问 raw runtime store，只消费主窗口推送的 compact snapshot。
- mini 资源必须轻量：小 spritesheet + 状态文案 + 音乐 metadata。

**验收**

- 主窗口、独立窗口、mini mode 显示同一 Agent 状态。
- 关闭 mini mode 不影响音乐播放和主页面状态。
- mini mode 切换语言后状态文案更新。

### FR-018 App 内 AI 生图装修

**需求**

AI 生图装修是正式用户功能，同时保留开发期资产生成路径：

- 开发期：Codex 可使用生成工具产出默认主题资产，提交到 `src/assets/jiaochang/**`。
- App 内：用户可在 `装点校场` 中输入风格 prompt、选择参考图、选择宽高比 / 场景类型，生成候选背景。
- API key 由用户在设置中显式配置，不内置。
- 首版 image provider 明确接入 `ChatGPT Image 2`。
- 必须在 If2Ai 模型设置页面新增 `ChatGPT Image 2` provider 配置入口，至少包含启用开关、API key / credential 引用、模型名、测试连接、默认尺寸 / 质量、用量提示。
- `装点校场` 只读取模型设置页中已启用的 `ChatGPT Image 2` 配置；页面内不得另建一套 API key 输入和存储。
- provider 必须通过 If2Ai 统一 provider/settings 管理，后续可扩展 Gemini / local image provider，但首版不以 Gemini 为目标。
- 生图任务异步执行，显示排队、生成中、成功、失败、取消。
- 生成图先进入候选列表，用户可预览、应用、收藏、删除。
- 支持恢复默认主题与恢复上一个主题。

安全要求：

- 不自动上传运行日志、prompt、文件内容作为参考图。
- 使用本地参考图前必须明确展示将被发送到外部模型。
- API key 不进入前端日志或 telemetry。
- 生图结果若含文字 / 水印 / 明显侵权元素，应提示用户重新生成。

**验收**

- 无 API key 时基础装修可用，生图入口显示配置引导。
- 有 API key 时可创建异步生图任务并轮询结果。
- 用户确认前不会覆盖当前主题。
- 可以恢复默认背景。

## 9. 视觉设计要求

所有视觉实现都以 `/Users/ryanliu/Downloads/platform.png` 为主风格锚点。Star Office UI 只作为功能、交互和像素 UI 组织参考；CeruMusic 只作为音乐获取与播放架构参考。

### 9.1 风格关键词

- 等距像素
- 东方庭院
- 武侠校场
- 明亮清爽
- 白墙黑瓦
- 红木桥 / 栏杆
- 水面 / 莲叶
- 旗幡 / 灯笼
- 书卷 / 武器架 / 木桩
- 现代 Agent 状态以轻 UI overlay 呈现，不破坏像素场景

### 9.1.1 设计系统建议

前端 UI/UX 评审建议建立 `Jiaochang Visual System`，避免每个组件各画各的：

| Token | 建议 |
| --- | --- |
| 主背景 | `shendiao-courtyard` 白墙黑瓦日间主题 |
| 强调色 | 朱红、竹青、湖蓝、金黄；避免单一米色或紫蓝主题 |
| 状态色 | idle 灰绿、planning 金、researching 蓝、executing 朱红、blocked 深红、done 翠绿 |
| 字体 | 校场专用像素字体体系；标题、按钮、状态气泡、地图标签优先像素字体，正文 / 长日志使用系统 CJK 字体兜底 |
| 面板 | 半透明宣纸 / 竹简质感，但保持 4.5:1 文本对比 |
| 图标 | 控件用 lucide；场景状态用原创像素 icon |
| 动效 | 角色 6-10 fps sprite；UI transition 120-180ms |

### 9.1.1A 像素字体规范

校场 UI 需要使用对应的像素字体，形成与图片资产一致的像素化界面语言：

- 字体来源优先参考 [Star Office UI](https://github.com/ringhyacinth/Star-Office-UI) 的像素字体组织方式，但不得复制未授权字体文件；如字体许可允许，可按 license 引入，否则选择可商用 / 可分发的 CJK 像素字体。
- 必须覆盖 `zh-CN`、`en-US`、`ja-JP`、`ko-KR` 四语，不能出现 tofu 方块或字重明显不一致。
- 建议建立 CSS font tokens：`--jiaochang-font-pixel`, `--jiaochang-font-pixel-cjk`, `--jiaochang-font-body`。
- 标题、导航 tooltip、状态 badge、地图气泡、音乐播放器短标签使用像素字体。
- 右侧运行面板、长段战报、错误详情、文件路径、代码片段使用系统 UI / monospace 兜底，保证可读性。
- 像素字体需要通过 `font-display: swap` 加载，避免字体未加载时页面空白。
- 所有字号使用固定 token，不随 viewport 宽度缩放；移动端通过布局换行处理，不压缩到不可读。

### 9.1.2 文案语言系统

为了让校场不只是换皮，核心 UI 文案建议统一：

| 通用词 | 校场词 |
| --- | --- |
| Dashboard | 校场 |
| Timeline | 出招记录 |
| Status | 招式状态 |
| Error | 卡招 / 受阻 |
| Summary | 战报 |
| Done | 收招 |
| Running | 出招中 |
| Agent | 弟子 / 主控 Agent，视具体语境慎用 |
| Tool | 器具 / 工具 |

注意：专业操作仍保留清晰技术词，例如 `tool call`、`file diff`、`runtime event` 可在详情层出现。

### 9.2 禁止项

- 不使用深色赛博朋克风。
- 不使用大面积紫蓝渐变。
- 不使用 Star Office UI 原图或第三方受限资产。
- 不出现真实影视人物肖像、片名文字、水印。
- 不把 UI 说明文字覆盖在场景中央。
- 不用像素字体承载大段正文。
- 不为了拟物装饰降低按钮、输入框、错误提示的可用性。

## 10. 权限与安全

- 不新增公网访问能力。
- 不上传用户运行数据到第三方图像 / 音频服务。
- AI 生图装修若未来接入远程模型，必须经过设置页显式开启。
- `昨日战报` 必须脱敏 token、secret、完整 credential 路径。
- 音乐播放参考 CeruMusic 的合规框架定位：If2Ai 只提供播放器框架和合规音源 adapter，不内置、不分发、不破解第三方音乐源。
- 音频只从 bundled 原创 / 免版权 asset、用户明确导入资源，或未来显式安装的合规插件 adapter 播放。
- 本地导入文件保存用户授权后的持久化访问记录 / metadata；不得把音频内容写入日志或 telemetry。
- 插件音源必须默认关闭、可卸载、可审计，并且不能绕过平台授权、DRM 或付费限制。
- 独立窗口和 mini mode 只消费 compact snapshot，不额外扩大文件、音乐、runtime 权限。
- AI 生图装修必须明确提示哪些输入会发送给外部模型；默认不携带运行日志、聊天内容、文件内容。
- 四语翻译不得把用户私有数据写进翻译 catalog 或远程翻译服务。

## 11. 可观测性

建议记录轻量前端 telemetry：

- `jiaochang_opened`
- `jiaochang_agent_selected`
- `jiaochang_music_play`
- `jiaochang_music_pause`
- `jiaochang_music_error`
- `jiaochang_music_track_changed`
- `jiaochang_music_adapter_enabled`
- `jiaochang_music_local_permission_lost`
- `jiaochang_theme_changed`
- `jiaochang_image_generation_started`
- `jiaochang_image_generation_failed`
- `jiaochang_window_opened`
- `jiaochang_mini_opened`
- `jiaochang_locale_changed`

不得记录：

- 完整 prompt。
- 文件内容。
- API key / token。
- 本地音频绝对路径。
- 完整远程音频 URL。
- 生图参考图内容。
- 插件凭证 / 服务密码。

## 12. 成功指标

| 指标 | 目标 |
| --- | --- |
| 入口可发现性 | 用户能在左侧 sidebar 直接看到 `校场` |
| 状态识别速度 | 用户 3 秒内能判断 Agent 是否 idle / running / blocked |
| 数据可信度 | 真实运行时状态与 Chat / event log 不冲突 |
| 稳定性 | 无运行数据、无音频、图片加载失败时均有降级 UI |
| 性能 | 常规桌面机器场景动画不卡顿，CPU 占用可接受 |
| 可回溯性 | 用户能从至少 80% 的 timeline event 回到 Chat / tool / file 上下文 |
| 降噪效果 | blocked 状态能给出最近错误与下一步建议，而不是只显示红色状态 |
| 审美一致性 | 默认主题、状态文案、空态、战报、音乐控件使用同一套校场语言 |
| 音乐自动获取 | 插件音源启用后可完成歌单拉取、队列导入、URL 解析、缓存、播放 |
| 多语言完整度 | 四语 catalog 无缺 key，韩文/日文不溢出 |

## 12.1 风险与策略改善

| 风险 | 影响 | 策略 |
| --- | --- | --- |
| 视觉先行导致数据不可信 | 用户不再相信校场 | MVP 必须标注 fixture，真实数据接入后以 projection 为唯一真相 |
| 过度动画影响阅读 | 看板变成干扰源 | reduced motion、低频动效、信息层与场景层分离 |
| 音乐系统范围失控 | 拖慢校场主功能 | FEAT-JC-005 做 bundled/local/持久化授权，FEAT-JC-006 正式做插件音源 |
| 插件音源合规风险 | 法务和安全风险 | 默认无第三方插件，安装前权限提示，远程缓存单独开关 |
| 多 Agent 数据源不稳定 | roster 显示错误 | adapter 给出 confidence / source 字段，UI 显示低置信降级 |
| 页面性能受大图影响 | 首屏慢、内存高 | 背景分辨率分档、懒加载、sprite atlas、CSS transform-only 动画 |
| Chat 跳转上下文不完整 | 用户无法排查 | event model 显式携带 session/message/tool/file anchors |
| 本地音乐授权失效 | 重启后无法播放 | 持久化授权记录 + 播放前校验 + 重新定位入口 |
| 四语维护成本 | 文案漂移、缺 key | i18n catalog + fallback + missing-key dev warning |
| 独立窗口状态分叉 | 主窗/小窗显示不一致 | compact snapshot bridge，mini 只读消费主 truth |
| AI 生图误传敏感内容 | 隐私风险 | 明确用户确认、禁止自动带入运行日志、provider 设置隔离 |

## 13. 实施分期

### 分期策略

Staff 架构建议把校场拆成“可看见、可信、可复盘、可调度、可沉浸”五个台阶：

1. **可看见**：入口、场景、fixture、音乐空态。
2. **可信**：runtime adapter、真实状态、Chat 上下文跳转。
3. **可复盘**：timeline、路径回放、昨日战报。
4. **可调度**：策略面板、多人 roster、blocked 建议。
5. **可沉浸**：主题装修、音乐联动、可视化、独立窗口和 mini mode。

任何阶段不得牺牲前一阶段的可信度与稳定性。

### FEAT-JC-001: Shell + fixture MVP

- 增加 `AppSection: 'jiaochang'`。
- GlobalNavbar 增加 `校场`。
- 新建 `src/modules/jiaochang/JiaochangPage.tsx`。
- 使用 fixture 数据渲染背景、Agent、状态面板、音乐播放器空状态。
- 生成并接入默认图片资源。

验收：

- `npm run build` 或项目现有 TS 检查通过。
- 校场入口可切换，页面非空。

### FEAT-JC-002: Runtime data adapter

- 新建 `src/modules/jiaochang/data/*`。
- 从 projection / run event log / session summary 读取真实状态。
- fixture 只作为 fallback。
- typed view model 增加 `source`、`confidence`、`anchors` 字段。
- timeline event 支持跳回 Chat / tool / file 上下文。

验收：

- 运行中 Agent 状态变化能映射到场景。
- 错误事件能进入 blocked 区域。
- 至少一个真实或 fixture event 可返回 Chat 上下文。

### FEAT-JC-003: Multi-agent + timeline

- roster、Agent 选择、事件时间线。
- 支持 subagent / reviewer / tool worker。
- Agent 路径回放。
- 区域热力 / blocked 区域高亮。

验收：

- 至少 3 个 Agent fixture + 真实数据 adapter。
- fixture run 可回放状态路径。

### FEAT-JC-004: 装修与资产管理

- `装点校场` 抽屉。
- 背景 / sprite / decor 切换。
- localStorage 持久化主题。
- 战报折扇 / 卷轴 UI。
- reduced motion 支持。
- 四语基础 UI catalog：`zh-CN` / `en-US` / `ja-JP` / `ko-KR`。

验收：

- 默认主题与备用主题可切换。
- 动效关闭后仍能识别状态。
- 四语切换后基础页面、状态、空态无缺 key。

### FEAT-JC-005: 音乐播放器

- 参考 CeruMusic 架构实现 If2Ai 原生音乐播放系统。
- 新增 `src/modules/jiaochang/audio/**`：
  - `JiaochangAudioProvider.tsx`
  - `useJiaochangAudio.ts`
  - `audio-events.ts`
  - `audio-state.ts`
  - `audio-visualizer.ts`
  - `track-library.ts`
  - `source-adapters.ts`
  - `resolve-track-url.ts`
  - `audio-cache.ts`
- 实现双 `HTMLAudioElement` slot、播放队列、事件订阅、localStorage 设置、无曲目空态。
- 复刻 CeruMusic 的 URL 获取策略：播放前调用 `resolveTrackUrl(track, quality)`，按 bundled/local/service/plugin/cache 顺序解析真实 URL。
- 实现 `bundled` adapter 与 `local` adapter；`service` / `plugin` adapter 只保留接口和 disabled 状态。
- 本地导入必须持久化文件访问权限，重启后可恢复曲库。
- 实现 request id / abort token，避免快速切歌时旧 URL 覆盖新歌。
- 实现播放 URL ready check：写入 audio 后等待 `canplay` 或超时，再开始播放。
- 实现 typed fallback：resolve 失败、canplay 超时、audio error 均可尝试候选源或下一首。
- 实现最小 crossfade：淡出当前 slot、淡入 secondary slot、swap 后清理旧 src。
- 实现可关闭的 WebAudio 频谱 / 波形可视化。
- 实现音乐与状态的温和联动：focus / blocked / done mood。
- FEAT-JC-005 不实现第三方音乐插件；只预留 `source: 'plugin'` 类型边界，正式插件音源在 FEAT-JC-006 实现。

验收：

- 用户点击后可播放 bundled track；无 track 时不崩。
- 用户导入本地音频后可生成 track metadata 并播放本地 URL。
- 重启 app 后，本地导入曲目在授权有效时仍可播放。
- `resolveTrackUrl` 命中缓存、bundled、local 三种路径均有测试覆盖。
- 上一首 / 下一首 / seek / volume / mute / repeat mode 行为正确。
- 切歌后只有 active slot 继续播放，secondary slot 被清理。
- 快速连续切歌不会出现旧请求覆盖当前曲目。
- URL 解析失败或 `canplay` 超时会进入 typed error/fallback，不会白屏或无限 loading。
- 可视化失败不会影响播放。
- 状态联动可关闭，且不会违反浏览器自动播放策略。
- 无任何第三方音乐源内置。

### FEAT-JC-005b: Strategy panel

- 新增校场策略面板。
- blocked / long-running / multi-agent / done 四类状态给出下一步建议。
- 建议必须引用 event / summary 证据，无法确定时标注 `推断`。

验收：

- blocked fixture 能展示最近错误、可能原因、建议动作。
- done fixture 能展示产物、风险、建议 review / commit。
- 策略面板不自动执行破坏性操作。

### FEAT-JC-006: Music source adapters

- 正式实现 CeruMusic 式插件音源能力。
- 设计 If2Ai 原生 Rust side isolate / worker adapter sandbox，不复制 CeruMusic Electron VM host，也不在 renderer 直接执行插件代码。
- 支持用户安装 / 卸载 / 启用 / 禁用 adapter。
- 支持 adapter config schema、连接测试、歌单列表、歌单歌曲、歌词获取。
- 支持远程 URL 缓存开关、缓存索引、缓存清理。
- 支持自动获取音乐资源：用户选择插件 / 服务后，可拉取歌单、导入队列，并在播放前自动解析真实 URL。
- adapter 进程 / worker 必须有超时、取消、并发上限、输出大小限制、错误隔离和审计日志。
- adapter 必须至少暴露这些能力：
  - `pluginInfo`
  - `configSchema`
  - `testConnection(config)`
  - `getPlaylists(config)`
  - `getPlaylistSongs(config, playlistId)`
  - `resolveTrackUrl(track, quality)`
  - `getLyric(track)`
- 支持两类 adapter：
  - `service`：类似 Navidrome/Subsonic，服务端返回歌单和 stream URL。
  - `plugin`：类似 CeruMusic 音源插件，根据 `source + track metadata + quality` 解析 URL。
- 任何第三方服务 adapter 均不得内置凭证或绕过授权。

验收：

- 未安装 adapter 时，校场音乐仍可用 bundled/local。
- 安装 adapter 前必须展示权限、来源和合规提示。
- 启用 adapter 后可完成 `getPlaylists -> getPlaylistSongs -> resolveTrackUrl -> play` 链路。
- adapter 运行错误不会影响 If2Ai 主进程或 Agent runtime。

### FEAT-JC-007: Independent window and mini mode

- 新增独立校场窗口。
- 新增 mini mode。
- 通过 compact snapshot bridge 同步状态。
- mini mode 支持四语状态 pill、拖动、置顶、点击恢复。

验收：

- 主窗口 / 独立窗口 / mini mode 状态一致。
- mini mode 不直接读 raw runtime events。
- 关闭任一窗口不影响 Agent run。

### FEAT-JC-008: In-app AI decoration

- 将 AI 生图装修做成 app 内功能。
- 首版接入 `ChatGPT Image 2`，并在 If2Ai 模型设置页新增该 provider 配置入口。
- 使用 If2Ai provider/settings 管理 API key、credential 引用、模型名、默认尺寸 / 质量和测试连接。
- 支持 prompt、参考图、宽高比、候选图、应用、收藏、恢复默认。
- 开发期生成资产仍保留为 Codex 资产生产路径。

验收：

- 无 `ChatGPT Image 2` key / credential 时显示配置引导；有 key 时可异步生成候选图。
- 用户确认前不覆盖当前主题。
- 不上传运行日志或敏感上下文。

### FEAT-JC-009: Four-language i18n

- 建立 `src/modules/jiaochang/i18n/**`。
- 支持 `zh-CN`、`en-US`、`ja-JP`、`ko-KR`。
- 状态气泡、loading、音乐、装点抽屉、mini mode 全部走 catalog。
- 支持 missing key fallback 与 dev warning。

验收：

- 四语切换无空 key。
- 韩文、日文按钮不溢出。
- mini mode 语言同步。

## 14. 测试要求

最低测试：

- `AppSection` type / route coverage。
- `ContentRouter` 能 route 到 JiaochangPage。
- `Jiaochang data adapter` 对 unknown / empty / error 数据有稳定输出。
- `JiaochangAudioProvider` 无 track、单 track、多 track 三种状态。
- `useJiaochangAudio` 事件订阅可 unsubscribe，且不会重复触发。
- `resolveTrackUrl` 覆盖 bundled/local/cache miss/cache hit/adapter disabled/error cases。
- `LocalMusicPermissionStore` 覆盖授权保存、重启恢复、文件缺失、重新定位。
- `MusicSourceAdapter` 覆盖 getPlaylists / getPlaylistSongs / resolveTrackUrl / getLyric mocked success and failure。
- 快速连续调用 `playTrack` 时，旧 request id 返回不会覆盖当前 track。
- audio `canplay` 超时会触发 fallback 或 typed error。
- 双 audio slot 切歌后 active / secondary 状态正确。
- WebAudio visualizer 在 `AudioContext` 不可用或 analyser 创建失败时 graceful fallback。
- `StrategyPanel` 对 blocked / done fixture 给出证据化建议。
- `PathReplay` 对 fixture run 可播放、暂停、跳到错误点。
- `JiaochangI18n` 四语 catalog 完整性测试。
- `JiaochangWindowBridge` compact snapshot 序列化 / 反序列化测试。
- `ImageDecorationTask` pending / done / error / cancel 状态测试。
- `JiaochangPage` fixture smoke render。

手工验证：

- 启动 app，点击左侧 `校场`。
- 切换 Chat -> 校场 -> Chat，确认 session 不丢。
- 模拟 Agent 状态：idle / executing / blocked。
- 验证图片资源加载。
- 验证音乐播放器点击播放、暂停、seek、音量、静音、上一首 / 下一首。
- 验证 bundled 曲目、本地导入曲目两条路径均可播放。
- 重启 app 后验证本地授权曲目仍可播放。
- 启用 mock/service adapter 后验证歌单拉取、导入、URL 自动解析、播放。
- 断开/删除本地文件后，播放器显示 typed error 并允许移除失效曲目。
- 验证切歌 crossfade 或淡入淡出不会出现两个音源同时失控播放。
- 验证 reduced motion 开启后动效明显减少。
- 验证 blocked 状态能看到最近错误与建议，而不仅是红色标记。
- 验证至少一个 timeline event 可以回到 Chat / tool / file 上下文。
- 验证四语切换：中文、英文、日文、韩文。
- 验证独立窗口与 mini mode 状态同步。
- 验证 AI 生图装修无 key / 有 key / 失败 / 恢复默认四种路径。
- 窄屏检查无重叠。

## 15. Codex 实现提示

把本 PRD 交给 Codex 时，建议使用如下执行约束：

```text
请在 /Users/ryanliu/Documents/IfAI/if2Ai 实现 docs/product-specs/jiaochang-pixel-agent-board-prd.md。

硬约束：
- 先做 FEAT-JC-001，不要一次性改完整后端。
- 先保证校场是可信 runtime cockpit，再做沉浸式视觉；fixture 必须显式标注。
- 实时参考 Star Office UI：https://github.com/ringhyacinth/Star-Office-UI；不复制其资产，只参考功能、交互模型、多语言和像素字体组织方式。
- 实时参考 CeruMusic：https://github.com/timeshiftsauce/CeruMusic；重点参考音乐资源获取、插件音源、播放控制、缓存和事件订阅策略。
- 所有校场像素图片资产必须严格按 /Users/ryanliu/Downloads/platform.png 的风格、色彩、等距视角、像素密度和建筑语言生成；风格漂移的资源必须重生成。
- 校场 UI 标题、按钮、状态气泡、地图标签和播放器短标签必须使用像素字体；正文、日志、代码、文件路径使用可读 CJK/monospace 兜底。
- 音乐播放参考 https://github.com/timeshiftsauce/CeruMusic 的架构思想，但不要复制 Electron/Vue/Pinia 代码；改写为 If2Ai React/Tauri 原生模块。
- 音乐系统必须遵守 CeruMusic 类似的合规边界：播放器框架优先，不内置第三方曲库，不破解或绕过授权。
- 必须实现 CeruMusic 式插件音源能力，但拆到 FEAT-JC-006；FEAT-JC-005 先完成 bundled/local/持久化授权和 adapter 接口。
- 本地音乐导入必须持久化文件访问权限，重启后可恢复授权曲库。
- 使用现有 AppShell / GlobalNavbar / ContentRouter 接入一级入口。
- 新代码放在 src/modules/jiaochang/**，资源放在 src/assets/jiaochang/**。
- runtime projection 后续会稳定提供 subagent identity、tool ledger、run progress；当前实现必须在 src/modules/jiaochang/data/** 预留 typed selector / adapter，不要把这些字段写死在 fixture UI 中。
- 音乐代码放在 src/modules/jiaochang/audio/**，至少包含 provider/hook/state/events/visualizer/track-library 分层。
- 音源获取必须复刻 CeruMusic 的策略：track metadata 不等于 playable URL；播放前必须通过 resolveTrackUrl 解析真实 URL，支持 bundled/local/cache，预留 service/plugin adapter。
- 不要把外部 URL、插件解析逻辑、缓存逻辑散落在 UI 组件里；必须集中在 source-adapters / resolve-track-url / audio-cache。
- 插件音源 sandbox 采用 Rust side isolate / worker；禁止 renderer 直接执行插件代码。
- 校场必须支持 zh-CN/en-US/ja-JP/ko-KR 四语，所有新增文案走 src/modules/jiaochang/i18n/**。
- AI 生图装修既要保留开发期资产生成，也要作为 app 内功能；首版接入 ChatGPT Image 2，并在模型设置页面新增 provider 配置入口；用户确认前不得覆盖主题。
- 独立窗口和 mini mode 需要通过 compact snapshot bridge 设计，不能复制第二套 runtime truth。
- UI 不直接订阅 raw Tauri events；数据通过 typed adapter/view model。
- 音乐播放器不自动播放。
- 无真实 runtime 数据时使用明确标注的 fixture/demo 状态。
- 做 UI 时采用“场景承载情绪，面板承载信息”的原则；不要把大量文字压在像素地图上。
- blocked / done 状态必须给出证据化策略建议，不只显示状态色。

完成后报告：
- 改动文件清单。
- 资源文件清单。
- 验证命令与结果。
- 真实数据路径与 fixture 路径分别说明。
- 已实现 / 未实现的创新建议清单。
- 四语覆盖情况。
- 音源 adapter / 本地授权 / AI 生图 / mini mode 的已完成范围。
- 未完成项和下一 Pack 建议。
```

## 16. 已关闭问题与剩余待定

### 已关闭

1. 音乐资源获取：按 CeruMusic 策略自动获取，支持插件音源 adapter。
2. 本地导入：需要持久化文件访问权限。
3. 窗口形态：未来支持独立窗口和 mini mode。
4. AI 生图装修：既作为开发期资产生成，也作为 app 内用户功能。
5. 多语言：支持中文、英文、日文、韩文四语。
6. Runtime projection：后续会稳定提供 subagent identity、tool ledger、run progress；校场预留 typed adapter / selector。
7. 插件音源 sandbox：采用 Rust side isolate / worker，不在 renderer 直接执行。
8. AI 生图 provider：首版接入 ChatGPT Image 2，并在模型设置页新增 provider 配置入口。

### 仍待实现前确认

1. 本地音乐持久化授权在 macOS / Windows / Linux 各自采用哪种 Tauri 权限机制？
2. ChatGPT Image 2 provider 在 If2Ai 现有模型设置页中复用哪一套 credential store / provider registry。
