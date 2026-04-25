# Agent Loop & Conversation Runtime

**版本**: 2.0
**最后更新**: 2026-04-24
**实现语言**: Rust (backend) + TypeScript/React (frontend)
**架构**: 3-tier (IPC Adapter -> Orchestration -> Runtime)

---

## 1. 架构总览

### 1.1 三层架构

经过 MIG-001 重构，Agent Loop 采用三层分离架构：

```
+-------------------------------------------------------------------+
|                    FRONTEND (React / TypeScript)                   |
|                                                                   |
|  ChatWorkspace --> api/streaming.ts --> Tauri IPC                  |
|                                    <-- runtime-projection/        |
+-------------------------------+-----------------------------------+
                                | invoke("start_agent_stream", ...)
                                | invoke("run_agent_turn", ...)
                                | listen("agent-token")
+-------------------------------v-----------------------------------+
|                     IPC ADAPTER LAYER                             |
|  src-tauri/src/commands/agent/mod.rs  (~146 行, 薄层)             |
|                                                                   |
|  +---------------------+    +------------------------+            |
|  | run_agent_turn      |    | start_agent_stream     |            |
|  | (非流式)            |    | (流式, 主生产路径)     |            |
|  +----------+----------+    +------------+-----------+            |
+-------------|----------------------------|-----------+------------+
              |                            |           |
              |  stop_agent_stream --------+           |
              |  respond_permission -------+           |
+-------------v----------------------------v-----------v------------+
|                   ORCHESTRATION LAYER                             |
|  src-tauri/src/modules/application/turn_service/                  |
|                                                                   |
|  +-------------+  +----------+  +-----------+  +---------------+ |
|  | mod.rs      |  | run.rs   |  | stream.rs |  | stream_task.rs| |
|  | prepare_    |  | run_turn |  | stream_   |  | run_stream_   | |
|  | chat_inputs |  |          |  | turn      |  | task (~1500L) | |
|  +-------------+  +----------+  +-----------+  +-------+-------+ |
|                                                         |         |
|  +------------------------------------------------------+-------+ |
|  | stream_finalize.rs  (后循环: 结果解析, 保存, 记忆, 轨迹)     | |
|  +--------------------------------------------------------------+ |
+--------------------------+--------------------+-------------------+
                           |                    |
+--------------------------v--------------------v-------------------+
|                       RUNTIME LAYER                               |
|  src-tauri/src/modules/runtime/                                   |
|                                                                   |
|  +---------------------------+  +-------------------------------+ |
|  | conversation.rs           |  | stream_emitter.rs             | |
|  | ConversationRuntime       |  | AgentStreamEmitter            | |
|  | ::run_turn() 同步工具循环 |  | Tauri 事件发射边界            | |
|  +---------------------------+  +-------------------------------+ |
|                                                                   |
|  session.rs      | permissions.rs  | budget.rs   | compact.rs    |
|  usage.rs        | cost_guard.rs   | event_log.rs| lifecycle.rs  |
+-----------------------------------------------------------------+
```

### 1.2 双路径概览

| 维度 | 非流式 (Path A) | 流式 (Path B) |
|------|-----------------|---------------|
| 入口 | `run_agent_turn` | `start_agent_stream` |
| 执行模型 | 同步 await | tokio::spawn 后台任务 |
| 工具循环位置 | ConversationRuntime::run_turn() | run_stream_task() |
| 权限交互 | 仅静态策略 (prompter=None) | 交互式 TauriPermissionPrompter |
| 事件推送 | 无 (返回完整结果) | 持续 SSE 经 Tauri "agent-token" |
| 会话保存 | 循环结束后一次 | 每50 token 周期保存 + 结束后 |
| 返回值 | RunTurnResponse (完整文本) | stream_id (立即返回) |
| Resume 支持 | 无 | 有 (resume_cursor) |
| 成本保护 | CostGuard 单次检查 | CostGuard 每次迭代检查 |
| 迭代限制 | max_iterations 配置 | 硬编码 10 次 |

### 1.3 关键文件位置

| 文件 | 角色 | 行数 |
|------|------|------|
| `src-tauri/src/commands/agent/mod.rs` | IPC 适配器 | ~146 |
| `src-tauri/src/modules/application/turn_service/mod.rs` | 编排入口 + prepare_chat_inputs | - |
| `src-tauri/src/modules/application/turn_service/run.rs` | 非流式路径 | ~821 |
| `src-tauri/src/modules/application/turn_service/stream.rs` | 流式路径准备 | ~500 |
| `src-tauri/src/modules/application/turn_service/stream_task.rs` | 流式工具循环 | ~1500 |
| `src-tauri/src/modules/application/turn_service/stream_finalize.rs` | 流式后处理 | - |
| `src-tauri/src/modules/runtime/conversation.rs` | 非流式 ConversationRuntime | ~48KB |
| `src-tauri/src/modules/runtime/stream_emitter.rs` | Tauri 事件发射 | ~318 |
| `src-tauri/src/modules/runtime/permissions.rs` | 权限策略 | ~7.4KB |
| `src-tauri/src/modules/runtime/session.rs` | 消息/会话类型 | ~17.7KB |
| `src-tauri/src/modules/application/tool_executor.rs` | 工具执行桥接 | ~184 |
| `src/transport/contracts.ts` | StreamTokenPayload 线协议 | - |
| `src/runtime-projection/runtime-event-translator.ts` | 前端事件翻译 | - |
| `src/runtime-projection/runtime-projection-bridge.ts` | 前端事件订阅 | - |
| `src/api/streaming.ts` | 前端流式 API 门面 | ~98 |

---

## 2. 共享准备阶段

两条路径在进入各自的工具循环之前，共享完全相同的准备流程：

```
User Message + session_id + permission_mode
         |
         v
 +-------------------------------------+
 | 1. Session Restoration              |
 |    SessionManager::restore_session() |
 |    + scan_inbound_for_secrets()      |
 +------------------+------------------+
                    |
                    v
 +-------------------------------------+
 | 2. Execution Context Resolution     |
 |    SessionContextResolver            |
 |    ::resolve_from_session()          |
 |    --> workdir, session_id,          |
 |        project_id, permission_mode   |
 +------------------+------------------+
                    |
                    v
 +-------------------------------------+
 | 3. Inbound Lifecycle Hook           |
 |    (流式路径: BeforeInbound)         |
 |    (非流式路径: 跳过)               |
 +------------------+------------------+
                    |
                    v
 +-----------------------------------------------------------+
 | 4. prepare_chat_inputs() -- TurnService                   |
 |                                                           |
 |  a. Request Intelligence --> classify()                   |
 |     --> ExecutionModeDecision (路由门控)                   |
 |                                                           |
 |  b. Provider Resolution --> resolve_chat_runtime_provider |
 |     + 复杂度模型路由 (smart routing)                      |
 |                                                           |
 |  c. Memory Coordination --> prepare_context()             |
 |     --> MemoryInjectionArtifacts + memory_items           |
 |                                                           |
 |  d. Identity Resolution --> resolve_identity()            |
 |     --> session soul/persona 覆盖                         |
 |                                                           |
 |  e. Strategy Overlay --> ActiveStrategyOverlayResolver    |
 |                                                           |
 |  f. Prompt Coordination --> PromptCoordinator::coordinate |
 |     --> PromptAssemblyDecision                            |
 |                                                           |
 |  g. Learned Traits --> LearnedTraitsStore::list_active(8) |
 |                                                           |
 |  h. Prompt Planning --> build_prompt_plan()               |
 |     --> PromptPlanResult (system prompt blocks + text)    |
 |                                                           |
 |  i. Runtime Model Hint --> priority-90 system block       |
 |                                                           |
 |  Output: PreparedChatInputs                               |
 |    - provider: RuntimeProviderResolution                  |
 |    - prompt: PromptPlanResult                             |
 |    - memory_items: Vec<MemoryItemProjection>              |
 |    - execution_mode_decision                              |
 |    - prompt_assembly_decision                             |
 +----------------------------+------------------------------+
                              |
                              v
 +-------------------------------------+
 | 5. Execution Mode Gate              |
 |    SpecializedSurface --> Err (短路) |
 |    DirectExecute / AutoPlan         |
 |    / PlanThenConfirm --> 继续       |
 +------------------+------------------+
                    |
                    v
            +------+------+
            |   分支:     |
            | Path A 或   |
            | Path B      |
            +-------------+
```

---

## 3. 流式路径 (Path B) -- 主生产路径

### 3.1 流式准备阶段 (stream.rs)

```
start_agent_stream() IPC 命令
         |
         v
TurnService::stream_turn(StreamTurnRequest)
         |
         +-- 生成 stream_id, run_id
         +-- 获取 WebviewWindow (主窗口)
         +-- 恢复 Session
         +-- 解析 Execution Context
         +-- 提取/验证 resume_cursor (如有)
         +-- 安全扫描 + BeforeInbound 生命周期钩子
         +-- 转换 session messages --> InputMessage[]
         +-- prepare_chat_inputs() (共享, 见 Section 2)
         +-- 获取工具定义 + skill attenuation allowlist
         +-- Execution Mode Gate
         +-- 构建 StreamTaskInputs (~40 字段)
         +-- 创建 oneshot cancel channel
         +-- 创建 AgentStreamEmitter
         |
         +-- tokio::spawn(run_stream_task(inputs))
         |
         v
  返回 stream_id <-- 调用方立即收到
```

### 3.2 流式工具循环 (stream_task.rs)

```
run_stream_task(inputs) -- tokio 后台任务
    |
    +-- 初始化累积器:
    |     accumulated_text, accumulated_thinking
    |     token_count, accumulated_usage
    |     stream_failed, has_successful_tool
    |     terminal_status, timeline_session_messages
    |
    +-- Emit TurnStarted (harness)
    |
+===v==========================================================+
|   OUTER LOOP (max_iterations = 10)                           |
|                                                              |
|   +------------------------------------------------------+   |
|   | 1. 取消检查 cancel_rx.try_recv()                     |   |
|   |    YES --> emit stream_complete(cancelled), break     |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +----------------------------v-------------------------+   |
|   | 2. 迭代限制检查 tool_loop_iter >= max_iterations     |   |
|   |    YES --> terminal_status = max_iterations, break    |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +----------------------------v-------------------------+   |
|   | 3. 构建本次请求                                      |   |
|   |    ContextGovernor.admit() --> 预算裁剪               |   |
|   |    sanitize_messages_for_provider()                   |   |
|   |    MessageRequest {                                   |   |
|   |      model, max_tokens, messages,                     |   |
|   |      system: prompt_blocks,                           |   |
|   |      tools: Some(definitions),                        |   |
|   |      stream: true                                     |   |
|   |    }                                                  |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +----------------------------v-------------------------+   |
|   | 4. 成本保护 CostGuard.check_before_llm_call()       |   |
|   |    FAIL --> emit stream_error, break                  |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +----------------------------v-------------------------+   |
|   | 5. 流式 LLM 调用                                    |   |
|   |    stream_message_with_resilience()                   |   |
|   |    (含超时重试 + failover 逻辑)                      |   |
|   |    FAIL --> retry 或 emit stream_error, break         |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +============================v=========================+   |
|   |   INNER LOOP -- 流式事件处理                         |   |
|   |                                                      |   |
|   |   stream.next_event().await 匹配:                    |   |
|   |                                                      |   |
|   |   ContentBlockDelta:                                 |   |
|   |     TextDelta                                        |   |
|   |       --> 累积到 accumulated_text                    |   |
|   |       --> emit text_delta 事件                       |   |
|   |     ThinkingDelta                                    |   |
|   |       --> 累积到 accumulated_thinking               |   |
|   |       --> emit thinking_delta 事件                   |   |
|   |     InputJsonDelta                                   |   |
|   |       --> tool_arguments[tool_id] += partial_json    |   |
|   |       (按 block index 累积工具参数)                  |   |
|   |     SignatureDelta --> (no-op)                        |   |
|   |                                                      |   |
|   |   ContentBlockStart:                                 |   |
|   |     Thinking --> emit thinking_start 事件            |   |
|   |     ToolUse  --> index_to_tool_id[index] = id        |   |
|   |               --> index_to_tool_name[index] = name   |   |
|   |               --> emit tool_call_update(queued)       |   |
|   |                                                      |   |
|   |   ContentBlockStop:                                  |   |
|   |     --> 从 index_to_tool_id 提取完成的 tool          |   |
|   |     --> (tool_id, tool_name, input_json)              |   |
|   |     --> 加入 pending_tool_uses                       |   |
|   |                                                      |   |
|   |   MessageStart:                                      |   |
|   |     --> max-merge usage 字段到 current_call_usage    |   |
|   |                                                      |   |
|   |   MessageDelta:                                      |   |
|   |     --> max-merge usage 字段                         |   |
|   |                                                      |   |
|   |   MessageStop:                                       |   |
|   |     --> commit current_call_usage                    |   |
|   |         --> accumulated_usage                        |   |
|   |     --> break inner loop                             |   |
|   |                                                      |   |
|   |   Ok(None) -- 流结束:                                |   |
|   |     --> 抢救残余 usage, break inner                  |   |
|   |                                                      |   |
|   |   Err:                                               |   |
|   |     Timeout --> retry outer 或 break                 |   |
|   |     Other   --> emit stream_error, break             |   |
|   +========================+=====+=======================+   |
|                            |     |                           |
|   (正常 MessageStop)       |     | (错误路径)               |
|                            v     v                           |
|   +------------------------------------------------------+   |
|   | 6. pending_tool_uses 为空?                           |   |
|   |    YES --> terminal_status = model_stop, break outer  |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +----------------------------v-------------------------+   |
|   | 7. flush_assistant_timeline_segment()                |   |
|   |    将累积文本/thinking 作为 assistant 消息保存       |   |
|   +----------------------------+-------------------------+   |
|                                |                             |
|   +============================v=========================+   |
|   |   TOOL EXECUTION LOOP                                |   |
|   |   for each (tool_id, tool_name, input_json):         |   |
|   |                                                      |   |
|   |   +------------------------------------------------+ |   |
|   |   | a. 权限决策 (3 层)                              | |   |
|   |   |    1) 会话覆盖: permission_overrides[session]   | |   |
|   |   |       tool_name 匹配 或 通配 "*"               | |   |
|   |   |    2) 如有覆盖: 直接 Allow/Deny                | |   |
|   |   |    3) 否则: permission_policy.authorize(         | |   |
|   |   |         tool_name, input,                       | |   |
|   |   |         Some(&mut TauriPermissionPrompter))     | |   |
|   |   |       --> 发射 "permission-request" 事件        | |   |
|   |   |       --> 等待前端用户决策                      | |   |
|   |   |       --> PermissionOutcome (Allow/Deny)         | |   |
|   |   +------------------------------------------------+ |   |
|   |   | b. emit tool_call_update(running)               | |   |
|   |   +------------------------------------------------+ |   |
|   |   | c. 工具执行                                     | |   |
|   |   |    If ALLOW:                                    | |   |
|   |   |      tool_executor.execute_with_trace(          | |   |
|   |   |        tool_name, input, trace_id)              | |   |
|   |   |    If DENY:                                     | |   |
|   |   |      --> 合成 error tool_result                 | |   |
|   |   +------------------------------------------------+ |   |
|   |   | d. 安全净化 (sanitize tool output)              | |   |
|   |   +------------------------------------------------+ |   |
|   |   | e. emit tool_call_update(completed/error)       | |   |
|   |   |    payload: tool_result, duration_ms, policy    | |   |
|   |   +------------------------------------------------+ |   |
|   |   | f. 追加到 session_messages:                     | |   |
|   |   |    - ToolUse message (assistant role)           | |   |
|   |   |    - ToolResult message (tool role)             | |   |
|   |   +------------------------------------------------+ |   |
|   |   | g. 追加到 timeline_session_messages             | |   |
|   |   +------------------------------------------------+ |   |
|   +======================================================+   |
|                                |                             |
|                                v                             |
|                   CONTINUE OUTER LOOP ---------+             |
|                   (下一轮 LLM 看到 tool results)             |
+==============================================================+
         |
         v  (循环退出)
```

### 3.3 流式终结阶段 (stream_finalize.rs)

```
finalize_stream_task()
    |
    +-- 1. 未验证文件声明重写 (guardrail)
    |      contains_unverified_file_claim --> override text
    |
    +-- 2. TaskOutcomeResolver::resolve()
    |      ExecutionTruth x ConversationTruth --> TaskOutcome
    |      --> completed / partial_success / failed
    |      --> degraded_reason, resume_available
    |
    +-- 3. PersistedTurnOutcome 合成 + timeline-message 追加
    |
    +-- 4. Compaction 检查
    |      should_compact() --> compact_session()
    |
    +-- 5. App session 保存 (SessionManager)
    |
    +-- 6. 轨迹记录 (ShareGPT JSONL)
    |
    +-- 7. dispatch_after_turn() -- 记忆候选提取
    |      + RollingSummarizer 触发
    |
    +-- 8. LearningModule: record_turn + periodic reflection
    |
    +-- 9. emit stream_complete via AgentStreamEmitter
    |      Payload:
    |        task_outcome, degraded_reason
    |        resume_available, resume_cursor
    |        context_budget_usage
    |        memory_context items
    |        prompt_diagnostics
    |        turn_cost (TokenUsage --> USD)
    |        routing_info (smart-routing summary)
    |        session_totals (running per-session)
    |
    +-- 10. Harness: emit TurnFinished
    |
    +-- 11. MemoryTicker.on_turn_complete()
    |
    +-- 12. Auto-compact (RollingSummarizer path)
    |
    +-- 13. Usage store commit (provider_id, model, tokens, cost)
```

---

## 4. 非流式路径 (Path A)

### 4.1 非流式流程 (run.rs + conversation.rs)

```
run_agent_turn() IPC 命令
    |
    v
TurnService::run_turn(RunTurnRequest)
    |
    +-- 恢复 Session
    +-- 解析 Execution Context
    +-- prepare_chat_inputs() (共享, 见 Section 2)
    +-- 构建 RealApiClient (wrapping ProviderClient)
    +-- 构建 PermissionPolicy (from mode)
    +-- 构建 ToolRegistryExecutor (with skill attenuation)
    +-- 构建 ConversationRuntime:
    |     .with_context_budget(budget)
    |     .with_working_memory(8 turns, budget tokens)
    |     .with_turn_hook(memory_ticker)
    |     .with_session_context(session_id, project_id)
    +-- CostGuard.check_before_llm_call()
    |
+===v==========================================================+
|   runtime.run_turn(user_message, None)                       |
|   ConversationRuntime 拥有同步工具循环                        |
|                                                              |
|   1. session.messages.push(user_message)                     |
|                                                              |
|   LOOP (max_iterations 检查):                                |
|                                                              |
|     2. ContextBudget 检查                                    |
|        超限 --> Err(SessionError)                            |
|                                                              |
|     3. WorkingMemory 窗口过滤 (8 turns)                      |
|        wm.clear() --> wm.extend(session.messages)            |
|        messages_for_request = wm.messages().to_vec()         |
|                                                              |
|     4. 构建 ApiRequest:                                      |
|        system_prompt: self.system_prompt.clone()             |
|        messages: messages_for_request                        |
|        tools: Some(self.tool_executor.get_definitions())     |
|                                                              |
|     5. api_client.stream(request) --> Vec<AssistantEvent>    |
|        (收集所有事件到 Vec, 非真正流式)                       |
|                                                              |
|     6. build_assistant_message(events)                       |
|        --> (ConversationMessage, Option<TokenUsage>)         |
|        记录 usage 到 UsageTracker                            |
|                                                              |
|     7. session.messages.push(assistant_message)              |
|                                                              |
|     8. 提取 pending_tool_uses:                               |
|        assistant_message.blocks.iter()                       |
|          .filter_map(ContentBlock::ToolUse)                  |
|        如果为空 --> break (turn 结束)                        |
|                                                              |
|     9. FOR EACH (tool_use_id, tool_name, input):             |
|        +------------------------------------------+          |
|        | a. permission_policy.authorize(           |          |
|        |      tool_name, input, None)              |          |
|        |    (无交互 prompter, 仅静态策略)          |          |
|        +------------------------------------------+          |
|        | b. If ALLOW:                              |          |
|        |    - PreToolUse hook 运行                 |          |
|        |    - tool_executor.execute(name, input)   |          |
|        |    - PostToolUse hook 运行                |          |
|        |    - 安全净化 tool output                 |          |
|        +------------------------------------------+          |
|        | c. If DENY:                               |          |
|        |    - 合成 error tool_result               |          |
|        +------------------------------------------+          |
|        | d. session.messages.push(tool_result)      |          |
|        +------------------------------------------+          |
|                                                              |
|    10. CONTINUE LOOP (LLM 看到 tool results)                |
|                                                              |
|   turn_hook.on_turn_complete() -- 记忆钩子                   |
|                                                              |
|   Output: TurnSummary {                                      |
|     assistant_messages, tool_results,                        |
|     iterations, usage                                        |
|   }                                                          |
+==============================================================+
         |
         v
 +------------------------------------------------------+
 |  Post-Turn Finalization (在 run.rs 中)                |
 |                                                      |
 |   1. 提取 response_text + thinking_content           |
 |   2. Skill proposal 检测                             |
 |   3. Compaction 检查 --> compact_session()            |
 |   4. Session 保存 (SessionManager)                   |
 |   5. Conversation recall ingest (FTS)                |
 |   6. 轨迹记录 (ShareGPT JSONL)                      |
 |   7. 记忆候选提取 + dispatch_after_turn              |
 |   8. LearningModule: record_turn + reflection        |
 |   9. FrozenSnapshot 完整性验证                       |
 |  10. WorkingMemory 预算审计                          |
 |  11. WeibullDecay 重要度衰减                         |
 |  12. MemoryPromotionEngine 扫描                      |
 |  13. Harness TurnFinished 发射                       |
 +------------------------------------------------------+
         |
         v
 返回 RunTurnResponse { message, session_id, thinking }
```

### 4.2 非流式与流式的关键差异

| 方面 | 非流式 | 流式 |
|------|--------|------|
| 工具循环所有者 | ConversationRuntime::run_turn() | run_stream_task() |
| 权限提示 | None (仅策略) | TauriPermissionPrompter (交互式) |
| 上下文窗口 | WorkingMemory 8 turns | ContextGovernor 动态裁剪 |
| LLM 调用方式 | api_client.stream() 收集全部 | SSE 流式逐事件处理 |
| Hook 支持 | PreToolUse + PostToolUse | 无 (直接在循环中处理) |
| Resume | 不支持 | 支持 (resume_cursor) |
| 重试/Failover | 无 | stream_message_with_resilience() |
| 生命周期钩子 | 无 BeforeInbound | 有 BeforeInbound |

---

## 5. 工具执行生命周期

### 5.1 工具执行全链路

```
LLM 流式响应 (SSE)
    |
    v
+-----------------------------------------------+
| SSE Parser                                    |
|   ContentBlockStart(ToolUse) --> 跟踪 index   |
|   InputJsonDelta --> 按 index 累积 JSON       |
|   ContentBlockStop --> 提取完整 tool_use       |
+------------------------+----------------------+
                         |
                         v
+-----------------------------------------------+
| 权限决策 (3 层)                               |
|                                               |
|  Layer 1: 会话覆盖查询                        |
|    permission_overrides[session_id]            |
|      --> tool_name 精确匹配                   |
|      --> 通配符 "*" 匹配                      |
|    如有 --> 直接使用 (跳过 Layer 2-3)          |
|                                               |
|  Layer 2: 权限策略                             |
|    permission_policy.authorize(                |
|      tool_name, input, prompter)              |
|                                               |
|  Layer 3: 交互式提示 (仅流式路径)             |
|    TauriPermissionPrompter                     |
|      --> window.emit("permission-request")    |
|      --> 等待 mpsc channel 接收用户决策       |
|      --> respond_permission IPC 回传          |
|                                               |
|  Output: Allow / Deny { reason }              |
+------------------------+----------------------+
                         |
                         v
+-----------------------------------------------+
| 工具执行                                      |
|                                               |
|  ToolRegistryExecutor                         |
|    .execute_with_trace(name, input, trace_id) |
|    |                                          |
|    +-- 1. Allowlist 检查 (skill attenuation)  |
|    +-- 2. parse_tool_input_json(input)        |
|    +-- 3. load_control_plane_switches()       |
|    +-- 4. emit observability "tool.start"     |
|    +-- 5. tokio::task::block_in_place {       |
|    |        ToolExecutionBroker               |
|    |          .execute_with_trace()           |
|    |        或 legacy dispatch                |
|    |      }                                   |
|    +-- 6. emit observability "tool.ok/err"    |
|    |                                          |
|    v                                          |
|  ToolRegistry.dispatch()                      |
|    --> DashMap 查找 tool entry                |
|    --> disabled 检查                          |
|    --> tokio::time::timeout 超时保护          |
|    --> handler(args, SharedToolContext)        |
|    --> ToolOutput (text + optional images)    |
+------------------------+----------------------+
                         |
                         v
+-----------------------------------------------+
| 后处理                                        |
|                                               |
|  1. 安全净化 (大小限制, 注入检测, 截断)       |
|  2. 构建 tool_result ConversationMessage      |
|  3. Emit tool_call_update(completed/error)    |
|  4. 追加到 session_messages (API 格式)        |
|  5. 追加到 timeline_session_messages          |
|  6. Harness: emit ToolCompleted               |
+-----------------------------------------------+
```

### 5.2 消息结构

每次工具调用产生两条消息追加到会话历史：

```
+--------------------------------------------------+
| Message 1: ToolUse (assistant role)              |
|   role: Assistant                                |
|   blocks: [                                      |
|     ContentBlock::ToolUse {                      |
|       id: "toolu_xxx",                           |
|       name: "bash",                              |
|       input: "{\"command\":\"ls\"}"              |
|     }                                            |
|   ]                                              |
+--------------------------------------------------+
| Message 2: ToolResult (tool role)                |
|   role: Tool                                     |
|   blocks: [                                      |
|     ContentBlock::ToolResult {                   |
|       tool_use_id: "toolu_xxx",                  |
|       tool_name: "bash",                         |
|       output: "file1.txt\nfile2.txt",            |
|       is_error: false                            |
|     }                                            |
|   ]                                              |
+--------------------------------------------------+
```

下一轮 LLM 请求会包含这两条消息，使模型知道工具已执行及其结果。

---

## 6. 前端事件处理链

### 6.1 端到端事件流

```
+---------------------------------------------------------------+
|                     RUST BACKEND                              |
|                                                               |
|  AgentStreamEmitter.emit_payload(StreamTokenPayload)          |
|    --> window.emit("agent-token", payload)                    |
|                                                               |
|  发射的事件类型:                                              |
|    text_delta, thinking_start, thinking_delta,                |
|    tool_call_update (queued/running/completed/error),         |
|    final_text_override, stream_complete, stream_error         |
|                                                               |
|  其他事件通道:                                                |
|    "permission-request" -- 权限请求                           |
|    "memory_event" -- 记忆生命周期                             |
|    "memory_after_turn" -- 批量记忆决策                        |
+-------------------------------+-------------------------------+
                                | Tauri IPC bridge
+-------------------------------v-------------------------------+
|                    TRANSPORT LAYER                            |
|                                                               |
|  transport/contracts.ts                                       |
|    StreamTokenPayload (wire shape, snake_case)                |
|    AGENT_TOKEN_EVENT = "agent-token"                          |
|    PERMISSION_REQUEST_EVENT = "permission-request"            |
|    MEMORY_EVENT = "memory_event"                              |
+-------------------------------+-------------------------------+
                                |
            +-------------------+-------------------+
            |                                       |
+-----------v--------------+    +-------------------v-----------------+
| LEGACY PATH              |    | PROJECTION PIPELINE                 |
| (ChatWorkspace 直接处理) |    |                                     |
|                          |    | api/streaming.ts                    |
| listenToStream()         |    |   listenToAgentTokenStream()        |
|   --> 按 stream_id 过滤  |    |                                     |
|                          |    | runtime-projection-bridge.ts        |
| 事件处理:                |    |   wireRuntimeProjectionListeners()  |
|  text_delta --> 文本累积 |    |     |                               |
|  thinking_* --> 思考显示 |    |     v                               |
|  tool_call_update        |    | runtime-event-translator.ts         |
|    --> 工具卡片          |    |   translateAgentTokenPayload()      |
|  stream_complete --> 完成|    |   event_type --> CanonicalEvent:    |
|  stream_error --> 错误   |    |     text_delta                      |
|                          |    |       --> stream_text_delta          |
|                          |    |     thinking_start                   |
|                          |    |       --> stream_thinking_start      |
|                          |    |     thinking_delta                   |
|                          |    |       --> stream_thinking_delta      |
|                          |    |     tool_call_update                 |
|                          |    |       --> stream_tool_call_update    |
|                          |    |     final_text_override              |
|                          |    |       --> stream_final_text_override |
|                          |    |     stream_complete                  |
|                          |    |       --> stream_complete            |
|                          |    |     stream_error                     |
|                          |    |       --> stream_error               |
|                          |    |     |                               |
|                          |    |     v                               |
|                          |    | runtime-event-queue.ts              |
|                          |    |   microtask-batched push            |
|                          |    |     |                               |
|                          |    |     v                               |
|                          |    | runtime-event-reducer.ts            |
|                          |    |   reduceRuntimeEventBatch()         |
|                          |    |   --> RuntimeProjectionSnapshot     |
|                          |    |     |                               |
|                          |    |     v                               |
|                          |    | runtime-projection-store.ts         |
|                          |    |   useSyncExternalStore bridge       |
|                          |    |     |                               |
|                          |    |     v                               |
|                          |    | use-runtime-projection.ts           |
|                          |    |   React hook --> UI 组件            |
+--------------------------+    +-------------------------------------+
```

### 6.2 StreamTokenPayload 完整字段

```typescript
interface StreamTokenPayload {
  stream_id: string
  text?: string                        // 文本增量
  thinking?: string                    // 思考增量
  event_type:
    | 'text_delta'
    | 'thinking_delta'
    | 'thinking_start'
    | 'tool_call_update'
    | 'final_text_override'
    | 'stream_complete'
    | 'stream_error'

  // 工具执行字段
  tool_call_id?: string                // 工具调用唯一 ID
  tool_name?: string                   // 工具名称
  tool_status?: 'queued' | 'running' | 'completed' | 'error'
  tool_args?: Record<string, unknown>  // 工具参数 (JSON)
  tool_result?: string                 // 工具结果/输出
  tool_duration_ms?: number            // 执行耗时

  // 策略与上下文字段
  effective_workdir?: string           // 工具运行目录
  policy_decision?: 'allow' | 'deny' | 'prompt'
  evidence_id?: string                 // 审计 trace ID
  request_id?: string                  // API 请求关联 ID

  // 完成字段
  task_outcome?: 'completed' | 'partial_success' | 'failed'
  degraded_reason?: string
  resume_available?: boolean
  resume_cursor?: string

  // 成本/用量字段
  context_budget_usage?: ContextBudgetUsage
  turn_cost?: TurnCost
  session_totals?: SessionUsageTotals

  // 记忆与路由
  memory_context?: MemoryContextItem[]
  routing_info?: RoutingInfo
  prompt_diagnostics?: PromptDiagnosticsSummary
}
```

### 6.3 权限交互流程

```
Backend                          Frontend
   |                                |
   | emit("permission-request",    |
   |   { tool_name, scope })       |
   |------------------------------->|
   |                                | 显示权限对话框
   |                                | 用户点击 Allow/Deny
   |                                |
   | invoke("respond_permission",   |
   |   { session_id, decision,      |
   |     tool_name, scope })        |
   |<-------------------------------|
   |                                |
   | mpsc channel 传递决策          |
   | TauriPermissionPrompter 返回   |
   | 工具执行继续或中止             |
   |                                |
```

---

## 7. 跨组件数据流

| 数据项 | 生产者 | 消费者 | 传输方式 |
|--------|--------|--------|----------|
| StreamTokenPayload | AgentStreamEmitter (Rust) | 前端 (两条通道) | Tauri `window.emit("agent-token")` |
| PermissionRequestPayload | TauriPermissionPrompter (Rust) | 前端权限对话框 | Tauri `window.emit("permission-request")` |
| PermissionDecision | 前端对话框 | Rust mpsc channel | `respond_permission` IPC invoke |
| MemoryContextItem[] | MemoryCoordinator (Rust) | 前端 MemoryChip | `stream_complete.memory_context` 字段 |
| TurnCost | UsageTracker (Rust) | 前端 TokenChip | `stream_complete.turn_cost` 字段 |
| RoutingInfo | complexity_model_routing (Rust) | 前端 routing 展示 | `stream_complete.routing_info` 字段 |
| ContextBudgetUsage | ContextGovernor (Rust) | 前端 ContextBar | `stream_complete.context_budget_usage` |
| PromptDiagnostics | PromptPlanner (Rust) | 前端诊断面板 | `stream_complete.prompt_diagnostics` |
| SessionUsageTotals | Usage store (Rust) | 前端 UsageSettingsPage | `stream_complete.session_totals` |
| CancelSignal | 前端 stop 按钮 | Rust oneshot channel | `stop_agent_stream` IPC invoke |

---

## 8. 差距文档核对

对照 `docs/bs_gap/02-agent-loop-gap.md` 中记录的问题：

| 差距文档问题 | 原判定 | 当前状态 | 证据 |
|-------------|--------|----------|------|
| `run_agent_turn` 的 `tools: None` | CRITICAL | **已修复** | `conversation.rs` -- `tools: Some(self.tool_executor.get_definitions())` |
| `start_agent_stream` 忽略 tool_use 事件 | CRITICAL | **已修复** | `stream_task.rs` 有完整的 `index_to_tool_id` 跟踪、`ContentBlockStart(ToolUse)` 处理、`InputJsonDelta` 累积、`ContentBlockStop` 提取和顺序工具执行 |
| 前端无工具事件类型 | MEDIUM | **已修复** | `runtime-event-translator.ts` 处理 `tool_call_update` --> `stream_tool_call_update`; ChatWorkspace 渲染工具卡片 |
| 流式路径无 tool_result 回传 | MEDIUM | **已修复** | `stream_task.rs` 将 tool results 追加到 `session_messages`, 继续外层循环 |
| 两条路径能力不一致 | SEVERE | **基本修复** | 两条路径现在都有完整的工具循环 |
| 流式路径无权限检查 | MEDIUM | **已修复** | `stream_task.rs` -- 完整权限解析: 会话覆盖 + TauriPermissionPrompter + 审计 |

### 剩余设计差异 (非 bug, 属于设计选择)

- 非流式路径: `prompter=None` --> 无交互式权限对话框, 仅静态策略
- 非流式路径: 无 BeforeInbound 生命周期钩子
- 非流式路径: 无 resume cursor 支持
- 非流式路径: 无流式重试/resilience
- 非流式路径: 无 skill attenuation (工具定义过滤)

---

## 9. 终止条件汇总

| 条件 | Path A (非流式) | Path B (流式) | Terminal Status |
|------|-----------------|---------------|-----------------|
| LLM 响应无工具调用 | break | break | `model_stop_no_tools` |
| 超过最大迭代数 | Err(MaxIterationsExceeded) | break | `max_iterations_reached` |
| 用户取消 | N/A (同步) | cancel_rx.try_recv() | `cancelled_by_user` |
| LLM API 致命错误 | Err(ApiError) | break | `stream_error` |
| LLM API 超时 (可重试) | N/A | retry (MAX_STREAM_RETRY_ON_TIMEOUT) | (重试后可能恢复) |
| 成本限制超出 | Err (调用前) | break (调用前) | `failed_to_start_stream` |
| 上下文预算超限 | Err(SessionError) | ContextGovernor trim (非致命) | (裁剪后继续) |
| 执行模式门控 | Err (短路) | Err (短路) | N/A (从未进入循环) |
| 工具执行错误 | tool_result(is_error=true), 继续 | tool_result(is_error=true), 继续 | (LLM 决定下一步) |
| 权限拒绝 | tool_result(error), 继续 | tool_result(error), 继续 | (LLM 决定下一步) |

---

## 10. 关键架构模式

### 10.1 工具定义始终发送

两条路径在每次 LLM 请求中都发送 `tools: Some(definitions)`。不存在条件性 `tools: None` 路径。

### 10.2 双权限路径

- **非流式**: 无交互式 prompter (传 None), 仅依赖 PermissionPolicy 静态规则
- **流式**: 交互式 TauriPermissionPrompter, 通过 Tauri 事件 + mpsc channel 实现前端交互

### 10.3 统一消息格式

两条路径累积相同的 ConversationMessage 类型, 会话持久化格式相同, 记忆钩子接收相同的消息形态。

### 10.4 WorkingMemory 滑动窗口

完整历史保存在 `session.messages` 中, 但发给 LLM 的只有滑动窗口内容 (可配置: 8 turns, max tokens)。每次迭代重建窗口。

### 10.5 Session Context 作用域

`session_id` + `project_id` 是记忆路由的必要参数。非流式和流式路径都提供真实的 scope (Phase 8B.11 已修复)。

### 10.6 迭代限制

- 流式: 硬编码 10 次 (`max_iterations`)
- 非流式: 通过 ConversationRuntime 的 `max_iterations` 配置

两者都在循环开始时检查, 防止无限工具循环。
