# 执行计划索引 (Exec Plans Index)

本目录包含 If2Ai 的执行计划。执行计划是具体工程工作的路线图，包含目标、步骤、进度追踪和决策日志。

> **Executor 读取规则**：找到下方标记为 `[当前]` 的条目，使用其 `文件` 字段路径加载当前 Phase YAML。  
> **人类操作规则**：Phase N 完成后，将 `[当前]` 移到 Phase N+1，同时把 Phase N+1 YAML 中的 `phase_status: draft` 改为 `active`。

## 📋 全部计划总览

| 状态           | Phase        | 标题                                                             | 文件                                                             | Backlog                                                                                  |
| -------------- | ------------ | ---------------------------------------------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| [已完成]       | Phase 5A     | Agent 核心修复                                                   | `docs/exec-plans/active/phase-5a-agent-core-fix.yaml`            | F1, N5, N12                                                                              |
| [已完成]       | Phase 5B     | 流式工具循环重写                                                 | `docs/exec-plans/active/phase-5b-streaming-tool-loop.yaml`       | F2, F8, F15, UI-5                                                                        |
| [已完成]       | Phase 5C     | 安全与系统连接                                                   | `docs/exec-plans/active/phase-5c-safety-and-system.yaml`         | N1, N3(阶段1+2), F3-F6, F14, F17, N4, UI-1                                               |
| [已完成]       | Phase 5D     | UX 完善                                                          | `docs/exec-plans/active/phase-5d-ux-completion.yaml`             | N10, N7, F7, F9-F11, UI-8(前端), N11                                                     |
| ~~**[当前]**~~ | ~~Phase 5E~~ | ~~长期增强 + 全局迁移~~                                          | ~~`docs/exec-plans/active/phase-5e-long-tail-enhancement.yaml`~~ | ~~F12, F13, F16, UI-4, UI-6, UI-7, N6, N8-N9, N3(阶段3:18工具), F25~~                    |
| [已完成]       | Phase 5E     | 长期增强 + 全局迁移                                              | `docs/exec-plans/active/phase-5e-long-tail-enhancement.yaml`     | F12, F13, F16, UI-4, UI-6, UI-7, N6, N8-N9, N3(阶段3:18工具), F25                        |
| [已完成]       | Phase 6A     | 控制平面加固（Post-CLI）                                         | `docs/exec-plans/active/phase-6a-control-plane-hardening.yaml`   | POSTCLI-CP1~CP3, POSTCLI-KERNEL1, POSTCLI-OBS1, POSTCLI-UX1, POSTCLI-TEST1, POSTCLI-OPS1 |
| [已完成]       | Phase 6B     | 记忆控制平面 — SQLite P0 + 向量搜索 + HRR + 自学习               | `docs/exec-plans/active/phase-6b-memory-control-plane.yaml`      | ADR-001~ADR-010 (SQLite P0, Token Budget, Vector Search, HRR, Self-Learning, Trajectory) |
| 草案           | Phase 6BW    | Memory Wiring — 孤岛模块接入 + 前端 UI + 测试修复                   | `docs/exec-plans/active/phase-6bw-memory-wiring.yaml`              | ADR-012 (AppState, ContextBudget, ActiveRetrieval, Trajectory, WorkingMemory, LearningModule, Provider 升级, Memory UI) |
| [已完成]       | Phase 6C     | 流可靠性控制平面 — TaskOutcome + Correlation + Governor + Resume | `docs/exec-plans/active/phase-6c-llm-stream-reliability.yaml`    | STREAM-REL-A1, C1, UX1, B1, D1, E1                                                       |
| [已完成]       | Phase 6D     | Skills 控制平面 — Bundled/Review/Distribution/Authoring          | `docs/exec-plans/active/phase-6d-skills-control-plane.yaml`      | SKILL-CP-P0-CORE~SKILL-CP-P3-AGENT-AUTHORING                                             |
| 草案           | Phase 6E     | Agent Loop Harness — 运行时观测与控制框架                          | `docs/exec-plans/active/phase-6e-agent-loop-harness.yaml`         | TASK-011-01~TASK-011-07 (EventBus, Telemetry, AgentLoop Integration, Recorder, IPC, harness-cli, Tests) |
| 草案           | Phase 6F     | Skill Control Plane v2 — Hermes Alignment                          | `docs/exec-plans/active/phase-6f-skill-control-plane-v2.yaml`     | 6F.1~6F.10 (SkillsGuard, SkillManager, Hub Sources, Hub State, Sync, Commands, Config, UI, Tests) |
| 草案           | Phase 6G     | Onboarding & Configuration Platform                                | `docs/exec-plans/active/phase-6g-onboarding.yaml`                 | TASK-014-01~TASK-014-18 (18 tasks, 17 slices) |
| **统一执行序列** | —           | Phase 6B + 6E + 6F 完整执行计划                                    | `docs/exec-plans/active/phase-6b+6e-unified-execution-sequence.md` | 26 个 slice，最优并行顺序                                                |

> **Phase 5 系列说明**：Phase 5 拆分为 5 个子 Phase（5A-5E），覆盖从 BS_Gap 审计报告中发现的全部修复项（F1-F17, F25, UI-1~UI-8, N1, N3-N12）。  
> Phase 5A 已标记为 active，可立即由 executor 执行。Phase 5B-5E 为 draft 状态，依赖前一个 Phase 完成后激活。  
> 激活方式：将对应 YAML 文件中的 `phase_status: draft` 改为 `active`，并将本表中状态改为 `[当前]`。

> **Phase 5 系列说明**：Phase 5 拆分为 5 个子 Phase（5A-5E），覆盖从 BS_Gap 审计报告中发现的全部修复项（F1-F25、UI-1~UI-8、N1-N12）。  
> Phase 5A 已标记为 active，可立即由 executor 执行。Phase 5B-5E 为 draft 状态，依赖前一个 Phase 完成后激活。  
> 激活方式：将对应 YAML 文件中的 `phase_status: draft` 改为 `active`，并将本表中状态改为 `[当前]`。

## ✅ 已完成计划 (Completed Plans)

查看 [completed/](./completed/) 目录了解已完成的工作。

### 已完成

- **Project-Initialization** - 项目初始化
  - 完成时间：2026-04-11
  - 成果：项目目录结构、配置文件、文档框架

## 📝 执行计划模板

每个执行计划应包含以下部分：

```markdown
# 计划名称 (Plan Name)

## 概述 (Overview)

- 计划目标：...
- 预计时长：...
- 所有者：...
- 关键结果：...

## 背景 (Context)

为什么现在需要这个计划？前提条件是什么？

## 工作项 (Work Items)

### 阶段 1: 初期工作

- [ ] Task 1
  - 所有者：@person
  - 预计：1 天
  - 检查点：...

### 阶段 2: 主体工作

- [ ] Task 2
- [ ] Task 3

### 阶段 3: 验证和完成

- [ ] 验证工作
- [ ] 文档更新
- [ ] 性能测试

## 进度追踪 (Progress)

| 日期       | 进度 | 关键事件 | 更新者 |
| ---------- | ---- | -------- | ------ |
| 2026-04-11 | 0%   | 计划创建 | @owner |

## 决策日志 (Decision Log)

### 决策 1: [标题]

- **背景**: 我们面临的问题
- **选项**: A, B, C
- **决定**: 选择 A
- **原因**: 理由
- **日期**: 2026-04-11
- **所有者**: @person

## 风险和缓解 (Risks)

| 风险   | 可能性 | 影响 | 缓解策略 |
| ------ | ------ | ---- | -------- |
| Risk 1 | High   | High | Strategy |

## 关键检查点 (Milestones)

- ✅ Checkpoint 1 (2026-04-15)
- ⏳ Checkpoint 2 (2026-04-20)
- ⏳ Final Completion (2026-04-25)

## 相关文档

- [相关设计文档](../../design-docs/)
- [相关规范](../../product-specs/)
```

## 🚀 新计划创建指南

1. 创建文件 `[NAME]-of-plan.md` 在 `active/` 目录
2. 填充上述模板
3. 从 [AGENTS.md](../../AGENTS.md) 链接到新计划
4. 在此索引中添加条目
5. 计划完成后，将文件移到 `completed/` 目录

## ⏱️ 计划生命周期

```
创建 (Create)
  ↓
进行中 (Active)
  ├─ 每周更新进度
  ├─ 记录决策
  └─ 调整风险评估
  ↓
完成 (Completed)
  ├─ 最终进度报告
  ├─ 后期总结
  └─ 移到 completed/ 目录
```

## 📊 计划监控

关键指标：

- 按时完成率 > 90%
- 工作项估算准确度 > 80%
- 无重大意外风险

系统会自动扫描计划文件，检查：

- ✅ 最后更新时间 < 1 周
- ✅ 进度字段有值
- ✅ 决策日志有条目
- ✅ 所有工作项有所有者和预计

---

**版本**: 0.1.0 | **最后更新**: 2026-04-12
