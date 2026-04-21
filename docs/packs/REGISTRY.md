# Pack Registry

> 所有 active / pending / done 的 Pack 一览。
>
> 由 `./scripts/pack scan` 维护文件 LOC；`Status` 列由人/agent 在每个 pack done 后更新。
>
> 最后更新: 2026-04-21

---

## Active

| Pack                                     | Type | Goal | Owner |
| ---------------------------------------- | ---- | ---- | ----- |
| _(无 — 下一刀建议 GFR-T1-B-2 schema 集群)_ | —    | —    | —     |

## Recently Done

| Pack                                                                       | Type     | Goal                                                                                                                | Done       |
| -------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------- | ---------- |
| [GFR-T1-C-3](./refactor/GFR-T1-C-3-mcp-stdio-manager.md)                   | refactor | Extract McpServerManager cluster from mcp_stdio; mod.rs 621 → 270 LOC; T1-C 收尾                               | 2026-04-21 |
| [GFR-T1-C-2](./refactor/GFR-T1-C-2-mcp-stdio-types.md)                     | refactor | Extract 16 MCP protocol DTOs from mcp_stdio; mod.rs 763 → 621 LOC                                                  | 2026-04-21 |
| [GFR-T1-C-1](./refactor/GFR-T1-C-1-mcp-stdio-tests-and-rpc.md)             | refactor | mcp_stdio 目录化 + tests (916 LOC) + JSON-RPC framing 抽出；mod.rs 1725 → 763 LOC                                | 2026-04-21 |
| [GFR-T1-B-5](./refactor/GFR-T1-B-5-extract-config-parsers.md)              | refactor | Extract 7 settings parsers + IO helper from `runtime/config/mod.rs`; mod.rs now 711 LOC (< 800 hard limit, T1-B 收尾) | 2026-04-21 |
| [GFR-T1-B-2](./refactor/GFR-T1-B-2-extract-config-schema.md)               | refactor | Extract schema simple types (4 types + 5 parsers) from `runtime/config/mod.rs`                                      | 2026-04-21 |
| [GFR-T1-B-tests](./refactor/GFR-T1-B-tests-extract.md)                     | refactor | Extract `#[cfg(test)] mod tests` block (757 LOC) into sibling `runtime/config/tests.rs`                             | 2026-04-21 |
| [GFR-T1-B-4](./refactor/GFR-T1-B-4-extract-config-memory.md)               | refactor | Extract memory cluster (4 types + 4 impls + 4 parsers + 1 helper) from `runtime/config/mod.rs`                      | 2026-04-21 |
| [GFR-T1-B-3](./refactor/GFR-T1-B-3-extract-config-mcp.md)                  | refactor | Extract MCP cluster (9 types + 3 impls + 4 parsers) from `runtime/config/mod.rs`                                    | 2026-04-21 |
| [GFR-T1-B-1](./refactor/GFR-T1-B-1-extract-config-json-helpers.md)         | refactor | Extract 14 JSON parse helpers from `runtime/config.rs` (also git mv to config/mod.rs); start of T1-B 5-slice arc    | 2026-04-21 |
| [GFR-005e](./refactor/GFR-005e-extract-session-bridge.md)                  | refactor | Extract session-bridge helpers (app_session_to_runtime + log_context_fingerprint) from `agent.rs`                   | 2026-04-21 |
| [GFR-005d](./refactor/GFR-005d-extract-permission-helpers.md)  | refactor | Move parse_permission_mode + build_permission_policy into existing application/permission_service.rs                | 2026-04-21 |
| [GFR-005c](./refactor/GFR-005c-extract-timeline-flush.md)      | refactor | Extract timeline-flush cluster (PersistedTurnOutcome + flush_assistant_timeline_segment) from `agent.rs`            | 2026-04-21 |
| [GFR-005b](./refactor/GFR-005b-extract-stream-error-reason.md) | refactor | Extract stream-error-reason classification (2 fns) from `agent.rs`                                                  | 2026-04-21 |
| [GFR-005a](./refactor/GFR-005a-extract-resume-cursor.md)       | refactor | Extract resume-cursor cluster (1 struct + 5 fns) from `agent.rs`                                                    | 2026-04-21 |
| [GFR-001](./refactor/GFR-001-extract-real-api-client.md)       | refactor | Extract `RealApiClient` + `block_conversion` cluster from `agent.rs`                                                | 2026-04-21 |
| [GFR-002a](./refactor/GFR-002a-extract-prompt-sanitize.md)     | refactor | Extract sanitize cluster (`SanitizationStats` + 3 fns) from `agent.rs`                                              | 2026-04-21 |
| [GFR-002c](./refactor/GFR-002c-extract-prompt-preflight.md)    | refactor | Extract preflight estimators (4 fns: char/token count + summarize/truncate) from `agent.rs`                         | 2026-04-21 |
| [GFR-002b](./refactor/GFR-002b-extract-prompt-governor.md)     | refactor | Extract governor cluster (RequestPreflightStats + ContextGovernor + apply_request_preflight_limits) from `agent.rs` | 2026-04-21 |
| [GFR-003](./refactor/GFR-003-extract-permission-service.md)    | refactor | Extract `TauriPermissionPrompter` from `agent.rs` (AppState refactor deferred)                                      | 2026-04-21 |

---

## Refactor Pipeline (GFR-001 ~ GFR-T2-B)

完整 28 个 GFR 路线图：详见 [docs/packs/refactor/](./refactor/)。

| GFR        | 状态     | Source                                                                         | Destination                                                      |
| ---------- | -------- | ------------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| GFR-001    | **done** | `commands/agent.rs` `RealApiClient` + `block_conversion`                       | `application/real_api_client.rs` + `runtime/block_conversion.rs` |
| GFR-002    | merged   | merged → GFR-002a/b/c (sliced 2026-04-21 因 700 LOC 单 pack 太大)              | —                                                                |
| GFR-002a   | **done** | `commands/agent.rs` sanitize cluster (≈3510–3672, ~162 LOC)                    | `application/prompt_planner/sanitize.rs`                         |
| GFR-002b   | pending  | `commands/agent.rs` governor cluster (RequestPreflightStats + ContextGovernor) | `application/prompt_planner/governor.rs`                         |
| GFR-002c   | pending  | `commands/agent.rs` preflight estimators (token/char count helpers)            | `application/prompt_planner/preflight.rs`                        |
| GFR-003    | pending  | `commands/agent.rs` permission lifecycle                                       | `application/permission_service.rs`                              |
| GFR-004    | pending  | `commands/agent.rs` 散点 emit                                                  | `application/stream_emitter_service.rs`                          |
| GFR-005    | sliced   | sliced into 005a (done) + 005b/c/d/e (pending — 人写)                          | `application/turn_service` (final destination)                   |
| GFR-005a   | **done** | `commands/agent.rs` resume-cursor cluster (struct + 5 fns)                     | `runtime/resume_cursor.rs`                                       |
| GFR-005b   | **done** | `commands/agent.rs` stream-error-reason classification (2 fns)                 | `runtime/stream_error_reason.rs`                                 |
| GFR-005c   | **done** | `commands/agent.rs` timeline-flush cluster (1 struct + 1 fn)                   | `runtime/timeline_flush.rs`                                      |
| GFR-005d   | **done** | `commands/agent.rs` permission helpers (parse_permission_mode + build_policy)  | `application/permission_service.rs` (extends existing)           |
| GFR-005e   | **done** | `commands/agent.rs` session-bridge helpers (app_session_to_runtime + log_ctx)  | `control_plane/session_bridge.rs`                                |
| GFR-006    | pending  | `commands/agent.rs` tool exec + trajectory                                     | `application/{tool_executor,trajectory_service}.rs`              |
| GFR-007    | pending  | `chat-ui.tsx` markdown helpers                                                 | `src/modules/markdown/`                                          |
| GFR-008    | pending  | `chat-ui.tsx` skills report                                                    | `src/modules/skills-report/`                                     |
| GFR-009    | pending  | `chat-ui.tsx` tool projection                                                  | `src/modules/tool-projection/`                                   |
| GFR-010    | pending  | `chat-ui.tsx` + `ProjectRail.tsx`                                              | `src/modules/project-rail/`                                      |
| GFR-011    | pending  | `chat-ui.tsx` composer                                                         | `src/modules/chat/composer/`                                     |
| GFR-012    | pending  | `chat-ui.tsx` transcript                                                       | `src/modules/chat/transcript/`                                   |
| GFR-013    | pending  | `chat-ui.tsx` shell shrink                                                     | `src/modules/chat/shell/`                                        |
| GFR-014    | pending  | `App.tsx` boot shell                                                           | `src/modules/boot-shell/`                                        |
| GFR-015    | pending  | `App.tsx` shell-router + window-bridge                                         | `src/modules/{shell-router,window-bridge}/`                      |
| GFR-016    | pending  | `tauri.ts` types                                                               | `src/transport/contracts.ts`                                     |
| GFR-017    | pending  | `tauri.ts` feature wrappers                                                    | `src/transport/{browser,session,...}.ts`                         |
| GFR-018    | pending  | `tauri.ts` listeners                                                           | `src/runtime-projection/translator/`                             |
| GFR-T1-A   | **cancelled** | source 是 orphan 死代码（plugins/lib+hooks + commands/lib，共 6057 LOC）；2026-04-21 整体 `chore(dead-code)` 删除 | — |
| GFR-T1-C-1 | **done** | `runtime/mcp_stdio.rs` 目录化 + tests (916 LOC) + JSON-RPC framing             | `runtime/mcp_stdio/{mod,tests,rpc}.rs`                           |
| GFR-T1-C-2 | **done** | `runtime/mcp_stdio/mod.rs` 16 MCP DTOs (~150 LOC)                              | `runtime/mcp_stdio/types.rs`                                     |
| GFR-T1-C-3 | **done** | `runtime/mcp_stdio/mod.rs` McpServerManager cluster (~350 LOC); T1-C 收尾     | `runtime/mcp_stdio/manager.rs`                                   |
| **GFR-T1-C** | **done** | runtime/mcp_stdio 总收尾：mod.rs 1725 → 270 LOC (-84%) 跨 3 sub-packs        | `runtime/mcp_stdio/{mod,manager,types,rpc,tests}.rs`             |
| GFR-T1-B   | sliced   | sliced into B-1 (done) + B-2..5 (pending — 人写)                               | `runtime/config/{json_helpers,schema,permission,sandbox,merge}.rs` |
| GFR-T1-B-1 | **done** | `runtime/config.rs` JSON parse helpers (12 fn + 2 utility, ~230 LOC)           | `runtime/config/json_helpers.rs` (+ git mv to config/mod.rs)     |
| GFR-T1-B-3 | **done** | `runtime/config/mod.rs` MCP cluster (9 types + 3 impls + 4 parsers, ~250 LOC)  | `runtime/config/mcp.rs`                                          |
| GFR-T1-B-4 | **done** | `runtime/config/mod.rs` memory cluster (~440 LOC)                              | `runtime/config/memory.rs`                                       |
| GFR-T1-B-tests | **done** | `runtime/config/mod.rs` test block (757 LOC) extracted to sibling tests.rs   | `runtime/config/tests.rs`                                        |
| GFR-T1-B-2 | **done** | `runtime/config/mod.rs` schema simple types (4 types + 5 parsers, ~190 LOC)    | `runtime/config/schema.rs`                                       |
| GFR-T1-B-5 | **done** | `runtime/config/mod.rs` 7 parsers + IO helper (~280 LOC); T1-B 收尾            | `runtime/config/parsers.rs`                                      |
| **GFR-T1-B** | **done** | runtime/config 总收尾：mod.rs 2640 → 711 LOC (-73%) 跨 6 sub-packs            | `runtime/config/{json_helpers,mcp,memory,schema,parsers,tests}.rs` |
| GFR-T1-C~I | pending  | T1 后端 god-files                                                              | 各自 module 子目录                                               |
| GFR-T2-A   | pending  | `ProviderSetupStep.tsx`                                                        | `onboarding/steps/provider/`                                     |
| GFR-T2-B   | pending  | `SkillsSettingsPage.tsx`                                                       | `settings/pages/skills/`                                         |

---

## Feature Pipeline

所有非 refactor 的工作都落在 `docs/packs/feature/`。命名前缀按任务性质区分（脚本不强制；只是命名约定）：

| 前缀    | 用途                                                      | Pack 模板    |
| ------- | --------------------------------------------------------- | ------------ |
| `FEAT-` | 新功能 / 新页面 / 新 IPC 命令                             | CHARTER §6.1 |
| `BUG-`  | 修 bug（必须先写复现测试）                                | CHARTER §6.2 |
| `PERF-` | 性能优化 / 算法升级（必须有 before/after 数据）           | CHARTER §6.3 |
| `DEP-`  | 升级依赖 / 升级框架（diff 只能是 lockfile + manifest 类） | CHARTER §6.4 |
| `CPD-`  | 已有的 chat-prompt-dispatch 系列（legacy 命名，沿用）     | CHARTER §6.1 |

### Active

| Pack                                                            | 状态            | Goal                                         |
| --------------------------------------------------------------- | --------------- | -------------------------------------------- |
| [MIG-001](./feature/migration-core/MIG-001-canonical-chat-execution-spine.md) | active | 把 chat turn 主链收口成 canonical orchestrator |

### Bug / Perf / Dep

| Pack   | 状态 | Goal |
| ------ | ---- | ---- |
| _(无)_ | —    | —    |

> 新 Pack 用 `./scripts/pack init <PACK-ID> --type feature --slug <slug> --files <a> [<b>...]` 生成。
> `migration-core` 是当前优先级最高的 feature pipeline；老 `CPD-001` 保留作历史 pack，不再作为默认 active 入口。

---

## Tier 3 — Watchlist (800–1500 LOC)

> 已知的"边缘 god-file"。由 `./scripts/pack scan` 维护 LOC；超过 1500 LOC 自动晋升为 Tier 1/2。
> `lint-architecture` 把这些文件视为 baseline 已知技术债，不报 error；新增超过 800 LOC 的文件会立即报 error。

| 文件                                                          |  LOC |
| ------------------------------------------------------------- | ---: |
| `src-tauri/src/modules/browser/session.rs`                    | 1408 |
| `src-tauri/src/modules/memory/audit.rs`                       | 1094 |
| `src-tauri/src/modules/api/providers/claw_provider.rs`        | 1223 |
| `src-tauri/src/modules/api/providers/openai_compat.rs`        | 1071 |
| `src-tauri/src/modules/memory/pinned/store.rs`                | 1009 |
| `src-tauri/src/modules/runtime/compact.rs`                    |  962 |
| `src-tauri/src/modules/learning/strategy_rollout.rs`          |  967 |
| `src-tauri/src/modules/learning/candidate_evaluator.rs`       |  966 |
| `src-tauri/src/modules/learning/strategy_registry_service.rs` |  951 |
| `src-tauri/src/modules/learning/strategy_registry.rs`         |  845 |
| `src-tauri/src/modules/learning/promotion_gate.rs`            |  822 |
| `src-tauri/src/modules/harness/gate.rs`                       |  951 |
| `src-tauri/src/modules/harness/trace_aggregator.rs`           |  803 |
| `src-tauri/src/modules/session/manager.rs`                    |  966 |
| `src-tauri/src/modules/tools/builtin/skill.rs`                |  985 |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs`         |  952 |
| `src-tauri/src/modules/tools/registry.rs`                     |  850 |
| `src-tauri/src/modules/skills/guard/threat_patterns.rs`       |  982 |
| `src-tauri/src/modules/skills/guard/mod.rs`                   |  808 |
| `src-tauri/src/commands/slash.rs`                             |  980 |
| `src-tauri/src/commands/memory.rs`                            |  975 |
| `src-tauri/src/commands/skills_hub.rs`                        |  850 |
| `src/modules/settings/pages/TtsTestPage.tsx`                  |  921 |

---

## 状态字典

- `pending` — stub，禁止执行
- `active`  — 可被 agent 执行
- `done`    — verify + review 通过 + 已 commit
- `blocked` — 阻塞原因写在 pack 内
- `merged`  — 在另一个 pack 内合并完成
