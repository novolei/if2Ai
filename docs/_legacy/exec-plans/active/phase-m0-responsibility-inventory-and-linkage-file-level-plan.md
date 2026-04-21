# Phase M0 Responsibility Inventory And Linkage File-Level Plan

> 覆盖 `m0.6 + m0.7`。
>
> 最后更新: 2026-04-20

## 1. 目的

把 `M0` 的后半段收口工作拆成文件级任务，确保 `M1/M2` 有清晰的 god-file 拆分输入，同时所有索引文档都能把 executor 导到正确入口。

## 2. 覆盖切片

1. `m0.6` god-file responsibility inventory
2. `m0.7` coverage matrix and plan linkage

## 3. 新增文件

- `docs/staff-remediation/m0-god-file-responsibility-inventory.md`

## 4. 修改文件

- [docs/staff-remediation/README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)
- [docs/exec-plans/index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/index.md:1)
- [docs/exec-plans/active/phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
- [docs/exec-plans/active/phase-m0-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-executor-runbook.md:1)

## 5. 执行顺序

### 5.1 `L1` write god-file responsibility inventory

必须盘点：

1. [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
2. [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
3. [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
4. [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

每个文件必须写出：

1. current responsibilities
2. responsibilities that belong elsewhere
3. target phase
4. destination module
5. safe extraction order
6. risky entanglements

### 5.2 `L2` update staff remediation docs index

在 [README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1) 中：

1. 加入 `m0-god-file-responsibility-inventory.md`
2. 把 `M0` 的 file-level plans 全部列出来
3. 确保端到端执行映射不再写成“仅由 runbook 承载”

### 5.3 `L3` update exec-plan global index

在 [docs/exec-plans/index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/index.md:1) 中确保：

1. `Phase M` 仍是 Staff remediation 的唯一执行入口
2. `M0` 可被正确定位到 runbook 与 file-level plans
3. executor 不会误从旧 phase 直接跳进 `M1/M2`

### 5.4 `L4` update phase-m execution index and runbook

必须保证：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1) 能把 executor 指向 `M0` runbook
2. [phase-m0-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-executor-runbook.md:1) 能把 executor 指向三份 file-level plans

## 6. 不允许做的事

1. 不只写“文件太大”
2. 不给 inventory 漏 destination module
3. 不让索引只链接到 phase YAML 而不链接 runbook / file-level plans

## 7. 完成定义

只有当 executor 只看索引文档就能找到：

1. `M0` 的 phase YAML
2. `M0` 的 runbook
3. `M0` 的三份 file-level plans
4. `M1/M2` 的后续拆分输入

这一段才算完成。
