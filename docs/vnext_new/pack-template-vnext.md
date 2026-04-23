# VNEXT-XXX: <one-line goal — 引用 task.md T-XXX>

## Status
- State: active
- Task Ref: T-XXX（docs/vnext_new/task.md）
- Spec Ref: §X.X <标题>（docs/vnext_new/spec.md）
- Design Ref: §X.X <标题>（docs/vnext_new/design.md）
- Depends On: T-YYY (必须先完成)
- Last Updated: YYYY-MM-DD

---

## Goal
<2–3 行：要什么行为存在。必须与 task.md T-XXX 的"输出"字段对齐>

## Spec (verifiable — 每条配 1 个 test name)
- <行为 1> → 测试 `tests::<module>::<test_name>`
- <行为 2> → 测试 `tests::<module>::<test_name>`
- <UI 行为>  → e2e step in `e2e/<file>.spec.ts`

## Files (scope — write list)
- src-tauri/src/.../foo.rs        (new|modify)
- src/.../bar.ts                  (new|modify)
- <只列出本 Pack 允许修改的文件，与 task.md "关键代码路径"对齐>

## Reads (read-only inputs)
- docs/vnext_new/spec.md §X.X     ← 必须点名具体章节
- docs/vnext_new/design.md §X.X   ← 必须点名具体章节
- src-tauri/src/.../existing.rs    ← 只读依赖
- src/.../existing.ts              ← 只读依赖

## Contract (review must check — vNext 通用 + 本 Pack 特有)

### vNext 通用约束（所有 VNEXT- Pack 必须通过）
- [ ] 不新增直接消费 raw Tauri event 的 UI surface（No New Raw Consumer）
- [ ] 不把 transcript 写入 session.json 作为长期事实源
- [ ] 不让 harness / projection / session manager 各自生成独立 run truth
- [ ] 所有新 runtime event 可关联 session_id / run_id
- [ ] 无 `unwrap()` / `expect()` / `todo!()` 在非测试代码
- [ ] 跨模块用 `crate::modules::*`

### 本 Pack 特有约束
- <从 spec.md 对应章节提取的不变量>
- <例如：correlation 字段必须同时包含 run_id 和 stream_id>

## Out of Scope
- ❌ <与 task.md 对齐的排除项>
- ❌ 不顺手"优化"/"清理"代码
- ❌ 不在 Pack 没列出的文件里做修改

## Verify
- ./scripts/pack run VNEXT-XXX
- <vNext 专项检查，按需选择：>
  - [ ] Contract Drift Test: `npm test -- --testPathPattern="contract-drift"`
  - [ ] Single Truth Test: `npm test -- --testPathPattern="single-truth"`
  - [ ] Event Log Integrity: `cargo test -- event_log`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
- 前序 Task 的验收不被破坏（跑对应 Phase 的 regression suite）
