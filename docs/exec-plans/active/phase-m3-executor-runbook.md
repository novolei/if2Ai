# Phase M3 Executor Runbook

> 将 `phase-m3-memory-coordinator.yaml` 细化为 executor 可直接执行的 memory 平台化手册。
>
> 最后更新: 2026-04-20

## 1. 目的

`M3` 的任务是把 If2Ai 的 memory 从分散能力收口成：

`object model -> MemoryCoordinator -> WritePolicy -> QualityGate -> RecallAssembler -> Reflection seam`

## 2. 本阶段必须产出的结果

1. memory object model 正式进入代码。
2. MemoryCoordinator 出现，形成 `prepare_context / after_turn` 主叙事。
3. write / recall / quality / reflection 都有 typed decision。
4. 前后端都能解释“为什么写 / 为什么不写 / 为什么召回”。

## 3. 读取顺序

### 3.1 执行者最小必读

执行者开工 `M3` 时，只要求先读下面 5 个入口：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
2. [phase-m3-memory-coordinator.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-memory-coordinator.yaml:1)
3. 本 runbook
4. 当前 slice 对应的 file-level plan
5. 当前 slice 明确引用的真实代码入口文件

### 3.2 `M3` file-level plan 入口

1. [phase-m3-object-model-and-coordinator-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-object-model-and-coordinator-file-level-plan.md:1)
2. [phase-m3-policy-quality-and-recall-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-policy-quality-and-recall-file-level-plan.md:1)
3. [phase-m3-frontend-reflection-and-tests-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-frontend-reflection-and-tests-file-level-plan.md:1)

### 3.3 设计依据按需查阅

只有当当前 slice 需要核对 memory 语义或投影口径时，再回查以下设计文档：

1. [memory-self-evolution-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/memory-self-evolution-design.md:1)
2. [runtime-contracts-and-event-projection-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/runtime-contracts-and-event-projection-design.md:1)

## 4. 落地决策

1. 优先复用现有 `src-tauri/src/modules/memory/` 与 `src-tauri/src/modules/learning/`
2. 不再把 memory 视为同质文本块
3. `M3` 不负责 promote candidate strategy，那是 `M5`

## 5. 严格执行顺序

1. `m3.0` preflight memory inventory
2. `m3.1` object model
3. `m3.2` MemoryCoordinator
4. `m3.3` write policy
5. `m3.4` quality gate
6. `m3.5` recall assembler
7. `m3.6` frontend projection
8. `m3.7` reflection seam
9. `m3.8` regression tests

## 6. Slice 详细执行说明

## 6.1 `m3.0` Preflight Memory Inventory

### 必查文件

- [src-tauri/src/modules/memory/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)
- [src-tauri/src/modules/learning/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/mod.rs:1)

### 必做动作

1. 盘出 working / pinned / compiled / summary / ticker / reflection 当前入口。
2. 盘出哪些地方仍把 memory 当字符串拼装。

## 6.2 `m3.1` Object Model

### 必须落实

1. 形成 `Fact / Preference / Strategy / Episode` 或可证明等价映射。
2. 每类对象包含 scope / stability / evidence 约束。

## 6.3 `m3.2` MemoryCoordinator

### 必须落实

1. 建立 `prepare_context / after_turn`
2. scattered recall / injection / write 接缝统一收口

### checkpoint

`m3.2` 后必须确认这不是新的 God-file。

## 6.4 `m3.3` Write Policy

### 必须落实

1. `allow / deny / prompt`
2. 附带 `reason_code / memory_type / scope / evidence_id`

## 6.5 `m3.4` Quality Gate

### 必须落实

1. 去重
2. 冲突处理
3. scope 漏写拦截
4. evidence 缺失拦截

## 6.6 `m3.5` Recall Assembler

### 必须落实

注入顺序固定为：

1. rules
2. pinned
3. critical facts
4. preferences
5. compiled
6. episodes

并产出 recall usefulness 基础埋点。

## 6.7 `m3.6` Frontend Projection

### 必须落实

1. `memory-store` 接入 write decision / recall evidence
2. 区分 candidate / approved / recalled / pinned / compiled

## 6.8 `m3.7` Reflection Seam

### 必须落实

Reflection 输出至少包含：

1. `issue_type`
2. `evidence`
3. `proposed_strategy`
4. `expected_gain`
5. `risk_level`

### 风险提示

reflection 不得直接 promote。

## 6.9 `m3.8` Regression Tests

### 必须落实

1. object model tests
2. write policy tests
3. recall order tests
4. dirty/ambiguous candidate tests

## 7. 交付物检查清单

- [ ] memory object model 已进入代码
- [ ] MemoryCoordinator 已出现
- [ ] WritePolicy 已出现
- [ ] QualityGate 已出现
- [ ] RecallAssembler 已出现
- [ ] frontend memory projection 已接入
- [ ] reflection seam 已出现
- [ ] memory regression tests 已出现
- [ ] 已按 3 份文件级任务单完成 `m3.1 ~ m3.8`

## 8. 推荐提交顺序

1. `m3.1 + m3.2`
2. `m3.3 + m3.4 + m3.5`
3. `m3.6 + m3.7 + m3.8`

对应文件级任务单：

1. [phase-m3-object-model-and-coordinator-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-object-model-and-coordinator-file-level-plan.md:1)
2. [phase-m3-policy-quality-and-recall-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-policy-quality-and-recall-file-level-plan.md:1)
3. [phase-m3-frontend-reflection-and-tests-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m3-frontend-reflection-and-tests-file-level-plan.md:1)
## 9. 完成标志

只有当 reviewer 能看懂“memory 为什么发生这个决定”，而不是只能看到一个最终文本结果时，`M3` 才算完成。
