# FEAT-ID-005: Memory Identity Tagging And Observability

## Status

- State: `active`
- Owner: `@executor`
- Depends On: `FEAT-ID-001`, `FEAT-ID-002`, `FEAT-ID-003`
- Last Updated: `2026-04-22`

---

## Goal

为 If2Ai memory 子系统引入 identity-aware tagging，使新写入的 memory entry 能记录 `soul_id / persona_id`，并在 diagnostics / audit / debug surface 中可观测。

该 Pack 的重点是“归属打标签”，不是“重写 memory”。

---

## Why Now

1. Persona 一旦引入，如果 memory 不知道 identity 归属，后续会出现长期记忆污染。
2. 当前系统尚未深度 identity-aware retrieval，是做 schema ahead of behavior 的最好时机。
3. 观测性必须先于复杂优化；没有标签就无法分析身份与记忆的关系。

---

## Spec

1. memory entry / audit record 新增 `soul_id`、`persona_id` 可选字段  
   → 测试 `memory::tests::memory_entry_identity_fields_round_trip`

2. after-turn memory write 路径拿到 `ResolvedIdentity` 并写入 identity tags  
   → 测试 `memory::tests::after_turn_write_includes_identity_tags`

3. 无 Persona 时允许只写 `soul_id`，无 identity metadata 时保持兼容  
   → 测试 `memory::tests::identity_tags_are_optional`

4. legacy memory entry 无 identity 字段仍可读取  
   → 测试 `memory::tests::legacy_memory_entry_without_identity_deserializes`

5. memory-related diagnostics / audit surface 可见 identity tags  
   → 测试 `memory::tests::audit_payload_exposes_identity_tags`

6. prompt / runtime debug surface 可看到当前 resolved identity 与 memory tagging 来源  
   → 测试 `application::tests::memory_debug_surface_contains_identity_context`

7. 本 Pack 不得改变现有 retrieval / compaction 主行为  
   → 测试 `memory::tests::existing_recall_behavior_is_unchanged`

---

## Files (scope)

- `src-tauri/src/modules/memory/**`
- `src-tauri/src/modules/application/memory_*`
- `src-tauri/src/modules/application/turn_service/**`
- `src-tauri/src/modules/runtime/stream_emitter.rs`
- `src-tauri/src/commands/memory.rs` (if needed)

---

## Reads

- `src-tauri/src/modules/identity/**`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`
- `docs/packs/feature/migration-core/MIG-005-real-memory-lifecycle.md`

---

## Contract

- identity tagging 为增量字段，不得破坏现有 memory entry schema 兼容性
- retrieval / compaction / compiler 行为首版不得做语义变更
- Soul 是 primary identity tag，Persona 是 secondary tag
- 当 identity 缺失时，memory 仍按旧逻辑工作

---

## Implementation Notes

1. 推荐在 memory entry、audit payload、after-turn decision record 三处统一加 identity metadata。
2. 如果当前 memory 子系统 entry 类型很多，优先修改“持久化 entry + audit/debug payload”这两层，不要求所有中间态都立刻覆盖。
3. 日志 / diagnostics 中对 identity 的暴露应避免泄露完整 prompt，仅输出 id/version/source 等结构化信息。

---

## Verify

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml memory`

---

## Acceptance

- 新 memory entry 带 identity tags
- legacy memory entry 兼容读取
- audit / diagnostics 可观测 identity tags
- retrieval / compaction 主行为无退化

---

## Out Of Scope

- 不实现 identity-aware retrieval ranking
- 不实现 reflection / compaction / consolidation 策略分流
- 不做 memory store 物理分库
