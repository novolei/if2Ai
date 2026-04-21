# Implementation Packs

> If2Ai 从“文档体系驱动”切换到“任务包驱动”后的唯一执行入口层。
>
> 最后更新: 2026-04-21

---

## 1. 为什么新增这一层

过去 If2Ai 主要采用：

`设计文档 -> backlog -> exec-plan YAML -> file-level plan -> 实现`

这套方式对人类项目管理是友好的，但对 Cursor / Codex 这类执行型 AI 不友好，常见问题是：

1. 中间层过多，目标信号被稀释。
2. 同一目标在多个文档里重复出现，但表述不完全一致。
3. AI 需要自己判断哪份文档是“本轮真相”，容易漂移。
4. 文档强调 phase 完成，却没有把“本次改哪些文件、验收是什么”压缩成单一硬约束。

为解决这些问题，If2Ai 现在采用新的执行方法：

`Gap 模块真相 -> Implementation Pack -> 代码实现 -> Truth 回写`

也就是说：

- `docs/staff-remediation/gap-modules/` 继续保存长期真相。
- `docs/implementation-packs/` 成为短期执行入口。

---

## 2. 这一层的定位

Implementation Pack 不是 roadmap，不是 backlog，也不是 phase YAML 的替代版拆分文档。

它只做一件事：

> 为一次具体代码落地提供唯一、短小、强约束、可验收的执行包。

每个 pack 都应该能回答下面 7 个问题：

1. 本次目标到底是什么？
2. 为什么现在要做它？
3. 允许改哪些文件？
4. 不允许碰哪些范围？
5. 当前真相文档是什么？
6. UClaw 对标是什么？
7. 怎么才算验收通过？

如果一份文档不能直接回答这 7 个问题，它就不应该成为 Cursor 的主执行入口。

---

## 3. 与现有文档层次的关系

### A. Truth 层

长期存在，回答“系统现在是什么，缺什么，目标结构是什么”。

主要包括：

- `docs/staff-remediation/gap-modules/`
- `docs/staff-remediation/if2ai-workflow-truth.md`
- `docs/staff-remediation/if2ai-canonical-domain-model.md`

### B. Pack 层

短期执行入口，回答“本次到底改什么、改哪里、怎么验收”。

主要包括：

- `docs/implementation-packs/**`

### C. Code 层

代码、测试、少量必要的 truth 回写。

---

## 4. 目录结构约定

建议每个 Gap 模块对应一个目录：

```text
docs/implementation-packs/
├── README.md
├── TEMPLATE.md
├── ACTIVE.md
└── chat-prompt-dispatch/
    ├── CPD-001-turn-spine.md
    └── ...
```

约束：

1. 一个 pack 只对应一个明确目标。
2. pack 名称使用 `模块缩写-序号-短标题`。
3. 同一时刻只允许极少数 `ACTIVE` pack，最好只有一个。

---

## 5. 编写规则

每个 Implementation Pack 必须包含以下固定段落：

1. `Goal`
2. `Why Now`
3. `Allowed Files`
4. `Forbidden Files`
5. `Source Of Truth`
6. `UClaw References`
7. `Required Changes`
8. `Acceptance`
9. `Out Of Scope`
10. `Execution Notes`

硬规则：

1. `Allowed Files` 必须具体到文件或目录。
2. `Forbidden Files` 必须明确列出顺手最容易扩散的范围。
3. `Acceptance` 必须至少包含一个真实命令或一个可验证的代码状态。
4. `Out Of Scope` 必须写明这次明确不做什么。
5. 不允许把完整 phase YAML 复制进 pack。

---

## 6. Cursor / Codex 使用规则

每次让执行型 AI 落代码时，原则上只喂：

1. 当前 active pack
2. active pack 里引用的最小真相文档
3. active pack 允许修改的代码文件

不要直接让 AI 自己在下面这些大范围里游走：

- `docs/exec-plans/active/**`
- 大量 phase closeout 文档
- 多份互相重叠的 runbook / review note

推荐提示词风格：

> 按 `docs/implementation-packs/.../XXX.md` 实现。  
> 不要读取其他规划文档，除非该 pack 明确引用。  
> 先复述 Goal、Allowed Files、Acceptance、Out Of Scope，再开始修改代码。

---

## 7. 回写规则

一次 pack 完成后，只建议回写三类内容：

1. pack 本身的状态
2. 对应 gap 模块文档里的实现证据
3. `if2ai-workflow-truth.md` 中对应 workflow 的状态变化

尽量不要再额外生成：

- completion summary
- phase closeout
- “本轮完成报告”

除非这些是明确要求的对外交付物。

---

## 8. ACTIVE 机制

根目录的 [ACTIVE.md](./ACTIVE.md) 用来声明当前唯一建议执行的 pack。

规则：

1. 一次最好只有一个 active target。
2. 如果必须并发，也应保证 write scope 不重叠。
3. 若 active pack 改变，必须先更新 `ACTIVE.md`。

---

## 9. 迁移建议

从旧流程切换到新流程时：

1. 保留原有 gap 模块文档，不删。
2. 不再继续扩大 `exec-plans` 的日常执行职责。
3. 新工作一律先从 gap 模块生成 implementation pack。
4. Pack 成为 Cursor 的唯一主入口。

---

## 10. 相关文件

- [Template](./TEMPLATE.md)
- [Active Pack](./ACTIVE.md)
- [Migration Pack Index](./MIGRATION_INDEX.md)
- [Gap Modules Index](../staff-remediation/gap-modules/index.md)
