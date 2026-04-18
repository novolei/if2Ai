# If2Ai Harness Archive Module File-by-File Spec

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**模块路径**: `src-tauri/src/modules/harness/archive/`  
**一句话定位**: 将 `modules/harness/archive` 拆成可直接实现的 Rust 模块级规格，明确每个文件的职责、稳定 API、边界约束、测试要求与推荐交付顺序。

---

## 1. 为什么需要这份文档

前面的文档已经回答了：

- archive 为什么存在
- archive 放在哪里
- CLI 应该长什么样
- proposer visibility 为什么重要

但工程实现仍然需要一个更直接的问题答案：

`archive/` 目录里每个 Rust 文件到底干什么？

这份文档就是为这个问题准备的。

---

## 2. 模块目标

`modules/harness/archive` 的职责不是“存文件”这么简单，而是要同时满足 4 类消费者：

1. harness runtime
2. 人类调试与 review
3. CI / automation
4. future proposer / optimizer

因此它必须提供：

- 稳定的 layout
- 稳定的 query API
- visibility enforcement
- formatter / export support
- 与 CLI 对齐的命令入口

---

## 3. 推荐目录结构

```text
src-tauri/src/modules/harness/archive/
├── mod.rs
├── types.rs
├── layout.rs
├── io.rs
├── query.rs
├── filters.rs
├── visibility.rs
├── frontier.rs
├── export.rs
├── formatter.rs
├── cli.rs
└── errors.rs
```

说明：

- `types.rs`
  放 archive 域自己的稳定 DTO，而不是复用 CLI 临时 struct
- `io.rs`
  专门承担目录与文件读写
- `formatter.rs`
  专门承担 human-readable / JSON 输出

---

## 4. 文件级规格

### 4.1 `mod.rs`

职责：

- 作为 archive 子模块总入口
- 统一 re-export 稳定 API
- 不承载业务逻辑

应暴露：

- archive root resolver
- query service
- export service
- errors / core types

不应包含：

- 目录扫描逻辑
- CLI 参数解析逻辑
- 格式化细节

要求：

- 保持薄入口
- 所有核心类型从这里可发现，但不在此定义

### 4.2 `types.rs`

职责：

- 定义 archive 域内稳定类型

建议类型：

- `ArchiveRoot`
- `ArchiveManifest`
- `RunRef`
- `TaskRef`
- `CandidateRef`
- `FrontierRef`
- `ExportBundleRef`
- `VisibilityPolicy`
- `QueryOutputMode`

要求：

- 类型必须与 `modules-harness-core-contracts.md` 兼容
- 不直接依赖 CLI 框架类型
- 可 JSON 序列化

### 4.3 `layout.rs`

职责：

- 封装所有 archive 路径计算与布局约定

主要 API：

- `resolve_archive_root() -> ArchiveRoot`
- `runs_dir(root: &ArchiveRoot) -> PathBuf`
- `run_dir(root: &ArchiveRoot, run_id: &str) -> PathBuf`
- `candidate_dir(root: &ArchiveRoot, candidate_id: &str) -> PathBuf`
- `frontier_dir(root: &ArchiveRoot, campaign_id: &str) -> PathBuf`
- `exports_dir(root: &ArchiveRoot) -> PathBuf`

要求：

- 所有路径拼接必须集中在这里
- 不允许其他模块手写目录结构字符串
- 必须支持 platform-neutral root resolution

测试要求：

- 路径快照测试
- 空格路径 / 相对路径 / 覆盖根路径测试

### 4.4 `io.rs`

职责：

- 负责 archive 文件系统读写

主要 API：

- `read_json<T>()`
- `write_json_atomic<T>()`
- `read_jsonl<T>()`
- `append_jsonl<T>()`
- `ensure_dir()`
- `read_optional_file()`

要求：

- 写入必须尽量 atomic
- 读取错误要归一化为 archive errors
- 不在这里做业务筛选或 visibility 决策

测试要求：

- JSON / JSONL round-trip
- 部分文件缺失
- schema 版本不兼容

### 4.5 `query.rs`

职责：

- 提供 archive 查询的主业务服务

这是 archive 层最核心的文件之一。

主要 API：

- `list_runs(filters: RunFilters) -> Vec<RunSummary>`
- `show_run(run_id: &str) -> RunDetail`
- `list_tasks(filters: TaskFilters) -> Vec<TaskSummary>`
- `show_task(task_id: &str) -> TaskDetail`
- `list_candidates(filters: CandidateFilters) -> Vec<CandidateSummary>`
- `show_candidate(candidate_id: &str) -> CandidateDetail`
- `show_trace(selector: TraceSelector) -> TraceDetail`

要求：

- 只做查询编排，不做 CLI parsing
- 所有查询结果都应先经过 visibility gate
- 必须优先读取 indexes，再回退到全量扫描

不应包含：

- 输出格式化
- 终端渲染
- patch diff 呈现细节

### 4.6 `filters.rs`

职责：

- 定义查询筛选条件与过滤组合验证

建议类型：

- `RunFilters`
- `TaskFilters`
- `CandidateFilters`
- `FrontierFilters`
- `TraceSelector`
- `FilterValidationError`

要求：

- 所有命令过滤条件都先转成结构化 filters
- 非法组合必须在这里拒绝

示例非法组合：

- 同时指定 `--run-id` 与互斥的 broad `--since/--until` 组合
- 同时请求 `final-test` 与 `--proposer-visible-only`

### 4.7 `visibility.rs`

职责：

- 执行 search/test 可见性约束

这是 archive 层的第二个关键文件。

主要 API：

- `enforce_run_visibility(...)`
- `enforce_candidate_visibility(...)`
- `filter_proposer_visible<T>(...)`
- `require_non_final_test(...)`

要求：

- `final-test` 默认 hidden
- visibility denied 必须显式报错
- 不允许静默降级成“查不到”

failure 行为：

- 返回 `VisibilityDenied`
- 在 JSON 模式下输出结构化错误对象

### 4.8 `frontier.rs`

职责：

- 读取与解释 frontier snapshot

主要 API：

- `list_frontiers(filters: FrontierFilters) -> Vec<FrontierSummary>`
- `show_frontier(filters: FrontierFilters) -> FrontierDetail`
- `top_k_by_axis(...)`
- `diff_frontier_membership(...)`

要求：

- 支持按 axis 排序
- 支持显示 dominance / non-dominance 解释
- 不在这里做 full candidate diff

### 4.9 `export.rs`

职责：

- 从 runtime-local archive 生成 workspace export bundle

主要 API：

- `create_export_bundle(request: ExportRequest) -> ExportBundleRef`
- `show_export_bundle(bundle_id: &str) -> ExportBundleDetail`
- `redact_artifacts(...)`

要求：

- 必须支持 selective export
- 必须支持 redaction policy
- 必须明确 bundle manifest

输出至少包含：

- bundle id
- source refs
- included artifacts
- redaction summary

### 4.10 `formatter.rs`

职责：

- 负责 archive 查询结果的输出格式化

主要 API：

- `format_runs_table(...)`
- `format_run_detail(...)`
- `format_candidate_diff(...)`
- `format_error(...)`
- `to_json_output(...)`

要求：

- human-readable 与 JSON 输出分离
- 不把 formatter 写进 query service
- 所有 JSON 输出必须带 `schema_version`

### 4.11 `cli.rs`

职责：

- 把 CLI 参数映射到 query/export 服务

主要 API：

- `handle_archive_command(...)`
- `parse_global_options(...)`
- `dispatch_runs_command(...)`
- `dispatch_candidates_command(...)`
- `dispatch_trace_command(...)`

要求：

- 只处理命令层映射
- 不做核心业务逻辑
- 所有错误统一走 `errors.rs`

### 4.12 `errors.rs`

职责：

- 统一 archive 模块错误类型

建议错误：

- `ArchiveNotFound`
- `RunNotFound`
- `TaskNotFound`
- `CandidateNotFound`
- `TraceNotFound`
- `VisibilityDenied`
- `UnsupportedSchemaVersion`
- `InvalidFilterCombination`
- `CorruptArchiveArtifact`
- `ExportFailed`

要求：

- 错误必须可 human-readable
- 错误必须可转 JSON code/message
- 不泄露底层实现细节

---

## 5. 模块间依赖方向

推荐依赖方向：

```text
cli -> query/export -> filters/visibility/frontier -> io/layout/types/errors
formatter -> types/errors
query -> formatter (禁止)
layout -> io (禁止)
visibility -> cli (禁止)
```

解释：

- `query` 不能依赖 `formatter`
- `layout` 不能依赖 `io`
- `visibility` 必须是纯规则层

---

## 6. 典型调用流

### 6.1 `runs show`

```text
cli
  -> filters
  -> query.show_run
     -> visibility.enforce_run_visibility
     -> io.read_json / read_jsonl
     -> formatter.format_run_detail
```

### 6.2 `candidates diff`

```text
cli
  -> filters
  -> query.show_candidate / frontier helpers
  -> io.read_json
  -> formatter.format_candidate_diff
```

### 6.3 `export create`

```text
cli
  -> export.create_export_bundle
     -> visibility
     -> io
     -> redact_artifacts
     -> write bundle manifest
```

---

## 7. 实现阶段建议

### Phase 1

先实现：

- `types.rs`
- `layout.rs`
- `io.rs`
- `errors.rs`
- `query.rs` 的 `runs show / list`
- `formatter.rs` 基础表格输出

### Phase 2

再实现：

- `filters.rs`
- `visibility.rs`
- `trace show`
- `tasks show`
- `export.rs`

### Phase 3

最后实现：

- `frontier.rs`
- `candidates diff`
- proposer-visible filtering
- final-test enforcement

---

## 8. 测试矩阵

### 8.1 单元测试

- `layout.rs`
  路径生成
- `filters.rs`
  过滤组合合法性
- `visibility.rs`
  final-test hidden
- `formatter.rs`
  human/JSON 输出快照

### 8.2 集成测试

- 从一个最小 archive fixture 跑 `runs list/show`
- 从 candidate fixture 跑 `candidates show/diff`
- 从 export fixture 跑 `export create`

### 8.3 回归测试

- corrupted JSON artifact
- missing summary
- hidden final-test artifact
- redaction policy 生效

---

## 9. 常见反模式

不要这样做：

1. 在多个文件里手写 archive 路径
2. 在 `cli.rs` 直接读 JSON 文件
3. 在 `query.rs` 里拼表格
4. 对 `VisibilityDenied` 静默返回空列表
5. 只保留 summary，不读取 raw trace ref

---

## 10. 与其他文档的关系

- archive 布局： [modules-harness-archive-layout.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-archive-layout.md)
- CLI 查询面： [archive-query-cli-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/archive-query-cli-spec.md)
- 实施计划： [modules-harness-implementation-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-implementation-plan.md)
