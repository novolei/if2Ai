# LLM Stream Reliability Control Plane v1（v2修订版）

> 最后更新：2026-04-13  
> 适用范围：If2Ai Desktop（Tauri Rust + React）  
> 关联模块：`src-tauri/src/commands/agent.rs`、`src-tauri/src/modules/api/providers/claw_provider.rs`、`src-tauri/src/modules/control_plane/*`、`src/App.tsx`、`src/components/ui/chat-ui.tsx`

---

## 1. 背景与问题定义

当前系统已完成一轮控制平面增强（上下文裁剪、tool 配对修复、错误分类透传、审计链路增强），但在高负载或长上下文场景仍会出现：

1. `network_timeout` 导致整轮任务结束；
2. 工具执行真相与对话流真相短暂分叉，产生“第二真相”感知；
3. 运行日志可排障但不具备运营级治理能力（缺少稳定 SLO 指标闭环）。

本文件定义一个与现有架构兼容、可渐进落地的 **LLM 流可靠性控制平面**。

---

## 2. 设计目标（Goals）

### 2.1 功能目标

1. 引入显式任务状态机，替代“一刀切终止”；
2. 将大工具结果产物化（artifact），避免上下文恶性膨胀；
3. 建立运营级指标体系，实现从“可读日志”到“可调优治理”；
4. 建立统一关联键（`request_id + trace_id + stream_id`）减少排障歧义；
5. 建立统一 `ContextGovernor`（token/message-char/artifact 三闸门）；
6. 建立 `resume_cursor` 任务恢复协议（超时可续跑，不重放已确认副作用）。

### 2.2 SLO 目标

- 99% 会话请求在 60s 内给出终态或可恢复态；
- timeout 场景 100% 提供可恢复路径（而非静默失败）；
- 可在 1 分钟内通过 `stream_id + session_id + trace_id + request_id` 完成故障链路定位。

### 2.3 非目标（Non-Goals）

- 不重写 `ToolExecutionBroker`；
- 不替换现有 provider 协议；
- 不引入破坏性前后端通信协议变更（采用向后兼容字段扩展）。

---

## 3. 现有链路复盘（Current Truth Chain）

当前链路分为三层真相：

1. **执行真相（Execution Truth）**  
   来源：`if2ai.audit` 里的 `tool_execution_started/finished/failed`。
2. **对话真相（Conversation Truth）**  
   来源：`start_agent_stream` 流状态（`stream_complete` / `stream_error`）。
3. **UI真相（User-visible Truth）**  
   来源：`tool_call_update` + `stream_error/complete` 收敛逻辑。

超时时这三者可能短暂不一致，必须通过状态机与结果聚合模型统一。

---

## 4. 参考库复核与借鉴（rust / claw-cli）

## 4.1 可借鉴项

1. `rust/crates/api/src/error.rs`  
   - `ApiError::is_retryable` 的错误分类体系可复用到 stream 治理策略。
2. `rust/crates/api/src/providers/claw_provider.rs`  
   - 已有 `send_with_retry + backoff`，可扩展为流式分层超时策略。
3. `rust/crates/runtime/src/conversation.rs`  
   - `max_token_budget` + 工具配对语义 +权限策略分层，适合映射到 Context Governor。
4. `rust/crates/runtime/src/compact.rs`  
   - `should_compact/compact_session` 可作为上下文分层压缩策略基线。

## 4.2 禁止照搬项

1. provider 统一 `30s timeout`（对 streaming 不友好）；
2. CLI 失败即退出语义（桌面端必须可恢复）；
3. 手动 `/compact` 主导的策略（桌面端应自动治理并可观测）。

---

## 5. 目标架构（v2）

## 5.1 任务状态机（新增）

在 `start_agent_stream` 生命周期引入显式状态：

`RUNNING -> DEGRADED -> PARTIAL_SUCCESS | FAILED | COMPLETED`

状态语义：

- `RUNNING`：正常处理中；
- `DEGRADED`：发生 `timeout/transport`，但存在可恢复上下文或已执行证据；
- `PARTIAL_SUCCESS`：存在成功副作用工具（如 `file_write`），但模型尾段未完成；
- `FAILED`：无可恢复证据或前置阶段失败；
- `COMPLETED`：流与任务均正常结束。

### 透传要求

在现有 `stream_diag_summary` 基础上新增：

- `task_outcome`（枚举：`completed | partial_success | failed`）
- `degraded_reason`（可选）
- `resume_available`（bool）
- `resume_token`（可选，后续阶段启用）

前端新增：

- 当 `task_outcome=partial_success` 显示“可继续完成描述”按钮；
- 点击触发 resume turn（不是从头重试）。

## 5.2 工具结果产物化（Artifactization）

当前已有 `tool_result_handle + digest + preview`。下一步升级：

1. 超过阈值（如 1KB/4KB）的工具输出写入：
   - `~/.if2ai/artifacts/<trace_id>.json`
2. 对话上下文只保留：
   - `tool_name`、`tool_use_id`、`status`、`digest`、`artifact_id`、`preview`
3. 审计事件新增：
   - `artifact_id`（可选）
4. 前端工具卡新增：
   - “查看完整结果（artifact）”入口（只读查看，不回灌模型上下文）。

## 5.3 运营级观测治理（Metrics Plane）

新增指标：

- `p95_first_token_ms`
- `timeout_rate_by_model`
- `request_size_chars_p95`
- `partial_success_rate`
- `resume_success_rate`

推荐维度：

- `model`
- `workdir/project_id`
- `session_size_bucket`
- `tool_mix`（是否含重输出工具）

输出方式：

- 先写结构化日志（JSON lines）；
- 后续接入 dashboard（本地或服务端聚合）。

## 5.4 统一关联键（Correlation Key）

在现有 `stream_audit_link` 基础上补齐：

1. `request_id` 写入 `stream_diag_summary`；
2. `trace_id` 继续作为工具执行证据主键；
3. `stream_id` 作为单轮交互主键。

统一诊断串：

- `diag_key = stream_id + trace_id + request_id`

前端工具卡支持复制该诊断串，保证用户反馈可直接定位后端链路。

## 5.5 ContextGovernor（三闸门统一）

将 `compact.rs` 的 token-budget 思路迁入当前 preflight，形成统一准入器：

1. `token_budget_gate`（估算 token）；
2. `message_char_budget_gate`（消息条数 + 字符预算）；
3. `artifact_gate`（大 tool_result 必须产物化，不允许原文回灌）。

准入规则：

- 三闸门全部通过才可发起模型请求；
- 任一闸门失败，先压缩/句柄化后再准入，不直接请求模型。

## 5.6 任务恢复协议（Resume Protocol）

timeout/transport error 不再直接终止：

1. 状态进入 `DEGRADED`；
2. 生成 `resume_cursor`（最近一致点：session_id、stream_id、last_trace_id、turn_seq）；
3. 前端显示“一键继续”；
4. 继续时只补齐尾段，不重放已确认副作用步骤。

---

## 6. 与当前代码结构的映射

## 6.1 后端（Rust）

### `src-tauri/src/commands/agent.rs`

- 扩展 `stream_diag_summary` 字段：`task_outcome`、`degraded_reason`、`resume_available`；
- 新增 `TaskOutcomeResolver`（内部函数）统一聚合执行真相与流真相；
- 在 `stream_error` 分支中：
  - 先决策 `DEGRADED/PARTIAL_SUCCESS/FAILED`；
  - 再输出对应 payload，避免“一刀切 failed”。
- 新增 `resume_cursor` 生成与恢复校验（续跑尾段，不重放副作用）。

### `src-tauri/src/modules/api/providers/claw_provider.rs`

- 新增分层超时配置（建议）：
  - `connect_timeout_ms`
  - `stream_read_timeout_ms`
  - `overall_timeout_ms`
  - `max_retries`
  - `initial_backoff_ms`
  - `max_backoff_ms`
- 保留重试，但只允许在“无副作用未确认”阶段自动重试。

### `src-tauri/src/modules/control_plane/audit.rs`

- 扩展 `AuditEvent` 可选字段：
  - `stream_id`
  - `request_id`
  - `artifact_id`
  - `task_outcome`
- 维持向后兼容：字段可选，旧消费者不受影响。

## 6.2 前端（React）

### `src/App.tsx`

- 接收并持久化 `task_outcome/resume_available/resume_token`；
- `stream_error` 时优先展示任务态，不再默认“整轮失败”文案。
- 接收 `resume_cursor` 并提供“一键继续”触发入口。

### `src/components/ui/chat-ui.tsx`

- 工具卡新增：
  - `artifact_id` 可视化
  - “查看完整结果”入口
  - “复制诊断串”（`stream_id + trace_id + request_id`）
- assistant 错误卡新增：
  - `partial_success` 专属文案与“继续完成描述”按钮。

---

## 7. 必须项 A-E 与任务边界（去重版）

> 原则：每个任务只落一个主模块；跨模块仅做字段透传，不重复实现同一能力。

### A. TaskOutcome 聚合层（主责：`commands/agent.rs`）

交付：

1. `execution_truth`（工具证据）
2. `conversation_truth`（流状态）
3. `user_visible_truth`（`completed | partial_success | failed`）
4. `partial_success` 明确透传前端

边界：

- A 不实现 artifact 落盘；
- A 不实现 provider 超时参数化（由 B 负责）。

### B. Provider 分层超时配置（主责：`claw_provider.rs` + `runtime/config.rs`）

交付：

- `connect_timeout_ms`
- `stream_read_timeout_ms`
- `overall_timeout_ms`
- `max_retries` / `initial_backoff_ms` / `max_backoff_ms`

边界：

- B 只负责传输策略，不改 UI 任务文案。

### C. 统一关联键（主责：`agent.rs` + `audit.rs` + `chat-ui.tsx`）

交付：

1. `request_id` 写入 `stream_diag_summary`
2. `diag_key = stream_id + trace_id + request_id`
3. 工具卡支持复制诊断串

边界：

- C 不实现恢复协议；
- C 不改重试策略。

### D. ContextGovernor（三闸门，主责：`agent.rs` + `runtime/compact`）

交付：

1. token 预算闸门（估算）
2. message/char 预算闸门
3. artifact 句柄化闸门

边界：

- D 只做请求准入，不定义任务终态（由 A 决策）。

### E. Resume 协议（主责：`agent.rs` + `App.tsx`）

交付：

1. `DEGRADED` 进入条件
2. `resume_cursor` 生成与校验
3. “一键继续”续跑（resume turn）

边界：

- E 不重做超时参数（由 B 提供）；
- E 不重做上下文裁剪（由 D 提供）。

## 7.1 执行顺序与优先级（串行无冲突）

> 关键原则：`agent.rs`、`chat-ui.tsx`、`runtime/config.rs` 属于高冲突热点，按串行推进，不并行改同一热点文件。

### Step 1（P0）：A 后端落地（仅聚合，不改传输策略）

目标：

- 在 `commands/agent.rs` 完成 `execution_truth / conversation_truth / user_visible_truth` 聚合；
- 明确 `task_outcome=completed|partial_success|failed` 判定。

验收：

- timeout + 有成功工具证据 => `partial_success`；
- timeout + 无成功工具证据 => `failed`。

### Step 2（P0）：C 后端落地（仅关联，不改恢复逻辑）

目标：

- `request_id` 写入 `stream_diag_summary`；
- 形成 `diag_key = stream_id + trace_id + request_id`；
- 审计链路可通过 `diag_key` 反查。

验收：

- 单条 `diag_key` 能定位到 stream + audit + tool 证据。

### Step 3（P1）：A+C 前端收口（一次消费，避免反复改协议）

目标：

- `App.tsx`、`chat-ui.tsx` 一次性接入 `task_outcome` 和 `diag_key`；
- 展示 `partial_success` 专属文案；
- 工具卡支持复制诊断串。

验收：

- 前端不再把 `partial_success` 误显示为全失败；
- 复制诊断串可直接用于后端定位。

### Step 4（P1）：B 分层超时配置

目标：

- 在 `claw_provider.rs` + `runtime/config.rs` 落地分层 timeout/retry/backoff 参数；
- 仅调整传输治理，不改任务态文案与 UI 逻辑。

验收：

- `connect_timeout_ms`/`stream_read_timeout_ms`/`overall_timeout_ms` 可动态生效；
- `max_retries` 与 backoff 参数受配置控制。

### Step 5（P1）：D ContextGovernor 三闸门

目标：

- 将 token/message-char/artifact 三闸门统一到 preflight；
- 三闸门全通过才发请求。

验收：

- preflight 可观测每次请求通过/拦截原因；
- 大输出走 artifact 句柄，不再原文回灌。

### Step 6（P2）：E Resume 协议

目标：

- timeout 后进入 `DEGRADED`；
- 生成并校验 `resume_cursor`；
- 前端“一键继续”走 resume turn（不重放已确认副作用）。

验收：

- `DEGRADED` 任务可稳定续跑；
- `resume_success_rate` 可统计。

## 7.2 冲突规避规则（强约束）

1. `agent.rs` 串行改动顺序固定：A -> C -> D -> E；
2. 协议扩展遵循“后端先加可选字段，前端后消费”；
3. B 与 D 不并行发布（避免 timeout/retry 与 preflight 互相掩盖）；
4. 任一步出现退化，先回滚当前 flag，不跨步修复；
5. 每一步结束都跑同一组回归场景，防止前后语义漂移。

## 7.3 代码形态约束（防止超级大文件）

> 本节同时作为后续代码更新的硬约束，避免继续堆积到单个“庞然大物”文件。

### 7.3.1 文件职责与规模上限

1. 单文件坚持单一有界职责（state machine、provider transport、preflight governor、ui render 分离）；
2. Rust/TS 核心实现文件软上限 `<= 600` 行，超过需触发拆分评审；
3. 任一文件超过 `900` 行禁止继续叠加功能，必须先拆分再开发；
4. 任一文件超过 `1200` 行视为结构风险，作为 P0 技术债优先偿还。

### 7.3.2 模块拆分目标（当前范围）

针对 `src-tauri/src/commands/agent.rs` 建议拆分为：

1. `stream_lifecycle.rs`（流循环与状态流转）
2. `task_outcome.rs`（execution/conversation/user_visible 聚合）
3. `context_governor.rs`（token/message-char/artifact 三闸门）
4. `stream_resume.rs`（degraded/resume_cursor/续跑协议）
5. `stream_observability.rs`（diag summary、metrics、correlation key）

前端建议拆分：

1. `chat-ui` 仅保留渲染与交互编排；
2. 错误分类、诊断串、工具卡明细组装下沉到独立 util/view-model 文件。

### 7.3.3 迭代执行规则（避免边做边膨胀）

1. 每个 Step（1~6）最多允许触达一个“热点大文件”的主逻辑；
2. 若本次变更预计增加超过 `120` 行到同一文件，先做“拆分提交”再做“功能提交”；
3. 新增能力优先新模块，不允许把新子系统直接拼接到已有超长文件底部；
4. review 必须显式检查“是否可再拆分”，不满足则 `REVIEW_FAIL`。

### 7.3.4 验收与门禁

1. slice 验收新增一项：`code_shape_gate`（单责 + 可拆分 + 无超长扩张）；
2. 对 `>900` 行文件新增改动时，PR/exec-plan 必须附“拆分计划与迁移点”；
3. 若未满足模块化约束，禁止标记该步骤完成。

## 7.4 每步回归场景（统一口径）

1. timeout + 有副作用工具成功；
2. timeout + 无工具成功；
3. 大 tool_result（触发 artifact 句柄化）；
4. 多 session 并发（校验诊断串链路）；
5. degraded -> resume 成功与失败路径。

---

## 8. 回滚与风控

新增 feature flags：

- `if2ai.stream_task_outcome_enabled`
- `if2ai.tool_artifact_enabled`
- `if2ai.stream_metrics_enabled`

回滚策略：

1. 任一阶段异常可按 flag 单独关闭；
2. 保留旧日志字段，不破坏历史排障；
3. artifact 为附加路径，关闭后仍可走 handle 模式。

---

## 9. 最终成功标准（Definition of Done）

1. 用户不再将“流超时”误判为“工具全失败”；
2. 大结果上下文不再触发体积恶化链式超时；
3. 指标可驱动调优，而非凭感觉修补；
4. 故障定位可稳定完成：`session_id -> stream_id -> trace_id -> request_id -> tool/audit/artifact`。

---

## 10. 附：术语

- **第二真相**：执行真相、对话真相、UI真相不一致导致的用户误判。
- **Artifact**：工具原始输出的离线持久化对象，不直接进入模型上下文。
- **Task Outcome**：任务级最终结果（`completed/partial_success/failed`）。
- **ContextGovernor**：请求发送前的三闸门治理器（token/message-char/artifact）。
- **Resume Cursor**：任务从最近一致点继续执行所需的恢复游标。

