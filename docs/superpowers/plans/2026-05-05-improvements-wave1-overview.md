# P0 Wave-1 Overview — 2026-05-05

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §7.1
> **流程**：Superpowers SKILLS（见 [`CLAUDE.md`](../../../CLAUDE.md) §Development Workflow）
> **范围**：第一波 6 项 P0 改善点 — 阻塞 vNext 闭环 + 数据安全
> **预计周期**：4 周（2026-05-05 → 2026-06-02）

---

## 1. 目标

完成下列 6 项改善，让 vNext 真值收敛达到「**前端 god-component / 后端 god-file / Harness 二次真相 / Supervisor 收口 / 数据丢失 bug**」全部出清状态。完成后即可启动第二波 P1。

## 2. 任务清单

| ID | 标题 | 类别 | 体量估算 | 依赖 | 状态 |
|----|------|------|----------|------|------|
| ER-01 | 修复 `setActiveModel` `auth_variant` 数据丢失 | 错误设计 | S（半天，1 PR） | — | pending |
| DR-01 | Supervisor 收口为唯一 lifecycle owner | Gap | M（1 周，2-3 PR） | — | pending |
| GF-02 | 拆分 `work_loop.rs`（3,226 → ≤1,000/file） | God-file | L（2 周，4-6 PR） | DR-01 收口稳定 | pending |
| GF-03 | 渐进拆 `App.tsx`（3,310 → ≤500） | God-file | L（2-3 周，5-7 PR） | — | pending |
| GF-01 | 拆 `chat-ui.tsx`（5,043 → ≤500/file） | God-file | XL（3 周，7-9 PR） | DT-03 末次合并 | pending |
| DT-01 | MIG-023：HarnessRunReport 从 canonical run-log 派生 | 二次真相 | XL（3 周，独立 worktree） | — | pending |

## 3. 基线度量（2026-05-05）

锁定 baseline 用于每个 PR 的 Before/After 对照：

| 项 | LOC | 目标 |
|----|-----|------|
| `src/components/ui/chat-ui.tsx` | **5,043** | 拆出 7 个子模块；主组件 ≤ 500 |
| `src/App.tsx` | **3,310** | 抽出 ≥5 个 sub-effect；主体 ≤ 500 |
| `src/lib/tauri.ts` | 3,414 | （第二波 RD-01 处理） |
| `src-tauri/src/modules/application/turn_service/work_loop.rs` | **3,226** | 拆为 mod / skill_resolution / routing / decision；每个 ≤ 1,000 |
| `src-tauri/src/modules/application/turn_service/stream_finalize.rs` | 1,503 | （第二波 GF-05 处理） |
| `src-tauri/src/modules/runtime/conversation.rs` | 1,471 | （第二波 GF-07 处理） |

测试覆盖：
- 后端：`cargo test --lib` 通过基线（执行前 capture 一次）
- 前端：`npm test` 通过基线（执行前 capture 一次）

## 4. 依赖图

```text
          ┌──────────────┐
          │    ER-01     │  (独立，先做)
          └──────────────┘

          ┌──────────────┐
          │    DR-01     │ ─┐
          │  Supervisor  │  │
          │   收口        │  │
          └──────────────┘  │
                            ▼
                     ┌──────────────┐
                     │    GF-02     │
                     │  work_loop   │
                     │   拆分        │
                     └──────────────┘

          ┌──────────────┐
          │    GF-03     │  (独立)
          │   App.tsx    │
          └──────────────┘

          ┌──────────────┐  末次 PR 合并 DT-03 (chat-ui 直读 projection)
          │    GF-01     │
          │  chat-ui     │
          │  拆分        │
          └──────────────┘

          ┌──────────────┐  独立 worktree；最大件
          │    DT-01     │
          │  Harness     │
          │  派生         │
          └──────────────┘
```

并行能力：ER-01 / DR-01 / GF-03 / GF-01 / DT-01 可同时进行；GF-02 等 DR-01 完成后再启动。

## 5. 子流划分

### 子流 A · 解耦（顺序）
1. **DR-01** Supervisor 收口（先做，因 GF-02 涉及 supervisor 调用点）
2. **GF-02** work_loop 拆分

### 子流 B · 前端瘦身（并行）
3. **GF-03** App.tsx 渐进抽 sub-effect
4. **GF-01** chat-ui.tsx 渐进抽子模块；末次 PR 合并 DT-03

### 子流 C · 治理收敛（独立）
5. **DT-01** Harness 派生（建议独立 worktree）

### 立即子任务
6. **ER-01** 数据丢失修复（半天热身，验证 Superpowers 流程）

## 6. 验证规则

每个 PR 必须执行：

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml <相关 filter> --lib
npm test                            # 若涉及前端
npm run build:web                   # 若涉及前端
```

涉及 Tauri 通道或权限的改动须执行 smoke：
- start chat turn → 触发权限 → reload → 完成 turn → reload 再读 history。

## 7. PR 描述模板

```markdown
## 改善 ID
ER-01 / DR-01 / GF-* / DT-01 （引用 docs/IMPROVEMENTS-2026-05-05.md）

## 变更摘要
<1-3 句>

## Before / After
- LOC: <baseline> → <after>
- 关键 invariant: <未变 / 新增>

## 验证
- [ ] cargo fmt --check
- [ ] cargo clippy -D warnings
- [ ] cargo test 相关 filter
- [ ] npm test （若涉及前端）
- [ ] npm run build:web （若涉及前端）
- [ ] 手工 smoke （若涉及通道/权限）

## 关联 Plan
docs/superpowers/plans/2026-05-05-<id>-<topic>.md
```

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| GF-01 拆分破坏 chat-ui 渲染契约 | 末次才改数据流；前面的 PR 仅做纯渲染抽出，行为零变化 |
| DR-01 supervisor 收口破坏回放 | 先写 invariant 测试（snapshot ↔ run-log seq 一致性），再动调用点 |
| DT-01 Harness 派生体量大 | 独立 worktree；分两阶段（aggregator 切 reader → 删 EventBus） |
| GF-02 work_loop 拆分涉及 skill 路由 | brainstorming 先穷举 skill_resolution 的所有触发路径；plan 阶段画依赖图 |
| 多人/多并行 PR 冲突 | 每个子流串行；GF-03 / GF-01 渐进 PR 间 rebase 频繁 |

## 9. 进度追踪

| 周 | 目标 |
|----|------|
| W1 (05-05 → 05-12) | ER-01 完成；DR-01 brainstorm + plan；GF-03 第 1 个 sub-effect；GF-01 第 1 个子模块；DT-01 brainstorm |
| W2 (05-12 → 05-19) | DR-01 完成；GF-02 启动；GF-03 / GF-01 持续；DT-01 plan |
| W3 (05-19 → 05-26) | GF-02 主体完成；GF-03 / GF-01 持续；DT-01 第一阶段（aggregator → reader） |
| W4 (05-26 → 06-02) | GF-01 末次（含 DT-03）；GF-03 收尾；GF-02 收尾；DT-01 第二阶段 |

每周末更新 [`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) 中对应条目状态（待 MT-03 完成 Status 列后落入表格）。

## 10. 单 ID Plan 文件链接（待创建）

- [ ] `2026-05-05-er01-set-active-model-auth-variant.md`
- [ ] `2026-05-05-dr01-supervisor-lifecycle-owner.md`
- [ ] `2026-05-05-gf02-work-loop-split.md`
- [ ] `2026-05-05-gf03-app-tsx-extraction.md`
- [ ] `2026-05-05-gf01-chat-ui-split.md`
- [ ] `2026-05-05-dt01-harness-derive-from-runlog.md`
