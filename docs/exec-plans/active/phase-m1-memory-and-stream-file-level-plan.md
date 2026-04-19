# Phase M1 Memory Injection And Stream Emitter File-Level Plan

> 将 `M1` 的 `m1.4 + m1.5` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m1.4` Extract memory injection service boundary
2. `m1.5` Introduce stream emitter boundary

目标是把 If2Ai 后端主链里两个最容易继续膨胀的横切关注点先收口：

- memory injection
- stream event emission

## 2. 当前事实基线

### 2.1 memory injection 现状

当前 memory 注入至少分散在两段逻辑中：

1. [append_memory_injection_sections](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:58)
2. [retrieve_memory_context](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:846)

并分别在：

1. [run_agent_turn](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:946)
2. [start_agent_stream](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1477)

里重复接线。

也就是说，当前 memory 注入并不是一个正式服务，而是：

- prompt markdown sections 注入
- retrieved memory context 注入
- frontend memory evidence payload

三件事混在 command 路径里。

### 2.2 stream emission 现状

当前 stream 事件主要通过：

- [StreamTokenPayload](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:148)
- 多处 `window.emit("agent-token", payload)`

直接在 [start_agent_stream](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1477) 内部散射发出。

已知事件包括：

1. `text_delta`
2. `thinking_delta`
3. `thinking_start`
4. `tool_call_update`
5. `stream_error`
6. `stream_complete`

结果是：

1. event shape 演化困难
2. canonical contract 无法稳定接入
3. legacy payload 和未来 envelope 只能继续在 command 里纠缠

## 3. 实施原则

1. `m1.4` 先收口“注入入口”，不在这一步完成 `M3` 的 MemoryCoordinator。
2. `m1.5` 先收口“发射入口”，不在这一步完成 `M2` 的前端 translator/reducer。
3. 先把横切逻辑变成 service/boundary，再谈语义增强。
4. 这一轮不改变用户可见行为，不主动改变 memory recall 策略或 stream 事件语义。

## 4. 严格执行顺序

1. `C0` preflight inventory
2. `C1` 建 memory injection service skeleton
3. `C2` 回写 `run_agent_turn`
4. `C3` 回写 `start_agent_stream`
5. `C4` 建 stream emitter skeleton
6. `C5` 先接 `text/thinking/error/complete`
7. `C6` 再接 `tool_call_update`
8. `C7` compile + smoke verification

禁止并行：

1. `memory injection service` 改造
2. `stream emitter` 改造
3. `M2` 前端 translator/store 改造

## 5. 文件级实施方案

## 5.1 `C0` Preflight Inventory

### 必查文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)
- [src-tauri/src/commands/stream_outcome.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/stream_outcome.rs:1)
- [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)

### 必做动作

1. 列出所有 `window.emit("agent-token", ...)` 类型和调用点。
2. 列出 memory 注入数据的三个出口：
   - prompt section
   - prompt fragment
   - frontend evidence items
3. 标出 run/non-stream 两条路径的复用与重复逻辑。

## 5.2 `C1` 建 Memory Injection Service Skeleton

### 新增文件

- `src-tauri/src/modules/application/memory_injection_service.rs`

### 修改文件

- `src-tauri/src/modules/application/mod.rs`
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

### 最低结构建议

```rust
pub struct MemoryInjectionArtifacts {
    pub prompt_sections: Vec<String>,
    pub retrieved_prompt_fragment: Option<String>,
    pub memory_items: Vec<crate::commands::agent::MemoryContextItemPayload>,
}

pub struct MemoryInjectionRequest<'a> {
    pub session_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub workdir: Option<&'a str>,
    pub user_message: &'a str,
}

pub async fn prepare_memory_injection(
    state: &crate::commands::AppState,
    req: MemoryInjectionRequest<'_>,
) -> MemoryInjectionArtifacts
```

### 这一刀要收口的逻辑

把下面两类逻辑从 `agent.rs` 迁进 service：

1. pinned / compiled / rules section 注入
2. retrieved memory prompt fragment + structured memory items

### 这一步不要做的事

1. 不要把 `MemoryContextItemPayload` 大规模搬出 `agent.rs`
2. 不要重写 `ActiveRetrievalManager`
3. 不要改变 memory recall 排序

### 推荐策略

第一版允许 `memory_injection_service.rs` 先内部复用：

1. [build_memory_injection](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:81)
2. `state.active_retrieval_manager`

### 验收标准

`agent.rs` 不再同时定义 `append_memory_injection_sections` 和 `retrieve_memory_context` 的核心逻辑。

## 5.3 `C2` 回写 `run_agent_turn`

### 修改文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:946)

### 必须落实

把当前：

1. `append_memory_injection_sections(...)`
2. `retrieve_memory_context(...)`
3. `system_prompt.push(...)`

改成通过 `memory_injection_service::prepare_memory_injection(...)` 获取一个统一结果对象。

### 推荐结果形态

`run_agent_turn` 只做：

1. 调 service
2. 把 `prompt_sections` append 到 system prompt
3. 把 `retrieved_prompt_fragment` append 到 system prompt
4. 记录 `memory_items` 用于后续 trajectory / UI payload

## 5.4 `C3` 回写 `start_agent_stream`

### 修改文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1477)

### 必须落实

streaming 路径也统一改成复用 `prepare_memory_injection(...)`，避免 non-stream / stream 两条链各自拼 memory。

### 这一刀要特别注意

当前 streaming 路径需要把 `memory_items` 带到最后的 `stream_complete`，因此 service 返回对象必须天然支持：

1. prompt 侧消费
2. frontend payload 侧消费

## 5.5 `C4` 建 Stream Emitter Skeleton

### 新增文件

- `src-tauri/src/modules/runtime/stream_emitter.rs`

### 修改文件

- `src-tauri/src/modules/runtime/mod.rs`
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:148)

### 最低结构建议

```rust
pub struct AgentStreamEmitter {
    window: tauri::Window,
}

impl AgentStreamEmitter {
    pub fn new(window: tauri::Window) -> Self { ... }

    pub fn emit_text_delta(&self, payload: ...) { ... }
    pub fn emit_thinking_delta(&self, payload: ...) { ... }
    pub fn emit_tool_update(&self, payload: ...) { ... }
    pub fn emit_stream_error(&self, payload: ...) { ... }
    pub fn emit_stream_complete(&self, payload: ...) { ... }
}
```

### 这一刀的真正目标

不是先做漂亮 API，而是把“事件如何发出去”从 command 逻辑里收口成一个边界。

## 5.6 `C5` 先接基础事件

### 第一批接入事件

1. `text_delta`
2. `thinking_delta`
3. `thinking_start`
4. `stream_error`
5. `stream_complete`

### 理由

这些事件不依赖 tool loop 状态机，比 `tool_call_update` 更容易先收口。

### 验收标准

`agent.rs` 至少不再为这些事件直接写 `window.emit("agent-token", StreamTokenPayload { ... })`

## 5.7 `C6` 再接 `tool_call_update`

### 涉及位置

当前 `tool_call_update` 事件在 streaming 路径有多处散落调用，包括：

1. tool queued
2. tool running
3. tool completed
4. tool error
5. stream failure 时强制结算 in-flight tool cards

### 必须落实

这些都要收口到 `stream_emitter.rs`，但这一步不能改变现有 tool card 的前端消费语义。

### 特别注意

`policy_decision`、`evidence_id`、`effective_workdir` 这些字段后续会和 canonical envelope 对齐，所以 emitter API 设计不能把它们压没。

## 5.8 `C7` Compile + Smoke Verification

### 必跑

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### 建议补充人工 smoke

至少手工核查一条：

1. 纯文本流式回复
2. 含至少一个工具调用的流式回复
3. 含 memory recall 的回复
4. 一条 stream_error 路径

## 6. 禁止并行动作

在本任务单执行期间，禁止同时做：

1. `M2` 的 runtime event translator 落地
2. `tool_execution_broker` 的大改
3. `MemoryCoordinator` 提前落地
4. `StreamTokenPayload` 字段语义大改

## 7. 推荐提交顺序

1. `memory_injection_service skeleton + run_agent_turn wiring`
2. `start_agent_stream memory wiring`
3. `stream_emitter skeleton + base events`
4. `tool_call_update migration + compile verification`

## 8. 完成标志

只有当 reviewer 能明确指出下面三件事时，这一批切片才算完成：

1. memory 注入现在有正式 service 边界，而不是 command 内部散落 helper
2. stream 事件现在有正式 emitter 边界，而不是 command 内部散落 emit
3. `M2` 可以基于这个 emitter 边界继续演进 canonical event projection，而不用再从 `agent.rs` 的散点调用里考古
