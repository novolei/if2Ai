# GFR Registry

> If2Ai god-file 重构路线图（28 个 GFR pack）+ Tier 3 watchlist。
>
> 由 `./scripts/rfp scan` 维护 LOC；人工只需更新 `Status` 列。
>
> 最后更新: 2026-04-21
> Charter: [CHARTER.md](./CHARTER.md)

---

## Roadmap（28 个 GFR pack，必须按编号串行执行）

### A. `commands/agent.rs`（4058 LOC → 目标 ≤ 800 LOC shim）

| GFR         | Source 段                                                                                                                               | Destination                                                                          | 状态       |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ---------- |
| **GFR-001** | `RealApiClient`（≈329–481）                                                                                                             | `application/real_api_client.rs`                                                     | **active** |
| GFR-002     | `prompt` 装配 / sanitize / governor / preflight（≈3258–3905）                                                                           | `application/prompt_planner.rs` 已有，搬入子模块                                     | pending    |
| GFR-003     | `TauriPermissionPrompter` + `respond_permission` + permission senders（≈3933–4060）                                                     | `application/permission_service.rs`（新建）                                          | pending    |
| GFR-004     | stream chunk → emit 的散点（散落 in `start_agent_stream`）                                                                              | `application/stream_emitter_service.rs` + 复用 `runtime/stream_emitter.rs`           | pending    |
| GFR-005     | `run_agent_turn` + `start_agent_stream` 主体（≈944–3093）                                                                               | `application/turn_service.rs` 已有，吸收主编排                                       | pending    |
| GFR-006     | `record_trajectory_if_possible`（≈904–943）+ `ToolRegistryExecutor` + `ControlPlaneRuntimeSwitches`（≈293–706）+ tool 启发式（707–805） | `application/trajectory_service.rs`（新建） + `application/tool_executor.rs`（新建） | pending    |

### B. `components/ui/chat-ui.tsx`（4736 LOC → 目标 ≤ 600 LOC layout shell）

| GFR     | Source 段                                                                                                                                   | Destination                                                             | 状态    |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------- |
| GFR-007 | 零-hook helpers：`MarkdownContent` + `CodeBlock` + ASCII/narrative/keyword 规范化 + `MessageCopyButton` + 文本工具（≈2910–3720, 3724–3954） | `src/modules/markdown/`                                                 | pending |
| GFR-008 | `SkillsSlashReport` + `parseSkillsSlashReport` + 所有 normalize / glyph helpers（≈3245–3464）                                               | `src/modules/skills-report/`                                            | pending |
| GFR-009 | tool 摘要 / glyph / 状态 / `summarizeToolResult` 系列（≈3955–4286）                                                                         | `src/modules/tool-projection/`                                          | pending |
| GFR-010 | `ProjectFilesRail` + 子组件（≈1301–1788） + 合并 `src/components/ProjectRail.tsx`（1001 LOC）                                               | `src/modules/project-rail/`                                             | pending |
| GFR-011 | `ComposerDock` + `SlashCommandSuggestions` + `AtFileSuggestions`（≈1915–2432, 2667–2909）                                                   | `src/modules/chat/composer/` + `src/modules/chat/composer/suggestions/` | pending |
| GFR-012 | `ChatTranscript` + `ChatMessage` + `MemoryStoreToolCard` + `ToolCallMessage`（≈1789–1914, 2435–2666, 2910–3244）                            | `src/modules/chat/transcript/`                                          | pending |
| GFR-013 | `ChatUI` 剩余主体 → 退化为 layout shell；`EmptyState` / `LoadingIndicator` / `RecoveryCard` / `ErrorCard` 移走（≈4287+）                    | `src/modules/chat/shell/` + `src/modules/system-feedback/`              | pending |

### C. `App.tsx`（2514 LOC → 目标 ≤ 100 LOC）

| GFR     | Source 段                                             | Destination                                                                | 状态    |
| ------- | ----------------------------------------------------- | -------------------------------------------------------------------------- | ------- |
| GFR-014 | splash + onboarding gate + activation overlay 切换    | `src/modules/boot-shell/`（已有部分，扩）                                  | pending |
| GFR-015 | top-level surface router + window-bridge + 跨窗口事件 | `src/modules/shell-router/`（新建） + `src/modules/window-bridge/`（新建） | pending |

### D. `lib/tauri.ts`（2386 LOC → 目标 ≤ 100 LOC deprecated shim）

| GFR     | Source 段                                                                                                                                 | Destination                                                                 | 状态    |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------- |
| GFR-016 | 所有 `interface` / `type` 声明（≈40+）                                                                                                    | `src/transport/contracts.ts` 已有，逐 type 迁入                             | pending |
| GFR-017 | feature thin invoke wrappers（按 feature：browser → session → project → skills → settings → harness → memory）                            | `src/transport/{browser,session,project,skills,settings,harness,memory}.ts` | pending |
| GFR-018 | listener helpers（`listenMemoryEvent` / `listenToStream` / `listenToPermissionRequests` / `listenBrowserStatus` / `listenToChatPrefill`） | `src/runtime-projection/translator/`（已有部分）                            | pending |

### E. Tier 1 后端模块内 god-file

| GFR      | Source                                                | Destination                                                                       | 状态    |
| -------- | ----------------------------------------------------- | --------------------------------------------------------------------------------- | ------- |
| GFR-T1-A | `modules/plugins/lib.rs`（2995）                      | `modules/plugins/{manifest,registry,settings,marketplace,hooks}/`                 | pending |
| GFR-T1-B | `modules/runtime/config.rs`（2640）                   | `modules/runtime/config/{schema,permission,sandbox,boundary,merge}/`              | pending |
| GFR-T1-C | `modules/runtime/mcp_stdio.rs`（1725）                | `modules/runtime/mcp/{rpc,transport,bootstrap}/`                                  | pending |
| GFR-T1-D | `modules/commands/lib.rs`（2667）                     | 拆出 `plugin_bridge.rs` + `runtime_bridge.rs`                                     | pending |
| GFR-T1-E | `modules/memory/providers/sqlite_provider.rs`（1579） | `modules/memory/providers/sqlite/{schema,index,scoring,migration}/`               | pending |
| GFR-T1-F | `modules/memory/ticker.rs`（1363）                    | `modules/memory/scheduler/{turn,session,daily,recovery}/`                         | pending |
| GFR-T1-G | `modules/runtime/prompt.rs`（1258）                   | 搬入已有 `application/prompt_planner.rs`（部分逻辑保留 runtime/）                 | pending |
| GFR-T1-H | `modules/runtime/conversation.rs`（1236）             | 与 `application/turn_service.rs` 合并；保留 `runtime/conversation/` 作 trait host | pending |
| GFR-T1-I | `commands/tts.rs`（1368）                             | `modules/tts/{health,warmup,synthesize,stream,demo,voices}/` + thin commands      | pending |

### F. Tier 2 前端 page/component god-file

| GFR      | Source                                                   | Destination                                                                                             | 状态                |
| -------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------- |
| GFR-T2-A | `modules/onboarding/steps/ProviderSetupStep.tsx`（1764） | 拆出 sub-step + `provider-cards/` + `model-selector/` + `connection-tester/`                            | pending             |
| GFR-T2-B | `modules/settings/pages/SkillsSettingsPage.tsx`（1477）  | master-detail：`skills/list/` + `skills/detail/` + `skills/install-dialog/` + `skills/marketplace-tab/` | pending             |
| GFR-T2-C | `components/ProjectRail.tsx`（1001）                     | 在 GFR-010 中合并到 `src/modules/project-rail/`（不单独 GFR）                                           | merged into GFR-010 |

---

## Tier 3 — Watchlist (800–1500 LOC)

> 增长曲线值得监控；触发上限自动晋升 Tier 1/2。`./scripts/rfp scan` 周期刷新。

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

- `pending` — stub 已建，未激活；agent 禁止执行
- `active` — Execute Plan + Verify Whitelist 已补，agent 可执行
- `done` — verify 通过 + commit 落地
- `blocked` — 拆分被阻塞，原因写在对应 GFR pack
- `merged` — 在另一个 GFR 里合并完成（如 GFR-T2-C → GFR-010）
