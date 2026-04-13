# ADR-005 Relationship with Upstream claw-cli — 详细实施 backlog

## 概述

**目标**: 建立与 claw-cli 的兼容性，确保 schema 对齐和未来互操作性
**ADR**: [ADR-005](./ADR-005-Relationship-with-Upstream-Claw-CLI.md)
**优先级**: P1 (信息记录为主)
**预估工时**: 1-2 天

---

## 子任务清单

### TASK-005-01: Schema 对齐验证

**目标**: 验证 if2Ai 与 claw-cli 的 MemoryEntry schema 兼容性

**具体任务**:
- [ ] 审查 if2Ai 的 `MemoryEntry` 结构
- [ ] 对比 claw-cli 的 Python `MemoryEntry` dataclass
- [ ] 确认以下字段兼容:
  - `key` (String)
  - `content` (String)
  - `category` (Core/Daily/Conversation)
  - `created_at` (RFC3339)
  - `updated_at` (RFC3339)
- [ ] 确认扩展字段（importance, access_count, trust_score）仅 if2Ai 有
- [ ] 编写兼容性报告

**验收标准**:
- [ ] 基础字段 100% 兼容
- [ ] 扩展字段有文档说明
- [ ] 兼容性报告已编写

---

### TASK-005-02: Session Schema 对齐验证

**目标**: 验证 if2Ai 与 claw-cli 的 Session schema 兼容性

**具体任务**:
- [ ] 审查 if2Ai 的 `Session` 结构
- [ ] 对比 claw-cli 的 Python `Session` dataclass
- [ ] 确认 `project_id` 字段为 if2Ai 独有
- [ ] 编写兼容性报告

**验收标准**:
- [ ] 基础字段 100% 兼容
- [ ] `project_id` 扩展有文档说明
- [ ] 兼容性报告已编写

---

### TASK-005-03: 压缩算法溯源文档

**目标**: 记录 if2Ai 压缩算法与 upstream 的关系

**具体任务**:
- [ ] 审查 `src-tauri/src/modules/runtime/compact.rs`
- [ ] 对比 hermes-agent 和 iClaw 的 context_compressor.py
- [ ] 在代码中添加溯源注释:
  ```rust
  // This implementation is adapted from:
  // - iClaw/agent/context_compressor.py (UClaw)
  // - hermes-agent/agent/context_compressor.py
  //
  // Changes from upstream:
  // - Rust async/await instead of Python asyncio
  // - Token counting using tiktoken-rs instead of tiktoken
  ```
- [ ] 确保 compact.rs 头部注释完整

**验收标准**:
- [ ] 代码头部有完整溯源注释
- [ ] 每个函数有 Rust doc 注释
- [ ] 与 upstream 的差异已说明

---

### TASK-005-04: 数据导出工具

**目标**: 实现 claw-cli 兼容格式导出

**具体任务**:
- [ ] 在 `SqliteMemoryProvider` 或独立工具中实现导出函数
- [ ] 实现 `export_for_clawcli(path: PathBuf) -> Result<(), MemoryError>`
  - 导出所有 MemoryEntry 为 claw-cli 兼容的 JSON 格式
  - 仅包含基础字段（不含 importance, access_count, trust_score）
- [ ] 编写测试验证导出格式

**验收标准**:
- [ ] 导出函数存在
- [ ] 导出格式与 claw-cli 兼容
- [ ] 测试覆盖

---

### TASK-005-05: README 或 ADR 索引文档

**目标**: 创建 claw-cli 关系文档

**具体任务**:
- [ ] 在 `docs/design-docs/postCLI/` 创建 `UPSTREAM-RELATIONSHIP.md`
- [ ] 文档内容:
  - if2Ai 与 claw-cli/hermes-agent 的关系
  - Schema 兼容性表格
  - 共同设计决策列表
  - 分歧点及原因
  - 未来互操作性计划

**验收标准**:
- [ ] 文档存在且完整
- [ ] 与 CLAUDE.md 的引用关系清晰

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P1 | TASK-005-01 | Schema 对齐验证 |
| P1 | TASK-005-02 | Session 对齐验证 |
| P1 | TASK-005-03 | 溯源文档 |
| P2 | TASK-005-04 | 数据导出工具 |
| P2 | TASK-005-05 | 关系文档 |

---

## 验收总览

- [ ] MemoryEntry schema 与 claw-cli 100% 兼容
- [ ] Session schema 与 claw-cli 兼容
- [ ] 压缩算法有完整溯源注释
- [ ] 可导出 claw-cli 兼容格式
- [ ] 关系文档已创建
