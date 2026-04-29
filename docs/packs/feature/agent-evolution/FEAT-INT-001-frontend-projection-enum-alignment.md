# FEAT-INT-001: 前端投影扩展 + 后端 RuntimeEventType 对齐

## Status
- State: active

## Goal
把 EVO-000 在前端 contracts.ts 中占位的 10 个 RuntimeEventType 字面量打通后端 enum 通路，并在前端新建 reducer 路由、envelope translator、typed payload interfaces 及 zustand slice。后端 enum 扩展到 18 变体，前端骨架仅含 reducer/store/translator，不含任何 React UI 组件。

## Spec (verifiable — 每条配 1 个 test name)
- 后端 10 个新 variant 序列化为 snake_case 字符串 → `tests::evolution_phase0::runtime_event_type_serializes_to_snake_case`
- serde 双向 round-trip 无损 → `tests::evolution_phase0::runtime_event_type_round_trip`
- enum 共 18 变体（断言 discriminant 数量）→ `tests::evolution_phase0::runtime_event_type_count_matches_frontend`
- 构造含新 variant 的 envelope 序列化后 event_type 字段正确 → `tests::evolution_phase0::runtime_event_envelope_carries_evolution_event_type`
- `daemon_health` envelope → `DaemonHealthPayload` → `runtime-event-translator.test.ts::translates_daemon_health`
- 未知 eventType → `null` → `runtime-event-translator.test.ts::unknown_event_type_returns_null`
- schema 不合法（缺 schema_version）→ `null` → `runtime-event-translator.test.ts::invalid_envelope_returns_null`
- payload 缺必填字段 → `null` → `runtime-event-translator.test.ts::missing_required_payload_field_returns_null`

## Files (scope — write list)
- `src-tauri/src/modules/runtime/contracts/common.rs`  (modify — 新增 10 个 RuntimeEventType 变体 + `#[serde(rename_all = "snake_case")]`)
- `src-tauri/tests/evolution_phase0.rs`                (modify — 追加 4 个 Rust 测试)
- `src/transport/runtime-event-payloads.ts`            (new — 10 个 payload TS interface)
- `src/transport/runtime-event-translator.ts`          (new — `translateEnvelope` 纯函数)
- `src/transport/runtime-event-reducer.ts`             (new — `evolutionEventReducer` 纯函数，10 个 case)
- `src/transport/runtime-event-translator.test.ts`     (new — 4 个 vitest 测试)
- `src/transport/index.ts`                             (modify — re-export 3 个新模块)
- `src/state/evolution-event-store.ts`                 (new — `useEvolutionEventStore` zustand slice，ring buffer cap=50)
- `src/state/index.ts`                                 (modify — re-export evolution-event-store)

## Reads (read-only inputs)
- `src/transport/contracts.ts`、`src/state/bootstrap-store.ts`  (前端类型参考)
- 各 Pack 的 payload 结构体（读对应模块取字段名）：
  `runtime/daemon/mod.rs`(DaemonState)、`skills/sedimentation/mod.rs`(SkillDraft)、
  `runtime/context_compression/mod.rs`(CompressionOutcome)、`skills/guard/constitution.rs`(Violation)、
  `learning/self_edit/proposal.rs`(SelfEditProposal)、`smart_browser/session_health.rs`(BrowserHealthStatus)、
  `skills/domain_knowledge/mod.rs`(DomainKnowledgeEntry)、`runtime/working_checkpoint.rs`(WorkingCheckpoint)、
  `learning/self_edit/verification.rs`(VerificationVerdict)、`smart_browser/content_simplifier.rs`(SimplifiedContent)

## Contract (review must check)
- 不改任何 IPC 命令名；仅扩展 RuntimeEventType enum（I3 — 只增不删）
- 不改 sqlite schema / localStorage key（I4）
- 不引入新 Cargo / npm 依赖（vitest / zustand 已存在）
- `cargo clippy -D warnings` + `npm run lint` 全 PASS（I7）
- 不实现 React 组件；reducer/store/translator 骨架 only；payload 字段 snake_case→camelCase 1:1 对齐

## Out of Scope
- ❌ 不实现任何 React 组件（Drawer / Timeline / Panel / Dashboard）
- ❌ 不修改 ChatWorkspace / SmartBrowserCockpit 等现有页面
- ❌ 不改后端事件 emitter / sqlite schema / IPC 命令名
- ❌ 不改 contracts.ts 中已有的 8 个 RuntimeEventType 字面量

## Depends on
全部 Phase 1-4, 6, 7（FEAT-TE/SE/SH/AE/DK/BR 系列）

## Verify
- `./scripts/pack run FEAT-INT-001`
- `cargo test --test evolution_phase0 runtime_event_type`
- `npm run test -- runtime-event-translator`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
