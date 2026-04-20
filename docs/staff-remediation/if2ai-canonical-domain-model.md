# If2Ai Canonical Domain Model

> If2Ai 整改计划 Phase M0.1 产出。固定后续所有 phase 共享的领域实体定义、生命周期、所属层、当前代码入口与命名漂移迁移基线。
>
> 最后更新: 2026-04-20
> 上位设计: [canonical-domain-model-and-workflow-truth-design.md](./canonical-domain-model-and-workflow-truth-design.md)
> 横向对照: [uclaw-if2ai-architecture-migration-report.md](./uclaw-if2ai-architecture-migration-report.md)

## 1. 文档目的

本文件解决 If2Ai 当前的三类真相漂移问题：

1. 同一概念在前端、后端、文档、命名空间里出现多个名字，导致后续 phase 无法共享语义。
2. `agent / worker / run / turn / stream / task` 等核心概念混用，使 `commands/agent.rs` 内 4000+ 行编排无法稳定外迁。
3. `activation`、`execution_mode` 等平台级语义未被显式承认，要么被当作普通设置，要么完全缺位。

本文件是 M1~M5 所有切片的命名锚点。任何后续重构如需新命名，必须先在本文件登记，否则视为漂移。

## 2. Canonical Naming Rules

1. 任一 canonical 实体只允许有一个 canonical 名称；其它历史名称只能出现在迁移映射表里，不得在新增代码 / 新增文档中使用。
2. canonical 名称采用 `snake_case` 单数。复合实体使用 `<owner>_<thing>`（例：`activation_license`、`harness_eval`、`execution_mode`）。
3. canonical 名称必须能同时映射到：
   - Rust 类型（在 `src-tauri/src/modules/runtime/contracts/` 或对应 module 中）
   - TS 类型（M0.3 起在 `src/transport/contracts.ts` 中）
   - 设计文档章节
4. 当一个名词同时承载“运行时真相”和“UI 投影”时，runtime 名为权威源，UI 名必须以 `*Projection` / `*View` / `*Card` 等显式后缀派生。
5. 所有“计划中 / 实验中”的实体必须在 catalog 内显式打 `status: proposed`，不得隐含为 canonical。

## 3. Canonical Entity Catalog

下表为 M0.1 必须覆盖的 10 个核心实体。所有字段口径与 [phase-m0-executor-runbook.md §7.2](../exec-plans/active/phase-m0-executor-runbook.md) 对齐。

### 3.1 `agent`

| field                | value                                                                                                                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `agent`                                                                                                                                                                                                                   |
| definition           | 当前对话与执行行为的策略主体，承载 system prompt、tool 选择策略、记忆注入策略、`execution_mode` 路由偏好。                                                                                                                |
| not_definition       | 不是一次 `run`；不是用户 profile；不是 LLM provider；不是 message 角色 `assistant`。                                                                                                                                      |
| lifecycle            | `defined → selected → updated → retired`（当前仅 `defined → selected` 经由内置 agent 注册路径生效；`updated/retired` 尚无 storage）                                                                                       |
| owning_layers        | application（策略）+ runtime（注入）+ UI projection（选择器）                                                                                                                                                             |
| current_code_entry   | 内置 agent 列表 [src-tauri/src/commands/slash.rs `list_agents`](../../src-tauri/src/commands/slash.rs)，注入路径在 [src-tauri/src/commands/agent.rs](../../src-tauri/src/commands/agent.rs) `start_agent_stream` 内联完成 |
| future_primary_owner | M1 `agent_service`（独立 service，从 `commands/agent.rs` 抽离）                                                                                                                                                           |

### 3.2 `session`

| field                | value                                                                                                                                                                             |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `session`                                                                                                                                                                         |
| definition           | 一次连续对话与短期执行上下文的容器，绑定 `project`，聚合 turns、permission 决策、stream 状态、session-scoped memory。                                                             |
| not_definition       | 不是 `project`；不是 memory；不是 OS 级 user session；不是单次 `run`。                                                                                                            |
| lifecycle            | `created → active → switched → archived → deleted`（`archived` 当前未真正落盘，仅靠 `pinned` 标记区分）                                                                           |
| owning_layers        | application + persistence + UI projection                                                                                                                                         |
| current_code_entry   | [src-tauri/src/modules/session/manager.rs](../../src-tauri/src/modules/session/manager.rs)，commands [src-tauri/src/commands/session.rs](../../src-tauri/src/commands/session.rs) |
| future_primary_owner | M1 `session_service`（仍在 `modules/session/`，但要剥离 commands 层中的副作用）                                                                                                   |

### 3.3 `project`

| field                | value                                                                                                                                                           |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `project`                                                                                                                                                       |
| definition           | 长期任务空间，聚合工作目录、文件上下文、多个 `session`、project-scoped memory、产出物。                                                                         |
| not_definition       | 不是单 `session` 上下文窗口；不是 git repo；不是文件夹本身。                                                                                                    |
| lifecycle            | `created → active → evolve → archived`（`evolve / archived` 当前无显式状态字段）                                                                                |
| owning_layers        | application + persistence + UI projection                                                                                                                       |
| current_code_entry   | [src-tauri/src/modules/projects/](../../src-tauri/src/modules/projects/)，commands [src-tauri/src/commands/project.rs](../../src-tauri/src/commands/project.rs) |
| future_primary_owner | M1 `project_service`                                                                                                                                            |

### 3.4 `run`

| field                | value                                                                                                                                                                        |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `run`                                                                                                                                                                        |
| definition           | 一次 runtime 执行实例，对应一次 prompt-to-completion 的端到端编排过程，可包含多次 tool 调用与 permission 等待。                                                              |
| not_definition       | 不是 `session`；不是 LLM 单次 stream chunk；不是 `agent` 配置版本。                                                                                                          |
| lifecycle            | `created → started → streaming → waiting_permission → completed                                                                                                              | failed | cancelled` |
| owning_layers        | runtime（执行）+ contracts（事件包络）+ UI projection（消息条目）                                                                                                            |
| current_code_entry   | 当前由 [src-tauri/src/commands/agent.rs](../../src-tauri/src/commands/agent.rs) 中 `run_agent_turn` / `start_agent_stream` / `stop_agent_stream` 直接编排，无独立 `Run` 类型 |
| future_primary_owner | M1 `runtime::run`（事件 envelope 在 M0.3 contracts，编排 service 在 M1）                                                                                                     |

### 3.5 `worker`

| field                | value                                                                                                                                                                                                             |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `worker`                                                                                                                                                                                                          |
| definition           | 在一次 `run` 内派生的执行子单元，承担工具执行、子任务隔离、并发裁剪。                                                                                                                                             |
| not_definition       | 不是 `agent`（`agent` 是策略，`worker` 是执行）；不是顶层用户对象；不是后台守护进程。                                                                                                                             |
| lifecycle            | `spawned → running → completed                                                                                                                                                                                    | failed | cancelled` |
| owning_layers        | runtime                                                                                                                                                                                                           |
| current_code_entry   | 当前 codebase **尚无独立 worker 抽象**；工具执行直接由 [src-tauri/src/modules/tools/](../../src-tauri/src/modules/tools/) 内联展开。设计在 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)。 |
| future_primary_owner | M1 后期 + M2，按 `if2ai-worker-adoption-design.md` 切入                                                                                                                                                           |

### 3.6 `memory`

| field                | value                                                                                                                                                                                                                                                            |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `memory`                                                                                                                                                                                                                                                         |
| definition           | 跨 turn / 跨 session 可复用的上下文资产单元；具有 scope（session/project/global）、kind（working/summary/episodic/pinned/compiled/reflection）与重要度。                                                                                                         |
| not_definition       | 不是聊天记录全文；不是 LLM context window 本身；不是 trajectory（trajectory 是历史回放材料，不是可复用资产）。                                                                                                                                                   |
| lifecycle            | `proposed → scanned → written → recalled → promoted                                                                                                                                                                                                              | demoted → expired` |
| owning_layers        | runtime（capture / recall）+ persistence（providers）+ application（policy）                                                                                                                                                                                     |
| current_code_entry   | provider 在 [src-tauri/src/modules/memory/](../../src-tauri/src/modules/memory/)（Sqlite / Vector / Hybrid），commands 在 [src-tauri/src/commands/memory.rs](../../src-tauri/src/commands/memory.rs)；audit 事件在 `modules::memory::audit::register_app_handle` |
| future_primary_owner | M3 `MemoryCoordinator` + `WritePolicy` + `QualityGate` + `RecallAssembler`                                                                                                                                                                                       |

### 3.7 `automation`

| field                | value                                                                                                                          |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| canonical_name       | `automation`                                                                                                                   |
| definition           | 可重复执行的任务定义（trigger + steps + 资源约束），与一次性 `run` 区分。                                                      |
| not_definition       | 不是普通对话消息；不是 OS 级 cron；不是 skill / slash command 本身。                                                           |
| lifecycle            | `defined → scheduled → triggered → run → completed                                                                             | failed`（当前仅 scheduler skeleton 存在） |
| owning_layers        | application + runtime                                                                                                          |
| current_code_entry   | scheduler skeleton 在 [src-tauri/src/modules/scheduler/](../../src-tauri/src/modules/scheduler/)，无 canonical Automation 类型 |
| future_primary_owner | M5 之后专题，本轮 M0~M4 不展开                                                                                                 |

### 3.8 `harness_eval`

| field                | value                                                                                                                                                                                                                                                                |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `harness_eval`                                                                                                                                                                                                                                                       |
| definition           | 用于比较 prompt / policy / memory / route strategy 的评测运行实例，承载 baseline vs candidate 对比、grader 输出与 gate 决策依据。                                                                                                                                    |
| not_definition       | 不是普通用户 `run`；不是 trajectory record 本身；不是 telemetry。                                                                                                                                                                                                    |
| lifecycle            | `defined → recording                                                                                                                                                                                                                                                 | replaying → graded → compared → archived` |
| owning_layers        | application（治理）+ runtime（执行）+ persistence（report）                                                                                                                                                                                                          |
| current_code_entry   | trace 设施在 [src-tauri/src/modules/harness/](../../src-tauri/src/modules/harness/)，opt-in（`IF2AI_HARNESS_ENABLED=1`）；commands [src-tauri/src/commands/harness.rs](../../src-tauri/src/commands/harness.rs) 提供 telemetry 查询，但无 eval / compare / gate 概念 |
| future_primary_owner | M4 governance 系列 + M5 promotion gate                                                                                                                                                                                                                               |

### 3.9 `activation_license`

| field                | value                                                                                                                                                                                                                                                                                                                                                                         |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `activation_license`                                                                                                                                                                                                                                                                                                                                                          |
| definition           | If2Ai 平台门禁实体：表示一台 install 是否被授权使用 agent 主回路；具备 issued / refreshed / revoked / expired / deactivated 状态机。                                                                                                                                                                                                                                          |
| not_definition       | 不是 settings 页面里的某个 toggle；不是 onboarding 步骤；不是 provider API key；不是 license file 的本地缓存本身。                                                                                                                                                                                                                                                            |
| lifecycle            | `requested → issued → redeemed → refreshed → revoked                                                                                                                                                                                                                                                                                                                          | expired | deactivated`（当前 codebase 仅实现 `redeemed` 一段，无 refresh/revoke/deactivate） |
| owning_layers        | application（platform gate）+ runtime（gate enforcement）+ UI projection（gate overlay）                                                                                                                                                                                                                                                                                      |
| current_code_entry   | [src-tauri/src/commands/activation.rs](../../src-tauri/src/commands/activation.rs)，仅 4 commands：`activation_validate / activation_start / activation_test_message / activation_complete`。当前实质是“首启动仪式 + 标记 onboarding 完成”，**不是硬门禁**。设计目标在 [activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md)。 |
| future_primary_owner | M0.4 contract + M1 `activation_service` + M2 boot shell gate                                                                                                                                                                                                                                                                                                                  |

### 3.10 `execution_mode`

| field                | value                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| canonical_name       | `execution_mode`                                                                                                                                                                                                                                                                                                                                                                                     |
| definition           | runtime 对单次 chat 入口请求做出的执行模式判定，是 runtime truth，由 backend classifier 在 prompt dispatch 阶段产生，前端只投影。                                                                                                                                                                                                                                                                    |
| not_definition       | 不是用户可编辑的 `scenario profile`（Chat / Coding / Research / Planning / Review 这一层是 scenario，不是 execution_mode）；不是 `agent` 的策略偏好；不是 UI tab 选择。                                                                                                                                                                                                                              |
| canonical values     | `direct_execute` / `auto_plan_execute` / `plan_then_confirm` / `specialized_surface`                                                                                                                                                                                                                                                                                                                 |
| lifecycle            | `classified → routed → executed → reflected`（每次请求短生命周期）                                                                                                                                                                                                                                                                                                                                   |
| owning_layers        | runtime（classifier + decision）+ contracts（`ExecutionModeDecision`）+ UI projection（解释性 chip）                                                                                                                                                                                                                                                                                                 |
| current_code_entry   | 当前 codebase **无 ExecutionMode / classifier / decision 任何实体**。最接近的代码是 [src-tauri/src/modules/memory/intent.rs](../../src-tauri/src/modules/memory/intent.rs) 的 memory intent 分类器（rule-based），但这是 memory recall 用途，**不是请求执行模式**。设计在 [request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md)。 |
| future_primary_owner | M0.5 contract + M1 `request_intelligence_service` + M2 解释性投影                                                                                                                                                                                                                                                                                                                                    |

## 4. Entity Lifecycle Table

| entity               | states                                                                              | terminal states                 | persistence                          |
| -------------------- | ----------------------------------------------------------------------------------- | ------------------------------- | ------------------------------------ |
| `agent`              | defined / selected / updated / retired                                              | retired                         | 内置目录 + 用户自定义（设计中）      |
| `session`            | created / active / switched / archived / deleted                                    | deleted                         | sessions_dir JSON                    |
| `project`            | created / active / evolve / archived                                                | archived                        | projects_dir JSON                    |
| `run`                | created / started / streaming / waiting_permission / completed / failed / cancelled | completed / failed / cancelled  | trajectory（部分）                   |
| `worker`             | spawned / running / completed / failed / cancelled                                  | completed / failed / cancelled  | 暂无                                 |
| `memory`             | proposed / scanned / written / recalled / promoted / demoted / expired              | expired                         | sqlite + vector                      |
| `automation`         | defined / scheduled / triggered / run / completed / failed                          | completed / failed              | 暂无                                 |
| `harness_eval`       | defined / recording / replaying / graded / compared / archived                      | archived                        | trace files（opt-in）                |
| `activation_license` | requested / issued / redeemed / refreshed / revoked / expired / deactivated         | revoked / expired / deactivated | local config（部分）                 |
| `execution_mode`     | classified / routed / executed / reflected                                          | reflected                       | 每次请求短生命周期，trace 入 harness |

## 5. Layer Ownership Table

| entity               | application | runtime  | persistence     | contracts | ui projection |
| -------------------- | ----------- | -------- | --------------- | --------- | ------------- |
| `agent`              | yes         | yes      | partial         | M1        | yes           |
| `session`            | yes         | partial  | yes             | M1        | yes           |
| `project`            | yes         | partial  | yes             | M1        | yes           |
| `run`                | partial     | yes      | trajectory only | **M0.3**  | yes           |
| `worker`             | proposed    | proposed | no              | M1+       | no            |
| `memory`             | yes         | yes      | yes             | **M0.3**  | yes           |
| `automation`         | proposed    | proposed | no              | M5+       | no            |
| `harness_eval`       | partial     | partial  | partial         | M4        | partial       |
| `activation_license` | partial     | no       | partial         | **M0.4**  | partial       |
| `execution_mode`     | no          | no       | no              | **M0.5**  | no            |

`yes` = 有正式实现；`partial` = 有部分实现但未收敛；`no` = 未成立；`proposed` = 仅设计文档存在。

## 6. Naming Drift And Migration Map

下表覆盖 [runbook §7.2](../exec-plans/active/phase-m0-executor-runbook.md) 要求的 5 类漂移。每条迁移在登记进 canonical 之前不得在新代码中使用旧名。

### 6.1 `turn / run / stream / task`

| current usage                           | location                                                       | drift                                | canonical                                 | notes                                                   |
| --------------------------------------- | -------------------------------------------------------------- | ------------------------------------ | ----------------------------------------- | ------------------------------------------------------- |
| `run_agent_turn`                        | [commands/agent.rs](../../src-tauri/src/commands/agent.rs)     | “turn”被当作 run 的同义词            | `run`                                     | M1 抽离 service 时 command 名可保留，但内部对象用 `Run` |
| `start_agent_stream` / `listenToStream` | commands/agent.rs + [src/lib/tauri.ts](../../src/lib/tauri.ts) | “stream”指向运行时事件流，非独立实体 | `run` 的事件投影                          | M0.3 把 stream 收口为 `RuntimeEventEnvelope`            |
| `task`                                  | tools / scheduler 散落使用                                     | 与 `automation` 步骤混淆             | 分别使用 `tool_call` 与 `automation_step` | M5+ 进一步收紧                                          |

### 6.2 `agent / worker / assistant`

| current usage | location                                              | drift                               | canonical                                                       | notes                                                                            |
| ------------- | ----------------------------------------------------- | ----------------------------------- | --------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `agent`       | commands/agent.rs / list_agents / OpenAI message role | 同名既指策略主体又指 message 角色   | 策略主体 = `agent`；message 角色 = `assistant`（仅 LLM 协议层） | UI 中显示给用户的“Assistant” 必须是 agent 的 display 字段，不得直接复用 LLM role |
| `worker`      | 设计文档存在                                          | 代码中无；`tool execution` 直接内联 | `worker`                                                        | 由 `if2ai-worker-adoption-design.md` 引入，不得提前散播                          |

### 6.3 `activation / onboarding / setup / access`

| current usage               | location                                                                                                                              | drift                                                                                  | canonical                                     | notes                                                                                      |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `activation_*` (4 commands) | [commands/activation.rs](../../src-tauri/src/commands/activation.rs)                                                                  | 被实现为 “首启动仪式 + 测试问候”，没有 license 状态机                                  | `activation_license`                          | M0.4 引入 `ActivationStatus` / `ActivationSnapshot` 后，commands 必须以 license 状态机为底 |
| `onboarding_*`              | [commands/onboarding.rs](../../src-tauri/src/commands/onboarding.rs) + [modules/onboarding/](../../src-tauri/src/modules/onboarding/) | 是合法实体，但当前同时承担 “激活完成标记”（`activation_complete` 写 onboarding state） | `onboarding` 与 `activation_license` 必须分离 | onboarding = 一次性向导；activation_license = 长期门禁                                     |
| `setup` / `access`          | UI 文案                                                                                                                               | 用于描述启动相关动作但无对应实体                                                       | 仅作为 UI 文案，禁止作为 type / module 名     | —                                                                                          |

### 6.4 `mode / profile / route / scenario`

| current usage          | location                              | drift            | canonical                                                            | notes                           |
| ---------------------- | ------------------------------------- | ---------------- | -------------------------------------------------------------------- | ------------------------------- |
| `mode`                 | UI 散落                               | 模糊             | `execution_mode`（runtime）或 `scenario_profile`（用户偏好），二选一 | M0.5 强制规范                   |
| `profile`              | tts / browser profile / settings 多处 | 同名复用不同语义 | `tts_profile` / `browser_profile` / `scenario_profile`（前缀必填）   | 不得单独使用 `profile` 作类型名 |
| `route` / `route_hint` | 设计文档                              | 代码中无         | `route_hint` 仅作为 `ExecutionModeDecision` 字段                     | M0.5                            |
| `scenario`             | 设计文档                              | 代码中无         | `scenario_profile`                                                   | 与 `execution_mode` 分层        |

### 6.5 `memory / summary / recall / knowledge`

| current usage                 | location                    | drift                                | canonical                                              | notes           |
| ----------------------------- | --------------------------- | ------------------------------------ | ------------------------------------------------------ | --------------- |
| `Memory*Provider`             | modules/memory/             | OK                                   | `memory`（统一入口），具体存储用 `MemoryProvider` 实现 | —               |
| `SessionSummary*`             | modules/memory/summary/     | summary 是 memory 子类型，非独立实体 | `memory(kind = session_summary)`                       | M3 收口         |
| `Pinned*`                     | modules/memory/pinned/      | pinned 是 memory 子类型              | `memory(kind = pinned)`                                | M3 收口         |
| `MemoryCompiler` / `compiled` | modules/memory/             | compiled 是 memory 子类型            | `memory(kind = compiled)`                              | M3 收口         |
| `recall`                      | memory_recall command       | OK，对应 `memory` 的 recall 动作     | `memory.recall`（动作）                                | —               |
| `knowledge`                   | UI 文案                     | 与 memory 混用                       | UI 文案可用，类型层禁止                                | —               |
| `trajectory`                  | modules/learning/trajectory | 是 ShareGPT 历史，非 memory          | 独立实体 `trajectory`（不在本 catalog，归 M5）         | 不得算入 memory |

## 7. Open Questions

以下问题留给 M0.3 及之后处理，本文件不预先决断：

1. `worker` 的最小可用边界（单 `run` 内的 `tool_call` 隔离、跨 `run` 的并发）应何时落地：见 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)，本轮 M0 仅登记，不实现。
2. `automation` 与 `skill` / `slash command` / `scheduler` 的边界：当前 scheduler skeleton 与 skills hub 共存，automation 是否 = scheduler-driven skill 的特例，由 M5+ 决定。
3. `harness_eval` 与 `trajectory` 的关系：trajectory 是输入材料，harness_eval 是评测运行；M4 之前不引入 cross-reference 表。
4. `activation_license` 是否需要硬绑定单台设备指纹：[activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md) 提到候选方案，M0.4 contract 中只占位字段，不实现。
5. `execution_mode` 的 reason taxonomy 是开放集合还是封闭枚举：M0.5 决定。本文件仅登记 4 个 canonical mode，不规定 reason 字段。
6. `agent` 是否引入 versioning（agent revision），对 harness candidate 体系是否必要：M5 阶段评估。
7. `session` 的 archived 状态如何与 memory promotion 互动：M3 决定。

## 8. 后续 Phase 引用约束

任何 M1~M5 的切片，如其设计 / 实现引入新实体或新名字，必须：

1. 在 PR 描述里指出在本 catalog 的位置（若不存在，先 PR 本文件再 PR 实现）。
2. 不得在不更新本文件的前提下，引入第二个表示同一概念的名字。
3. 命名漂移修复 PR 必须同时更新 §6 迁移表，移除已退场的旧名条目。

## 9. 关联文档

- 上位：[canonical-domain-model-and-workflow-truth-design.md](./canonical-domain-model-and-workflow-truth-design.md)
- 配套：[if2ai-workflow-truth.md](./if2ai-workflow-truth.md)
- 后续 contracts：[runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)、[activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md)、[request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md)
- 横向对照：[uclaw-if2ai-architecture-migration-report.md](./uclaw-if2ai-architecture-migration-report.md)
- 执行计划：[phase-m0-canonical-contracts-and-truth.yaml](../exec-plans/active/phase-m0-canonical-contracts-and-truth.yaml)、[phase-m0-executor-runbook.md](../exec-plans/active/phase-m0-executor-runbook.md)、[phase-m0-truth-documents-file-level-plan.md](../exec-plans/active/phase-m0-truth-documents-file-level-plan.md)
