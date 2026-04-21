# Phase M3 Frontend Reflection And Tests File-Level Plan

> 将 `M3` 的 `m3.6 + m3.7 + m3.8` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m3.6` Project memory decisions into frontend stores
2. `m3.7` Introduce reflection feedback seam
3. `m3.8` Add memory regression and dirty data tests

目标是让 memory 的解释性真正穿透到前端表面，同时为后续 self-evolution 闭环埋下 reflection 与 regression 的正式接缝。

## 2. 当前事实基线

> **2026-04-20 audit fix**：`src/stores/memory-slice.ts` 已在
> M2 audit 阶段被删除（M2 audit decision，见
> `src/stores/README.md`）。新主路径走
> `src/runtime-projection/runtime-projection-store.ts` 的
> `snapshot.memory.recentEvents` + `snapshot.memory.writeDecisions`。
> 本计划下文中所有"修改 `memory-slice.ts`"的指引应理解为
> "修改 `runtime-projection` 投影层 + 对应 hook"。

### 2.1 前端 memory projection 已覆盖事件层、待覆盖决策层

当前 `src/runtime-projection/runtime-projection-store.ts` 已承载：

1. `snapshot.memory.recentEvents` — 后端 `memory_event` 通道滚动 ring
2. `snapshot.memory.lastRecallItems` — `stream_complete` 召回评据
3. `snapshot.memory.writeDecisions` — Phase M3-B 已建 typed projection
   字段 + `memory_write_decision` Tauri 事件源（rolling ring，cap 32）

permission prompt 走 `snapshot.approvals[sessionId]`（M2.8 已切）。

还没有正式区分（M3.6 R2 需要的 UI 维度）：

1. candidate
2. approved
3. recalled
4. pinned
5. compiled
6. reflection note

### 2.2 reflection 现状仍偏 learning 内部对象

当前 [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1) 的 `Reflection` 结构主要是：

1. `pattern`
2. `insight`
3. `confidence`
4. `source_session`

还不是 blueprint 需要的：

1. `issue_type`
2. `evidence`
3. `proposed_strategy`
4. `expected_gain`
5. `risk_level`

### 2.3 测试现状是“零散已有，缺平台回归”

当前 memory 各子模块已有不少单测，但还缺面向 `M3` 平台能力的回归保护，尤其缺：

1. object model regression
2. recall order regression
3. dirty / ambiguous candidate tests
4. reflection seam tests

## 3. 实施原则

1. 前端只投影 memory truth，不自己造 reason。
2. reflection 先形成 candidate seam，不直接改 active strategy。
3. tests 优先保护主链路，不追求一次性覆盖所有 legacy 模块。
4. 这一轮不做 harness compare，那是 `M4/M5` 的事。

## 4. 严格执行顺序

1. `R0` preflight inventory
2. `R1` memory frontend projection 扩展
3. `R2` chat/memory UI 接线
4. `R3` reflection note contract
5. `R4` reflection seam integration
6. `R5` object model / policy / recall tests
7. `R6` dirty candidate / reflection tests
8. `R7` compile + test verification

禁止并行：

1. 前端 memory store 重构
2. reflection 结构重构
3. memory regression 测试补齐

因为这三项一起动时，最容易出现“结构变了，但前端和测试各自按旧语义理解”的错位。

## 5. 文件级实施方案

## 5.1 `R0` Preflight Inventory

### 必查文件

- [src/runtime-projection/runtime-projection-store.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-store.ts:1)
- [src/runtime-projection/runtime-projection-bridge.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-bridge.ts:1)
- [src/runtime-projection/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/types.ts:1)
- [src/components/memory/MemoryChip.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryChip.tsx:1)
- [src/components/memory/MemoryWriteCard.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryWriteCard.tsx:1)
- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)
- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)

### 必做动作

1. 列出当前前端 memory UI 已消费哪些后端字段。
2. 列出 reflection 事件当前是否已经进入 harness event bus。
3. 标出缺少 regression 的核心路径。

## 5.2 `R1` memory frontend projection 扩展

### 新增文件

（无；M3-B 已在 `runtime-projection` 内完成 `MemoryWriteDecisionEvent`
+ `snapshot.memory.writeDecisions` 投影。剩余产品层视图组件可
后续按需新增于 `src/components/memory/`。）

### 修改文件

- [src/runtime-projection/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/types.ts:1)
  （已建 `MemoryWriteDecisionEvent` / `MemoryWriteDecisionProjection`）
- [src/runtime-projection/runtime-event-reducer.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-event-reducer.ts:1)
  （已建 `memory_write_decision` reducer 分支）
- [src/runtime-projection/runtime-projection-bridge.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-bridge.ts:1)
  （已建 `listenMemoryWriteDecision` dispatch）
- [src/transport/contracts.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/transport/contracts.ts:1)

### 第一版必须投影的对象

1. write candidates
2. write decisions
3. recalled objects
4. pinned/compiled categories
5. reflection notes summary

### 明确约束

> **2026-04-20 audit fix**：M2 audit 阶段已删除
> `src/stores/memory-slice.ts`（零 in-tree consumer），并在
> `src/stores/README.md` 文档化决议。memory 主路径**唯一 SoT**
> 是 `src/runtime-projection/runtime-projection-store.ts`；`src/state/`
> 目录今天**未建**——M3.6 不引入新 store 目录，所有 memory 投影
> 字段都加在 `runtime-projection-store.snapshot.memory.*`。

## 5.3 `R2` chat/memory UI 接线

### 修改文件

- [src/components/memory/MemoryChip.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryChip.tsx:1)
- [src/components/memory/MemoryWriteCard.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryWriteCard.tsx:1)
- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)

### 必须落实

1. UI 能区分 candidate / approved / recalled / pinned / compiled。
2. 不把所有 memory 事件都压平成一个“写入成功/失败”日志。
3. 至少保留一处可见表面解释“为什么召回”和“为什么拒写”。

## 5.4 `R3` 建 reflection note contract

### 新增文件

- `src-tauri/src/modules/learning/reflection_note.rs`

### 修改文件

- [src-tauri/src/modules/learning/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)

### 第一版建议结构

1. `ReflectionNote`
2. `ReflectionIssueType`
3. `ReflectionEvidenceRef`
4. `StrategyProposal`

### 必须包含字段

1. `issue_type`
2. `evidence`
3. `proposed_strategy`
4. `expected_gain`
5. `risk_level`

### 当前兼容要求

旧 `Reflection` 可以先保留，但新的 coordinator / harness / frontend seam 应优先围绕 `ReflectionNote`。

## 5.5 `R4` reflection seam integration

### 修改文件

- [src-tauri/src/modules/learning/reflection.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/reflection.rs:1)
- [src-tauri/src/modules/memory/coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/coordinator.rs:1)
- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
- [src-tauri/src/modules/harness/agent_loop_integration.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/agent_loop_integration.rs:1)

### 必须落实

1. reflection 输出正式变成 `ReflectionNote` 或等价结构。
2. memory coordinator 的 `after_turn` 可以产出 reflection candidates。
3. harness event bus 至少能接收 reflection completed / candidate surfaced 事件。

### 刻意不在这一步做的事

1. 不直接把 reflection candidate promote 成 active strategy。
2. 不在这一步接 compare / rollback。

## 5.6 `R5` object model / policy / recall tests

### 新增或修改测试文件

- `src-tauri/src/modules/memory/object_model.rs` 同文件单测
- `src-tauri/src/modules/memory/write_policy.rs` 同文件单测
- `src-tauri/src/modules/memory/recall_assembler.rs` 同文件单测

### 必须覆盖

1. object type classification
2. write decision reasons
3. recall order
4. scope-sensitive recall

## 5.7 `R6` dirty candidate / reflection tests

### 新增或修改测试文件

- `src-tauri/src/modules/memory/quality_gate.rs` 同文件单测
- `src-tauri/src/modules/memory/conflict_resolution.rs` 同文件单测
- `src-tauri/src/modules/learning/reflection_note.rs` 同文件单测
- 视需要可补到 [src-tauri/src/integration_tests.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/integration_tests.rs:1)

### 必须覆盖

1. ambiguous candidate rejection
2. missing evidence rejection/prompt
3. conflicting fact resolution
4. reflection note shape stability

## 5.8 `R7` Compile + Test Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml memory`

### 如测试过滤器不足，至少补跑相关模块测试

### 必做人工检查

1. 前端 memory UI 读到的是 typed decision/result，不是随手拼的字符串。
2. reflection 已成为正式结构化输出。
3. 新测试至少覆盖一条 dirty candidate 拒绝路径。

## 6. 完成定义

只有当 reviewer 可以明确说出“前端已经能投影 memory 的决策过程，而且 reflection 已经形成 candidate seam 并有回归保护”时，这一段才算完成。
