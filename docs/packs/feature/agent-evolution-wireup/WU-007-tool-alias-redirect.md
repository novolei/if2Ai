# WU-007: Tool Alias 重定向（BR-003 Wire-up）

## Status
- State: active

## Goal
在 `control_plane/tool_execution_broker.rs` 命令派发入口处，调用 BR-003 的 `resolve_alias(tool_name)` 将 18 个旧工具名透明重写为 6 个原子工具名后，再进入现有执行管道。重定向在 prepare_step_execution 调用之前完成，不改变任何权限/audit 逻辑。

## Spec (verifiable)
- 旧工具名（如 `web_search_brave`）经 broker 后等同于对应原子工具名 → `tests::tool_alias::old_name_resolves_to_atomic_tool`
- 未知名称不被 resolve_alias 改写（透传）→ `tests::tool_alias::unknown_name_passes_through`
- resolve_alias 内部 panic 时工具名透传（不中断派发）→ `tests::tool_alias::resolve_alias_panic_falls_back`
- `IF2AI_DISABLE_TOOL_ALIAS=1` 时跳过 resolve_alias → `tests::tool_alias::env_flag_disables_alias`

## Files (scope — write list)
- `src-tauri/src/modules/control_plane/tool_execution_broker.rs`  (modify — 在 dispatch 入口调用 resolve_alias)
- `src-tauri/tests/tool_alias.rs`                                  (new — 4 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/tools/builtin/atomic_consolidation.rs`   (resolve_alias + ALIAS_TABLE)

## Contract (review must check)
- 不改 `ToolExecutionBroker` 结构或对外方法签名（I1）
- 不改任何 IPC 命令名（I3）
- `resolve_alias` 调用必须在 `prepare_step_execution` 之前（审计日志记录原子名，不记旧名）
- 不引入新 Cargo 依赖

## Out of Scope
- ❌ 不修改 BR-003 alias table 内容
- ❌ 不改 permission_service / audit log 逻辑
- ❌ 不暴露 alias table 为 IPC 查询接口
- ❌ 不在前端展示 alias 解析事件

## Depends on
- FEAT-BR-003（alias table 实现 done）
- WU-001 不是必须依赖（此 Pack 无需 emit event，但应兼容 WU-001 存在）

## Verify
- `./scripts/pack run WU-007`
- `cargo test --test tool_alias`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
