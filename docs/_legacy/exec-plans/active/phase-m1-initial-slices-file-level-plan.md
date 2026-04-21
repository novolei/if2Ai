# Phase M1 Initial Slices File-Level Plan

> 将 `M1` 的首批真实代码切片 `m1.1 + m1.2 + m1.3` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖 `M1` 的前三个切片：

1. `m1.1` turn service
2. `m1.2` provider service extraction
3. `m1.3` prompt planning skeleton

目标不是“一次做完 M1”，而是把后端主脑从 [commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1) 撬开第一个稳定缺口。

## 2. 当前事实基线

### 2.1 当前核心问题

当前 [commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1) 同时承担：

1. Tauri IPC command adapter
2. provider resolving
3. system prompt building
4. memory injection 拼接
5. runtime creation
6. stream emission
7. permission prompt bridging
8. learning / reflection / post-turn hooks

这意味着任何一项后端能力变动，都会扩大 `agent.rs` 的责任面。

### 2.2 已存在可复用资产

1. provider listing/configure/select 已经有 [modules/provider/service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/provider/service.rs:1)
2. model role resolving 已经有 [model_resolver.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/config/model_resolver.rs:1)
3. system prompt builder 已经有 [runtime/prompt.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/prompt.rs:1)
4. memory injection 已经有 [memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)

所以 `M1` 的第一步不是重写，而是把已有实现收口成正式服务边界。

## 3. 首批切片严格顺序

必须按下面顺序实施：

1. `B0` 建立 application 模块骨架
2. `B1` 落 `turn_service` 接缝
3. `B2` 落 provider runtime service
4. `B3` 落 prompt planner skeleton
5. `B4` 回写 `agent.rs` 调用路径
6. `B5` 编译与人工验收

禁止并行修改：

1. `agent.rs` 主执行流改造
2. `prompt.rs` 结构化 plan 接缝
3. provider runtime client 组装逻辑

因为这三者同时动，最容易把请求主路径搞断。

## 4. 文件级实施方案

## 4.1 `B0` 建立 application 模块骨架

### 新增文件

- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/application/provider_runtime_service.rs`
- `src-tauri/src/modules/application/prompt_planner.rs`

### 修改文件

- [src-tauri/src/modules/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/mod.rs:1)

### 必须落实

1. 在 `modules/mod.rs` 新增 `pub mod application;`
2. `application/mod.rs` 导出三类入口：
   - `turn_service`
   - `provider_runtime_service`
   - `prompt_planner`
3. 暂不引入 activation/request_intelligence/memory_injection service 文件，避免第一刀过大

### 验收

只要求 crate 编译可见，不要求功能接线完成。

## 4.2 `B1` 落 `turn_service` 接缝

### 目标

先把 `commands/agent.rs` 和“真实 turn 编排”隔开一层，让后续抽 provider 和 prompt 时不直接在 command 里下刀。

### 新增文件

- `src-tauri/src/modules/application/turn_service.rs`

### 修改文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

### `turn_service.rs` 最低结构建议

```rust
pub struct TurnServiceDeps {
    pub tool_registry: Arc<crate::modules::tools::ToolRegistry>,
}

pub struct TurnService {
    deps: TurnServiceDeps,
}

pub struct TurnRequest {
    pub session_id: String,
    pub message: String,
    pub requested_permission_mode: Option<String>,
}

pub struct TurnContext {
    pub execution_context: crate::modules::control_plane::SessionExecutionContext,
}
```

### 第一阶段不要做的事

1. 不要在 `TurnService` 里一次吞进所有 `AppState`
2. 不要现在就把 streaming/non-streaming 两条路径完全合并

### `agent.rs` 的第一刀

先把下面逻辑包进 `TurnService` 负责的接缝里：

1. 恢复 session
2. resolve execution context
3. 调用 provider runtime service
4. 调用 prompt planner

### 第一阶段推荐做法

保留 `run_agent_turn` / `start_agent_stream` 两个 Tauri command 不变，  
但让它们开始调用：

1. `TurnService::prepare_non_stream_turn(...)`
2. `TurnService::prepare_stream_turn(...)`

哪怕先只是把前置组装动作迁进去，也算完成第一刀。

### 验收标准

1. `run_agent_turn` 和 `start_agent_stream` 中至少不再直接调用 provider resolving helper
2. command 层开始变成“收参数 -> 调 service -> 继续 runtime path”

## 4.3 `B2` 落 provider runtime service

### 目标

把当前 `agent.rs` 里的：

- `load_provider_transport_policy`
- `create_runtime_provider_client_from_config`

迁出成正式 service，避免 provider runtime creation 继续藏在 command 文件里。

### 新增文件

- `src-tauri/src/modules/application/provider_runtime_service.rs`

### 修改文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:218)

### 迁出函数

当前直接可迁出的目标是：

1. `load_provider_transport_policy`
2. `create_runtime_provider_client_from_config`

### service API 建议

```rust
pub struct RuntimeProviderResolution {
    pub provider_client: crate::modules::api::ProviderClient,
    pub model: String,
    pub request_timeout: Duration,
}

pub async fn resolve_chat_runtime_provider(
    workdir: &Path,
) -> Result<RuntimeProviderResolution, String>
```

### 复用关系

内部应继续复用：

1. [ModelResolver::resolve_role_model](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/config/model_resolver.rs:180)
2. [ProviderTransportConfig](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/config.rs:1)
3. 现有 `ClawApiClient` / `OpenAiCompatClient` 组装逻辑

### 不允许做的事

1. 不要重写 provider service 全模块
2. 不要改变 fallback 行为
3. 不要在这一步切换 provider 配置存储模型

### 验收标准

1. `agent.rs` 不再定义 provider runtime creation helper
2. provider runtime creation 有独立 import path

## 4.4 `B3` 落 prompt planner skeleton

### 目标

把 prompt assembling 从“运行时顺手拼字符串”转成“有结构的 plan 入口”，哪怕第一版只是 skeleton。

### 新增文件

- `src-tauri/src/modules/application/prompt_planner.rs`

### 修改文件

- [src-tauri/src/modules/runtime/prompt.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/prompt.rs:355)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1028)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1594)

### skeleton 最低结构

```rust
pub enum PromptBlockKind {
    System,
    ProjectContext,
    SkillsIndex,
    MemoryInjection,
    RetrievedMemory,
    DynamicInstructions,
}

pub struct PromptBlock {
    pub kind: PromptBlockKind,
    pub title: &'static str,
    pub content: String,
}

pub struct PromptPlan {
    pub blocks: Vec<PromptBlock>,
}
```

### 第一阶段实施策略

不要上来重写 [load_system_prompt](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/prompt.rs:767)。  
更稳的做法是：

1. 先新增 `build_prompt_plan(...)`
2. 内部仍调用现有 `load_system_prompt(...)`
3. 先把当前已有 section 包装成 `PromptBlock`
4. memory injection 和 retrieved memory 先作为后附 block 挂入 plan

### 关键目的

这一刀的目的不是优化 prompt，而是给 `M4` 的 harness prompt composition trace 留正式入口。

### 不允许做的事

1. 不要同时重写 skills index builder
2. 不要把 memory coordinator 提前塞进来
3. 不要把 prompt skeleton 和 stream emitter 一起改

### 验收标准

1. 有 `PromptPlan` / `PromptBlock` 正式类型
2. `run_agent_turn` 或 `start_agent_stream` 至少一条路径开始通过 prompt planner 取 prompt

## 4.5 `B4` 回写 `agent.rs` 调用路径

### 修改目标

[commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

### 目标状态

`agent.rs` 里的调用链开始变成：

1. command 参数校验
2. restore session
3. resolve execution context
4. `provider_runtime_service::resolve_chat_runtime_provider(...)`
5. `prompt_planner::build_prompt_plan(...)`
6. 进入现有 runtime path

### 第一阶段允许保留

1. memory retrieval / injection 仍在 `agent.rs`
2. stream emitter 仍在 `agent.rs`
3. permission prompter bridge 仍在 `agent.rs`

原因：这是 `m1.4` 和 `m1.5` 的职责。

## 4.6 `B5` 编译与人工验收

### 必跑

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### 人工验收问题

reviewer 必须能回答：

1. provider runtime creation 现在在哪里？
2. prompt composition 现在从哪里进入？
3. `agent.rs` 现在主要还剩哪些责任？
4. 下一刀为什么应该先做 memory injection，而不是先碰 stream emitter？

## 5. 禁止并行动作

在这三片期间，禁止同时做：

1. `start_agent_stream` 的深度 tool loop 重写
2. `memory/inject.rs` 行为语义大改
3. `AppState` 结构重排
4. 前端 `M2` store/projection 改造

## 6. 提交建议

建议至少拆成 3 次逻辑提交：

1. `application skeleton + turn_service seam`
2. `provider runtime service extraction`
3. `prompt planner skeleton + agent wiring`

## 7. 完成标志

只有当下面三件事同时成立，这一批初始切片才算完成：

1. `agent.rs` 不再直接定义 provider runtime creation helper
2. prompt 组装开始通过正式 planner 入口
3. 后续 `m1.4` 能自然接到 `TurnService + PromptPlan` 这两条新接缝上
