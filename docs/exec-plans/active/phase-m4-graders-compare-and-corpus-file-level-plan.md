# Phase M4 Graders Compare And Corpus File-Level Plan

> 将 `M4` 的 `m4.3 + m4.4 + m4.5 + m4.7` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m4.3` Introduce grader skeletons and blocker rules
2. `m4.4` Wire classifier and memory evidence into harness traces
3. `m4.5` Add baseline-candidate compare flow
4. `m4.7` Add governance regression suites

目标是把 harness 从“有 report contract”推进到“能评分、能比较、能回归”。

## 2. 当前事实基线

### 2.1 现有 harness 事件还不足以直接治理

当前 [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1) 的 `AgentEvent` 已覆盖：

1. turn started/finished
2. llm requested/responded
3. tool called/result
4. permission prompted
5. reflection completed

但还没有系统化记录：

1. execution mode / reason codes
2. memory write / recall decisions
3. control-plane preflight decision artifacts

### 2.2 现有回归语料还不是治理语料

当前 `harness/suites/` 已有多份 phase/integration YAML，但它们更像阶段性测试资产，还不是分层治理语料库：

1. `smoke`
2. `critical path`
3. `memory-sensitive`
4. `tool-risk`
5. `resume-recovery`

### 2.3 当前还没有正式 grader family

虽然设计文档明确了 grader 家族，但仓库里还没有正式 skeleton 可以挂报告和 blocker 规则。

## 3. 实施原则

1. 先打通 evidence trace，再做 grader。
2. 先定义 blocker 规则，再跑 compare。
3. compare 必须基于同一语料，不允许“随便挑一个例子”。
4. regression corpus 必须分层，不再只按 phase 名称堆 YAML。

## 4. 严格执行顺序

1. `G0` preflight inventory
2. `G1` trace evidence 扩展
3. `G2` grader skeletons
4. `G3` blocker/recommendation rules
5. `G4` compare flow
6. `G5` corpus layering
7. `G6` runner/gate integration
8. `G7` manual verification

禁止并行：

1. AgentEvent 扩展
2. grader family 引入
3. corpus 分层重组

因为这三项一起动时，最容易让报告 shape、评分逻辑和语料组织全部互相卡死。

## 5. 文件级实施方案

## 5.1 `G0` Preflight Inventory

### 必查文件

- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
- [src-tauri/src/modules/harness/agent_loop_integration.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/agent_loop_integration.rs:1)
- [src-tauri/src/modules/harness/session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs:1)
- [src-tauri/src/modules/harness/telemetry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/telemetry.rs:1)
- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- `harness/suites/*.yaml`

### 必做动作

1. 列出当前 `AgentEvent` 无法覆盖的治理证据项。
2. 列出当前 suite 目录中哪些可以归类为 smoke/critical/memory/tool-risk/resume。
3. 列出 runner/gate 当前对结果 shape 的依赖。

## 5.2 `G1` trace evidence 扩展

### 新增文件

- `src-tauri/src/modules/harness/trace_evidence.rs`

### 修改文件

- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
- [src-tauri/src/modules/harness/agent_loop_integration.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/agent_loop_integration.rs:1)
- [src-tauri/src/modules/harness/session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs:1)

### 第一批必须补进的证据

1. execution mode / reason codes / route hint
2. memory write decisions
3. memory recall decisions
4. prepare_step_execution outputs
5. policy version / candidate tag

### 关键要求

这些证据必须结构化进入 trace，不允许依赖后期自然语言解析补救。

## 5.3 `G2` grader skeletons

### 新增文件

- `src-tauri/src/modules/harness/graders/mod.rs`
- `src-tauri/src/modules/harness/graders/task_success.rs`
- `src-tauri/src/modules/harness/graders/permission_compliance.rs`
- `src-tauri/src/modules/harness/graders/memory_alignment.rs`
- `src-tauri/src/modules/harness/graders/recovery_resilience.rs`
- `src-tauri/src/modules/harness/graders/resource_efficiency.rs`

### 修改文件

- [src-tauri/src/modules/harness/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs:1)

### 第一版要求

1. 先有统一 grader trait / result shape
2. 每个 grader 至少能返回 pass/fail/warn/blocker 候选信息
3. 不要求这一步把判定逻辑做到很深，但必须可挂到 run report

## 5.4 `G3` blocker/recommendation rules

### 新增文件

- `src-tauri/src/modules/harness/blockers.rs`
- `src-tauri/src/modules/harness/recommendation.rs`

### 修改文件

- `src-tauri/src/modules/harness/run_report.rs`

### 第一版必须写死的规则

1. `task_success_rate` 相比 baseline 下滑阈值
2. `permission_violation` 非零
3. `memory_leak_or_scope_violation` 非零
4. `resume_success_rate` 明显下降
5. `blocking_failure_count` 上升

### 约束

必须是明确阈值或明确条件，不允许只写“视情况决定”。

## 5.5 `G4` compare flow

### 新增文件

- `src-tauri/src/modules/harness/compare_runner.rs`
- `harness/evaluators/compare.py`

### 修改文件

- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必须落实

1. baseline 与 candidate 跑同一任务集
2. 输出 diff summary
3. 输出 blocker summary
4. 输出 recommendation

### 这一步不要做的事

1. 不要只比较单 run。
2. 不要只比较 token/latency 而忽略 success/blocker。

## 5.6 `G5` corpus layering

### 新增目录或文件

- `harness/corpus/smoke/`
- `harness/corpus/critical_path/`
- `harness/corpus/memory_sensitive/`
- `harness/corpus/tool_risk/`
- `harness/corpus/resume_recovery/`
- `harness/corpus/README.md`

### 修改文件

- 视情况调整 `harness/suites/*.yaml`

### 第一版必须落实

1. 语料按治理维度分层，不再只按 phase。
2. 每层至少列出样例任务清单。
3. 至少包含 activation / execution-mode / memory-sensitive 场景。

## 5.7 `G6` runner/gate integration

### 修改文件

- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必须落实

1. runner 能消费 grader/blocker/compare 新结果
2. gate 能消费 recommendation
3. `promote / hold / reject` 的结果 shape 稳定

## 5.8 `G7` Manual Verification

### 建议执行

- `python3 -m harness.runner --help`

### 必做人工检查

1. trace 中已能看到 execution-mode / memory / preflight 证据
2. compare 不是单样本比较
3. corpus 已有清晰分层入口

## 6. 完成定义

只有当 reviewer 可以明确说出“现在 harness 已经能依据结构化证据评分、比较并给出 blocker/recommendation”时，这一段才算完成。
