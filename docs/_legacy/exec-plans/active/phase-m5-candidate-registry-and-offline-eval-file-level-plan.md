# Phase M5 Candidate Registry And Offline Eval File-Level Plan

> 将 `M5` 的 `m5.4 + m5.5` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m5.4` Introduce candidate strategy registry
2. `m5.5` Run offline eval for candidate strategies

目标是把 `ReflectionNote -> candidate strategy -> offline compare` 这条主线做成正式、可审计、可回放的执行链。

## 2. 当前事实基线

### 2.1 当前 learned pattern 不等于 candidate registry

当前 [src-tauri/src/modules/learning/self_model.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/self_model.rs:1) 的 `LearnedPattern` 更像自我理解或经验记录，不是 blueprint 里的：

1. `draft`
2. `candidate`
3. `active`
4. `rollback`

这意味着 current self-model 不能直接充当 strategy registry。

### 2.2 当前 harness gate 已能表达 gate，但还不是 candidate eval pipeline

当前 [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1) 主要是 compile/test/behavior gate，尚未形成：

1. candidate strategy compare
2. promote / hold / reject recommendation
3. rollout state mutation

### 2.3 当前还没有策略版本身份

要做 offline eval，至少需要明确：

1. strategy id
2. source
3. candidate version
4. compare target baseline

这些当前都还没有正式 registry 结构。

## 3. 实施原则

1. registry 与 self-model 分离，不能混成一个结构。
2. offline eval 必须复用 `M4` compare/gate 能力，而不是重写第二套评估器。
3. candidate strategy 必须有 clear provenance。
4. 这一轮不允许“生成了建议就默认启用”。

## 4. 严格执行顺序

1. `C0` preflight candidate inventory
2. `C1` 建 strategy registry contract
3. `C2` 建 registry storage/service
4. `C3` reflection -> candidate registration
5. `C4` offline eval runner
6. `C5` recommendation/result persistence
7. `C6` manual verification

禁止并行：

1. candidate registry
2. offline eval runner
3. promotion semantics

因为这三项一起动时，最容易让 registry shape、评估路径和升级路径互相打架。

## 5. 文件级实施方案

## 5.1 `C0` Preflight Candidate Inventory

### 必查文件

- [src-tauri/src/modules/learning/self_model.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/self_model.rs:1)
- [src-tauri/src/modules/learning/reflection_note.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection_note.rs:1)
- [src-tauri/src/modules/harness/run_report.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/run_report.rs:1)
- [src-tauri/src/modules/harness/compare_report.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/compare_report.rs:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必做动作

1. 盘出当前可作为 strategy 来源的对象。
2. 标出 compare/gate 当前缺少哪些 strategy identity 字段。
3. 区分 learned pattern、自我认知与 candidate strategy 的边界。

## 5.2 `C1` 建 strategy registry contract

### 新增文件

- `src-tauri/src/modules/learning/strategy_registry.rs`

### 修改文件

- [src-tauri/src/modules/learning/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/mod.rs:1)

### 第一版建议结构

1. `StrategyRecord`
2. `StrategySource`
3. `RolloutState`
4. `StrategyScope`
5. `StrategyEvaluationRef`

### 必须显式包含

1. `strategy_id`
2. `source: reflection / human_feedback / curated_rule`
3. `rollout_state: draft / candidate / active / rollback`
4. `created_at / updated_at`
5. `based_on_reflection`
6. `last_recommendation`

## 5.3 `C2` 建 registry storage/service

### 新增文件

- `src-tauri/src/modules/learning/strategy_registry_store.rs`
- `src-tauri/src/modules/learning/strategy_registry_service.rs`

### 修改文件

- `src-tauri/src/modules/learning/strategy_registry.rs`

### 第一版必须落实

1. registry 可读写
2. rollout state 可变更
3. evaluation refs 可追踪

### 这一步不要做的事

1. 不要把 registry 挂到 self_model 内部。
2. 不要把 promote/reject 判定直接写进 store。

## 5.4 `C3` reflection -> candidate registration

### 修改文件

- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/learning/reflection_note.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection_note.rs:1)
- `src-tauri/src/modules/learning/strategy_registry_service.rs`

### 必须落实

1. 结构化 `ReflectionNote` 可被转成 `draft/candidate` strategy
2. 注册时必须记录来源和证据
3. reflection 不能越过 registry 直接改 active state

## 5.5 `C4` offline eval runner

### 新增文件

- `src-tauri/src/modules/learning/offline_eval.rs`
- `harness/evaluators/strategy_candidate_compare.py`

### 修改文件

- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)

### 必须落实

1. candidate strategy 必须走 harness compare
2. compare 输入包含 baseline 与 candidate strategy identity
3. 输出 `promote / hold / reject`

### 约束

offline eval 必须复用 `M4` 的 compare/gate 契约，不能再造一套自我进化专属结果形状。

## 5.6 `C5` recommendation/result persistence

### 修改文件

- `src-tauri/src/modules/learning/strategy_registry_service.rs`
- `src-tauri/src/modules/learning/offline_eval.rs`
- `src-tauri/src/modules/harness/compare_report.rs`

### 必须落实

1. compare recommendation 持久回写到 registry
2. registry 能看到最近一次 compare 结果
3. baseline/candidate 历史可追踪

## 5.7 `C6` Manual Verification

### 建议执行

- `python3 -m harness.runner --help`

### 必做人工检查

1. 至少一条 reflection note 能落成 candidate strategy。
2. candidate strategy 能完成一次 offline compare。
3. registry 能清楚区分 draft/candidate/active/rollback。

## 6. 完成定义

只有当 reviewer 可以明确说出“系统现在如何从 reflection 生成 candidate strategy，并如何通过 compare 得到 recommendation”时，这一段才算完成。
