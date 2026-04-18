# If2Ai `modules/harness/archive` 布局与查询设计

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**模块路径**: `src-tauri/src/modules/harness/archive/`  
**一句话定位**: 定义 If2Ai harness 的运行时经验档案、frontier 索引、candidate artifact 与查询接口，使 archive 既能服务产品运行期评测，也能服务 future proposer / optimizer。

---

## 1. 设计目标

这份文档解决 4 件事：

1. harness 运行时 archive 应放在哪里
2. runtime-local archive 和 workspace export bundle 如何区分
3. archive 目录和 artifact 如何组织
4. human / CLI / future proposer 如何稳定查询

---

## 2. 路径策略

### 2.1 逻辑根目录

Harness archive 的逻辑根目录定义为：

`If2Ai user-local data root / artifacts / harness /`

这是逻辑语义，不应在实现层直接硬编码为某个单一平台路径。

### 2.2 当前代码库下的推荐物理路径

基于当前 If2Ai 代码库广泛使用 `~/.if2ai/` 作为本地持久化根目录，在 Unix / macOS 上建议默认落到：

`~/.if2ai/artifacts/harness/`

这与现有路径风格保持一致，例如：

- `~/.if2ai/sessions/`
- `~/.if2ai/projects/`
- `~/.if2ai/skills/`
- `~/.if2ai/traces/`

### 2.3 平台无关实现要求

工程实现上不应把 `~/.if2ai/` 写死，而应优先解析：

- Tauri app data dir
- 或 `dirs` / `directories` 提供的本地应用数据目录

然后在当前产品风格下映射到 If2Ai 自己的 data root。

换句话说：

- 设计语义：`If2Ai user-local data root`
- 当前 Unix/macOS 默认实现：`~/.if2ai/`

### 2.4 为什么 archive 不应默认写入 workspace

因为 runtime archive 是用户态实时生成数据，它具有以下特点：

- 会持续增长
- 包含 traces、scores、candidate history
- 可能含敏感上下文
- 不是 repo source of truth

所以它默认不应写入当前项目仓库。

---

## 3. 两类存储对象

### 3.1 Runtime-Local Archive

这是主 archive。

定义：

- app 在用户机器本地持续写入
- 作为 harness 的长期经验仓库
- 默认位于 `~/.if2ai/artifacts/harness/` 或平台等价路径

它服务：

- 运行期 record/eval/search
- regression corpus
- frontier 维护
- future proposer selective access

### 3.2 Workspace Export Bundle

这是导出物，不是主 archive。

定义：

- 从 runtime-local archive 中按需导出
- 写入 workspace、CI artifact 或审查包
- 用于分享、对比、提交、人工分析

典型落点：

- `<workspace>/artifacts/harness-export/<bundle_id>/`
- CI artifact zip
- review bundle tarball

### 3.3 两者关系

关系应为：

`runtime-local archive -> selective export -> workspace export bundle`

而不是反过来。

原因：

- 主 archive 必须稳定、长期、append-only
- export bundle 是任务导向的、一次性的、可裁剪的

---

## 4. runtime-local archive 顶层布局

建议：

```text
~/.if2ai/artifacts/harness/
├── runs/
├── candidates/
├── frontiers/
├── indexes/
├── regressions/
├── task-packs/
└── exports/
```

说明：

- `runs/`
  每次 run 的完整结果
- `candidates/`
  候选 harness 的生命周期聚合目录
- `frontiers/`
  Pareto frontier 快照
- `indexes/`
  查询加速入口
- `regressions/`
  failure-driven case 库
- `task-packs/`
  运行时冻结的 task pack 快照
- `exports/`
  已生成的导出包记录

---

## 5. `runs/` 布局

```text
~/.if2ai/artifacts/harness/runs/<run_id>/
├── metadata.json
├── config.json
├── summary.json
├── archive_manifest.json
├── tasks/
│   ├── <task_id>/
│   │   ├── task_spec.json
│   │   ├── aggregate.json
│   │   └── trials/
│   │       ├── <trial_id>/
│   │       │   ├── trace.jsonl
│   │       │   ├── trace_summary.json
│   │       │   ├── outcome.json
│   │       │   ├── grades.json
│   │       │   ├── telemetry.json
│   │       │   ├── session_snapshot.json
│   │       │   └── workspace_diff.patch
└── frontier_snapshot.json
```

### 5.1 必须保留的 artifact

Meta-Harness 影响下，以下 artifact 为强制项：

- candidate code ref
- raw trace
- score / grade
- outcome snapshot
- diff / workspace effect

summary 只做索引，不可替代原始 artifact。

---

## 6. `candidates/` 布局

```text
~/.if2ai/artifacts/harness/candidates/<candidate_id>/
├── metadata.json
├── code/
│   ├── source_snapshot.tar.zst
│   ├── changed_files.json
│   └── patch.diff
├── runs.json
├── score_history.jsonl
├── frontier_membership.json
└── validation.json
```

### 6.1 为什么要有 candidate 聚合层

因为 Meta-Harness 的优化对象是 candidate harness，而不是单次 run。

一个 candidate 可能：

- 通过 validation
- 跑若干 search-set
- 最后跑 final-test

因此：

- `runs/` 是执行事实
- `candidates/` 是候选生命周期

---

## 7. frontier 与索引

### 7.1 `frontiers/`

```text
~/.if2ai/artifacts/harness/frontiers/<campaign_id>/<timestamp>.json
```

建议至少按这些 axis 维护非支配 frontier：

- `task_success`
- `policy_safety`
- `latency`
- `context_tokens`
- `cost`

### 7.2 `indexes/`

```text
~/.if2ai/artifacts/harness/indexes/
├── latest.json
├── by_task.json
├── by_axis.json
├── proposer_visible.json
└── final_test_hidden.json
```

说明：

- `latest.json`
  当前默认入口
- `by_task.json`
  某 task 维度的聚合视图
- `by_axis.json`
  top-k / frontier 查询加速
- `proposer_visible.json`
  proposer 默认可见索引
- `final_test_hidden.json`
  final-test 隐藏索引

---

## 8. SearchRole 与可见性策略

archive 中每个 task / run / artifact 都应带：

- `validation`
- `search_set`
- `final_test`

默认可见性：

- `validation`: proposer 可见
- `search_set`: proposer 可见
- `final_test`: proposer 不可见

如果 `ArchivePolicy.redact_sensitive_fields = true`：

- trace 输入裁剪
- file diff 脱敏
- session snapshot 结构化最小保留

---

## 9. 查询接口

建议同时提供：

- Rust API
- Tauri command
- CLI

最小 CLI 语义：

### 9.1 `top-k`

```bash
if2ai harness archive top-k --axis task_success --limit 20
```

### 9.2 `frontier`

```bash
if2ai harness archive frontier --campaign search_cp_v1
```

### 9.3 `show-run`

```bash
if2ai harness archive show-run --run run_2026_04_18_001
```

### 9.4 `show-trace`

```bash
if2ai harness archive show-trace --run run_2026_04_18_001 --task control-plane/workdir-isolation/001 --trial trial_001
```

### 9.5 `diff`

```bash
if2ai harness archive diff --candidate-a candidate_00017 --candidate-b candidate_00023
```

---

## 10. workspace export bundle

### 10.1 定义

workspace export bundle 是从 runtime-local archive 中筛选和复制出来的一次性导出物。

它不是主 archive，也不是 proposer 的默认工作底座。

### 10.2 推荐路径

建议导出到：

```text
<workspace>/artifacts/harness-export/<bundle_id>/
```

或 CI artifact 包。

### 10.3 适用场景

- PR review
- 团队共享失败样本
- CI 对比结果
- 提交外部 benchmark 结果
- 人工分析单个 candidate / run

### 10.4 导出内容

一个 export bundle 应按需包含：

- 被选中的 run metadata
- trace 子集
- outcome 子集
- grades
- summary
- patch.diff

默认不要求包含整个 runtime-local archive。

### 10.5 为什么必须区分 export bundle

不区分的话会出现两个问题：

1. 用户本地 archive 会污染 workspace
2. archive 的长期经验属性和 bundle 的临时分享属性会混在一起

---

## 11. requirements

### Functional Requirements

1. runtime-local archive 必须稳定落盘到用户本地 If2Ai data root
2. export bundle 必须可从 archive 选择性生成
3. archive 必须支持 frontier / top-k / diff / trace show
4. final-test 默认不得进入 proposer 可见索引

### Non-Functional Requirements

1. 路径命名稳定
2. JSON 优先
3. append-only 优先
4. export 不能反向污染 runtime-local archive

---

## 12. 风险与规避

### 风险 1: 把 archive 写进 workspace

问题：

- 导致仓库污染、数据暴涨、隐私风险上升

规避：

- 主 archive 强制走 user-local data root

### 风险 2: 把 export bundle 当主 archive

问题：

- 丢失长期历史、frontier 和 candidate 生命周期

规避：

- export 只作为派生物

### 风险 3: final-test 泄漏

规避：

- proposer 默认只读 `proposer_visible` 索引

---

## 13. 与 Meta-Harness 的映射

| Meta-Harness 设计 | If2Ai 映射 |
| ----------------- | ---------- |
| filesystem with prior code | `candidates/<candidate_id>/code/` |
| prior scores | `score_history.jsonl` / `summary.json` |
| prior execution traces | `trace.jsonl` |
| external evaluator | `runs/<run_id>/` + grading pipeline |
| Pareto frontier | `frontiers/` |
| validation outside proposer | `validation` + archive policy |
| minimum outer-loop structure | archive + frontier + validation + query |
