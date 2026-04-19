# If2Ai Backend Application And Control Plane Refactor Design

> 将 command-first 后端拆回 application service + control plane + runtime 合理边界。
>
> 最后更新: 2026-04-20

## 1. 目标

解决：

1. `commands/agent.rs` 过胖。
2. `AppState` 聚合过多对象但没有应用服务边界。
3. permission / boundary / sandbox / audit / request intelligence 未形成统一 control plane。

## 2. 目标后端模块

- `commands`
- `application`
- `runtime`
- `control_plane`
- `workers`
- `memory_learning`
- `observability`

## 3. Application Services

### 3.1 turn_service

- 负责一次 turn 的编排入口
- 调用 prompt plan、memory coordinator、provider service、stream emitter

### 3.2 session_service

- 负责 session create / switch / restore / archive

### 3.3 provider_service

- 负责 provider/model resolving

### 3.4 request_intelligence_service

- 负责 execution mode 分类、route decision、classifier evidence

### 3.5 activation_service

- 负责远端 activation request / status / redeem / revoke-check / refresh

### 3.6 license_lifecycle_service

- 负责 refresh loop、revocation、expiry、deactivation 回流

### 3.7 worker adoption boundary

- worker 在 If2Ai 中的首要角色是统一工具执行后端抽象
- 不等于 first-class background worker runtime
- 详情见 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)

## 4. Control Plane

### 4.1 boundary

- 文件、命令、网络、项目根边界

### 4.2 permission

- 用户设置、运行时要求、risk 对齐

### 4.3 sandbox

- 把 boundary + permission 物化为执行策略

### 4.4 audit

- tool、memory、activation、execution-mode 都进 audit

### 4.5 ingress classifier

- classifier 作为 control-plane 决策器，而不是前端 heuristics

### 4.6 worker preflight seam

- `prepare_step_execution` 的输出要直接服务于 worker execution
- worker 只读 execution input，不自行推断边界、权限、沙箱

## 5. 关键接口

### `prepare_step_execution`

输入：

- execution boundary
- required capabilities
- permission context

输出：

- resolved boundary
- permission decision
- sandbox policy

### `classify_request`

输入：

- prompt text
- workspace context
- side-effect hints
- prior session hints

输出：

- execution mode result

## 6. M1 拆分目标

从 `commands/agent.rs` 拆出：

1. prompt assembling
2. provider resolving
3. memory injection
4. stream emitting
5. request intelligence
6. worker skeleton 与 tool-execution seam

## 7. 执行切片

### Slice B1.1

- 新增 `application/turn_service.rs`
- `commands/agent.rs` 改为 adapter

### Slice B1.2

- 新增 `control_plane` 目录
- 加入 `prepare_step_execution`

### Slice B1.3

- 新增 `request_intelligence_service.rs`
- classifier 进入服务层

### Slice B1.4

- 新增 `activation_service.rs`、`license_lifecycle_service.rs`

### Slice B1.5

- 新增 `workers/contract.rs`、`workers/capability.rs`、`workers/registry.rs`
- 让 `prepare_step_execution` 对接 worker-facing 执行输入

## 8. 验收

1. `commands/agent.rs` 不再承担主业务编排。
2. permission / boundary / sandbox 有统一入口。
3. classifier 与 activation 进入 application/control-plane 正式位置。
4. worker 作为统一工具执行后端抽象出现，但不提前引入重型 worker runtime。
