# Pack Registry

> 所有 active / pending / done 的 Pack 一览。
>
> 由 `./scripts/pack scan` 维护文件 LOC；`Status` 列由人/agent 在每个 pack done 后更新。
>
> 最后更新: 2026-04-25

---

## Active

| Pack   | Type | Goal | Owner |
| ------ | ---- | ---- | ----- |
| [FEAT-JC-003](./feature/jiaochang/FEAT-JC-003-fr008-pixel-assets.md) | feature | FR-008 pixel asset seed | executor |
| [FEAT-JC-002](./feature/jiaochang/FEAT-JC-002-runtime-cockpit-i18n-strategy.md) | feature | Runtime cockpit adapter + i18n + strategy | executor |
| [FEAT-JC-001](./feature/jiaochang/FEAT-JC-001-jiaochang-shell.md) | feature | Jiaochang shell + fixture cockpit | executor |
| [APP-UPDATER-002](./feature/app-updater/APP-UPDATER-002-release-ci-signed-artifact.md) | feature | Release CI + signed Tauri updater artifact | executor |
| [APP-UPDATER-003](./feature/app-updater/APP-UPDATER-003-client-state-machine-ux.md) | feature | Client updater state machine + settings UX | executor |

## Recently Done

| Pack                                                               | Type     | Goal                                                                                                                  | Done       |
| ------------------------------------------------------------------ | -------- | --------------------------------------------------------------------------------------------------------------------- | ---------- |
| [APP-UPDATER-001](./feature/app-updater/APP-UPDATER-001-updater-transport-release-manifest-settings-ci.md) | feature | Updater transport + release manifest + settings state machine + CI release gates | 2026-04-25 |
| [FEAT-PCP-005](./feature/prompt-control-plane/FEAT-PCP-005-prompt-control-panel-and-diagnostics.md) | feature | Prompt control panel + diagnostics projection | 2026-04-22 |
| [FEAT-PCP-004](./feature/prompt-control-plane/FEAT-PCP-004-utility-and-coordinator-prompt-lanes.md) | feature | Utility / coordinator prompt lanes | 2026-04-22 |
| [FEAT-PCP-003](./feature/prompt-control-plane/FEAT-PCP-003-tool-prompt-catalog-and-injection.md) | feature | Tool prompt catalog + conditional injection | 2026-04-22 |
| [FEAT-PCP-002](./feature/prompt-control-plane/FEAT-PCP-002-scenario-and-task-focus-catalog.md) | feature | Scenario / task-focus prompt catalog | 2026-04-22 |
| [FEAT-PCP-001](./feature/prompt-control-plane/FEAT-PCP-001-prompt-coordinator-foundation.md) | feature | Prompt coordinator + assembly decision foundation | 2026-04-22 |
| [FEAT-ID-005](./feature/identity-foundation/FEAT-ID-005-memory-identity-tagging-and-observability.md) | feature | Memory identity tagging + observability | 2026-04-22 |
| [FEAT-ID-004](./feature/identity-foundation/FEAT-ID-004-settings-and-session-identity-ui.md) | feature | Settings + session identity UI | 2026-04-22 |
| [FEAT-ID-003](./feature/identity-foundation/FEAT-ID-003-session-persistence-and-identity-commands.md) | feature | Session persistence + identity commands | 2026-04-22 |
| [FEAT-ID-002](./feature/identity-foundation/FEAT-ID-002-prompt-planner-soul-persona-blocks.md) | feature | Prompt planner Soul / Persona blocks | 2026-04-22 |
| [FEAT-ID-001](./feature/identity-foundation/FEAT-ID-001-identity-domain-and-resolution.md) | feature | Identity domain model + resolver | 2026-04-22 |
| [MIG-007](./feature/prompt-planner-alignment/MIG-007-prompt-planner-modularization.md) | feature  | 将 prompt_planner/mod.rs 拆分为 block/diagnostics/build_request/planner 子模块，提升可维护性                              | 2026-04-22 |
| [MIG-006](./feature/prompt-planner-alignment/MIG-006-prompt-contribution-mechanism.md) | feature  | 实现 PromptContribution 机制 + PromptBuildMode + strict validation，让子系统独立贡献 prompt blocks                      | 2026-04-22 |
| [MIG-005](./feature/prompt-planner-alignment/MIG-005-prompt-block-structure-alignment.md) | feature  | PromptBlock 数据结构对齐 UClaw (source + priority + is_sensitive + validation_issues)                                  | 2026-04-22 |
| [MIG-004](./feature/migration-core/MIG-004-prompt-planning-traceability.md) | feature  | 把 prompt planner 升级成 traceable contract (trace_id + block_hash + diagnostics)                                      | 2026-04-22 |
| [MIG-015](./feature/migration-core/MIG-015-gateway-conversations-and-streaming-surface.md) | feature  | 让主聊天链走 gateway conversations/streaming surface (App.tsx 已完成 cutover)                                          | 2026-04-22 |
| [MIG-002](./feature/migration-core/MIG-002-execution-mode-routing-and-policy-enforcement.md) | feature  | 把 execution mode 与 step preflight 从 advisory 升级成真实 product gate (4 sub-packs: a/b/c/d)                          | 2026-04-22 |
| [GFR-T1-D-1](./refactor/GFR-T1-D-1-sqlite-provider-split.md)       | refactor | sqlite_provider 单刀目录化 + tests + scope + provider_impl 抽出；mod.rs 1579 → 268 LOC (-83%); T1-D 收尾              | 2026-04-21 |
| [GFR-T1-C-3](./refactor/GFR-T1-C-3-mcp-stdio-manager.md)           | refactor | Extract McpServerManager cluster from mcp_stdio; mod.rs 621 → 270 LOC; T1-C 收尾                                      | 2026-04-21 |
| [GFR-T1-C-2](./refactor/GFR-T1-C-2-mcp-stdio-types.md)             | refactor | Extract 16 MCP protocol DTOs from mcp_stdio; mod.rs 763 → 621 LOC                                                     | 2026-04-21 |
| [GFR-T1-C-1](./refactor/GFR-T1-C-1-mcp-stdio-tests-and-rpc.md)     | refactor | mcp_stdio 目录化 + tests (916 LOC) + JSON-RPC framing 抽出；mod.rs 1725 → 763 LOC                                     | 2026-04-21 |
| [GFR-T1-B-5](./refactor/GFR-T1-B-5-extract-config-parsers.md)      | refactor | Extract 7 settings parsers + IO helper from `runtime/config/mod.rs`; mod.rs now 711 LOC (< 800 hard limit, T1-B 收尾) | 2026-04-21 |
| [GFR-T1-B-2](./refactor/GFR-T1-B-2-extract-config-schema.md)       | refactor | Extract schema simple types (4 types + 5 parsers) from `runtime/config/mod.rs`                                        | 2026-04-21 |
| [GFR-T1-B-tests](./refactor/GFR-T1-B-tests-extract.md)             | refactor | Extract `#[cfg(test)] mod tests` block (757 LOC) into sibling `runtime/config/tests.rs`                               | 2026-04-21 |
| [GFR-T1-B-4](./refactor/GFR-T1-B-4-extract-config-memory.md)       | refactor | Extract memory cluster (4 types + 4 impls + 4 parsers + 1 helper) from `runtime/config/mod.rs`                        | 2026-04-21 |
| [GFR-T1-B-3](./refactor/GFR-T1-B-3-extract-config-mcp.md)          | refactor | Extract MCP cluster (9 types + 3 impls + 4 parsers) from `runtime/config/mod.rs`                                      | 2026-04-21 |
| [GFR-T1-B-1](./refactor/GFR-T1-B-1-extract-config-json-helpers.md) | refactor | Extract 14 JSON parse helpers from `runtime/config.rs` (also git mv to config/mod.rs); start of T1-B 5-slice arc      | 2026-04-21 |
| [GFR-005e](./refactor/GFR-005e-extract-session-bridge.md)          | refactor | Extract session-bridge helpers (app_session_to_runtime + log_context_fingerprint) from `agent.rs`                     | 2026-04-21 |
| [GFR-005d](./refactor/GFR-005d-extract-permission-helpers.md)      | refactor | Move parse_permission_mode + build_permission_policy into existing application/permission_service.rs                  | 2026-04-21 |
| [GFR-005c](./refactor/GFR-005c-extract-timeline-flush.md)          | refactor | Extract timeline-flush cluster (PersistedTurnOutcome + flush_assistant_timeline_segment) from `agent.rs`              | 2026-04-21 |
| [GFR-005b](./refactor/GFR-005b-extract-stream-error-reason.md)     | refactor | Extract stream-error-reason classification (2 fns) from `agent.rs`                                                    | 2026-04-21 |
| [GFR-005a](./refactor/GFR-005a-extract-resume-cursor.md)           | refactor | Extract resume-cursor cluster (1 struct + 5 fns) from `agent.rs`                                                      | 2026-04-21 |
| [GFR-001](./refactor/GFR-001-extract-real-api-client.md)           | refactor | Extract `RealApiClient` + `block_conversion` cluster from `agent.rs`                                                  | 2026-04-21 |
| [GFR-002a](./refactor/GFR-002a-extract-prompt-sanitize.md)         | refactor | Extract sanitize cluster (`SanitizationStats` + 3 fns) from `agent.rs`                                                | 2026-04-21 |
| [GFR-002c](./refactor/GFR-002c-extract-prompt-preflight.md)        | refactor | Extract preflight estimators (4 fns: char/token count + summarize/truncate) from `agent.rs`                           | 2026-04-21 |
| [GFR-002b](./refactor/GFR-002b-extract-prompt-governor.md)         | refactor | Extract governor cluster (RequestPreflightStats + ContextGovernor + apply_request_preflight_limits) from `agent.rs`   | 2026-04-21 |
| [GFR-003](./refactor/GFR-003-extract-permission-service.md)        | refactor | Extract `TauriPermissionPrompter` from `agent.rs` (AppState refactor deferred)                                        | 2026-04-21 |

---

## Refactor Pipeline (GFR-001 ~ GFR-T2-B)

完整 28 个 GFR 路线图：详见 [docs/packs/refactor/](./refactor/)。

| GFR            | 状态          | Source                                                                                                            | Destination                                                        |
| -------------- | ------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| GFR-001        | **done**      | `commands/agent.rs` `RealApiClient` + `block_conversion`                                                          | `application/real_api_client.rs` + `runtime/block_conversion.rs`   |
| GFR-002        | merged        | merged → GFR-002a/b/c (sliced 2026-04-21 因 700 LOC 单 pack 太大)                                                 | —                                                                  |
| GFR-002a       | **done**      | `commands/agent.rs` sanitize cluster (≈3510–3672, ~162 LOC)                                                       | `application/prompt_planner/sanitize.rs`                           |
| GFR-002b       | pending       | `commands/agent.rs` governor cluster (RequestPreflightStats + ContextGovernor)                                    | `application/prompt_planner/governor.rs`                           |
| GFR-002c       | pending       | `commands/agent.rs` preflight estimators (token/char count helpers)                                               | `application/prompt_planner/preflight.rs`                          |
| GFR-003        | pending       | `commands/agent.rs` permission lifecycle                                                                          | `application/permission_service.rs`                                |
| GFR-004        | pending       | `commands/agent.rs` 散点 emit                                                                                     | `application/stream_emitter_service.rs`                            |
| GFR-005        | sliced        | sliced into 005a (done) + 005b/c/d/e (pending — 人写)                                                             | `application/turn_service` (final destination)                     |
| GFR-005a       | **done**      | `commands/agent.rs` resume-cursor cluster (struct + 5 fns)                                                        | `runtime/resume_cursor.rs`                                         |
| GFR-005b       | **done**      | `commands/agent.rs` stream-error-reason classification (2 fns)                                                    | `runtime/stream_error_reason.rs`                                   |
| GFR-005c       | **done**      | `commands/agent.rs` timeline-flush cluster (1 struct + 1 fn)                                                      | `runtime/timeline_flush.rs`                                        |
| GFR-005d       | **done**      | `commands/agent.rs` permission helpers (parse_permission_mode + build_policy)                                     | `application/permission_service.rs` (extends existing)             |
| GFR-005e       | **done**      | `commands/agent.rs` session-bridge helpers (app_session_to_runtime + log_ctx)                                     | `control_plane/session_bridge.rs`                                  |
| GFR-006        | pending       | `commands/agent.rs` tool exec + trajectory                                                                        | `application/{tool_executor,trajectory_service}.rs`                |
| GFR-007        | pending       | `chat-ui.tsx` markdown helpers                                                                                    | `src/modules/markdown/`                                            |
| GFR-008        | pending       | `chat-ui.tsx` skills report                                                                                       | `src/modules/skills-report/`                                       |
| GFR-009        | pending       | `chat-ui.tsx` tool projection                                                                                     | `src/modules/tool-projection/`                                     |
| GFR-010        | pending       | `chat-ui.tsx` + `ProjectRail.tsx`                                                                                 | `src/modules/project-rail/`                                        |
| GFR-011        | pending       | `chat-ui.tsx` composer                                                                                            | `src/modules/chat/composer/`                                       |
| GFR-012        | pending       | `chat-ui.tsx` transcript                                                                                          | `src/modules/chat/transcript/`                                     |
| GFR-013        | pending       | `chat-ui.tsx` shell shrink                                                                                        | `src/modules/chat/shell/`                                          |
| GFR-014        | pending       | `App.tsx` boot shell                                                                                              | `src/modules/boot-shell/`                                          |
| GFR-015        | pending       | `App.tsx` shell-router + window-bridge                                                                            | `src/modules/{shell-router,window-bridge}/`                        |
| GFR-016        | pending       | `tauri.ts` types                                                                                                  | `src/transport/contracts.ts`                                       |
| GFR-017        | pending       | `tauri.ts` feature wrappers                                                                                       | `src/transport/{browser,session,...}.ts`                           |
| GFR-018        | pending       | `tauri.ts` listeners                                                                                              | `src/runtime-projection/translator/`                               |
| GFR-T1-A       | **cancelled** | source 是 orphan 死代码（plugins/lib+hooks + commands/lib，共 6057 LOC）；2026-04-21 整体 `chore(dead-code)` 删除 | —                                                                  |
| GFR-T1-C-1     | **done**      | `runtime/mcp_stdio.rs` 目录化 + tests (916 LOC) + JSON-RPC framing                                                | `runtime/mcp_stdio/{mod,tests,rpc}.rs`                             |
| GFR-T1-C-2     | **done**      | `runtime/mcp_stdio/mod.rs` 16 MCP DTOs (~150 LOC)                                                                 | `runtime/mcp_stdio/types.rs`                                       |
| GFR-T1-C-3     | **done**      | `runtime/mcp_stdio/mod.rs` McpServerManager cluster (~350 LOC); T1-C 收尾                                         | `runtime/mcp_stdio/manager.rs`                                     |
| **GFR-T1-C**   | **done**      | runtime/mcp_stdio 总收尾：mod.rs 1725 → 270 LOC (-84%) 跨 3 sub-packs                                             | `runtime/mcp_stdio/{mod,manager,types,rpc,tests}.rs`               |
| GFR-T1-D-1     | **done**      | `memory/providers/sqlite_provider.rs` 单刀切 (tests + scope + provider_impl)                                      | `sqlite_provider/{mod,scope,provider_impl,tests}.rs`               |
| **GFR-T1-D**   | **done**      | memory/sqlite_provider 总收尾：mod.rs 1579 → 268 LOC (-83%) 单刀完成                                              | `sqlite_provider/{mod,scope,provider_impl,tests}.rs`               |
| GFR-T1-B       | sliced        | sliced into B-1 (done) + B-2..5 (pending — 人写)                                                                  | `runtime/config/{json_helpers,schema,permission,sandbox,merge}.rs` |
| GFR-T1-B-1     | **done**      | `runtime/config.rs` JSON parse helpers (12 fn + 2 utility, ~230 LOC)                                              | `runtime/config/json_helpers.rs` (+ git mv to config/mod.rs)       |
| GFR-T1-B-3     | **done**      | `runtime/config/mod.rs` MCP cluster (9 types + 3 impls + 4 parsers, ~250 LOC)                                     | `runtime/config/mcp.rs`                                            |
| GFR-T1-B-4     | **done**      | `runtime/config/mod.rs` memory cluster (~440 LOC)                                                                 | `runtime/config/memory.rs`                                         |
| GFR-T1-B-tests | **done**      | `runtime/config/mod.rs` test block (757 LOC) extracted to sibling tests.rs                                        | `runtime/config/tests.rs`                                          |
| GFR-T1-B-2     | **done**      | `runtime/config/mod.rs` schema simple types (4 types + 5 parsers, ~190 LOC)                                       | `runtime/config/schema.rs`                                         |
| GFR-T1-B-5     | **done**      | `runtime/config/mod.rs` 7 parsers + IO helper (~280 LOC); T1-B 收尾                                               | `runtime/config/parsers.rs`                                        |
| **GFR-T1-B**   | **done**      | runtime/config 总收尾：mod.rs 2640 → 711 LOC (-73%) 跨 6 sub-packs                                                | `runtime/config/{json_helpers,mcp,memory,schema,parsers,tests}.rs` |
| GFR-T1-C~I     | pending       | T1 后端 god-files                                                                                                 | 各自 module 子目录                                                 |
| GFR-T2-A       | pending       | `ProviderSetupStep.tsx`                                                                                           | `onboarding/steps/provider/`                                       |
| GFR-T2-B       | pending       | `SkillsSettingsPage.tsx`                                                                                          | `settings/pages/skills/`                                           |

---

## Prompt Planner Alignment Pipeline (MIG-005 ~ MIG-008)

基于 [prompt-planner-gap-analysis.md](../staff-remediation/prompt-planner-gap-analysis.md) 的 UClaw 对齐路线图。

| Pack    | 状态    | Phase | Goal                                                                                  | Depends On |
| ------- | ------- | ----- | ------------------------------------------------------------------------------------- | ---------- |
| MIG-005 | done    | P0    | PromptBlock 数据结构对齐（source + priority + is_sensitive + validation_issues）      | MIG-004    |
| MIG-006 | done    | P1    | PromptContribution 机制 + PromptBuildMode + strict validation                         | MIG-005    |
| MIG-007 | done    | P2    | 模块化重构（拆分为 block.rs / diagnostics.rs / build_request.rs / planner.rs）       | MIG-006    |
| MIG-008 | done    | P2    | Coding mode 专用增强（continuation block + compaction policy + workspace augment）    | MIG-007    |

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
| `GAP-`  | 架构 Gap / 二次真相清理 Pack                              | CHARTER §6.1 |
| `TEAM-` | Agents Teams 能力 Pack                                    | CHARTER §6.1 |

### Active

| Pack   | Type | Goal | Owner |
| ------ | ---- | ---- | ----- |
| _(无)_ | —    | —    | —     |

### Migration-Core Pipeline

| 顺序 | 优先级 | Pack                                                                                           | 状态     | Goal                                                  |
| ---- | ------ | ---------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------- |
| 1    | P0     | [MIG-001](./feature/migration-core/MIG-001-canonical-chat-execution-spine.md)                  | **done** | 把 chat turn 主链收口成 canonical orchestrator        |
| 2    | P0     | [MIG-010](./feature/migration-core/MIG-010-local-gateway-bootstrap.md)                         | **done** | 建立 local gateway bootstrap seam                     |
| 3    | P0     | [MIG-012](./feature/migration-core/MIG-012-frontend-api-facade-and-transport-cutover.md)       | **done** | 建立前端 `src/api/*` facade，收缩 `tauri.ts` 业务职责 |
| 4    | P0     | [MIG-013](./feature/migration-core/MIG-013-app-shell-router-and-bootstrap-store.md)            | **done** | 建立 `AppShell + ContentRouter + bootstrap store`     |
| 5    | P0     | [MIG-014](./feature/migration-core/MIG-014-session-and-chat-store-foundation.md)               | **done** | 建立 session/chat store 基础层                        |
| 6    | P0     | [MIG-015](./feature/migration-core/MIG-015-gateway-conversations-and-streaming-surface.md)     | **done** | 让主聊天链走 gateway conversations/streaming surface  |
| 7    | P1     | [MIG-002](./feature/migration-core/MIG-002-execution-mode-routing-and-policy-enforcement.md)   | **done** | 让 execution mode / preflight 成为真实 product gate   |
| 8    | P1     | [MIG-004](./feature/migration-core/MIG-004-prompt-planning-traceability.md)                    | **done** | 把 prompt planner 升级成 traceable contract           |
| 9    | P1     | [MIG-005](./feature/migration-core/MIG-005-real-memory-lifecycle.md)                           | **done** | 打通真实 memory lifecycle 闭环                        |
| 10   | P1     | [MIG-011](./feature/migration-core/MIG-011-desktop-host-thin-shell.md)                         | **done** | 把 Tauri host 收口成 desktop host thin shell          |
| 11   | P2     | [MIG-003](./feature/migration-core/MIG-003-runtime-event-projection-truth.md)                  | **done** | 把前端切到 canonical runtime projection 单一真相路径  |
| 12   | P2     | [MIG-006](./feature/migration-core/MIG-006-frontend-shell-truth.md)                            | partial  | 建立统一 shell truth                                  |
| 13   | P2     | [MIG-007](./feature/migration-core/MIG-007-worker-tool-execution-contract.md)                  | **done** | 建立统一 worker/tool execution contract               |
| 14   | P3     | [MIG-008](./feature/migration-core/MIG-008-harness-replay-and-eval-on-canonical-run-report.md) | partial  | 让 harness replay/eval 建在 canonical run report 上   |
| 15   | P3     | [MIG-009](./feature/migration-core/MIG-009-activation-license-lifecycle.md)                    | partial  | 把 activation/license 升级成真实生命周期系统          |
| 16   | P0     | [MIG-016](./feature/migration-core/MIG-016-canonical-run-event-log-foundation.md)              | **done** | 建立 canonical run event log append-only 事实源       |
| 17   | P0     | [MIG-017](./feature/migration-core/MIG-017-runtime-projection-chat-truth-cutover.md)           | **done** | 让聊天主 UI 真正切到 projection 单一真相              |
| 18   | P1     | [MIG-018](./feature/migration-core/MIG-018-session-history-replay-and-paging.md)               | **done** | 建立 session history replay + paging                  |
| 19   | P1     | [MIG-019](./feature/migration-core/MIG-019-pending-permission-recovery.md)                     | **done** | 让 pending permission 可恢复 / 可重连                 |
| 20   | P1     | [MIG-020](./feature/migration-core/MIG-020-session-supervisor-foundation.md)                   | **done** | 建立 session supervisor 生命周期真相                  |
| 21   | P2     | [MIG-021](./feature/migration-core/MIG-021-resume-contract-and-run-recovery.md)                | partial  | 建立 typed resume / run recovery contract             |
| 22   | P2     | [MIG-022](./feature/migration-core/MIG-022-tool-attempt-ledger-and-timeline-contract.md)       | partial  | 建立 tool attempt ledger + timeline contract          |
| 23   | P3     | [MIG-023](./feature/migration-core/MIG-023-canonical-run-report-from-event-log.md)             | partial  | 让 harness/run report 改读 event log                  |

> Migration-Core canonical blueprint:
> [Current Architecture](../../ARCHITECTURE.md)
>
> [If2Ai vNext Session Runtime Blueprint](../design-docs/if2ai-vnext-session-runtime-blueprint.md)
>
> `MIG-003`、`MIG-007`、`MIG-016` ~ `MIG-023` 必须完整参照该蓝图进行更新与任务实施。

### Migration-Core Priority Notes

- `P0`
  - 先让主聊天链、gateway seam、frontend facade、AppShell/store 基础层成立。
  - 目标是把“可运行的主产品路径”从 command-heavy + god-file 形态，迁到稳定边界上。
- `P1`
  - 在主路径稳定后，把 route gate、prompt trace、memory lifecycle、desktop host 分层补齐。
  - 目标是让系统从“能跑”升级到“语义一致、可诊断、可持续扩展”。
- `P2`
  - 当 gateway conversations 和 store 基础层已经成立后，再做 projection/shell/tool contract 真正 cutover。
  - 目标是消灭前端双真相与工具执行语义漂移。
- `P3`
  - 最后再做 harness canonicalization 与 activation commercial lifecycle。
  - 这两项都很重要，但不应排在主产品链路稳定之前。

### Session Continuity Extension

- `P0`
  - 先建立 `run event log + projection truth`，把 session 从“当前对象状态”升级成“可重放事实 + 前端投影”。
  - 目标是给 paging、resume、report 提供唯一事实源。
- `P1`
  - 在事实源稳定后，补齐 `history replay / pending permission / session supervisor`。
  - 目标是让断线、刷新、权限阻塞都成为可恢复的 session lifecycle。
- `P2`
  - 再把 `resume contract + tool attempt ledger` 升级成 typed runtime contract。
  - 目标是让“不断流”不只体现在 UI，而是体现在可审计的运行语义上。
- `P3`
  - 最后把 canonical run report 接入 harness / eval。
  - 目标是把 replay、grader、audit 统一收口到 event-log truth 上。

### Migration-Core Code Audit 2026-04-23

| Pack | Code-aligned status | Evidence | Remaining Gap |
| ---- | ------------------- | -------- | ------------- |
| MIG-011 | done | `desktop_host::{builder,setup}` exists; `tray_action_resolution_only_accepts_native_host_ids` passes | skip unless host boundary regresses |
| MIG-003 | done | translator supports correlation.runId precedence; bridge is sole ingestion entry; V2 Single Truth gate PASS | T-003 (Chat Cutover) needed to retire compatibility raw-stream listeners |
| MIG-006 | partial | `AppShell`, `ContentRouter`, `bootstrapStore`, `sessionStore` exist | no complete shell truth store; activation/session/run/composer still split |
| MIG-007 | done | `ToolExecutionBroker` + `prepare_step_execution` enforced from `ToolRegistryExecutor` | skip; future attempt work belongs to MIG-022 |
| MIG-008 | partial | `HarnessRunReport`, graders, compare, suite report exist | not yet derived from canonical event log / run report |
| MIG-009 | partial | activation contracts, `LicenseLifecycleService`, `activation_get_status` projection seam exist | request/redeem/refresh/revoke/deactivate IPC and remote lifecycle still skeleton/placeholder |
| MIG-016 | done | `runtime/event_log.rs`, run_id wiring, seq tests, terminal event tests pass | skip; hardening belongs to GAP-002 / GAP-008 |
| MIG-017 | done (T-003) | chat UI reads projection-only; raw listener is transport-only; conversation-slice holds only user messages; `projectConversationMessagesFromRuns` contract documented | T-005 covers remaining raw listener retirement for permission/approvals |
| MIG-018 | done | `runtime/history.rs`, `get_session_history_page`, history replay TS tests pass | session.json fallback now traced via `fallback_reason` field (GAP-001 done) |
| MIG-019 | done | `pending_permission.rs`, `get_pending_permission`, recovery projection tests pass | skip; multi-viewer/team policy belongs to TEAM/GAP work |
| MIG-020 | done (T-006) | supervisor.rs with SupervisorSnapshot + 5 lifecycle hooks + state machine + persistence + get_supervisor_snapshot API + 5 tests | T-007 covers projector-side supervisor UI |
| MIG-021 | partial | resume cursor and recoverable UI fields exist | missing typed `resume_reason` / `safe_to_retry_mutations` contract |
| MIG-022 | partial | `tool_call_id` and `attempt_id` fields exist in run log | no stable attempt_id/attempt_no generation or ledger state machine |
| MIG-023 | partial | harness reports exist | reports still aggregate harness event bus/traces, not event-log-derived canonical report |

### Architecture Gap Pipeline

来自 [ARCHITECTURE.md](../../ARCHITECTURE.md) §7，用于清理 vNext 主线之外的二次真相、边界漂移与 god-file 风险。详见 [architecture-gaps/README.md](./feature/architecture-gaps/README.md)。

| 顺序 | 优先级 | Pack | 状态 | Goal |
| ---- | ------ | ---- | ---- | ---- |
| 1 | P0 | [GAP-001](./feature/architecture-gaps/GAP-001-session-json-fact-split.md) | **done** | 拆分 `session.json` 混合事实源 |
| 2 | P0 | [GAP-002](./feature/architecture-gaps/GAP-002-runtime-contract-unification.md) | **done** | 统一 runtime envelope / stream payload / run log contract |
| 3 | P0 | [GAP-003](./feature/architecture-gaps/GAP-003-frontend-projection-single-truth.md) | **done** | 前端 runtime UI 收敛到 projection-first |
| 4 | P1 | [GAP-004](./feature/architecture-gaps/GAP-004-command-boundary-thinning.md) | **done** | 瘦身 command boundary 与 AppState 聚合 |
| 5 | P1 | [GAP-005](./feature/architecture-gaps/GAP-005-stream-task-decomposition.md) | **done** | 拆分 `stream_task.rs` god-file 职责 |
| 6 | P1 | [GAP-006](./feature/architecture-gaps/GAP-006-memory-ui-read-model-unification.md) | **done** | 统一 memory UI 读模型 |
| 7 | P2 | [GAP-007](./feature/architecture-gaps/GAP-007-harness-event-log-truth-cutover.md) | draft | harness/report 改读 canonical event log |
| 8 | P2 | [GAP-008](./feature/architecture-gaps/GAP-008-contract-drift-guardrails.md) | draft | 建立前后端 contract drift guardrails |

### Agents Teams Pipeline

Agents Teams 必须建立在 vNext session runtime 之上。执行前置条件：MIG-016、MIG-017、MIG-019、MIG-020、MIG-022、MIG-023 至少完成对应基础能力。详见 [agents-teams/README.md](./feature/agents-teams/README.md)。

| 顺序 | 优先级 | Pack | 状态 | Goal |
| ---- | ------ | ---- | ---- | ---- |
| 1 | P0 | [TEAM-001](./feature/agents-teams/TEAM-001-team-domain-contracts.md) | draft | Team bounded context 与核心 contracts |
| 2 | P0 | [TEAM-002](./feature/agents-teams/TEAM-002-team-api-and-projection-skeleton.md) | draft | Team API facade 与 projection skeleton |
| 3 | P1 | [TEAM-003](./feature/agents-teams/TEAM-003-team-supervisor-mvp.md) | draft | TeamSupervisor planner/executor/reviewer MVP |
| 4 | P1 | [TEAM-004](./feature/agents-teams/TEAM-004-team-aware-runtime-correlation.md) | draft | team-aware runtime correlation 与 replay |
| 5 | P1 | [TEAM-005](./feature/agents-teams/TEAM-005-team-workspace-ui.md) | draft | TeamWorkspace 一级 UI |
| 6 | P2 | [TEAM-006](./feature/agents-teams/TEAM-006-team-memory-and-permission-policy.md) | draft | team memory scope 与 permission policy |
| 7 | P2 | [TEAM-007](./feature/agents-teams/TEAM-007-team-tool-ledger-and-review-gates.md) | draft | team tool ledger 与 review gates |
| 8 | P3 | [TEAM-008](./feature/agents-teams/TEAM-008-team-run-report-and-harness.md) | draft | team run report 与 harness |

### Bug / Perf / Dep

| Pack   | 状态 | Goal |
| ------ | ---- | ---- |
| _(无)_ | —    | —    |

### Prompt Control Plane Pipeline

| Pack | 状态 | Goal |
| ---- | ---- | ---- |
| [FEAT-PCP-001](./feature/prompt-control-plane/FEAT-PCP-001-prompt-coordinator-foundation.md) | **done** | Prompt coordinator + assembly decision foundation |
| [FEAT-PCP-002](./feature/prompt-control-plane/FEAT-PCP-002-scenario-and-task-focus-catalog.md) | **done** | Scenario / task-focus prompt catalog |
| [FEAT-PCP-003](./feature/prompt-control-plane/FEAT-PCP-003-tool-prompt-catalog-and-injection.md) | **done** | Tool prompt catalog + conditional injection |
| [FEAT-PCP-004](./feature/prompt-control-plane/FEAT-PCP-004-utility-and-coordinator-prompt-lanes.md) | **done** | Utility / coordinator prompt lanes |
| [FEAT-PCP-005](./feature/prompt-control-plane/FEAT-PCP-005-prompt-control-panel-and-diagnostics.md) | **done** | Prompt control panel + diagnostics projection |

> 新 Pack 用 `./scripts/pack init <PACK-ID> --type feature --slug <slug> --files <a> [<b>...]` 生成。
> `migration-core` 是当前优先级最高的 feature pipeline；老 `CPD-001` 保留作历史 pack，不再作为默认 active 入口。

---

## Tier 3 — Watchlist (800–1500 LOC)

> 已知的"边缘 god-file"。由 `./scripts/pack scan` 维护 LOC；超过 1500 LOC 自动晋升为 Tier 1/2。
> `lint-architecture` 把这些文件视为 baseline 已知技术债，不报 error；新增超过 800 LOC 的文件会立即报 error。

| 文件                                                            |  LOC |
| --------------------------------------------------------------- | ---: |
| `src-tauri/src/modules/application/turn_service/stream_task.rs` | 1249 |
| `src-tauri/src/modules/browser/session.rs`                      | 1408 |
| `src-tauri/src/modules/memory/audit.rs`                         | 1094 |
| `src-tauri/src/modules/api/providers/claw_provider.rs`          | 1223 |
| `src-tauri/src/modules/api/providers/openai_compat.rs`          | 1071 |
| `src-tauri/src/modules/memory/pinned/store.rs`                  | 1009 |
| `src-tauri/src/modules/runtime/compact.rs`                      |  962 |
| `src-tauri/src/modules/learning/strategy_rollout.rs`            |  967 |
| `src-tauri/src/modules/learning/candidate_evaluator.rs`         |  966 |
| `src-tauri/src/modules/learning/strategy_registry_service.rs`   |  951 |
| `src-tauri/src/modules/learning/strategy_registry.rs`           |  845 |
| `src-tauri/src/modules/learning/promotion_gate.rs`              |  822 |
| `src-tauri/src/modules/harness/gate.rs`                         |  951 |
| `src-tauri/src/modules/harness/trace_aggregator.rs`             |  803 |
| `src-tauri/src/modules/session/manager.rs`                      |  966 |
| `src-tauri/src/modules/tools/builtin/skill.rs`                  |  985 |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs`           |  952 |
| `src-tauri/src/modules/tools/registry.rs`                       |  850 |
| `src-tauri/src/modules/skills/guard/threat_patterns.rs`         |  982 |
| `src-tauri/src/modules/skills/guard/mod.rs`                     |  808 |
| `src-tauri/src/commands/slash.rs`                               |  980 |
| `src-tauri/src/commands/memory.rs`                              |  975 |
| `src-tauri/src/commands/skills_hub.rs`                          |  850 |
| `src/modules/settings/pages/TtsTestPage.tsx`                    |  921 |

---

## 状态字典

- `pending` — stub，禁止执行
- `active`  — 可被 agent 执行
- `done`    — verify + review 通过 + 已 commit
- `blocked` — 阻塞原因写在 pack 内
- `merged`  — 在另一个 pack 内合并完成
