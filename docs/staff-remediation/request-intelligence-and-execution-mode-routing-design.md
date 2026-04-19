# If2Ai Request Intelligence And Execution Mode Routing Design

> 迁移 UClaw 的四类聊天执行场景自动匹配机制，建立 If2Ai 的 request intelligence 主轴。
>
> 最后更新: 2026-04-20

## 1. 目标

解决：

1. chat 入口把所有请求都当作同一种执行形态。
2. execution mode、risk、complexity 尚未成为共享契约。
3. 前端没有 explainable routing 投影。

## 2. Canonical Execution Modes

1. `direct_execute`
2. `auto_plan_execute`
3. `plan_then_confirm`
4. `specialized_surface`

## 3. 场景含义

### 3.1 direct_execute

- 一步式
- 低风险
- 低依赖

### 3.2 auto_plan_execute

- 多步
- 低中风险
- 可自动推进

### 3.3 plan_then_confirm

- 多步
- 高风险或高副作用
- 先出计划，再确认

### 3.4 specialized_surface

- coding / compose / workflow 等专门入口

## 4. 决策链

必须采用三段式：

1. deterministic gate
2. heuristic scorer
3. ambiguous-only mini classifier

## 5. 输出合同

- `execution_mode`
- `risk_level`
- `complexity_level`
- `complexity_score`
- `reason_codes`
- `route_hint`
- `requires_plan`
- `classifier_policy_version`
- `classifier_matched_rule_ids`
- `classifier_slot_summary`
- `classifier_ambiguous_escalated`
- `classifier_escalation_source`

## 6. Route Hint

支持：

- `chat`
- `coding`
- `compose`
- `workflow`
- `background`

## 7. Scenario Profiles

上层 profile 参考：

- Chat
- Coding
- Research
- Planning
- Review

规则：

1. profile 是产品模板，不是 runtime truth
2. execution mode 才是 runtime truth
3. profile 可影响默认 model、prompt、gate mode、merge policy
4. profile 不得覆盖 classifier 最终结果

## 8. 前端投影

必须展示：

1. 当前 execution mode
2. risk / complexity
3. matched reasons
4. matched rules
5. manual override

位置：

- Chat
- Inspector
- Developer Settings
- Harness replay

## 9. 后端实现

新增：

- `application/request_intelligence_service.rs`
- `runtime/contracts/execution_mode.rs`
- `control_plane/ingress_classifier.rs`

## 10. Harness 联动

harness 必须记录：

- prompt
- execution mode
- reason codes
- route hint
- final outcome

并能对 classifier policy 做回放回归。

## 11. 执行切片

### Slice R1.1

- execution mode contract
- baseline reason code taxonomy

### Slice R1.2

- deterministic rules
- heuristic scorer

### Slice R1.3

- ambiguous-only classifier seam
- route decision

### Slice R1.4

- frontend execution-mode projection
- developer override

### Slice R1.5

- harness regression suite for classifier

## 12. 验收

1. 四类 execution modes 成为 canonical runtime truth。
2. 前端不再自己判断场景。
3. harness 能比较 classifier 策略版本。
