# Phase 6B + 6E 统一执行序列

> 本文档整合 Phase 6B (Memory Control Plane) 和 Phase 6E (Agent Loop Harness) 的任务依赖关系，提供最优执行顺序。

**文档版本**: 2026-04-14
**状态**: 草案

---

## 背景

### 两个 Phase 的关系

| Phase | 目标 | 核心模块 |
|-------|------|---------|
| **6B** | Memory Control Plane — 存储、检索、学习 | SQLite, Token Budget, Vector Search, HRR, Learning |
| **6E** | Agent Loop Harness — 观测、控制 | EventBus, Telemetry, SessionRecorder, harness-cli |

### 依赖关系矩阵

```
Phase 6B                          Phase 6E
─────────────────────────────────────────────────────────────
6B.1 SQLite P0 ────────────────────→ 6E.4 SessionRecorder
    │                                    (共用 SQLite 连接池)
    │
    └──→ 6B.2 Token Budget ────────→ 6E.2 Telemetry
    │    (Token 统计)                     (Token 统计收集)
    │
    └──→ 6B.3 Active Retrieval
    └──→ 6B.4 Vector Search
    └──→ 6B.5 HRR
    └──→ 6B.6 Self-Learning
    └──→ 6B.7 Trajectory
    └──→ 6B.8 Upstream
              │
              ↓
         6B.9 Final Integration
              │
              ↓
         6E.7 Integration Tests
```

### 关键发现

1. **6E.1 (EventBus)** — 完全独立，可最先开始
2. **6B.1 (SQLite P0)** — 创建共享基础设施，6E.4 需要它
3. **6E.4 (SessionRecorder)** — 需要 SQLite，依赖 6B.1 完成
4. **6E 模块** — 可在 6B 功能完成后观察其执行效果

---

## 推荐执行序列

### 阶段 1: 基础设施 (可并行)

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 1 | **6B.1** SQLite P0 + Security | 创建共享 SQLite 连接池 | 无 |
| 2 | **6E.1** EventBus Core | 完全独立，可最先开发 | 无 |

### 阶段 2: 核心模块开发

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 3 | **6E.2** TelemetryCollector | 依赖 6E.1 | 6E.1 |
| 4 | **6E.3** AgentLoopIntegration | 依赖 6E.1 | 6E.1 |
| 5 | **6B.2** Token Budget + WorkingMemory | 依赖 6B.1 | 6B.1 |

### 阶段 3: 数据层完成

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 6 | **6E.4** SessionRecorder | 依赖 6E.1 + SQLite | 6E.1, 6B.1 |
| 7 | **6B.3** Active Retrieval | 可在 6B.1 完成后开发 | 6B.1 |
| 8 | **6B.4** FastEmbed + LanceDB | 可在 6B.1 完成后开发 | 6B.1 |

### 阶段 4: 高级功能

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 9 | **6B.5** HRR | 可在 6B.4 完成后开发 | 6B.4 |
| 10 | **6B.6** Self-Learning | 可在 6B.1 完成后开发 | 6B.1 |
| 11 | **6B.7** Trajectory | 可在 6B.1 完成后开发 | 6B.1 |
| 12 | **6B.8** Upstream Compatibility | 最后开发 | 6B.1~6B.7 |

### 阶段 5: 控制层完成

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 13 | **6E.5** HarnessControl IPC | 依赖 6E.2 + 6E.4 | 6E.2, 6E.4 |
| 14 | **6E.6** harness-cli | 依赖 6E.5 | 6E.5 |

### 阶段 6: 集成测试

| 顺序 | 任务 | 理由 | 依赖 |
|------|------|------|------|
| 15 | **6B.9** Phase 6B Final Integration | 6B 全部完成后 | 6B.1~6B.8 |
| 16 | **6E.7** Phase 6E Integration Tests | 6E 全部完成后 | 6E.1~6E.6 |

---

## 执行甘特图

```
Week 1-2  │ Week 3    │ Week 4    │ Week 5    │ Week 6    │ Week 7-8
──────────┼───────────┼───────────┼───────────┼───────────┼────────────
6B.1 ████ │           │           │           │           │
6E.1 ████ │           │           │           │           │
         │ 6E.2 ███  │           │           │           │
         │ 6E.3 ███  │           │           │           │
         │ 6B.2 ████ │           │           │           │
         │ 6F.1 ████ │           │           │           │  ← 并行
         │ 6F.3 ████ │           │           │           │  ← 并行
         │ 6F.6 ████ │           │           │           │  ← 并行
         │ 6F.8 ███  │           │           │           │  ← 并行
         │           │ 6E.4 ████ │           │           │
         │           │ 6B.3 ████ │           │           │
         │           │ 6B.4 █████ │           │           │
         │           │ 6F.2 ████ │           │           │  (依赖 6F.1)
         │           │ 6F.4 ████ │           │           │  (依赖 6F.3)
         │           │ 6F.5 ███  │           │           │  (依赖 6F.3)
         │           │           │ 6B.5 ████ │           │
         │           │           │ 6B.6 ████ │           │
         │           │           │ 6B.7 ████ │           │
         │           │           │ 6F.7 ███  │           │  (依赖 6F.2)
         │           │           │           │ 6B.8 ███  │
         │           │           │           │ 6E.5 ████ │
         │           │           │           │ 6E.6 ████ │
         │           │           │           │ 6F.9 ████ │  (依赖 6F.1,2,5)
         │           │           │           │           │ 6B.9 ████
         │           │           │           │           │ 6E.7 ████
         │           │           │           │           │ 6F.10 ████
```

---

## 冲突风险评估

| 风险 | 影响 | 缓解策略 |
|------|------|---------|
| SQLite 连接池竞争 | 低 | 使用 `r2d2` 连接池，6E 和 6B 共用 |
| EventBus 事件过多 | 中 | unbounded channel，不阻塞；按需订阅 |
| 模块互相引用过多 | 低 | 6E 只观察 6B，不修改；通过 trait 接口解耦 |
| 并行开发冲突 | 低 | 每个 slice 有独立 `impl_targets`，不重叠 |
| Skill 6F 与 6B/6E 资源竞争 | 低 | 6F 是纯 skill 扩展，与 memory/harness 无依赖 |

**结论**: Phase 6B、6E、6F 无互斥冲突，可安全并行开发。

---

## Phase 6F: Skill Control Plane v2 — Hermes Alignment

### 背景

Phase 6D 已完成 Skills Control Plane v1，实现了基础 skill 发现、加载、review 状态机。
与 Hermes Agent 的 skill 架构对比，存在以下关键 Gap：

| Gap 类别 | Hermes 有 | if2Ai 缺 | 优先级 |
|---------|---------|---------|--------|
| **安全扫描** | 15+ 威胁类别，trust-level 感知策略 | 5 个简单 pattern | **P0** |
| **自主 CRUD** | create/edit/patch/delete/write_file/remove_file | 仅 create | **P0** |
| **Multi-Source Hub** | 7 个 adapter (GitHub, skills.sh, ClawHub, etc.) | 仅 skills.sh | **P1** |
| **Hub 状态** | quarantine/audit.log/lock.json/taps | 无 | **P1** |
| **Skill Sync** | manifest hash 追踪 + 用户修改检测 | 无 | **P1** |
| **Slash 命令** | /skill-name 直接调用 + 预加载 | 静态列表 | **P2** |
| **Config 变量** | metadata.hermes.config 解析 | 无 | **P2** |
| **Supporting Files** | references/templates/scripts/assets | 无 | **P2** |

### 设计文档

- **Design-ref**: [docs/design-docs/postCLI/Skill-Control-Plane-v2.md](../../design-docs/postCLI/Skill-Control-Plane-v2.md)
- **Backlog**: [docs/design-docs/postCLI/backlog/Skill-Control-Plane-v2-BACKLOG.md](../../design-docs/postCLI/backlog/Skill-Control-Plane-v2-BACKLOG.md)
- **Exec-plan**: [phase-6f-skill-control-plane-v2.yaml](./phase-6f-skill-control-plane-v2.yaml)

### Slice 概览

| Slice | 名称 | 优先级 | 依赖 | 工作量 |
|-------|------|--------|------|--------|
| **6F.1** | SkillsGuard Threat Scanner | P0 | 无 | ~800 行 |
| **6F.2** | SkillManager CRUD | P0 | 6F.1 | ~600 行 |
| **6F.3** | Hub Source Framework | P1 | 无 | ~500 行 |
| **6F.4** | Hub Source Implementations | P1 | 6F.3 | ~800 行 |
| **6F.5** | Hub State Management | P1 | 6F.3 | ~300 行 |
| **6F.6** | Skill Sync Manifest | P1 | 无 | ~400 行 |
| **6F.7** | Skill Commands | P2 | 6F.2 | ~300 行 |
| **6F.8** | Skill Config Variables | P2 | 无 | ~200 行 |
| **6F.9** | Frontend UI Extensions | P2 | 6F.1, 6F.2, 6F.5 | ~1500 行 |
| **6F.10** | Integration Tests | P1 | 全部 | ~500 行 |

### 推荐执行顺序

| 阶段 | Slice | 理由 |
|------|-------|------|
| **阶段 1** | 6F.1, 6F.3, 6F.6, 6F.8 | 独立无依赖，可并行 |
| **阶段 2** | 6F.2 | 依赖 6F.1 |
| **阶段 3** | 6F.4, 6F.5 | 依赖 6F.3 |
| **阶段 4** | 6F.7 | 依赖 6F.2 |
| **阶段 5** | 6F.9 | 依赖 6F.1, 6F.2, 6F.5 |
| **阶段 6** | 6F.10 | 依赖全部 |

### Hermes 代码引用

| Hermes 文件 | 核心功能 | 对应 Slice |
|------------|---------|-----------|
| `tools/skills_guard.py` | 15+ 威胁类别，安全策略 | 6F.1 |
| `tools/skill_manager_tool.py` | 完整 CRUD，安全扫描 | 6F.2 |
| `tools/skills_hub.py` | SkillSource trait, GitHubSource, HubState | 6F.3, 6F.4, 6F.5 |
| `tools/skills_sync.py` | manifest hash 追踪 | 6F.6 |
| `agent/skill_commands.py` | /skill-name 调用，预加载 | 6F.7 |
| `agent/skill_utils.py` | config 变量解析 | 6F.8 |

### 与 6B/6E 的关系

- **无依赖冲突**: 6F 是 skill 系统的扩展，与 6B (Memory) 和 6E (Harness) 完全正交
- **可并行开发**: 6F 的 slices 可与 6B/6E slices 并行执行
- **共享基础设施**: Skills 使用的审计/上下文与 6D 共享

---

## SQLite 共享策略

### 推荐：共用同一数据库

```sql
-- ~/.if2ai/if2ai.db (单一数据库)

-- Memory 模块 (6B)
CREATE TABLE memory_entries (...);
CREATE TABLE semantic_index (...);
CREATE TABLE episodic_index (...);

-- Harness 模块 (6E)
CREATE TABLE harness_recordings (...);
CREATE TABLE harness_events (...);
```

**优势**:
- 单一连接池，`r2d2::Pool` 管理
- 事务一致性
- 简化部署

---

## 关键里程碑

| 里程碑 | 任务 | 完成标准 |
|--------|------|---------|
| **M1** | 6B.1 + 6E.1 完成 | SQLite 持久化 + EventBus 可用 |
| **M2** | 6E.2 + 6E.3 + 6B.2 完成 | Telemetry 可收集 + Agent Loop 可埋点 |
| **M3** | 6E.4 + 6B.3~6B.4 完成 | SessionRecorder 可用 + Vector Search 可观察 |
| **M4** | 6B.5~6B.8 完成 | Memory Control Plane 全部功能 |
| **M5** | 6E.5~6E.6 完成 | harness-cli 可用 |
| **M6** | 6B.9 + 6E.7 完成 | 全部集成测试通过 |
| **M7** | 6F.1 + 6F.2 完成 | SkillsGuard 可用 + SkillManager CRUD 就绪 |
| **M8** | 6F.3 + 6F.4 + 6F.5 完成 | Hub Multi-Source 就绪 |
| **M9** | 6F.6~6F.8 完成 | Skill Sync + Commands + Config 就绪 |
| **M10** | 6F.9 + 6F.10 完成 | Frontend UI 完整 + 集成测试通过 |

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-04-14 | v0.1 | 初始版本，整合 6B + 6E 执行序列 |
| 2026-04-14 | v0.2 | 新增 Phase 6F (Skill Control Plane v2)，完整对齐 Hermes skill 架构 |
