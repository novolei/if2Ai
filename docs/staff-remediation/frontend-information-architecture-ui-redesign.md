# 前端信息架构与 UI/UX 重构设计

> 面向 If2Ai 当前 React/Tauri 前端的 Staff 级信息架构与体验整改方案。
>
> 最后更新: 2026-04-19
> 状态: Proposed

## 1. 设计目标

前端整改的核心不是“再画得更好看”，而是：

1. 减轻认知负担
2. 强化主任务路径
3. 提高可维护性
4. 提高可信度
5. 为 memory / harness / skills 等复杂能力提供稳定承载面

## 2. 当前问题

### 2.1 结构问题

- `App.tsx` 承担过多全局流程与状态
- `chat-ui.tsx` 体量过大，混合容器逻辑、视图逻辑、工作区逻辑、消息渲染逻辑
- `tauri.ts` 既是 transport 层，也是类型中心和接口聚合层

### 2.2 体验问题

- 主聊天、调试、设置、市场、记忆等面板并存，层级感不足
- 部分功能处于 preview 状态，但界面传达方式像正式能力
- 某些交互具有视觉完成度，但无障碍语义不足

### 2.3 信任问题

- seeded 数据与真实数据混用
- 专业能力标签可能显得“很强”，但缺少可靠的状态表达
- 文档与界面之间也存在产品定义漂移风险

## 3. 目标信息架构

建议将前端分成四个层级。

### L1: Primary Workspace

用户最常驻的空间：

- Home / Chat
- Project context
- Composer
- Stream output

### L2: Context Workspace

围绕当前任务展开：

- Project rail
- File preview
- Session list
- Memory evidence

### L3: Capability Surfaces

能力专页：

- Settings
- Skills
- Onboarding
- Browser viewer
- Voice/TTS/STT 调试

### L4: Developer & Governance Surfaces

开发与治理：

- Telemetry
- Memory debug
- Harness dashboards
- Internal diagnostics

原则：

- L4 不与 L1 争夺默认注意力
- L3 用于配置和专项操作，不承载主任务流程

## 4. 目标组件分层

建议前端按职责拆分：

- `app-shell`
  - 窗口壳、全局导航、路由壳
- `conversation`
  - 容器、composer、timeline、tool projection、stream states
- `workspace`
  - projects、sessions、preview、file references
- `memory`
  - chips、evidence、browser、pinned/compiled viewers
- `settings`
  - 领域配置页
- `observability`
  - telemetry、harness、diagnostics

## 5. 主路径设计

主路径必须尽可能短：

1. 选择项目
2. 输入任务
3. 看到 agent 输出与状态
4. 必要时查看文件/记忆/工具依据

不应默认出现的内容：

- 太多开发者指标
- 过多二级配置入口
- 不确定数据来源的安全/风险表述

## 6. Chat 体验整改

Chat 是产品核心，应聚焦三件事：

1. 用户现在要做什么
2. agent 正在做什么
3. 当前结果是否可信

### 6.1 Composer 原则

- 语义完整：label / aria / keyboard 可靠
- 明确主操作：发送、停止、语音、附件
- 不堆叠过多次要控件

### 6.2 Timeline 原则

- assistant 输出优先
- tool 与 thinking 为辅助证据
- error / degraded / resume 信息要清晰但不喧宾夺主

### 6.3 Evidence 原则

- memory 命中要可解释
- tool 使用要可折叠
- 恢复与降级要有明确状态文案

## 7. Settings 与能力页设计

设置页应从“功能堆栈”转为“能力域分组”：

- Model & Provider
- Memory
- Skills
- Voice
- Web Search
- Advanced / Developer

### 7.1 Skills 市场原则

- 未验证数据不可伪装成正式评分
- seeded/demo 数据必须显式标注
- install / review / approve 状态必须真实

### 7.2 Memory 页原则

- 普通用户看到价值
- 高阶用户看到控制权
- 开发者看到审计与调试

## 8. 无障碍与可信交互原则

必须系统化约束：

1. 图标按钮必须有 `aria-label`
2. 主输入必须有 label 或 `aria-label`
3. 少用 `div role="button"`，优先真实按钮
4. `outline-none` 不能替代 focus 设计
5. 未验证风险分不能当正式事实展示

## 9. 视觉语言建议

If2Ai 当前适合的视觉方向不是“花哨 AI 控制台”，而是：

- 克制
- 有层级
- 专业
- 可解释

建议强调：

- 主路径留白更充足
- 次级面板弱化
- 关键状态使用少量高信号颜色
- 不同 surface 的语气一致

## 10. 工程整改建议

### Sprint 1

- 拆 `App.tsx`
- 拆 `chat-ui.tsx`
- 修主路径无障碍问题
- 去除 seeded 风险数据伪正式表达

### Sprint 2

- 建立 `conversation/*` 与 `workspace/*` 边界
- 把 observability UI 收敛为独立 surface
- 统一错误、恢复、降级状态组件

### Sprint 3

- 建立 front-end IA 文档和组件准入规范
- 对关键页面做体验与语义回归检查

## 11. 验收标准

前端重构达标时应满足：

1. 主聊天链路更短、更清晰
2. God-file 明显缩小
3. 关键入口无障碍语义合格
4. debug/governance 面不再污染主路径
5. 技能/记忆等高级能力的状态表达更可信

## 12. 结论

If2Ai 的前端不应只是“承载很多 AI 功能的界面”，而应成为一个：

- 可理解
- 可扩展
- 可治理
- 可建立信任

的智能体工作台。
