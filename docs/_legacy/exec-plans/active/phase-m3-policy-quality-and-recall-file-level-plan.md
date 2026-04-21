# Phase M3 Policy Quality And Recall File-Level Plan

> 将 `M3` 的 `m3.3 + m3.4 + m3.5` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m3.3` Introduce write policy and typed decision model
2. `m3.4` Introduce quality gate and conflict handling
3. `m3.5` Introduce recall assembler and injection order

目标是把 memory 的“能不能写、写什么、怎么召回”从局部规则收口为 typed decision pipeline。

## 2. 当前事实基线

### 2.1 write policy 已有雏形，但语义不够完整

当前 [src-tauri/src/modules/memory/policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/policy.rs:1) 已经有：

1. `PolicyDecision`
2. `ReasonCode`
3. `MemoryPolicyEngine`

但它当前主要围绕：

1. content length
2. denied category
3. prompt threshold

还没有完整表达 blueprint 要求的：

1. memory type
2. scope completeness
3. evidence completeness
4. conflict handling

### 2.2 recall 现状仍偏 provider-centric

当前 [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1) 的 `ActiveRetrievalManager` 仍然主要做：

1. query intent classify
2. weighted fusion
3. context string assemble

这还不是正式的 `RecallAssembler`。

### 2.3 memory inject 已经有顺序，但不是 canonical recall order

当前 [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1) 主要体现的是：

1. pinned first
2. compiled second
3. rules section always included

这离 blueprint 里的：

`rules -> pinned -> critical facts -> preferences -> compiled -> episodes`

还差正式的对象排序层。

## 3. 实施原则

1. write policy 先 typed 化，再扩规则。
2. quality gate 和 write policy 分层，不要写成一个大文件。
3. recall assembler 是 object-aware assembler，不是字符串拼接 helper。
4. 这一步不改变长期策略推广，只解决 memory write/recall 主链可解释性。

## 4. 严格执行顺序

1. `P0` preflight inventory
2. `P1` typed write decision model
3. `P2` write policy boundary refactor
4. `P3` quality gate
5. `P4` conflict resolution rules
6. `P5` recall assembler
7. `P6` coordinator integration
8. `P7` compile + manual verification

禁止并行：

1. policy engine 深改
2. retrieval engine 深改
3. injection order 深改

因为这三项一起动时，最容易让 recall 行为和 write 行为同时失去基线。

## 5. 文件级实施方案

## 5.1 `P0` Preflight Inventory

### 必查文件

- [src-tauri/src/modules/memory/policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/policy.rs:1)
- [src-tauri/src/modules/tools/builtin/memory_store.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/builtin/memory_store.rs:1)
- [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)
- [src-tauri/src/modules/memory/audit.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/audit.rs:1)

### 必做动作

1. 列出当前所有 write decision 输出字段。
2. 列出 memory_store 工具当前如何构造 policy engine。
3. 列出 retrieval 和 inject 现在分别在哪一层做排序。

## 5.2 `P1` 建 typed write decision model

### 新增文件

- `src-tauri/src/modules/memory/write_decision.rs`

### 修改文件

- [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)
- [src-tauri/src/modules/memory/policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/policy.rs:1)

### 第一版建议结构

1. `MemoryWriteDecision`
2. `MemoryWriteDisposition`
3. `MemoryDecisionReasonCode`
4. `MemoryCandidateDescriptor`

### 最低要求

必须显式包含：

1. `allow / deny / prompt`
2. `memory_type`
3. `scope`
4. `evidence_id`
5. `reason_code`

### 当前兼容要求

旧 `PolicyDecision` / `ReasonCode` 可以先保留，但新主路径应开始返回 `MemoryWriteDecision`。

## 5.3 `P2` write policy boundary refactor

### 新增文件

- `src-tauri/src/modules/memory/write_policy.rs`

### 修改文件

- [src-tauri/src/modules/memory/policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/policy.rs:1)
- [src-tauri/src/modules/tools/builtin/memory_store.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/builtin/memory_store.rs:1)

### 目标

把当前 `MemoryPolicyEngine` 收成更清晰的边界：

1. legacy engine 可留
2. 新 coordinator 主路径走 `write_policy.rs`

### 第一批必须纳入的判定

1. content size
2. denied category
3. scope missing
4. evidence missing
5. sensitive content / threat scan result

### 关键要求

`memory_store` 工具不能因为 `M3` 重构而失效；必要时通过 adapter 兼容。

## 5.4 `P3` 建 quality gate

### 新增文件

- `src-tauri/src/modules/memory/quality_gate.rs`

### 修改文件

- [src-tauri/src/modules/memory/coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/coordinator.rs:1)

### quality gate 第一版必须检查

1. duplicate candidate
2. conflicting candidate
3. ambiguous candidate
4. weak evidence candidate

### 最低结构建议

```rust
pub struct QualityGateResult {
    pub accepted: Vec<...>,
    pub rejected: Vec<...>,
    pub warnings: Vec<...>,
}
```

### 不要做的事

1. 不要把 conflict resolution 写死在 coordinator。
2. 不要把 audit emission 写进 gate 本身。

## 5.5 `P4` 建 conflict resolution rules

### 新增文件

- `src-tauri/src/modules/memory/conflict_resolution.rs`

### 修改文件

- `src-tauri/src/modules/memory/quality_gate.rs`

### 第一批必须覆盖

1. same fact, different value
2. same preference, different polarity
3. global vs project/session scope collision
4. old stable fact vs new weak evidence fact

### 处理结果至少要区分

1. accept replacement
2. keep existing
3. require prompt
4. reject candidate

## 5.6 `P5` 建 recall assembler

### 新增文件

- `src-tauri/src/modules/memory/recall_assembler.rs`

### 修改文件

- [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)

### recall assembler 第一版必须产出

1. ordered recalled objects
2. injection sections
3. usefulness diagnostics skeleton

### 排序顺序必须显式固化

1. rules
2. pinned
3. critical facts
4. preferences
5. compiled
6. episodes

### 当前可复用实现

1. `ActiveRetrievalManager`
2. `build_memory_injection`

但这些实现要变成 assembler 的下游，不再各自直连调用点。

## 5.7 `P6` coordinator integration

### 修改文件

- [src-tauri/src/modules/memory/coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/coordinator.rs:1)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

### 必须落实

1. `prepare_context` 通过 `RecallAssembler` 产出 recall 结果。
2. `after_turn` 通过 `WritePolicy + QualityGate` 产出 write decision。

## 5.8 `P7` Compile + Manual Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`

### 必做人工检查

1. memory_store 工具仍能工作。
2. recall 顺序可读、可解释。
3. 新 write decision 至少能解释 allow/deny/prompt 的原因。

## 6. 完成定义

只有当 reviewer 可以明确回答“为什么这条 memory 被允许/拒绝，以及为什么这几类 memory 会按这个顺序召回”时，这一段才算完成。
