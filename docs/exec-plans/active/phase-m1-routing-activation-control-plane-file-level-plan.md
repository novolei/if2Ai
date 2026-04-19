# Phase M1 Routing Activation And Control-Plane File-Level Plan

> 将 `M1` 的 `m1.6 + m1.7 + m1.8` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m1.6` Add request intelligence service
2. `m1.7` Introduce activation and license lifecycle services
3. `m1.8` Introduce `prepare_step_execution` control-plane seam

目标是把 `M1` 的最后三刀收成一条连续的后端主线：

`request classification -> activation platform service -> tool execution preflight`

## 2. 当前事实基线

### 2.1 request intelligence 现状

当前 If2Ai 还没有正式的：

- `request_intelligence_service`
- `ingress_classifier`
- `execution_mode` 后端决策入口

也就是说，`execution mode` 在 If2Ai 现在还不是正式的 runtime truth。

### 2.2 activation 现状

当前 [commands/activation.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/activation.rs:1) 仍然是 onboarding/ceremony 风格的 activation command，主要做：

1. precondition validate
2. activation ceremony start
3. test message
4. onboarding complete

这还不是 blueprint 里要求的：

- remote activation request
- redeem / refresh / revoke-check
- deactivate / license lifecycle

### 2.3 control plane 现状

当前已有：

- [SessionContextResolver](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/session_context.rs:1)
- [BoundaryResolver](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/boundary_resolver.rs:1)
- [ToolExecutionBroker](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)
- [PermissionPolicy](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/permissions.rs:1)

但还没有 blueprint 语义上的：

`prepare_step_execution(boundary -> permission -> sandbox)`

这意味着工具执行前的统一决策入口 هنوز没有成型。

## 3. 实施原则

1. `m1.6` 先建立 request intelligence 的服务入口和合同，不一次做完整 classifier。
2. `m1.7` 先把 activation 从 ceremony command 收成平台 service 边界，不要求马上打通真实远端服务。
3. `m1.8` 先把 control-plane preflight 接缝定义出来，并接到少量高价值工具，不追求一次接满所有工具。
4. 这一轮重点是“服务边界成型”，不是“产品功能完成”。

## 4. 严格执行顺序

1. `R0` preflight inventory
2. `R1` request intelligence contract + service skeleton
3. `R2` activation service / license lifecycle service skeleton
4. `R3` activation command adapter reshaping
5. `R4` prepare_step_execution skeleton
6. `R5` broker / registry preflight seam
7. `R6` compile + manual review

禁止并行：

1. request intelligence service 改造
2. activation command 重做
3. tool execution preflight 接线

因为这三条同时动时，最容易把 runtime truth、onboarding truth 和 tool execution truth 搅在一起。

## 5. 文件级实施方案

## 5.1 `R0` Preflight Inventory

### 必查文件

- [src-tauri/src/commands/activation.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/activation.rs:1)
- [src-tauri/src/modules/control_plane/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)
- [src-tauri/src/modules/control_plane/session_context.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/session_context.rs:1)
- [src-tauri/src/modules/control_plane/boundary_resolver.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/boundary_resolver.rs:1)
- [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)
- [src-tauri/src/modules/runtime/permissions.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/permissions.rs:1)
- [src-tauri/src/modules/tools/registry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/registry.rs:1)
- [src-tauri/src/modules/tools/context.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/context.rs:1)

### 必做动作

1. 盘出 execution mode 当前缺失哪些后端输出字段。
2. 盘出 activation 当前哪些 command 是 ceremony 语义，哪些未来应保留。
3. 盘出 tool execution 当前在哪一步才真正发生 permission/boundary 判断。

## 5.2 `R1` Request Intelligence Contract + Service Skeleton

### 新增文件

- `src-tauri/src/modules/application/request_intelligence_service.rs`
- `src-tauri/src/modules/control_plane/ingress_classifier.rs`

### 修改文件

- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/modules/control_plane/mod.rs`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`

### 最低结构建议

```rust
pub struct RequestIntelligenceInput<'a> {
    pub user_message: &'a str,
    pub session_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub workdir: Option<&'a std::path::Path>,
}

pub struct RequestIntelligenceOutput {
    pub decision: ExecutionModeDecision,
}

pub async fn classify_request(
    input: RequestIntelligenceInput<'_>,
) -> RequestIntelligenceOutput
```

### 第一阶段必须落实

1. deterministic gate skeleton
2. heuristic scorer skeleton
3. route hint / reason codes 的 typed 输出

### 第一阶段不要做的事

1. 不要把 mini classifier 真正接成远程 LLM 依赖
2. 不要把 profile/template 系统混进 classifier

### 建议第一批 rules

至少落 4 类基线判断：

1. 高风险副作用 -> `plan_then_confirm`
2. 低风险一步式 -> `direct_execute`
3. 多步明显任务 -> `auto_plan_execute`
4. 明显专门入口 -> `specialized_surface`

## 5.3 `R2` Activation Service / License Lifecycle Service Skeleton

### 新增文件

- `src-tauri/src/modules/application/activation_service.rs`
- `src-tauri/src/modules/application/license_lifecycle_service.rs`
- `src-tauri/src/modules/runtime/contracts/activation.rs`

### 修改文件

- `src-tauri/src/modules/application/mod.rs`

### 最低结构建议

```rust
pub enum ActivationStatus { ... }

pub struct ActivationSnapshot {
    pub status: ActivationStatus,
    pub license_id: Option<String>,
    pub refresh_after_sec: Option<u64>,
    pub revoked: bool,
    pub expired: bool,
    pub deactivated: bool,
}
```

### 第一阶段必须落实

1. activation request / redeem / refresh / revoke-check / deactivate 的 service 方法签名
2. local persistence / snapshot 语义
3. boot truth 能消费的 activation snapshot 形状

### 第一阶段不要做的事

1. 不要把远端 API client 一次写死到具体供应商
2. 不要在这轮把前端 gate 流程一起做完

## 5.4 `R3` Activation Command Adapter Reshaping

### 修改文件

- [src-tauri/src/commands/activation.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/activation.rs:1)
- [src-tauri/src/commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:1)

### 目标

把 activation command 从“onboarding ceremony handler”开始收成“双层语义”：

1. legacy onboarding activation commands 暂保留
2. 新 platform activation commands 通过 application service 暴露

### 建议新增 command

1. `activation_get_status`
2. `activation_request_license`
3. `activation_redeem`
4. `activation_refresh`
5. `activation_revoke_check`
6. `activation_deactivate`

### 这一刀特别重要

不要直接删除现有：

- `activation_validate`
- `activation_start`
- `activation_test_message`
- `activation_complete`

它们仍服务当前 onboarding 现实。正确做法是把“平台 activation”作为并行正式入口新增出来。

## 5.5 `R4` Prepare Step Execution Skeleton

### 新增文件

- `src-tauri/src/modules/control_plane/prepare_step_execution.rs`

### 修改文件

- `src-tauri/src/modules/control_plane/mod.rs`

### 最低结构建议

```rust
pub struct PrepareStepExecutionInput<'a> {
    pub tool_name: &'a str,
    pub session_context: &'a SessionExecutionContext,
    pub args: &'a serde_json::Value,
}

pub struct PrepareStepExecutionOutput {
    pub boundary_decision: ...,
    pub permission_decision: ...,
    pub sandbox_policy: ...,
}
```

### 第一阶段必须落实

1. 把 boundary / permission / sandbox 三层串起来
2. 明确 granted / requires_approval / denied 三种结果

### 第一阶段不要做的事

1. 不要一次重写全部 tool execution path
2. 不要在这步直接引入完整 worker runtime

## 5.6 `R5` Broker / Registry Preflight Seam

### 修改文件

- [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)
- [src-tauri/src/modules/tools/context.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/context.rs:1)
- [src-tauri/src/modules/tools/registry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/registry.rs:1)

### 目标

先让 `ToolExecutionBroker` 开始在执行前调用 `prepare_step_execution(...)`，哪怕第一版只接少量高价值工具。

### 建议第一批接入工具

1. `bash`
2. `read_file`
3. `write_file` / `file_edit`
4. `glob_search` / `grep_search`

### 建议落地方式

1. broker 先构造 `PrepareStepExecutionInput`
2. 获取 preflight result
3. granted 才继续 dispatch
4. denied / requires_approval 先统一返回 typed error / audit event

### 风险提示

当前 `PermissionPolicy` 和 `ToolExecutionBroker` 已有一套现实逻辑，所以第一版不要试图“一步统一完全部权限体系”。  
更稳的路径是：

1. 先插一个显式 preflight seam
2. 先 shadow / log
3. 再逐步 enforce

## 5.7 `R6` Compile + Manual Review

### 必跑

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### 人工审查问题

reviewer 必须能回答：

1. execution mode 现在从哪里正式给出？
2. activation 现在哪里开始是平台 service，哪里还是 ceremony command？
3. tool execution 前现在有没有单一 preflight seam？
4. 为什么这一步还不该直接引入完整 worker runtime？

## 6. 禁止并行动作

在这份任务单执行期间，禁止同时做：

1. `M2` activation gate UI 实现
2. `M4` harness compare / blocker 落地
3. worker execution abstraction 的大规模接入

因为 `M1` 这一步的目标只是把 service 和 preflight seam 立住。

## 7. 推荐提交顺序

1. `request_intelligence_service + ingress_classifier skeleton`
2. `activation_service + license_lifecycle_service + activation contracts`
3. `activation command adapter reshape`
4. `prepare_step_execution + broker preflight seam`

## 8. 完成标志

只有当 reviewer 能明确指出下面三件事时，这一批切片才算完成：

1. request intelligence 已经成为正式后端服务，而不是未来愿景
2. activation 已经开始从 onboarding ceremony 语义分裂出平台服务语义
3. tool execution 前已经有单一 `prepare_step_execution` 接缝，而不是继续靠 command/broker 各自判断
