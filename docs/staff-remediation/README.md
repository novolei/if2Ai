# Staff Remediation Docs

> Staff 级系统整改与演进设计文档目录。
>
> 最后更新: 2026-04-20（M0 全部 7 个 slice 已落地：canonical 真相 + contracts skeleton + god-file responsibility inventory + plan linkage）

## 目录

- [if2ai-staff-remediation-blueprint.md](./if2ai-staff-remediation-blueprint.md)
  - 当前 codebase 全盘 review 结论
  - 前后端 / 架构 / harness / memory 风险
  - 90 天整改路线图
  - 智能体记忆、自我进化、评测治理升级方案
- [uclaw-if2ai-architecture-migration-report.md](./uclaw-if2ai-architecture-migration-report.md)
  - UClaw 与 If2Ai 的第二阶段横向对照实施报告
  - 可立即迁移 / 需改造迁移 / 不建议迁移
  - 前后端重构顺序与 phase 设计
- [canonical-domain-model-and-workflow-truth-design.md](./canonical-domain-model-and-workflow-truth-design.md)
  - canonical entity 定义
  - workflow truth 机制
  - 文档即真相约束
- [if2ai-canonical-domain-model.md](./if2ai-canonical-domain-model.md)
  - Phase M0.1 产出
  - 10 个 canonical 实体目录、生命周期、所属层、当前代码入口、未来归属
  - 5 类命名漂移迁移映射
- [if2ai-workflow-truth.md](./if2ai-workflow-truth.md)
  - Phase M0.2 + M0.4 产出
  - 16 条主 workflow 真相状态（canonical / partial / not_established）
  - 入口、真相归属、阻塞原因、下一阶段责任 phase
  - §3.17 boot truth 状态机（startup → onboarding → activation_gate → main_shell）
- canonical contract 入口（M0.3 + M0.4 + M0.5 skeleton）
  - Rust：[src-tauri/src/modules/runtime/contracts/](../../src-tauri/src/modules/runtime/contracts/)
    （`common` 含 `RuntimeEventEnvelope / RuntimeEventType / SchemaVersion`；
    `activation` 含 10 态 `ActivationStatus / ActivationSnapshot / ActivationAction` + 6 类 lifecycle action；
    `execution_mode` 含 4 类 canonical mode `ExecutionMode` + `ExecutionModeDecision` + `ClassifierEvidence`；
    `memory` 含 `MemoryKind / MemoryScope / MemoryDecision / MemoryProjection`）
  - TypeScript 对位：[src/transport/contracts.ts](../../src/transport/contracts.ts)
- [m0-god-file-responsibility-inventory.md](./m0-god-file-responsibility-inventory.md)
  - Phase M0.6 产出
  - 四大 god-file（`commands/agent.rs` / `App.tsx` / `chat-ui.tsx` / `tauri.ts`，共 13587 行）的责任、目标 module、安全抽离顺序、风险耦合点
  - M1 / M2 拆分输入清单（`commands/agent.rs` 全部归 M1；前端三件归 M2）
- [runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)
  - runtime contracts
  - event envelope
  - execution-mode / activation / memory 投影
- [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)
  - backend application service 拆分
  - control plane 重构
  - request intelligence / activation / policy 归位
- [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)
  - worker 迁移边界
  - 统一工具执行后端抽象
  - 首批工具接入与 phase 映射
- [activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md)
  - activation gate
  - 远端激活服务
  - 反激活 / revoke / refresh 生命周期
- [request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md)
  - 四类聊天执行场景自动匹配
  - request intelligence
  - execution mode routing 与 classifier evidence
- [harness-v2-governance-design.md](./harness-v2-governance-design.md)
  - Harness 从 trace recorder 升级为治理与发布门
  - 回放、评测、回归、策略验证与上线规则
- [memory-self-evolution-design.md](./memory-self-evolution-design.md)
  - 记忆对象模型
  - 在线 / 离线闭环
  - 自我进化与安全上线策略
- [frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md)
  - 前端 IA 重构
  - Chat / Settings / Debug surface 分层
  - UI/UX 与可信度整改原则
- [remediation-program-and-phase-roadmap.md](./remediation-program-and-phase-roadmap.md)
  - M0 ~ M5
  - 90 天路线图
  - 优先级、风险、执行计划映射

## 覆盖矩阵

`if2ai-staff-remediation-blueprint.md` 的各大章节与子文档覆盖关系如下：

- `1-6` 总体问题定义、定位、目标
  - 由 [if2ai-staff-remediation-blueprint.md](./if2ai-staff-remediation-blueprint.md) 继续承担总纲
- `7-8` 目标架构蓝图、可迁移内容分级
  - 由 [uclaw-if2ai-architecture-migration-report.md](./uclaw-if2ai-architecture-migration-report.md)
  - 由 [canonical-domain-model-and-workflow-truth-design.md](./canonical-domain-model-and-workflow-truth-design.md)
  - 由 [runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)
  - 由 [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)
  - 由 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)
  - 由 [activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md)
  - 由 [request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md)
- `9`
  - 由 [frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md)
- `10`
  - 由 [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)
  - 由 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)
- `11`
  - 由 [memory-self-evolution-design.md](./memory-self-evolution-design.md)
- `12`
  - 由 [harness-v2-governance-design.md](./harness-v2-governance-design.md)
- `13`
  - 由 [frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md)
  - 由 [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)
  - 由 [runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)
- `14-17`
  - 由 [remediation-program-and-phase-roadmap.md](./remediation-program-and-phase-roadmap.md)
  - 由 [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)
  - 由所有专题子设计共同支撑
- `18`
  - 由 [if2ai-staff-remediation-blueprint.md](./if2ai-staff-remediation-blueprint.md) 保持总纲总结

## 端到端执行映射

为避免只看到“设计文档覆盖”却看不到“执行工件落点”，以下给出 blueprint 到 exec artifact 的完整映射。

| 蓝图范围                                                                | 子设计文档                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | Phase YAML                                                                                                                                                              | Runbook                                                                                                                   | File-level plans                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `7-8` canonical truth / workflow truth / contracts / god-file inventory | [canonical-domain-model-and-workflow-truth-design.md](./canonical-domain-model-and-workflow-truth-design.md), [if2ai-canonical-domain-model.md](./if2ai-canonical-domain-model.md) (M0.1), [if2ai-workflow-truth.md](./if2ai-workflow-truth.md) (M0.2 + M0.4 boot truth), [m0-god-file-responsibility-inventory.md](./m0-god-file-responsibility-inventory.md) (M0.6), [src-tauri/src/modules/runtime/contracts/](../../src-tauri/src/modules/runtime/contracts/) (M0.3+M0.4+M0.5 Rust skeleton), [src/transport/contracts.ts](../../src/transport/contracts.ts) (TS twin), [runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md), [activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md), [request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md) | [phase-m0-canonical-contracts-and-truth.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-canonical-contracts-and-truth.yaml:1)                 | [phase-m0-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-executor-runbook.md:1) | [phase-m0-truth-documents-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-truth-documents-file-level-plan.md:1), [phase-m0-runtime-activation-execution-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-runtime-activation-execution-contracts-file-level-plan.md:1), [phase-m0-responsibility-inventory-and-linkage-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-responsibility-inventory-and-linkage-file-level-plan.md:1)                                                                                                                                              |
| `9-10` backend application/control plane/service extraction             | [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md), [if2ai-worker-adoption-design.md](./if2ai-worker-adoption-design.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [phase-m1-backend-service-extraction.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-backend-service-extraction.yaml:1)                       | [phase-m1-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-executor-runbook.md:1) | [phase-m1-initial-slices-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-initial-slices-file-level-plan.md:1), [phase-m1-memory-and-stream-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-memory-and-stream-file-level-plan.md:1), [phase-m1-routing-activation-control-plane-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md:1)                                                                                                                                                                                                  |
| `9 / 13` frontend runtime projection / IA / visible surfaces            | [runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md), [frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md), [activation-gate-and-license-lifecycle-design.md](./activation-gate-and-license-lifecycle-design.md), [request-intelligence-and-execution-mode-routing-design.md](./request-intelligence-and-execution-mode-routing-design.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [phase-m2-frontend-runtime-projection.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-frontend-runtime-projection.yaml:1)                     | [phase-m2-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-executor-runbook.md:1) | [phase-m2-runtime-projection-foundation-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-runtime-projection-foundation-file-level-plan.md:1), [phase-m2-state-boot-shell-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-state-boot-shell-file-level-plan.md:1), [phase-m2-chat-and-visible-surfaces-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m2-chat-and-visible-surfaces-file-level-plan.md:1)                                                                                                                                                                                    |
| `11` memory coordinator / write policy / recall / reflection            | [memory-self-evolution-design.md](./memory-self-evolution-design.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [phase-m3-memory-coordinator.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-memory-coordinator.yaml:1)                                       | [phase-m3-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-executor-runbook.md:1) | [phase-m3-object-model-and-coordinator-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-object-model-and-coordinator-file-level-plan.md:1), [phase-m3-policy-quality-and-recall-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-policy-quality-and-recall-file-level-plan.md:1), [phase-m3-frontend-reflection-and-tests-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-frontend-reflection-and-tests-file-level-plan.md:1)                                                                                                                                                            |
| `12` governance / harness / compare / gate                              | [harness-v2-governance-design.md](./harness-v2-governance-design.md), [backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [phase-m4-policy-harness-governance.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-policy-harness-governance.yaml:1)                         | [phase-m4-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-executor-runbook.md:1) | [phase-m4-execution-and-report-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-execution-and-report-contracts-file-level-plan.md:1), [phase-m4-graders-compare-and-corpus-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-graders-compare-and-corpus-file-level-plan.md:1), [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md:1), [governance-gate-rules.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/governance-gate-rules.md:1)                   |
| `14-17` self-evolution / strategy candidate / promotion / rollback      | [memory-self-evolution-design.md](./memory-self-evolution-design.md), [harness-v2-governance-design.md](./harness-v2-governance-design.md), [remediation-program-and-phase-roadmap.md](./remediation-program-and-phase-roadmap.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [phase-m5-self-evolution-and-strategy-promotion.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-self-evolution-and-strategy-promotion.yaml:1) | [phase-m5-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-executor-runbook.md:1) | [phase-m5-trajectory-failure-and-reflection-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-trajectory-failure-and-reflection-file-level-plan.md:1), [phase-m5-candidate-registry-and-offline-eval-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-candidate-registry-and-offline-eval-file-level-plan.md:1), [phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md:1), [governance-gate-rules.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/governance-gate-rules.md:1) |

## 适用场景

- Staff / Principal 级架构审查
- 重大技术债收敛
- Agent 平台能力升级规划
- exec-plan 编排前的总纲设计

## 使用建议

1. 先读蓝图文档，统一问题定义与优先级。
2. 再读 UClaw 对照迁移报告，明确哪些结构值得迁移、如何迁移、先迁什么。
3. 按覆盖矩阵阅读新增子设计文档，核对蓝图各章节都有落点。
4. 再把 program 文档下钻成 exec-plan。
5. 每完成一个阶段，回写本目录，保持“整改现状”与“规划目标”同步。
