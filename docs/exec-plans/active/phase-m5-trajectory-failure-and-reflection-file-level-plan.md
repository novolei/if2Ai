# Phase M5 Trajectory Failure And Reflection File-Level Plan

> 将 `M5` 的 `m5.1 + m5.2 + m5.3` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m5.1` Introduce trajectory scoring model
2. `m5.2` Introduce failure clustering pipeline
3. `m5.3` Generate structured reflection notes from runs

目标是先把 If2Ai 的“学习输入面”收口为可评分、可聚类、可生成 reflection notes 的结构化基础，而不是一上来就做 promotion。

## 2. 当前事实基线

### 2.1 trajectory 当前仍偏导出，不是评分系统

当前 [src-tauri/src/modules/learning/trajectory.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/trajectory.rs:1) 的核心定位仍是：

1. ShareGPT JSONL 导出
2. session -> trajectory 记录
3. 为未来训练留数据

它还不是 blueprint 需要的：

1. trajectory scoring
2. run outcome weighting
3. memory alignment signal
4. recovery quality signal

### 2.2 failure taxonomy 还没形成正式聚类管线

当前仓库还没有正式的 failure clustering pipeline 来稳定区分：

1. intent miss
2. policy issue
3. memory issue
4. recovery issue
5. tool misuse

也就是说，reflection 目前还缺一个可靠的中间归因层。

### 2.3 reflection 现状仍偏 session insight

当前 [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1) 的 `Reflection` 仍然更像：

1. pattern summary
2. insight text
3. confidence

这和 `M3` 的 `ReflectionNote` seam 并不等价，还不足以直接驱动 candidate strategy。

## 3. 实施原则

1. 先把 trajectory / failure / reflection 的输入输出结构写清，再谈 candidate registry。
2. trajectory score 不能只看 token/latency，必须体现 task completion / memory / recovery。
3. failure taxonomy 必须结构化，不依赖错误字符串关键词匹配作为唯一依据。
4. reflection 先产出 candidate hints，不直接改 active strategy。

## 4. 严格执行顺序

1. `T0` preflight evolution inventory
2. `T1` 建 trajectory score contract
3. `T2` 建 trajectory scorer
4. `T3` 建 failure taxonomy
5. `T4` 建 clustering pipeline
6. `T5` 回写 reflection inputs
7. `T6` 产出结构化 reflection notes
8. `T7` compile + manual verification

禁止并行：

1. trajectory scoring
2. failure clustering
3. reflection note 深改

因为这三项一起动时，最容易让“评分结果”“失败归因”“策略建议”三层互相污染。

## 5. 文件级实施方案

## 5.1 `T0` Preflight Evolution Inventory

### 必查文件

- [src-tauri/src/modules/learning/trajectory.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/trajectory.rs:1)
- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/learning/self_model.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/self_model.rs:1)
- [src-tauri/src/modules/learning/trust_tracker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/trust_tracker.rs:1)
- [src-tauri/src/modules/harness/run_report.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/run_report.rs:1)
- [src-tauri/src/modules/harness/compare_report.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/compare_report.rs:1)

### 必做动作

1. 盘出当前哪些 run outcome 字段已可作为 trajectory score 输入。
2. 标出 harness report 中哪些字段可用于 failure clustering。
3. 标出 reflection 当前还缺哪些结构化输入。

## 5.2 `T1` 建 trajectory score contract

### 新增文件

- `src-tauri/src/modules/learning/trajectory_score.rs`

### 修改文件

- [src-tauri/src/modules/learning/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/mod.rs:1)

### 第一版建议结构

1. `TrajectoryScore`
2. `TrajectoryScoreBreakdown`
3. `TrajectorySignal`

### 必须显式包含

1. task completion / success
2. recovery quality
3. memory alignment/usefulness
4. tool efficiency / resource cost

### 这一步不要做的事

1. 不要把 score 直接写回 self_model。
2. 不要把 promote/reject 语义提前塞进 score。

## 5.3 `T2` 建 trajectory scorer

### 新增文件

- `src-tauri/src/modules/learning/trajectory_scorer.rs`

### 修改文件

- [src-tauri/src/modules/learning/trajectory.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/trajectory.rs:1)

### 必须落实

1. scorer 能消费 run report / compare evidence / memory evidence
2. 输出可比较的 `TrajectoryScore`
3. 不再把 trajectory 仅视为导出物

### 第一阶段允许保留

1. `TrajectoryManager` 作为导出器继续存在

但新的演进主链应围绕 `trajectory_scorer.rs` 展开。

## 5.4 `T3` 建 failure taxonomy

### 新增文件

- `src-tauri/src/modules/learning/failure_taxonomy.rs`

### 修改文件

- [src-tauri/src/modules/learning/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/mod.rs:1)

### 第一版必须覆盖类别

1. `intent_miss`
2. `policy_issue`
3. `memory_issue`
4. `recovery_issue`
5. `tool_misuse`

### 必须落实

每一类都要有 typed evidence mapping，不允许只靠自然语言描述。

## 5.5 `T4` 建 clustering pipeline

### 新增文件

- `src-tauri/src/modules/learning/failure_clustering.rs`

### 修改文件

- `src-tauri/src/modules/learning/failure_taxonomy.rs`
- 视需要读取 harness report/compare structures

### 最低结构建议

1. `FailureCluster`
2. `ClusteredFailureSet`
3. `FailureSignature`

### 第一版必须落实

1. recurring failures 聚类
2. cluster 输出可供 reflection 直接消费
3. cluster 不只按错误文案聚类

## 5.6 `T5` 回写 reflection inputs

### 修改文件

- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/learning/reflection_note.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection_note.rs:1)

### 必须落实

reflection 输入开始接：

1. trajectory score
2. failure clusters
3. harness blocking failures
4. memory decisions

## 5.7 `T6` 产出结构化 reflection notes

### 修改文件

- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/learning/reflection_note.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection_note.rs:1)

### 必须落实

reflection 输出统一成 `ReflectionNote` 或等价结构，至少包括：

1. `issue_type`
2. `evidence`
3. `proposed_strategy`
4. `expected_gain`
5. `risk_level`

### 关键约束

这一步仍然只产出 notes，不直接注册为 active strategy。

## 5.8 `T7` Compile + Manual Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`

### 必做人工检查

1. trajectory 已不再只是导出 JSONL。
2. failure clusters 可被 reflection 消费。
3. reflection note 已能给出结构化 candidate hint。

## 6. 完成定义

只有当 reviewer 可以明确回答“系统现在如何给一次 run 打分、如何把失败归类、如何把这些输入转成结构化反思”时，这一段才算完成。
