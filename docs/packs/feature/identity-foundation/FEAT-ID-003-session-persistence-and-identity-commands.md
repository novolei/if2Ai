# FEAT-ID-003: Session Persistence And Identity Commands

## Status

- State: `active`
- Owner: `@executor`
- Depends On: `FEAT-ID-001`
- Last Updated: `2026-04-22`

---

## Goal

为 Session 持久化与 Tauri/Frontend API surface 引入 identity 字段与命令，使会话可以保存和修改 `soul_id / persona_id`，并在恢复会话后继续生效。

---

## Why Now

1. 用户要求能在 Settings 和会话层手动修改 Soul / Persona。
2. Prompt planner 只有接入 session override，identity 才具备产品意义。
3. 当前 session manager 已有 session-scoped override 的先例，可沿用成熟模式。

---

## Spec

1. `Session` / `SessionMeta` 新增 `soul_id` 与 `persona_id` 可选字段  
   → 测试 `session::tests::legacy_session_without_identity_deserializes`

2. 新创建 session 支持携带 identity override 或在创建后设置 identity  
   → 测试 `session::tests::create_session_with_identity_persists_fields`

3. 新增 session identity 修改方法，例如 `set_session_identity`  
   → 测试 `session::tests::set_session_identity_updates_metadata`

4. `list_sessions` / `get_session` 返回的 DTO 包含 identity 字段  
   → 测试 `commands::tests::session_commands_return_identity_fields`

5. 新增列出 built-in souls / personas 的 command 或 settings command surface  
   → 测试 `commands::tests::identity_list_commands_return_registry_data`

6. 设置默认 identity 的 command 可读写 runtime settings  
   → 测试 `commands::tests::identity_defaults_can_be_saved_and_loaded`

7. 所有 legacy session JSON 读取必须兼容  
   → 测试 `session::tests::missing_identity_fields_default_to_none`

---

## Files (scope)

- `src-tauri/src/modules/session/manager.rs`
- `src-tauri/src/modules/session/mod.rs`
- `src-tauri/src/commands/session.rs`
- `src-tauri/src/commands/settings.rs` or equivalent identity command file
- `src-tauri/src/main.rs`
- `src/lib/tauri.ts`
- `src/api/sessions.ts`
- `src/api/identity.ts` (new)
- `src/transport/contracts.ts` (if needed)

---

## Reads

- `src-tauri/src/modules/identity/**`
- `src-tauri/src/modules/runtime/config/**`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`

---

## Contract

- 新增 session 字段必须全部为 optional / backward-compatible
- 不允许破坏现有 session create/list/get/rename/pin/delete 语义
- identity list/default commands 必须是只读/显式写入，不要隐式改 session
- 前端 API facade 必须提供稳定 typed surface

---

## Implementation Notes

1. 推荐新增一个 session identity 更新命令，而不是复用 rename / memory toggle 命令。
2. 若 settings command 现有职责过于杂，可新建 `commands/identity.rs`，但必须保持命令注册清晰。
3. 前端 DTO 变更要同步到 `src/lib/tauri.ts` 与 `src/api/*` facade。

---

## Verify

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml session`
- `cargo test --manifest-path src-tauri/Cargo.toml commands`

---

## Acceptance

- Session identity 字段可持久化
- Legacy session 兼容读取
- Tauri command + API facade 已打通
- 前端能拿到 souls / personas 列表与 session identity 字段

---

## Out Of Scope

- 不做 Settings UI
- 不做 prompt planner 接线
- 不做 memory tagging
