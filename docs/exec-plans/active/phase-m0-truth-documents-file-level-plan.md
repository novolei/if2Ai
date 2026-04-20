# Phase M0 Truth Documents File-Level Plan

> 覆盖 `m0.0 + m0.1 + m0.2`。
>
> 最后更新: 2026-04-20

## 1. 目的

把 `M0` 前半段的“真相固定”工作拆成 executor 可直接执行的文件级任务，确保 canonical domain model 与 workflow truth 不是停留在 runbook 描述，而是能按文件、按顺序落地。

## 2. 覆盖切片

1. `m0.0` preflight inventory
2. `m0.1` canonical domain model
3. `m0.2` workflow truth registry

## 3. 新增文件

- `docs/staff-remediation/if2ai-canonical-domain-model.md`
- `docs/staff-remediation/if2ai-workflow-truth.md`

## 4. 修改文件

- [docs/staff-remediation/README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)
- 视需要小幅修订 [ARCHITECTURE.md](/Users/ryanliu/Documents/IfAI/if2Ai/ARCHITECTURE.md:1) 中引用说明，但本阶段不做大规模重写

## 5. 执行顺序

### 5.1 `T1` preflight evidence inventory

先只读不写，必须核查：

- [ARCHITECTURE.md](/Users/ryanliu/Documents/IfAI/if2Ai/ARCHITECTURE.md:1)
- [AGENTS.md](/Users/ryanliu/Documents/IfAI/if2Ai/AGENTS.md:1)
- [src-tauri/src/commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:1)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- [src-tauri/src/main.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/main.rs:1)

必须记录：

1. 当前实体命名
2. 当前 workflow 真实入口
3. activation / execution mode / memory / harness 当前成熟度
4. god-file 当前角色

### 5.2 `T2` write canonical domain model

在 [if2ai-canonical-domain-model.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-canonical-domain-model.md:1) 中必须写出：

1. 文档目的
2. canonical naming rules
3. canonical entity catalog
4. entity lifecycle table
5. layer ownership table
6. naming drift and migration map
7. open questions

至少覆盖实体：

1. `agent`
2. `session`
3. `project`
4. `run`
5. `worker`
6. `memory`
7. `automation`
8. `harness_eval`
9. `activation_license`
10. `execution_mode`

### 5.3 `T3` write workflow truth registry

在 [if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1) 中必须写出：

1. `app boot`
2. `onboarding`
3. `activation gate`
4. `chat prompt dispatch`
5. `stream projection`
6. `permission approval`
7. `session recovery`
8. `memory capture`
9. `memory recall`
10. `memory write`
11. `harness record`
12. `harness replay`
13. `harness eval`
14. `execution mode routing`
15. `specialized surface entry`
16. `deactivation / revoke fallback`

每条 workflow 必须具备：

- `workflow_name`
- `status`
- `backend_entry`
- `frontend_entry`
- `source_of_truth`
- `blockers`
- `next_phase_owner`

### 5.4 `T4` update staff remediation index

回写 [README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)：

1. 加入两份新文档目录入口
2. 在端到端映射中把 `M0` 从“仅 runbook”改成“runbook + file-level plans”
3. 明确 `M0` 的前半段由本文件承接

## 6. 不允许做的事

1. 不在这一段里引入 Rust/TS contract 代码
2. 不为了文档好看而把 `partial` 写成 `canonical`
3. 不在 `ARCHITECTURE.md` 做全面改写
4. 不把 activation gate 与 onboarding 合成一个 workflow

## 7. 完成定义

只有当 reviewer 能直接从这两份文档回答“这个系统的核心实体是什么”和“当前哪些 workflow 已建立、哪些还没建立”时，这一段才算完成。
