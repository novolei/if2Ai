# Phase M3 Object Model And Coordinator File-Level Plan

> 将 `M3` 的 `m3.1 + m3.2` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m3.1` Define memory object model in code
2. `m3.2` Introduce memory coordinator boundary

目标是先把 If2Ai 的 memory 从“各种模块凑起来的能力集合”收口成一套正式对象层和流程协调层。

## 2. 当前事实基线

### 2.1 当前 memory 主体仍是 `MemoryEntry`

当前 [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1) 暴露的基础对象仍然主要是：

1. `MemoryEntry`
2. `MemoryCategory`
3. `MemoryProvider`

这套模型足够做存储，但还不足以表达：

1. stable fact
2. user preference
3. strategy candidate
4. episode summary

### 2.2 当前主链路仍是 scattered memory usage

memory 当前真实使用点分散在：

1. [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)
2. [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1)
3. [src-tauri/src/modules/memory/policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/policy.rs:1)
4. [src-tauri/src/modules/memory/ticker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/ticker.rs:1)
5. [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

也就是说，还没有一个正式的 `prepare_context / after_turn` 主叙事。

### 2.3 learning 模块已存在，但尚未形成 memory seam

当前 learning 已有：

1. [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
2. [src-tauri/src/modules/learning/trajectory.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/trajectory.rs:1)
3. [src-tauri/src/modules/learning/self_model.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/self_model.rs:1)

但 reflection 还没有以正式 memory object / candidate seam 的形式接回 memory。

## 3. 实施原则

1. 先建 object model，再建 coordinator。
2. coordinator 是流程协调器，不是新一代 `agent.rs`。
3. 第一轮允许内部复用现有 `MemoryEntry`/`MemoryProvider`，但上层 API 必须显式 typed。
4. 这一轮不做策略推广，不碰 `M5` 的 promotion logic。

## 4. 严格执行顺序

1. `O0` preflight memory inventory
2. `O1` 建 memory object model
3. `O2` 建 classification / mapping helpers
4. `O3` 建 MemoryCoordinator skeleton
5. `O4` 接 `prepare_context`
6. `O5` 接 `after_turn`
7. `O6` 回写现有调用点
8. `O7` compile + manual verification

禁止并行：

1. object model 引入
2. coordinator 引入
3. write policy 深改

因为这三者一起动时，最容易让类型层和流程层同时漂移。

## 5. 文件级实施方案

## 5.1 `O0` Preflight Memory Inventory

### 必查文件

- [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)
- [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)
- [src-tauri/src/modules/memory/ticker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/ticker.rs:1)
- [src-tauri/src/modules/memory/compiler/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/compiler/mod.rs:1)
- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)

### 必做动作

1. 盘出当前哪些路径仍直接操作 `MemoryEntry`。
2. 标出 recall / inject / compile / ticker / reflection 的真实调用点。
3. 标出哪些地方仍把 memory 当纯字符串块处理。

## 5.2 `O1` 建 memory object model

### 新增文件

- `src-tauri/src/modules/memory/object_model.rs`

### 修改文件

- [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)

### 第一版建议对象

1. `MemoryObjectKind`
2. `FactRecord`
3. `PreferenceRecord`
4. `StrategyRecord`
5. `EpisodeRecord`
6. `MemoryEvidenceRef`
7. `MemoryStability`
8. `MemoryScopeDescriptor`

### 最低要求

每类对象必须显式包含或可推导：

1. `scope`
2. `stability`
3. `evidence`
4. `source`
5. `updated_at`

### 这一步不要做的事

1. 不要立刻替换底层 provider schema。
2. 不要强行把所有历史 `MemoryEntry` 完整迁移。
3. 不要在 object model 里直接塞 UI 文案字段。

## 5.3 `O2` 建 classification / mapping helpers

### 新增文件

- `src-tauri/src/modules/memory/object_mapping.rs`

### 修改文件

- [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)

### 目标

建立从现有 `MemoryEntry / MemoryCategory / pinned / compiled / summary / reflection` 到新对象层的映射接缝。

### 最低 API 建议

```rust
pub fn classify_memory_entry(entry: &MemoryEntry) -> MemoryObjectKind
pub fn map_entry_to_memory_object(entry: &MemoryEntry) -> MemoryObjectRecord
```

### 明确要求

1. 允许第一版是启发式映射。
2. 但必须把“映射依据”写成 typed rule，而不是散在调用点里。

## 5.4 `O3` 建 MemoryCoordinator skeleton

### 新增文件

- `src-tauri/src/modules/memory/coordinator.rs`

### 修改文件

- [src-tauri/src/modules/memory/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/mod.rs:1)

### 最低结构建议

```rust
pub struct MemoryCoordinator { ... }

pub struct PrepareContextInput<'a> { ... }
pub struct PrepareContextOutput { ... }

pub struct AfterTurnInput<'a> { ... }
pub struct AfterTurnOutput { ... }
```

### coordinator 第一版职责

1. 作为唯一 memory orchestration entry
2. 编排 retrieval / injection / write candidate collection
3. 对接后续 write policy / quality gate / recall assembler

### 刻意不在这一步做的事

1. 不直接内嵌所有 retrieval 逻辑。
2. 不直接内嵌 compiler/ticker 内部实现。
3. 不把 reflection/promotion 做成 coordinator 内部私逻辑。

## 5.5 `O4` 接 `prepare_context`

### 修改文件

- [src-tauri/src/modules/memory/coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/coordinator.rs:1)
- [src-tauri/src/modules/memory/retrieval.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/retrieval.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)

### 必须落实

`prepare_context` 第一版至少统一返回：

1. recalled objects
2. injection artifacts
3. retrieval diagnostics

### 当前可复用实现

1. `ActiveRetrievalManager`
2. `build_memory_injection`

### 关键要求

这些实现可以继续存在，但调用方不应再直接各自拉它们，而应通过 coordinator 编排。

## 5.6 `O5` 接 `after_turn`

### 修改文件

- [src-tauri/src/modules/memory/coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/coordinator.rs:1)
- [src-tauri/src/modules/memory/ticker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/ticker.rs:1)
- [src-tauri/src/modules/memory/compiler/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/compiler/mod.rs:1)

### after_turn 第一版必须承接

1. write candidates
2. ticker notifications
3. compiler / summary / audit 所需结果对象

### 这一步不要做的事

1. 不在 `after_turn` 里直接做 reflection promotion。
2. 不把 trajectory/harness compare 逻辑混进来。

## 5.7 `O6` 回写现有调用点

### 修改文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- [src-tauri/src/commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:1)

### 必须落实

1. 当前 `agent.rs` 中 memory prepare path 开始调用 `MemoryCoordinator::prepare_context(...)`
2. 当前 turn-complete path 开始调用 `MemoryCoordinator::after_turn(...)`

### 第一阶段允许保留

1. 过渡期兼容 glue
2. 若干旧 helper 仍存在

前提是主叙事已经通过 coordinator 走通。

## 5.8 `O7` Compile + Manual Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`

### 必做人工检查

1. coordinator 不是新 God-file。
2. `agent.rs` 中 memory 主路径已明显降责。
3. 新对象模型能解释 pinned/compiled/recalled/episode 的区别。

## 6. 完成定义

只有当 reviewer 可以明确指出“现在 memory 已经有正式对象层和协调层，而不是 scattered helper 拼起来的行为”，这一段才算完成。
