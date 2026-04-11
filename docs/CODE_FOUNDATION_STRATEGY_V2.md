# If2Ai 源码基础战略 - 单一源码真相模型 (v2)

**版本**: 2.0 (修订)  
**更新日期**: 2026-04-11  
**核心理念**: 单一源码真相 (Single Source of Truth)  
**目标**: 消除"两个版本"陷阱，只有一个唯一的工作库  

---

## 概览

### 核心决策

我们采用**单一源码真相模型**：

```
/rust
  │ (源码来源)
  ├── crates/ 中的源码 → 迁移到 src-tauri
  │
  └── [保留用于参考和历史存档]

       ↓ 迁移完成后

src-tauri (唯一工作库 + 唯一真相)
  ├── 包含所有迁移来的源码
  ├── 所有开发都在这里进行
  ├── 所有更新都在这里
  └── 这是生产代码库

docs/design-docs/
  └── 指导和验证 src-tauri 的实现
```

---

## 1. 现状分析

### 1.1 /rust 的角色

```
/rust/
├── Cargo.toml (workspace)
├── crates/
│   ├── api/              完整的 LLM 提供商实现
│   ├── runtime/          Agent 循环的引擎
│   ├── tools/            工具系统框架
│   ├── commands/         Tauri 命令处理
│   ├── plugins/          插件系统
│   └── ... (10+ crates)
└── README.md

现状: 完整的源码库，包含所有实现

用途: 源码来源 (迁移源)
  - 提供初始的、经过验证的源码
  - 迁移完成后，不再是持续开发库
  - 可保留作为历史参考或存档
```

### 1.2 src-tauri 的现状

```
src-tauri/
├── Cargo.toml           Tauri 配置
├── src/
│   ├── main.rs          入口点
│   ├── commands/        IPC 命令层 (存根)
│   └── modules/         业务逻辑层 (待完善)
└── tauri.conf.json

现状: Tauri 框架容器，待填入核心逻辑

问题: modules/ 下的代码还不完整
```

### 1.3 docs/ 的角色

```
docs/design-docs/
├── system-architecture-framework.md
├── module-boundaries-and-integration.md
├── agent-loop.md
├── provider-resolution.md
├── session-persistence.md
├── memory-system.md
├── messaging-gateway.md
├── agent-self-improvement.md
└── ... (9 份设计文档)

用途: 指导规范
  - 定义 src-tauri 中的代码应该如何组织
  - 验证 src-tauri 中的实现是否符合设计
  - 所有开发的单一真理来源
```

---

## 2. 战略决策: 单一源码真相结构

### 2.1 架构图

```
if2Ai/
│
├── rust/                         源码来源
│   ├── Cargo.toml (workspace)
│   ├── crates/
│   │   ├── api/                 ← 迁移
│   │   ├── runtime/             ← 迁移
│   │   ├── tools/               ← 迁移
│   │   ├── commands/            ← 迁移
│   │   ├── plugins/             ← 迁移
│   │   └── ...
│   └── [保留存档]
│
├── src-tauri/                    唯一工作库 ⭐
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── app_state.rs          # AppState 全局容器
│   │   ├── commands/             # Tauri IPC 网关
│   │   │   ├── agent.rs
│   │   │   ├── tools.rs
│   │   │   ├── session.rs
│   │   │   ├── memory.rs
│   │   │   └── mod.rs
│   │   │
│   │   └── modules/              # 业务逻辑 (迁移来的源码)
│   │       ├── agent/
│   │       │   ├── conversation.rs    (from /rust/crates/runtime)
│   │       │   ├── prompt.rs          (from /rust/crates/runtime)
│   │       │   └── mod.rs
│   │       ├── provider/
│   │       │   ├── client.rs          (from /rust/crates/api)
│   │       │   ├── routing.rs
│   │       │   ├── providers/         (from /rust/crates/api)
│   │       │   └── mod.rs
│   │       ├── tools/
│   │       │   ├── executor.rs        (from /rust/crates/tools)
│   │       │   ├── registry.rs
│   │       │   └── mod.rs
│   │       ├── session/
│   │       │   ├── storage.rs
│   │       │   ├── manager.rs
│   │       │   └── mod.rs
│   │       ├── memory/
│   │       │   ├── memory.rs
│   │       │   ├── user.rs
│   │       │   └── mod.rs
│   │       └── plugin/
│   │           ├── manager.rs
│   │           ├── loader.rs
│   │           └── mod.rs
│   │
│   └── 唯一的真实版本 → 持续开发！
│
└── docs/design-docs/             设计规范
    ├── agent-loop.md
    ├── provider-resolution.md
    ├── session-persistence.md
    ├── memory-system.md
    ├── messaging-gateway.md
    ├── agent-self-improvement.md
    └── ... (9 份)
```

### 2.2 单一源码真相的含义

**只有一个地方存储和开发代码**：

```
✅ 所有源码来自: /rust (迁移源)
✅ 所有开发进行: src-tauri (唯一工作库)
❌ 不存在: 两个版本并行维护的情况
❌ 不存在: 同步问题或版本不一致
```

**优势**:

| 特性 | 单一源码真相 | 双库并行 |
|-----|---------|--------|
| 同步问题 | ✅ 无 | ⚠️ 很多 |
| 维护复杂度 | ✅ 低 | ⚠️ 高 |
| 新人入门 | ✅ 一个库 | ⚠️ 两个库 |
| 修复 Bug | ✅ 一处改 | ⚠️ 两处都改 |
| 长期可维护 | ✅ 好 | ⚠️ 难 |

### 2.3 核心原则

#### 原则 1: /rust 是源码来源，非开发库

```
迁移前 (现在):
  /rust 中的源码 → 最好的参考实现

迁移后 (目标):
  src-tauri 中的源码 → 唯一的工作实现
  /rust → 可选存档或参考
```

#### 原则 2: 所有开发都在 src-tauri 中进行

```
新需求    → 在 src-tauri 中实现
Bug 修复  → 在 src-tauri 中修复
性能优化  → 在 src-tauri 中优化
没有向 /rust 反向同步的需要
没有"两个版本"需要维护一致
```

#### 原则 3: 设计文档是规范和验证

```
src-tauri 中的代码设计必须符合 docs/design-docs
遇到代码与设计的冲突时:
  ① 先质疑是否误解设计
  ② 如果设计有缺陷，更新设计文档
  ③ 然后让代码符合更新的设计

这样保证代码永远"言之有理"
```

#### 原则 4: 没有"两个真实版本"

```
❌ 不能出现的情况:
   "这个功能应该在哪里实现？/rust 还是 src-tauri?"
   这个问题说明架构有问题

✅ 应该的回答:
   "在 src-tauri 中实现，参考 /rust 的模式"
   这个答案说明架构清晰
```

---

## 3. 与设计文档的一致性

### 3.1 一致性验证

所有 9 份设计文档都针对 **src-tauri** 中的架构设计，而不是 /rust：

```
设计文档                    对应的 src-tauri 模块
────────────────────────────────────────────
agent-loop.md           → modules/agent/
provider-resolution.md  → modules/provider/
session-persistence.md  → modules/session/
tool-system.md          → modules/tools/
memory-system.md        → modules/memory/
messaging-gateway.md    → modules/gateway/ (new)
agent-self-improvement  → modules/reinforcement/ (new)
error-handling.md       → 跨模块错误处理
testing-strategy.md     → 测试框架
```

**结论**: ✅ 0 冲突，完全对齐

### 3.2 模块依赖关系

```
modules/
├── agent/           (Core)
│   └─ deps: provider, tools, session, memory
├── provider/        (Tier 1)
│   └─ deps: none
├── tools/           (Tier 1)
│   └─ deps: none
├── session/         (Tier 1)
│   └─ deps: none
├── memory/          (Tier 2)
│   └─ deps: session
└── plugin/          (Tier 1)
    └─ deps: tools

清晰的分层，无循环依赖！
```

---

## 4. 迁移计划

### 4.1 迁移流程

迁移是**一次性的、单向的**操作：

```
/rust 源码
  ↓ Step 1-7: 逐个模块迁移
  ├─ Agent (runtime) → modules/agent/
  ├─ Provider (api) → modules/provider/
  ├─ Tools → modules/tools/
  ├─ Session (commands) → modules/session/
  ├─ Memory → modules/memory/
  ├─ Plugin → modules/plugin/
  └─ Other utilities
  ↓ Step 8: AppState 集成
  ├─ 创建 app_state.rs
  └─ 初始化所有子系统
  ↓ Step 9: Commands 网关
  ├─ agent.rs, tools.rs, session.rs, memory.rs
  └─ 连接 Tauri IPC 和 AppState
  ↓ Step 10: 测试验证
  ├─ cargo build --release
  ├─ cargo test
  └─ cargo tauri dev

src-tauri (完整工作库)
  → 后续所有开发都在这里
```

### 4.2 预期耗时

- **总耗时**: 8-10 小时（不中断）
- **并行模式**: 4-5 小时（2-3 人）
- **每个模块**: 1-1.5 小时
- **检查点**: 每 2 个模块后运行 cargo test

### 4.3 回滚计划

```bash
# 如果迁移失败，快速回滚
git reset --hard pre-migration-20260411

# 分析原因
# 修正配置
# 重新开始
```

---

## 5. 维护规则 (长期)

### 5.1 代码更新流程

```
需求/Bug/改进
  ↓
① 检查 docs/design-docs 中是否有相关设计
② 如果有，按照设计实现在 src-tauri 中
③ 如果无，先更新设计文档，再实现
④ 实现后运行测试和代码审查
⑤ 合并到 main 分支
```

### 5.2 架构一致性维持

```
月度: 检查设计文档是否反映当前 src-tauri 实现
季度: 检查 /rust 中是否有重要改进应该搬运过来
年度: 完整的架构审计
```

### 5.3 /rust 的处理

```
迁移完成后，/rust 可以:
① 保留作为"历史存档"
② 用作快速查阅设计思路
③ 新的 Hermes 版本发布时，参考迁移
④ 或者根据团队决策，归档到冷存储

核心: /rust 不再是开发库，src-tauri 是唯一活跃库
```

---

## 6. 与概念对齐

### 6.1 "Single Source of Truth" 原则

在软件架构中，**单一源码真相** (Single Source of Truth, SSOT) 是指：

```
对于任何信息，只存在一个权威的、可信的来源
在我们的情况下：src-tauri 是代码的唯一来源
```

**优势**:
- 消除不一致性
- 降低维护成本
- 提高代码质量
- 团队对齐清晰

### 6.2 与"参考实现"模式的区别

```
参考实现模式 (Reference Implementation):
  code-v1 (参考)  ←→  code-v2 (实现)  (容易不同步)

单一源码真相 (Single Source of Truth):
  源码 (迁移) → 工作库 (唯一) → 持续开发 (清晰)
```

---

## 7. 成功标志

### 迁移完成后

- ✅ `cargo build --release` 成功
- ✅ `cargo test` 覆盖率 ≥ 80%
- ✅ `cargo tauri dev` 能启动
- ✅ AppState 能初始化所有模块
- ✅ 所有 Tauri Commands 能调用
- ✅ 代码审查通过

### 后续开发中

- ✅ 所有新代码都在 src-tauri 中
- ✅ 没有人问"这个应该在 /rust 还是 src-tauri"
- ✅ /rust 基本不动（可选存档）
- ✅ 设计文档与代码保持同步
- ✅ 没有"两个版本"的问题

---

## 8. 常见问题

**Q: 迁移后 /rust 会被删除吗?**
A: 不会。/rust 可以：
- 保留作为历史参考
- 新的 Hermes 版本发布时参考迁移
- 或者根据团队决策归档

**Q: 原来的"参考库"作用谁来替代?**
A: 由 git commit history + docs/design-docs 替代：
- git log 显示代码演变
- 设计文档解释"为什么"这样做

**Q: 会不会有"两个版本分支"?**
A: 不会。只有 src-tauri 是活跃库：
- /rust 迁移完后基本冻结
- 所有开发都在 src-tauri
- 不需要同步或对比

**Q: 紧急情况下参考 /rust 怎么办?**
A: 可以：
- git log -p /rust -- (查看历史实现)
- 快速检查某个实现思路
- 参考但不同步到 /rust

---

## 总结

### 核心决策

| 方面 | 决策 |
|------|------|
| 唯一源码库 | src-tauri |
| 源码来源 | /rust (迁移后可存档) |
| 规范指导 | docs/design-docs |
| 开发方式 | 只在 src-tauri 中 |
| 参考方式 | git history + 设计文档 |

### 优势总结

1. ✅ **清晰** - 只有一个真相，没有混淆
2. ✅ **高效** - 不需要维护两个版本的同步
3. ✅ **可维护** - 长期看代码库会更干净
4. ✅ **对齐** - 团队目标和行动清晰一致

### 后续行动

1. 阅读本文档 (15 min)
2. 执行迁移 (8-10 hours) - 参考 CODE_MIGRATION_EXECUTION.md
3. 验证完成 (1 hour)
4. 开始 Phase 2 开发 (后续周)

---

**版本**: 2.0 (单一源码真相模型)  
**完成日期**: 2026-04-11  
**状态**: ✅ 准备实施

