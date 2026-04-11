# 执行计划索引 (Exec Plans Index)

本目录包含 If2Ai 的执行计划。执行计划是具体工程工作的路线图，包含目标、步骤、进度追踪和决策日志。

> **Executor 读取规则**：找到下方标记为 `[当前]` 的条目，使用其 `文件` 字段路径加载当前 Phase YAML。  
> **人类操作规则**：Phase N 完成后，将 `[当前]` 移到 Phase N+1，同时把 Phase N+1 YAML 中的 `phase_status: draft` 改为 `active`。

## 📋 全部计划总览

| 状态       | Phase   | 标题         | 文件                                                    | Backlog    |
| ---------- | ------- | ------------ | ------------------------------------------------------- | ---------- |
| **[当前]** | Phase 1 | 核心框架搭建 | `docs/exec-plans/active/phase-1-foundation.yaml`        | BL-101~108 |
| [草稿]     | Phase 2 | 高级特性     | `docs/exec-plans/active/phase-2-advanced-features.yaml` | BL-201~205 |
| [草稿]     | Phase 3 | 扩展生态     | `docs/exec-plans/active/phase-3-ecosystem.yaml`         | BL-301~303 |

> **草稿说明**：Phase 2 和 Phase 3 已预生成供规划参考，在 Phase 1 的 `human_checkpoint` 通过前不应执行。  
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

**版本**: 0.1.0 | **最后更新**: 2026-04-11
