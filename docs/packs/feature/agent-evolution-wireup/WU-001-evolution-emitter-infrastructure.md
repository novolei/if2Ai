# WU-001: Evolution Emitter Infrastructure

## Status
- State: active

## Goal
新建 `runtime/evolution_emitter.rs`，封装 `RuntimeEventEnvelope` 构造 + `app_handle.emit("runtime_event", …)` + `RunEventLogger` durable 写入为一个 `emit_evolution_event` 辅助函数，供所有后续 WU-00x Pack 调用。前端 `App.tsx` 在 `onMount` 时订阅 `runtime_event` 频道，将每条信封送入 `evolutionEventStore.applyEnvelope(env)`。

## Spec (verifiable)
- `emit_evolution_event` 序列化失败时返回 `Err`，不 panic → `tests::evolution_emitter::emit_does_not_panic_on_bad_payload`
- 序列化正常时 envelope 的 `event_type` 字段与入参一致 → `tests::evolution_emitter::envelope_event_type_matches`
- `IF2AI_DISABLE_EVOLUTION_EMIT=1` 时调用立即返回 `Ok(())` 且不调用 `app_handle.emit` → `tests::evolution_emitter::env_disable_flag_skips_emit`
- 前端 listener 注册后 `applyEnvelope` 被调用一次 → `src/transport/runtime-event-translator.test.ts::app_listener_calls_apply_envelope`

## Files (scope — write list)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`    (new)
- `src-tauri/src/modules/runtime/mod.rs`                  (modify — pub mod evolution_emitter)
- `src-tauri/tests/evolution_emitter.rs`                  (new — 3 unit tests)
- `src/App.tsx`                                           (modify — onMount 订阅 runtime_event)

## Reads (read-only)
- `src-tauri/src/modules/runtime/contracts/common.rs`     (RuntimeEventEnvelope::new)
- `src-tauri/src/modules/runtime/event_log.rs`            (RunEventLogger)
- `src/state/evolution-event-store.ts`                    (applyEnvelope 签名)
- `src/api/streaming.ts`                                  (listenToStream 参考模式)

## Contract (review must check)
- 不新增 IPC 命令名；仅通过 `app_handle.emit("runtime_event", …)` 现有频道广播（I3）
- 不改 sqlite schema / localStorage key（I4）
- 不引入新 Cargo / npm 依赖（serde_json 已有）
- `cargo clippy -D warnings` + `npm run lint` PASS（I7）
- `emit_evolution_event` 签名：`fn emit_evolution_event<T: Serialize>(handle: &AppHandle, event_type: RuntimeEventType, family: &str, correlation: CorrelationIds, payload: &T) -> anyhow::Result<()>`

## Out of Scope
- ❌ 不实现任何 React 组件或 Drawer
- ❌ 不改已有 8 个 RuntimeEventType 的发射逻辑
- ❌ 不改 RunEventLogger 自身结构
- ❌ 不在此 Pack 内接入任何 draft 模块（draft 接入由 WU-002~008 完成）

## Depends on
- FEAT-INT-001（RuntimeEventType 18 变体 + evolutionEventStore 骨架已 done）

## Verify
- `./scripts/pack run WU-001`
- `cargo test --test evolution_emitter`
- `npm run test -- app_listener`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
