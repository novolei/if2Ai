# GAP-002: Runtime Contract Unification

## Status
- State: active
- Task Ref: T-001 (docs/vnext_new/task.md §2)
- Spec Ref: §8.2 GAP-002 (docs/vnext_new/spec.md)
- Design Ref: §4.3 Contract 统一设计 (docs/vnext_new/design.md)
- Depends On: 无
- Last Updated: 2026-04-24

---

## Goal
统一 RuntimeEventEnvelope / StreamTokenPayload / RunLogEntry 三体关系：StreamTokenPayload 可无损转换为 RuntimeEventEnvelope，RunLogEntry 从 RuntimeEventEnvelope 构造，correlation 字段统一 run_id + stream_id 双写，新增 attempt_id 到 CorrelationIds。

## Spec (verifiable — 每条配 1 个 test name)
- 每个 StreamTokenPayload event_type 都可映射到 RuntimeEventType → 测试 `stream_payload_maps_to_runtime_envelope`
- envelope → RunLogEntry → JSON → 反序列化等价 → 测试 `run_log_entry_from_envelope_round_trips`
- run_scoped events 的 correlation.run_id 必须被设置 → 测试 `correlation_run_id_is_set_for_run_scoped_events`

## Files (scope — write list)
- `src-tauri/src/modules/runtime/contracts/common.rs` (modify — CorrelationIds 新增 attempt_id)
- `src-tauri/src/modules/runtime/stream_emitter.rs` (modify — StreamTokenPayload 新增 correlation 字段 + to_envelope 转换)
- `src-tauri/src/modules/runtime/event_log.rs` (modify — RunLogEntry 新增 from_envelope 构造)
- `src-tauri/src/modules/application/turn_service/stream_task.rs` (modify — 添加 correlation: None 适配)
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs` (modify — 添加 correlation: None 适配)

## Reads (read-only inputs)
- docs/vnext_new/spec.md §8.2 GAP-002
- docs/vnext_new/design.md §4.3 Contract 统一设计
- docs/design-docs/if2ai-vnext-session-runtime-blueprint.md
- docs/packs/feature/migration-core/MIG-016-canonical-run-event-log-foundation.md
- src-tauri/src/modules/runtime/contracts/mod.rs

## Contract (review must check)

### vNext 通用约束
- [ ] 不新增直接消费 raw Tauri event 的 UI surface（No New Raw Consumer）
- [ ] 不把 transcript 写入 session.json 作为长期事实源
- [ ] 不让 harness / projection / session manager 各自生成独立 run truth
- [ ] 所有新 runtime event 可关联 session_id / run_id
- [ ] 无 unwrap() / expect() / todo!() 在非测试代码
- [ ] 跨模块用 crate::modules::*

### 本 Pack 特有约束
- run_id 是 runtime 主 correlation；stream_id 只能作为 transport compatibility alias
- StreamTokenPayload 必须包含足够信息无损转换为 RuntimeEventEnvelope
- RunLogEntry 的 from_envelope 构造必须保留 envelope 的全部 correlation 字段
- attempt_id 新增到 CorrelationIds 后，event_log 的 RunLogEntry.attempt_id 可从 correlation 填充
- 不新增未进入 canonical envelope 的 runtime event kind
- TS/Rust 字段命名必须保持一一对应（camelCase ↔ snake_case）

## Out of Scope
- ❌ 不改造前端 translator/reducer（属于 T-002）
- ❌ 不拆分 stream_task.rs god-file（属于 T-008 GAP-005）
- ❌ 不改造 command boundary（属于 T-009 GAP-004）
- ❌ 不顺手"优化"/"清理"代码
- ❌ 不在 Pack 没列出的文件里做修改

## Verify
- `./scripts/pack run GAP-002`
- vNext 专项门：
  - [ ] V1 Contract Drift: `cargo test -- runtime_contract` + TS 类型对照
  - [ ] V3 Event Log Integrity: `cargo test -- event_log`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
- 前序 Task 的验收不被破坏
