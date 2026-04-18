# If2Ai Harness Archive Query CLI Spec

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 定义 `if2ai harness archive ...` 这组查询命令的命令面、参数、输出约定、权限边界与实现任务，使 runtime-local archive 能被人类、CI 和 future proposer 稳定消费。

---

## 1. 为什么需要这份规格

我们已经在 archive 设计里明确了：

- archive 是主经验库
- raw trace / score / outcome / diff 必须可查询
- future proposer 需要 proposer-friendly selective access

但如果没有统一 CLI spec，archive 很容易退化成：

- 只有目录，没有稳定入口
- 只能人工 `ls` / `cat`
- 机器消费和人类消费混杂
- search/test visibility 约束难以执行

所以需要把 archive query surface 固定下来。

---

## 2. 设计目标

这份规格定义 6 件事：

1. CLI 命名空间与子命令结构
2. 默认路径解析策略
3. 每个命令的输入、输出、错误语义
4. proposer visibility / final-test hidden 约束
5. JSON 与人类可读输出模式
6. 第一期可交付的命令优先级

---

## 3. 命名空间与路径解析

### 3.1 命令前缀

统一使用：

```bash
if2ai harness archive <subcommand> ...
```

不建议：

- 直接暴露 `runs` / `frontier` 独立顶级命令
- 用内部模块名泄露实现细节

### 3.2 默认根路径

若用户未显式指定 `--archive-root`，CLI 必须解析到：

`If2Ai user-local data root / artifacts / harness /`

当前 Unix / macOS 默认：

`~/.if2ai/artifacts/harness/`

### 3.3 输出模式

所有查询命令必须支持：

- 默认 human-readable 输出
- `--json` 结构化输出

规则：

- 人类模式适合终端浏览
- `--json` 是脚本与 future proposer 的主入口

---

## 4. 命令分组

建议分成 6 组：

1. `runs`
2. `tasks`
3. `candidates`
4. `frontier`
5. `trace`
6. `export`

命令树建议：

```text
if2ai harness archive
├── runs list
├── runs show
├── tasks list
├── tasks show
├── candidates list
├── candidates show
├── candidates diff
├── frontier list
├── frontier show
├── trace show
├── trace grep
├── export create
└── export show
```

---

## 5. 全局参数

所有子命令建议共享以下参数：

- `--archive-root <path>`
  覆盖默认 archive 根目录
- `--json`
  输出 JSON
- `--limit <n>`
  控制列表输出条数
- `--search-role <validation|search-set|final-test>`
  过滤或约束可见性
- `--proposer-visible-only`
  只返回 proposer 可见 artifact
- `--no-color`
  关闭终端色彩

实现要求：

- `--json` 输出必须稳定、版本化、适合脚本解析
- `--search-role final-test` 若当前调用主体无权限，必须返回受控错误

---

## 6. `runs` 命令组

### 6.1 `runs list`

用途：

- 浏览最近的 harness run

示例：

```bash
if2ai harness archive runs list --limit 20
if2ai harness archive runs list --json
```

筛选参数：

- `--task-id <id>`
- `--pack-id <id>`
- `--status <passed|failed|partial>`
- `--since <iso8601>`
- `--until <iso8601>`

human 输出建议列：

- `run_id`
- `started_at`
- `status`
- `task_count`
- `pass_rate`
- `archive_root`

JSON 输出建议：

```json
{
  "schema_version": 1,
  "runs": [
    {
      "run_id": "run_2026_04_18_001",
      "started_at": "2026-04-18T09:30:00Z",
      "status": "failed",
      "task_count": 6,
      "pass_rate": 0.66
    }
  ]
}
```

### 6.2 `runs show`

用途：

- 查看某次 run 的聚合信息

示例：

```bash
if2ai harness archive runs show run_2026_04_18_001
if2ai harness archive runs show run_2026_04_18_001 --json
```

输出应包含：

- run metadata
- task aggregate summary
- blocking failures
- frontier membership
- exported bundle refs

---

## 7. `tasks` 命令组

### 7.1 `tasks list`

用途：

- 查看 archive 中出现过的 task

示例：

```bash
if2ai harness archive tasks list --pack-id general-productivity/information-work
```

筛选参数：

- `--domain <engineering|general-productivity|hybrid>`
- `--category <TaskCategory>`
- `--pack-id <id>`

输出应至少包含：

- `task_id`
- `latest_version`
- `domain`
- `category`
- `last_seen_run_id`

### 7.2 `tasks show`

用途：

- 查看一个 task 的历史表现与最近结果

示例：

```bash
if2ai harness archive tasks show general-productivity/information-work/multi-source-summary-001
```

输出应包含：

- 最新 task spec ref
- score history
- variance trend
- primary failure taxonomy
- linked candidate ids

---

## 8. `candidates` 命令组

### 8.1 `candidates list`

用途：

- 浏览 candidate harness

示例：

```bash
if2ai harness archive candidates list --limit 10
if2ai harness archive candidates list --proposer-visible-only --json
```

输出应包含：

- `candidate_id`
- `created_at`
- `validation_status`
- `search_score`
- `frontier_member`

### 8.2 `candidates show`

用途：

- 查看单个 candidate 的完整摘要

输出应包含：

- metadata
- changed files
- associated runs
- latest scores by axis
- proposer visibility

### 8.3 `candidates diff`

用途：

- 对比两个 candidate 的 patch、score 和 failure profile

示例：

```bash
if2ai harness archive candidates diff cand_001 cand_004
if2ai harness archive candidates diff cand_001 cand_004 --json
```

输出应包含：

- patch summary
- changed file list
- score delta by axis
- failure taxonomy delta
- frontier movement

设计要求：

- human 模式要优先显示 score delta 和 changed files，而不是直接倾倒完整 patch
- `--show-patch` 时才输出 patch body

---

## 9. `frontier` 命令组

### 9.1 `frontier list`

用途：

- 查看某 campaign 的 frontier 快照列表

示例：

```bash
if2ai harness archive frontier list --campaign default
```

输出应包含：

- `campaign_id`
- `snapshot_time`
- `member_count`
- `axes`

### 9.2 `frontier show`

用途：

- 查看某个 frontier snapshot 的成员

示例：

```bash
if2ai harness archive frontier show --campaign default --latest
```

输出应包含：

- frontier axes
- member candidates
- per-axis score
- dominance explanation

附加参数：

- `--top-k <n>`
- `--axis <task_success|policy_safety|latency|cost|context_tokens>`

---

## 10. `trace` 命令组

### 10.1 `trace show`

用途：

- 展示某个 trace 的结构化事件与 summary

示例：

```bash
if2ai harness archive trace show --run-id run_001 --task-id task_001 --trial-id trial_002
if2ai harness archive trace show trace_abc123 --json
```

参数：

- `--run-id`
- `--task-id`
- `--trial-id`
- `--event-limit <n>`
- `--event-type <type>`

输出应包含：

- trace metadata
- summary
- selected events
- linked grade refs

### 10.2 `trace grep`

用途：

- 对 archive 中 trace 做事件级过滤

示例：

```bash
if2ai harness archive trace grep --event-type PolicyDecision --reason contains:deny
if2ai harness archive trace grep --event-type ToolCalled --tool write_file --limit 50 --json
```

设计原则：

- 优先基于结构化字段过滤
- 避免把 grep 设计成任意原始文本搜索器

支持的筛选至少包括：

- `--event-type`
- `--tool`
- `--task-id`
- `--grader-id`
- `--taxonomy`

---

## 11. `export` 命令组

### 11.1 `export create`

用途：

- 从 runtime-local archive 选择性导出 bundle

示例：

```bash
if2ai harness archive export create --run-id run_001 --output ./artifacts/harness-export/review-001
```

参数：

- `--run-id`
- `--task-id`
- `--candidate-id`
- `--output <path>`
- `--include-traces`
- `--include-patches`
- `--redact-sensitive`

输出应包含：

- bundle id
- output path
- included artifacts summary

### 11.2 `export show`

用途：

- 查看已生成 export bundle 的 manifest

---

## 12. 可见性与权限约束

### 12.1 Search/Test 隔离

CLI 必须尊重：

- `validation`
- `search-set`
- `final-test`

可见性规则：

- 默认只返回 proposer 可见内容
- `final-test` artifact 默认隐藏
- 试图读取 hidden artifact 时，返回受控错误而不是静默忽略

### 12.2 错误语义

建议错误类型：

- `ArchiveNotFound`
- `RunNotFound`
- `TaskNotFound`
- `CandidateNotFound`
- `TraceNotFound`
- `VisibilityDenied`
- `UnsupportedSchemaVersion`
- `InvalidFilterCombination`

`--json` 模式建议输出：

```json
{
  "error": {
    "code": "VisibilityDenied",
    "message": "final-test artifacts are not proposer-visible"
  }
}
```

---

## 13. 第一阶段实现优先级

### Phase 1

优先实现：

1. `runs list`
2. `runs show`
3. `tasks show`
4. `trace show`
5. `export create`

理由：

- 足够支撑人类 review
- 足够支撑最小 regression 调试

### Phase 2

继续实现：

1. `candidates list`
2. `candidates show`
3. `candidates diff`
4. `frontier list`
5. `frontier show`

### Phase 3

再实现：

1. `trace grep`
2. proposer-visible filtering
3. final-test visibility enforcement
4. scripted top-k selection

---

## 14. Rust 模块建议

建议映射到：

```text
src-tauri/src/modules/harness/archive/
├── cli.rs
├── query.rs
├── filters.rs
├── visibility.rs
├── formatter.rs
└── errors.rs
```

建议核心 API：

- `list_runs`
- `show_run`
- `list_tasks`
- `show_task`
- `list_candidates`
- `show_candidate`
- `diff_candidates`
- `list_frontiers`
- `show_frontier`
- `show_trace`
- `grep_trace_events`
- `create_export_bundle`

---

## 15. 与其他文档的关系

- archive 布局： [modules-harness-archive-layout.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-archive-layout.md)
- 核心契约： [modules-harness-core-contracts.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-core-contracts.md)
- 实施计划： [modules-harness-implementation-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-implementation-plan.md)
