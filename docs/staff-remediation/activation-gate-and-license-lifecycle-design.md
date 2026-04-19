# If2Ai Activation Gate And License Lifecycle Design

> 按 UClaw 参考实现收敛 If2Ai 的 activation gate、远端激活服务、反激活与 license lifecycle。
>
> 最后更新: 2026-04-20

## 1. 目标

If2Ai 需要把 activation 从“可选设置项”升级为“平台门禁系统”。

必须实现：

1. startup
2. onboarding
3. activation gate
4. main shell

四段主链。

## 2. 参考来源

- `AppState` 持有 activation gate 状态：[AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift:33)
- `RootView` 在主壳层上覆盖 activation gate：[RootView.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/RootView.swift:33)
- `ActivationService` 远端接口：[ActivationService.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationService.swift:32)
- `ActivationLifecycleManager` 生命周期管理：[ActivationLifecycleManager.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationLifecycleManager.swift:8)

## 3. 目标状态机

- `checking_local`
- `needs_activation`
- `requesting_activation`
- `pending_approval`
- `redeeming`
- `activated`
- `offline_grace`
- `expired`
- `revoked`
- `deactivated`

## 4. 远端接口

### Required APIs

- `POST /v1/activations/request`
- `GET /v1/activations/request/{id}`
- `POST /v1/activations/redeem`
- `POST /v1/activations/redeem-by-code`
- `POST /v1/licenses/refresh`
- `POST /v1/licenses/revoke-check`
- `POST /v1/licenses/deactivate` 或等价反激活接口

## 5. 本地存储

- secure token / license storage
- activation persistence
- last trusted server time
- refresh interval
- pending request id

## 6. 前端设计

新增：

- `boot/activation-gate-state.ts`
- `boot/ActivationGate.tsx`
- `boot/license-lifecycle.ts`
- `boot/activation-client.ts`

要求：

1. gate 必须是主窗口上的硬门禁
2. revoke / expire / deactivate 必须回流 gate
3. Settings 提供明确的解绑/反激活入口

## 7. 后端设计

新增：

- `application/activation_service.rs`
- `application/license_lifecycle_service.rs`
- `runtime/contracts/activation.rs`
- `commands/activation.rs`

## 8. 执行切片

### Slice A1.1

- 定义 activation contract
- 增加 activation commands

### Slice A1.2

- 前端 boot state + gate overlay

### Slice A1.3

- refresh / revoke / expire loop

### Slice A1.4

- deactivation flow + settings entry

## 9. 验收

1. 未激活用户无法进入主工作台。
2. 被吊销、过期、反激活后能够回流 gate。
3. activation / refresh / revoke 有完整 telemetry 和 audit。
