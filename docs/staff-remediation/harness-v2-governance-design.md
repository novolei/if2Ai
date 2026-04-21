# Harness V2 治理设计

> 将 If2Ai harness 从“观测与录制工具”升级为“评测、回归、发布与策略治理系统”。
>
> 最后更新: 2026-04-19
> 状态: Proposed

## 1. 设计目标

Harness V2 的目标不是增加更多 trace 字段，而是回答一个更关键的问题：

> 哪些 agent 行为变化值得被默认启用，哪些必须被拦下？

因此 Harness V2 必须承担四类责任：

1. 可重放
2. 可评分
3. 可比较
4. 可阻断

## 2. 当前问题

当前 harness 已具备：

- `AgentEvent` event bus
- `SessionRecorder` JSONL 录制
- per-session telemetry
- Tauri IPC 查询能力

但还不具备：

- 可声明的任务集
- 可执行的评分器体系
- 可比较的策略版本运行结果
- 与 phase / release 绑定的 blocker 规则

结果是：

- 有 trace，但没有裁决
- 有埋点，但没有治理
- 有记录，但没有 release gate

## 3. 目标定位

Harness V2 应被视为 If2Ai 的 Experience Plane：

```text
Agent Runtime
  -> Trace Capture
  -> Outcome Snapshot
  -> Graders
  -> Evaluation Report
  -> Regression Corpus
  -> Policy/Release Decision
```

它既服务开发，也服务智能体自我进化。

## 4. 核心能力模型

### 4.1 Trace

记录事实过程：

- 用户输入
- tool 调用
- permission 决策
- compaction / resume / retry
- memory 注入、召回、拒绝与写入
- 最终输出与失败原因

要求：

- 全程结构化
- 不依赖自然语言 message 解析
- 有稳定 id 关联 task / run / session / trace

### 4.2 Outcome Snapshot

记录执行完成后的结果状态：

- workspace 状态
- session 状态
- memory 状态
- control plane 决策状态

这层用于回答：

- 任务是否真的推进了
- 是否产生了错误 side effect
- 是否把不该写的东西写进了系统

### 4.3 Graders

Grader 不是“看起来像对”，而是明确判定：

- 成功
- 失败
- 阻断失败
- 降级通过

推荐 grader 家族：

1. `TaskSuccessGrader`
2. `PermissionComplianceGrader`
3. `NoDeadLoopGrader`
4. `EvidenceFidelityGrader`
5. `MemoryAlignmentGrader`
6. `RecoveryResilienceGrader`
7. `ResourceEfficiencyGrader`

### 4.4 Reports

运行报告必须同时满足：

- 人类可读
- 机器可解析
- 可做版本对比

最小字段建议：

- task id
- policy version
- model/provider
- success / blocking failures
- token / latency / tool counts
- memory recall stats
- retry / resume stats

### 4.5 Regression Corpus

将历史失败、重要任务、上线前基准案例固化为回归语料。

语料分层：

1. `smoke`
2. `critical path`
3. `memory-sensitive`
4. `tool-risk`
5. `resume-recovery`
6. `ux-quality`

## 5. 治理模型

## 5.1 决策对象

Harness V2 评估的对象不只可以是代码版本，也应包括：

- prompt policy version
- memory write policy
- memory retrieval strategy
- tool routing policy
- retry / timeout strategy
- browser strategy

## 5.2 运行模式

### Shadow

- 记录 candidate 策略结果
- 不影响默认用户路径

### Compare

- baseline 与 candidate 跑同一语料
- 输出差异与 blocker

### Gate

- 若 blocker 未通过，则禁止升级

### Archive

- 归档可复盘结果，用于后续 clustering 与学习

## 5.3 Blocker 规则

建议定义以下阻断条件：

1. `task_success_rate` 低于 baseline 超过阈值
2. `blocking_failure_count` 上升
3. `permission_violation` 非零
4. `resume_success_rate` 明显下降
5. `memory_leak_or_scope_violation` 非零
6. `hallucinated_evidence_claim` 非零
7. 成本增加过大但收益不足

## 6. 与自我进化的关系

Harness V2 是自我进化系统的裁判，而不是参与者。

闭环如下：

1. Runtime 产生 trace
2. Learning 生成 candidate strategy
3. Harness 回放 candidate
4. 若通过 gate，再允许灰度启用
5. 若失败，则归档为 regression case

这保证了“会学习”不会变成“会随机漂移”。

## 7. 与前端的关系

UI 需要两个面：

### 开发者面

- TelemetryDrawer
- memory lifecycle
- recorder status
- last run report

### 治理面

- regression dashboard
- candidate vs baseline diff
- failure taxonomy summary
- release readiness

原则：

- 开发者观测面和发布治理面不能混为一个 drawer
- 面向最终用户的主聊天界面不直接暴露低层 harness 细节

## 8. 数据与契约建议

## 8.1 Run Identity

每次 harness run 应有：

- `run_id`
- `task_id`
- `suite_id`
- `policy_version`
- `trace_id`
- `session_id`

## 8.2 结果结构

> 2026-04-21 更新：原稿把 `Recommendation` 嵌进 `HarnessRunReport`，
> 实际落地时 `Recommendation` 改为 m4.8 gate 的输出，与 run/compare/suite 报告平行。
> 下面按代码现实重写。

实际落地的契约层级（M4 reconciliation 后的官方口径）：

```text
HarnessRunReport         (run-level，src-tauri/src/modules/harness/run_report.rs)
  -> TaskRunResult
  -> AggregateMetrics
  -> BlockingFailure[]
  -> EvidenceBundle (memory_after_turn / prepare_step / execution_mode /
                     permission_prompts / permission_resolved /
                     stream_errors / resume_invocations)

BaselineVsCandidate      (compare 输出，src-tauri/src/modules/harness/compare.rs)
  -> AggregateDiff
  -> BlockerDiff
  -> GraderVerdictDiff
  -> EvidenceSummaryDiff
  -> ReportVersionCompatibility

RegressionCorpus         (语料输入，src-tauri/src/modules/harness/corpus.rs)
  -> CorpusTask[]
  -> CorpusTier (smoke / critical_path / memory_sensitive /
                 tool_risk / resume_recovery)

SuiteReport              (corpus/suite 聚合输出，src-tauri/src/modules/harness/suite_report.rs)
  -> TaskRunOutcome[]
  -> TierSummary[]
  -> SuiteGrade
  -> TaskClassification

Recommendation           (gate 决策输出，src-tauri/src/modules/harness/gate.rs)
  -> GateDecision (Promote / Hold / Reject)
  -> 稳定 reason codes
  -> 由 GatePolicy 在消费 BaselineVsCandidate 或 SuiteReport 后渲染
```

`Recommendation` 取值（gate 决策）：

- `promote`
- `hold`
- `reject`

强制默认：缺少 `Recommendation` / `BaselineVsCandidate` / `SuiteReport` 时，gate 一律不返回 `Promote`。

## 8.3 Failure Taxonomy

至少区分：

- intent_miss
- evidence_miss
- policy_violation
- memory_scope_leak
- tool_failure
- dead_loop
- recovery_failure
- cost_regression

## 9. 实施建议

### Sprint 1

- 修正 harness 启用模型
- 定义 core contracts
- 建立最小 run report

### Sprint 2

- 加入 base graders
- 建立 regression corpus
- 实现 baseline vs candidate compare

### Sprint 3

- 引入 release blockers
- 接 memory / learning policy compare
- 输出可用于 exec-plan 的 structured status

## 10. 验收标准

Harness V2 达标时，项目应满足：

1. 至少一套 smoke suite 可稳定运行
2. 至少一套 memory-sensitive suite 可比较 baseline/candidate
3. release blocker 可自动给出 pass/fail
4. regression case 能复用历史失败
5. candidate strategy 未经 gate 不可默认启用

## 11. 与当前仓库的衔接建议

落地点建议优先围绕：

- `src-tauri/src/modules/harness/*`
- `src-tauri/src/commands/harness.rs`
- `src/components/chat/TelemetryDrawer.tsx`
- `harness/runner.py`
- `docs/exec-plans/active/phase-9a-trustworthy-foundation.yaml`

## 12. 结论

Harness V2 的价值不在“看到更多日志”，而在“更少把错误策略带给用户”。

If2Ai 若要真正走向记忆增强与自我进化，Harness 必须先成为一个可靠的治理系统。
