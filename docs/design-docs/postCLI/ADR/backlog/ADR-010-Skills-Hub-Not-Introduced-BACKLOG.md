# ADR-010 Skills Hub Not Introduced — 详细实施 backlog

## 概述

**目标**: 确认 Skills Hub 不引入，强化 ToolRegistry 作为替代方案
**ADR**: [ADR-010](./ADR-010-Skills-Hub-Not-Introduced.md)
**优先级**: N/A (文档确认类)
**预估工时**: 0.5 天

---

## 子任务清单

### TASK-010-01: 文档确认

**目标**: 确认 Skills Hub 不引入的决定

**具体任务**:
- [ ] 阅读 `docs/design-docs/postCLI/ADR/ADR-010-Skills-Hub-Not-Introduced.md`
- [ ] 确认决定清晰记录:
  - 2026-02 ClawHub 安全事件引用
  - 单用户桌面模型理由
  - Trust model 不匹配理由
- [ ] 无需代码实现（仅文档）

**验收标准**:
- [ ] ADR 文档完整
- [ ] 决定理由充分

---

### TASK-010-02: ToolRegistry 强化确认

**目标**: 确认 ToolRegistry 覆盖本地工具安装需求

**具体任务**:
- [ ] 审查 `src-tauri/src/modules/tools/registry.rs`
- [ ] 确认现有功能:
  - 工具注册
  - 工具发现
  - 工具执行
- [ ] 确认支持本地目录加载
- [ ] 记录 ToolRegistry 能力矩阵

**验收标准**:
- [ ] ToolRegistry 功能完整
- [ ] 能力矩阵已记录

---

### TASK-010-03: 未来签名工具包设计 (可选)

**目标**: 为未来安全工具安装预留设计

**具体任务**:
- [ ] 设计 `SignedToolPackage` 结构（ADR 中已有）
- [ ] 确认 Ed25519 签名验证需求
- [ ] 在 ADR 中补充设计细节（如果需要）
- [ ] 不实现代码（仅设计）

**验收标准**:
- [ ] 设计文档完整
- [ ] 预留扩展点

---

## 验收总览

- [ ] ADR-010 决定已确认
- [ ] ToolRegistry 能力已确认
- [ ] 未来设计已规划
