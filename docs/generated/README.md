# docs/generated/ — 自动生成内容目录

> **规则：本目录下所有文件均为机器生成，禁止手动编辑。**  
> 若需修改，应修改生成它的工具或规则，然后重新生成。

---

## 目录结构

```
docs/generated/
├── README.md                    # 本文件 (手动维护的唯一例外)
├── QUALITY_SCORE.md             # 质量评分卡，由 executor 每个 Phase 完成后更新
├── reports/                     # Harness 执行报告摘要（由 executor 写入）
│   ├── phase-1-summary.md       # Phase 1 完成后生成
│   ├── phase-2-summary.md       # Phase 2 完成后生成
│   └── phase-3-summary.md       # Phase 3 完成后生成
└── api-snapshots/               # Rust 公开接口快照（由 cargo doc 生成）
    └── if2ai-backend.json       # JSON 格式的接口定义快照
```

**注意**：`harness/suites/` 中的套件 YAML 文件**也是**由 AI 生成的，但放在 `harness/` 目录下（而不是这里），因为它们被 `harness/runner.py` 直接引用执行。

---

## 谁生成什么

| 文件 | 生成者 | 生成时机 | 生成方式 |
|------|--------|---------|---------|
| `QUALITY_SCORE.md` | **Executor** (Claude Code) | 每个 Phase 完成后 | 读取 `.harness-reports/` 聚合 |
| `reports/phase-N-summary.md` | **Executor** (Claude Code) | 到达 `human_checkpoint` 时 | 汇总 slice 执行结果 |
| `api-snapshots/if2ai-backend.json` | **cargo doc** | CI 或手动触发 | `cargo rustdoc --output-format json` |
| `harness/suites/*.yaml` | **AI** (Copilot / Claude) | 创建每个 Phase exec-plan 时 | 根据 design-docs 中的接口定义生成 |

---

## QUALITY_SCORE.md 格式规范

Executor 每次写 `QUALITY_SCORE.md` 时必须遵循以下格式：

```markdown
# If2Ai 质量评分卡

**更新时间**: YYYY-MM-DD  
**当前 Phase**: Phase N  
**整体状态**: 🟡 In Progress / 🟢 Phase N Complete

## Phase 完成状态

| Phase | 状态 | 完成时间 | 编译 | 测试 | Harness |
|-------|------|---------|------|------|---------|
| Phase 1 | ✅ | YYYY-MM-DD | ✅ PASS | ✅ PASS | ✅ 9/9 |
| Phase 2 | 🔄 | — | — | — | — |
| Phase 3 | ⏳ | — | — | — | — |

## 当前 Phase 详情

| Slice | 标题 | 状态 | Gate |
|-------|------|------|------|
| 1.1 | 模块骨架 | ✅ done | compile ✅ |
| 1.2 | 修复编译错误 | 🔴 pending | — |
...

## 代码质量指标

- **测试覆盖率**: N% (目标: Phase1 ≥60%, Phase2 ≥70%, Phase3 ≥75%)
- **unwrap() 使用**: N 处 (目标: 0，测试除外)
- **编译错误**: 0
- **编译警告**: N

## 记录

- YYYY-MM-DD: Phase X slice Y 完成，Gate PASS
```

---

## harness/suites/ 生成规范

当创建新 Phase 的 exec-plan 时，AI 同时应生成对应的 harness suite 文件。

**命名规范**：

| exec-plan 中的 suite_path | 对应文件 | 对应 slice |
|--------------------------|---------|-----------|
| `harness/suites/tool_registry_basic.yaml` | 工具注册基础测试 | 1.5 |
| `harness/suites/e2e_conversation.yaml` | E2E 对话流程 | 1.8 |
| `harness/suites/phase1_integration.yaml` | Phase 1 集成 | 1.9 |
| `harness/suites/context_compression.yaml` | 上下文压缩 | 2.2 |
| `harness/suites/phase2_integration.yaml` | Phase 2 集成 | 2.6 |
| `harness/suites/self_improvement_baseline.yaml` | RL 基线 | 3.3 |
| `harness/suites/phase3_integration.yaml` | Phase 3 集成 | 3.4 |

**Suite YAML 必须包含**：
- `suite_id`：唯一标识符
- `target_pass_rate`：通过率阈值（通常 1.0）
- `runner`：运行器类型（`cargo_test` 或 `llm_eval`）
- `test_cases`：每个测试有 `id`、`test_fn`、`expect`

---

## 初始质量评分卡

> 以下是初始状态，executor 应在每个 Phase 完成后**替换**此文件内容为最新评分。
