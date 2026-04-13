# Desktop 混合策略落地蓝图（Post-CLI）

最后更新：2026-04-13

## 背景与目标

当前 Desktop APP 已具备基础权限模式、工具调用和会话管理能力，但在多会话并发、工具边界统一、执行审计可追踪方面仍存在结构性风险。  
本蓝图采用“**Claw-CLI 内核能力 + Desktop 控制平面**”的混合策略：

- 复用 CLI 侧成熟内核：`permissions`、`sandbox`、`hooks`、`config`。
- 构建 Desktop 专属控制平面：会话上下文隔离、统一边界解析、执行审计、前端可解释策略反馈。

核心目标：

1. 任意工具调用都可证明“是谁、在何处、按何策略、执行了什么、结果如何”。
2. 任意路径与命令执行都不能越过项目工作目录边界。
3. 多会话并发下不存在上下文串扰与权限漂移。

---

## 架构总览（组件图）

```text
┌──────────────────────────────────────────────────────────────┐
│                    Frontend (src/*)                         │
│ Chat UI / Permission Dialog / Tool Cards / Session View     │
└──────────────────────────────┬───────────────────────────────┘
                               │ Tauri IPC
┌──────────────────────────────▼───────────────────────────────┐
│             Command Facade (src-tauri/src/commands/*)       │
│ agent.rs / tools.rs / session.rs / project.rs               │
└──────────────────────────────┬───────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────┐
│                 Desktop Control Plane（新增）                │
│ 1) SessionContextResolver                                    │
│ 2) PolicyBroker                                              │
│ 3) BoundaryResolver                                          │
│ 4) ToolExecutionBroker                                       │
│ 5) AuditEmitter                                              │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│      Runtime Kernel (src-tauri/modules/runtime/*)           │
│ permissions.rs / sandbox.rs / hooks.rs / config.rs          │
│ (对齐 rust/crates/runtime 的策略语义)                        │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│         Tool Adapters (src-tauri/modules/tools/*)           │
│ file_* / grep/glob/content_search / bash/REPL/PowerShell    │
└──────────────────────────────────────────────────────────────┘
```

---

## 设计原则

1. **策略单源**：权限判定、路径边界、会话上下文必须有统一入口，不允许分散在工具内部。
2. **默认拒绝**：新工具默认不可执行，需显式声明权限级别和上下文需求。
3. **会话隔离**：`workdir` 和 permission context 必须会话级隔离，禁止全局可变上下文共享。
4. **证据优先**：涉及“已创建/已修改/已删除”语义时，必须有工具成功证据绑定。
5. **可回滚**：每个阶段必须有 feature flag 和降级路径。

---

## 当前代码映射（As-Is）

### A. 命令入口层（已具备基础）

- `src-tauri/src/commands/agent.rs`
  - 已有 `build_permission_policy`、流式工具执行、permission prompt 事件。
  - 已引入会话级 `resolve_session_workdir` 与 `dispatch_with_context` 的方向。
- `src-tauri/src/commands/tools.rs`
  - 已支持 `permission_mode`。
  - 已新增 `session_id`（可选）与高风险工具会话绑定约束。

### B. 工具注册层（已演进）

- `src-tauri/src/modules/tools/registry.rs`
  - 已有 `dispatch_with_context`，具备按上下文隔离执行能力。

### C. 工具适配层（仍需标准化）

- 文件/搜索/命令工具仍存在“实现风格不一致”问题：
  - 路径解析、边界校验、错误语义、审计字段未统一抽象。

### D. Runtime 内核层（可复用能力）

- `src-tauri/src/modules/runtime/*` 与 `rust/crates/runtime/*` 存在同源能力：
  - `permissions.rs`、`sandbox.rs`、`hooks.rs`、`config.rs`。
- 目前两侧策略语义有局部差异，需要明确“Desktop 语义覆盖层”。

---

## To-Be：混合策略实施路线（按模块）

### 模块 1：Command Facade 收敛（`commands/*`）

目标：所有工具调用都必须进入控制平面。

实施：

1. `agent.rs` 与 `tools.rs` 统一接入 `ToolExecutionBroker`。
2. `session.rs` 增加 session/project 绑定一致性校验接口。
3. 禁止任何直接调用 `ToolRegistry::dispatch` 的新代码路径。

产出：

- 统一的执行入口协议：`session_id`, `project_id`, `permission_mode`, `tool_name`, `args`。

---

### 模块 2：控制平面落地（新增目录建议）

建议新增：

- `src-tauri/src/modules/control_plane/session_context.rs`
- `src-tauri/src/modules/control_plane/policy_broker.rs`
- `src-tauri/src/modules/control_plane/boundary_resolver.rs`
- `src-tauri/src/modules/control_plane/tool_execution_broker.rs`
- `src-tauri/src/modules/control_plane/audit.rs`

职责：

1. **SessionContextResolver**  
   统一解析 `session -> project -> canonical_workdir`，返回不可变上下文快照。

2. **PolicyBroker**  
   基于 `PermissionPolicy` 执行 allow/deny/escalation 决策，支持 UI prompt。

3. **BoundaryResolver**  
   统一 `resolve -> normalize -> canonicalize -> boundary check`。

4. **ToolExecutionBroker**  
   串联：参数校验 -> policy -> boundary -> dispatch_with_context -> 审计。

5. **AuditEmitter**  
   统一记录结构化事件（开始、决策、结束、失败、超时）。

---

### 模块 3：Runtime Kernel 对齐（`modules/runtime/*`）

目标：复用 Claw-CLI 内核语义，减少策略分叉。

实施：

1. 对齐 `permissions` 判定语义与工具等级映射规则。
2. 将 `bash` 执行路径尽量复用 `runtime/bash.rs` 的 sandbox 状态解析和输出结构。
3. 对齐 `hooks` 生命周期（pre/post tool use）及错误传播语义。
4. 对齐 `config` 的权限/沙箱字段加载优先级（user/project/local）。

---

### 模块 4：工具适配层标准化（`modules/tools/builtin/*`）

目标：工具不再各自实现边界策略。

实施：

1. 文件与搜索工具统一使用 `BoundaryResolver`。
2. 命令执行工具统一使用上下文 `current_dir`，禁止拼接式 `cd ... && ...`。
3. 工具返回结构增加标准审计字段（execution id / evidence id）。

---

### 模块 5：前端可解释策略反馈（`src/*`）

目标：用户可见“决策依据”和“执行证据”。

实施：

1. 工具卡片显示：effective workdir、权限模式、策略决策。
2. permission dialog 显示：当前模式、目标模式、工具级风险说明。
3. 对最终文案进行“证据绑定守卫”，无证据不允许宣称已完成写操作。

---

## 迁移顺序（分阶段）

### Phase A（控制平面最小闭环，1-2 周）

- 完成 `SessionContextResolver + ToolExecutionBroker + BoundaryResolver` 最小版。
- `agent.rs/tools.rs` 全量接入 broker。
- 保持行为等价，不做策略语义变更。

**验收**

- 所有执行日志可关联 session/workdir。
- 无直接 `dispatch` 新调用路径。

### Phase B（内核对齐，1-2 周）

- `permissions/sandbox/hooks/config` 与 CLI runtime 对齐。
- `bash` 统一走 sandbox 状态模型。

**验收**

- 同一权限配置在 CLI/Desktop 的判定语义一致。
- `sandboxStatus` 可稳定透传到前端。

### Phase C（观测与审计，1 周）

- 上线结构化审计事件（JSON line + trace id）。
- 增加安全仪表指标：越界拒绝率、权限升级率、无证据回复率。

**验收**

- 任意事故可通过 session trace 回放定位。

### Phase D（并发压测与演练，1 周）

- 多会话并发压测（读写/搜索/命令工具）。
- 故障演练：策略漂移、上下文冲突、沙箱不可用降级。

**验收**

- 无上下文串扰。
- 触发异常时自动降级只读并告警。

---

## 风险与回滚点

### R1：入口统一导致调用失败率上升

- 风险：broker 初期误判造成工具拒绝增加。
- 回滚：`controlPlane.controlPlaneV2Enabled=false`（或环境变量 `IF2AI_CONTROL_PLANE_V2_ENABLED=0`）回退旧入口。

### R2：边界校验过严导致合法请求失败

- 风险：历史流程依赖宽松路径行为。
- 回滚：`controlPlane.boundaryEnforceMode=shadow`（或环境变量 `IF2AI_BOUNDARY_ENFORCE_MODE=shadow`），切 shadow 模式（仅告警不阻断）。

### R3：沙箱兼容性问题（跨平台）

- 风险：Linux/macOS/Windows 行为差异导致功能回退。
- 回滚：`controlPlane.sandboxStrictMode=false`（或环境变量 `IF2AI_SANDBOX_STRICT_MODE=0`），保留执行但标记风险与审计事件。

### 开关矩阵（灰度治理）

| 开关                                 | 默认值    | 说明                                | 目标场景       |
| ------------------------------------ | --------- | ----------------------------------- | -------------- |
| `controlPlane.controlPlaneV2Enabled` | `true`    | 是否启用控制平面 V2（broker 路径）  | 整体回退       |
| `controlPlane.boundaryEnforceMode`   | `enforce` | `enforce`=阻断越界，`shadow`=仅告警 | 边界灰度       |
| `controlPlane.sandboxStrictMode`     | `true`    | 是否启用严格沙箱策略                | 沙箱兼容性灰度 |

### R4：会话强绑定导致旧调用链断裂

- 风险：旧前端未传 `session_id`。
- 回滚：短期兼容 read-only 无状态工具，逐步强制高风险工具必须带 session。

### R5：并发隔离改造引入死锁/性能抖动

- 风险：context 生命周期管理不当。
- 回滚：按 session 串行执行开关，先保安全再优化吞吐。

---

## 验收指标（建议）

1. 越界写入事件数：`0`（P0 指标）。
2. 跨会话串扰事件数：`0`（P0 指标）。
3. 无证据写操作宣称率：`0`（P0 指标）。
4. 权限拒绝误杀率：< `1%`（P1 指标，结合白名单优化）。
5. 事故定位平均时长（MTTR）：< `10 min`（P1 指标）。

---

## 实施建议结论

最适合当前 Desktop APP 的策略不是“完全移植 Claw-CLI”，而是：

- **内核能力对齐**（permissions/sandbox/hooks/config）
- **控制平面强化**（session 隔离、统一边界、审计可追踪、前端可解释）

该路径能在保持现有功能连续性的同时，系统性降低边界类事故的复发概率。
