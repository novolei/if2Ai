# If2Ai Agent Gap 模块索引

> 按具体 Gap 建立独立模块目录；每个 Gap 模块内统一提供：
>
> - `01-usage-guide.md`
> - `02-implementation.md`
> - `03-framework.md`
>
> 最后更新: 2026-04-21
> 对标来源:
> - `/Users/ryanliu/Documents/iClaw/UClaw/UClawApp`
> - `/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs`

---

## 模块目录

### [chat-prompt-dispatch](./chat-prompt-dispatch/01-usage-guide.md)

主聊天 happy path 的 Gap 模块：

- `commands/agent.rs` 仍是主脑
- `TurnService` 只接管 preflight，未接管完整 turn spine
- `chat_prompt_dispatch` workflow 仍是 `partial`

### [execution-mode-policy-routing](./execution-mode-policy-routing/01-usage-guide.md)

执行模式与策略分流 Gap 模块：

- request intelligence 仍是 advisory
- `execution_mode_routing` workflow 仍是 `not_established`
- `prepare_step_execution` 仍大量停留在 shadow / governance path

### [runtime-projection-and-shell](./runtime-projection-and-shell/01-usage-guide.md)

前端投影层与主壳层 Gap 模块：

- `App.tsx` / `chat-ui.tsx` 仍承担隐性编排
- runtime projection 与老 listener 并行存在
- Boot / MainShell 仍未完全收口主状态真相

### [memory-write-recall-lifecycle](./memory-write-recall-lifecycle/01-usage-guide.md)

记忆写入、召回、生命周期 Gap 模块：

- `MemoryCoordinator` 已出现，但 write / recall 仍半闭环
- write policy / quality gate / recall usefulness 仍未 fully 进入真实主链
- `memory_capture / recall / write` workflows 仍是 `partial`

### [harness-eval-and-replay](./harness-eval-and-replay/01-usage-guide.md)

Harness 评测与回放 Gap 模块：

- run report / compare / gate 已存在
- `harness_replay`、`harness_eval` 仍未形成正式 workflow
- 治理底座先于产品主链

### [self-evolution-strategy-lifecycle](./self-evolution-strategy-lifecycle/01-usage-guide.md)

自我进化与策略生命周期 Gap 模块：

- candidate / compare / promote / rollback 已存在
- learning 仍主要是治理闭环，不是产品闭环
- 真实自动触发与主链收益尚弱

### [activation-license-lifecycle](./activation-license-lifecycle/01-usage-guide.md)

激活与 license 生命周期 Gap 模块：

- activation overlay 已存在
- remote lifecycle / refresh / revoke / deactivate 仍是 skeleton
- `activation_gate` 仍未 fully canonical

---

## 模块划分原则

1. 一个目录只讨论一类可独立收口的 Gap。
2. 每个模块内部固定三段：
   - `01-usage-guide.md`：这个 gap 为什么重要、谁该读、如何使用
   - `02-implementation.md`：UClaw 对标、If2Ai 证据、代码缺口
   - `03-framework.md`：从系统主链角度解释该 gap 为什么阻断产品闭环
3. 任何后续 exec plan 或 redesign，优先引用具体 Gap 模块，而不是抽象地说“M1/M2/M3 继续推进”。

---

## 关联真相文档

- [If2Ai Workflow Truth Registry](../if2ai-workflow-truth.md)
- [UClaw 对照迁移报告](../uclaw-if2ai-architecture-migration-report.md)
- [Staff Remediation README](../README.md)
