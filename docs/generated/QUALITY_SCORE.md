# If2Ai 质量评分卡

> **自动生成文件** — 由 executor 每个 Phase 完成后更新。禁止手动修改。

**更新时间**: 2026-04-15
**当前 Phase**: Phase 6B 完成（Memory Control Plane）
**整体状态**: ✅ Phase 6B 全部完成（9/9 slices）

---

## Phase 完成状态

| Phase                       | 状态           | 完成时间   | 编译       | 测试        | Harness |
| --------------------------- | -------------- | ---------- | ---------- | ----------- | ------- |
| Phase 1 — 核心框架          | ✅ 完成 (12/12) | 2026-04-12 | ✅ 0 errors | ✅           | ⚠️       |
| Phase 2 — 高级特性          | ⏳ 等待 Phase 1 | —          | —          | —           | —       |
| Phase 3 — 扩展生态          | ⏳ 等待 Phase 2 | —          | —          | —           | —       |
| Phase 4 — 工具激活与边界    | ✅ 完成 (12/12) | 2026-04-12 | ✅ 0 errors | ✅ 188 tests | ⚠️       |
| Phase 5A — Agent 核心修复   | ✅ 完成 (4/4)   | 2026-04-13 | ✅ 0 errors | ✅ 188 tests | ✅       |
| Phase 5B — 流式工具循环重写 | ✅ 完成 (5/5)   | 2026-04-13 | ✅ 0 errors | ✅ 188 tests | ✅       |
| Phase 5C — 安全与系统连接   | ✅ 完成 (7/7)   | 2026-04-13 | ✅ 0 errors | ✅ 198 tests | ✅       |
| Phase 5D — UX 完善          | ✅ 完成 (8/8)   | 2026-04-13 | ✅ 0 errors | ✅ 202 tests | ✅       |
| Phase 5E — 长期增强         | ✅ 完成 (6/6)   | 2026-04-13 | ✅ 0 errors | ✅ 215 tests | ✅       |
| Phase 6A — 控制平面加固     | ✅ 完成 (8/8)  | 2026-04-13 | ✅ 0 errors | ✅ 222 tests | ✅       |
| Phase 6B — Memory Control Plane | ✅ 完成 (9/9)  | 2026-04-15 | ✅ 0 errors | ✅ 573 tests | ✅       |
| Phase 6F — Skill Control Plane v2 | ✅ 完成 (12/12) | 2026-04-14 | ✅ 0 errors | ✅ 348 tests | ✅       |

---

## Phase 1 — Slice 详情

| Slice | 标题                          | 状态   | 备注                                                   |
| ----- | ----------------------------- | ------ | ------------------------------------------------------ |
| 1.1   | 建立 Rust 模块骨架            | ✅ done |                                                        |
| 1.2   | 修复所有编译错误（52 个）     | ✅ done |                                                        |
| 1.3   | ConversationRuntime 核心实现  | ✅ done |                                                        |
| 1.4   | ProviderManager 实现          | ✅ done |                                                        |
| 1.5   | ToolRegistry + 3 个基础工具   | ✅ done | ToolRegistry + bash/file_read/json_parse               |
| 1.6   | SessionManager 实现           | ✅ done | JSON file persistence, create/restore/delete           |
| 1.7   | Tauri Commands 网关           | ✅ done | AppState + run_agent_turn/list_sessions/delete_session |
| 1.8   | 前端接线（React invoke）      | ✅ done | tauri.ts wrapper + App.tsx updated                     |
| 1.9   | ProjectManager Rust 后端      | ✅ done | Project CRUD + dual-path storage                       |
| 1.10  | React 项目导航 + 欢迎界面     | ✅ done | ProjectRail + WelcomeScreen                            |
| 1.11  | Session 状态监控 + 真实 Agent | ✅ done | SessionStatus + run_agent_turn persistence             |
| 1.12  | Phase 1 集成测试              | ✅ done | Integration tests (6 tests)                            |

---

## Phase 4 — Slice 详情

| Slice | 标题                            | 状态   | 备注                                            |
| ----- | ------------------------------- | ------ | ----------------------------------------------- |
| 4.1   | 注册内置工具 + LLM 接收工具定义 | ✅ done | register_builtin_tools + tools传递给LLM         |
| 4.2   | ToolContext 工作目录与权限模式  | ✅ done | SharedToolContext + workdir allowlist           |
| 4.3   | file_read 工具 workdir 限制     | ✅ done | 完善的文件读取 workdir 检查                     |
| 4.4   | bash 工具 workdir 限制          | ✅ done | cd $workdir && $command 包装                    |
| 4.5   | execute_tool Tauri 命令         | ✅ done | 前端工具调用命令                                |
| 4.6   | Per-Project PermissionMode      | ✅ done | 序列化/反序列化 + 默认 WorkspaceWrite           |
| 4.7   | ToolSet 分类系统                | ✅ done | TOOLSETS 常量 + ToolSetRegistry                 |
| 4.8   | 文件操作类工具                  | ✅ done | file_write/glob_search/content_search/file_edit |
| 4.9   | Web 类工具                      | ✅ done | web_fetch/web_search/http_request               |
| 4.10  | Memory 类工具                   | ✅ done | memory_store/recall/forget/purge/export         |
| 4.11  | Cron 类工具                     | ✅ done | cron_add/list/remove/run/runs                   |
| 4.12  | Phase 4 集成测试                | ✅ done | 工具注册 + 边界测试，188 tests                  |

---

## Phase 5A — Slice 详情

| Slice | 标题                         | 状态   | 备注                                      |
| ----- | ---------------------------- | ------ | ----------------------------------------- |
| 5a.1  | F1 — ToolExecutor trait 扩展 | ✅ done | get_definitions() + run_turn 传工具定义   |
| 5a.2  | N12 — call_api 一致性        | ✅ done | 优先使用 request.tools，fallback registry |
| 5a.3  | N5 — SystemPrompt for stream | ✅ done | SystemPromptBuilder 替代 system: None     |
| 5a.4  | 5A 集成测试                  | ✅ done | 两条路径工具定义和系统提示验证            |

---

## Phase 5B — Slice 详情

| Slice | 标题                         | 状态   | 备注                                                                     |
| ----- | ---------------------------- | ------ | ------------------------------------------------------------------------ |
| 5b.1  | F8 — StreamTokenPayload 扩展 | ✅ done | tool_call_update 事件 + 6 个 tool 字段                                   |
| 5b.2  | F2 — InputJsonDelta 累积     | ✅ done | HashMap 累积 + ToolUse 提取 + queued 事件                                |
| 5b.3  | F2 — 工具执行 + 多轮循环     | ✅ done | 完整工具循环: 权限检查 → 执行 → result 回传 → 继续 LLM + P1 并行工具修复 |
| 5b.4  | UI-5 — Message.role "tool"   | ✅ done | 前端工具消息渲染（ToolCallMessage 组件）                                 |
| 5b.5  | 5B 集成测试                  | ✅ done | session/types/permissions 测试通过                                       |

---

## Phase 6F — Slice 详情

| Slice | 标题                          | 状态   | 备注                                                                    |
| ----- | ----------------------------- | ------ | --------------------------------------------------------------------- |
| 6F.1  | SkillsGuard Threat Scanner    | ✅ done | 60+ patterns, 15 categories, invisible unicode, structural limits      |
| 6F.2  | SkillManager CRUD             | ✅ done | CRUD actions, validation, atomic writes, security scan integration     |
| 6F.3  | Hub Source Framework          | ✅ done | SkillSource trait, GitHubSource adapter, 4 auth methods               |
| 6F.4  | Hub Source Implementations     | ✅ done | 6 additional sources (skills.sh, ClawHub, Marketplace, etc.)       |
| 6F.5  | Hub State Management         | ✅ done | HubPaths/HubLock/HubState/AuditEvent, quarantine/lock/audit           |
| 6F.6  | Skill Sync Manifest         | ✅ done | manifest v1/v2, compute_dir_hash, sync with user mod detection           |
| 6F.7  | Skill Commands              | ✅ done | normalize_command_key, scan/resolve/invocation builder, platform compat  |

---

## Phase 6B — Slice 详情

| Slice | 标题                          | 状态   | 备注                                                                    |
| ----- | ----------------------------- | ------ | ----------------------------------------------------------------------- |
| 6b.1  | SQLite P0 + Security          | ✅ done | SqliteMemoryProvider, PathSanitizer, AtomicWrite, AccessControl         |
| 6b.2  | Token Budget + WorkingMemory  | ✅ done | ContextBudget 4000/10-20-30-40%, FrozenSnapshot, WeibullDecay           |
| 6b.3  | Active Retrieval + Intent     | ✅ done | QueryIntent 5 types, RRF Fusion (k=60), ActiveRetrievalManager          |
| 6b.4  | FastEmbed + LanceDB           | ✅ done | FastEmbedProvider 384d, LanceDBMemory IVF-PQ, VectorMemoryProvider      |
| 6b.5  | HRR Algebraic Reasoning       | ✅ done | HRR bind/unbind/bundle, HolographicStore, HybridMemoryProvider          |
| 6b.6  | Self-Learning Modules         | ✅ done | SelfModel, StandardReflectionEngine, TrustTracker (+0.05/-0.10)         |
| 6b.7  | Trajectory Learning           | ✅ done | ShareGPT JSONL export, TrajectoryManager, privacy controls              |
| 6b.8  | Upstream Compatibility        | ✅ done | claw-cli export (strips extended fields), provenance docs               |
| 6b.9  | Integration Test Suite        | ✅ done | harness/suites/phase6b_integration.yaml (21 test cases), 573 tests pass |

---

## 代码质量指标（当前）

- **编译错误**: 0（Phase 1 目标: 0）✅
- **编译警告**: 0（clippy -D warnings 通过）
- **测试覆盖率**: 215 tests pass
- **unwrap() 用量**: 少量（main.rs binary entry point + tools/lib.rs pre-existing）
- **Phase 1 进度**: 12/12 slices done (1.1-1.12 all complete)
- **测试结果**: 573 tests pass (all workspace)

---

## 变更记录

| 日期       | 事件                                                                                                                                                    |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-04-12 | Slice 1.11 重新实现：run_agent_turn 使用真实 ConversationRuntime + MockApiClient + ToolRegistryExecutor bridges                                         |
| 2026-04-11 | 项目初始化，Phase 1 开始                                                                                                                                |
| 2026-04-11 | Slice 1.1 完成：Rust 模块骨架建立                                                                                                                       |
| 2026-04-11 | 前端迁移完成：Svelte → React + shadcn/ui                                                                                                                |
| 2026-04-11 | Slice 1.2 完成：52 个编译错误全部修复，124 测试通过                                                                                                     |
| 2026-04-11 | Slice 1.3 完成：ConversationRuntime 核心实现，6 测试通过                                                                                                |
| 2026-04-11 | Slice 1.4 完成：ProviderManager 实现，28 测试通过                                                                                                       |
| 2026-04-12 | Slice 1.5 完成：ToolRegistry + bash/file_read/json_parse，148 tests pass                                                                                |
| 2026-04-12 | Slice 1.6 完成：SessionManager + JSON 持久化，create/restore/delete                                                                                     |
| 2026-04-12 | Slice 1.7 完成：Tauri Commands gateway，AppState + run_agent_turn/list_sessions/delete_session                                                          |
| 2026-04-12 | Slice 1.8 完成：前端 React 接线，tauri.ts invoke wrapper + App.tsx 更新                                                                                 |
| 2026-04-12 | 新增 Phase 1 slices 1.9-1.12：Project 系统 + Chat UI + 真实 Agent 集成                                                                                  |
| 2026-04-12 | **重置执行**：executor 重新从 1.9 开始，完整实现 Project + Chat 功能                                                                                    |
| 2026-04-12 | Slice 1.9 完成：ProjectManager + Session ↔ Project 双路径存储，833 行代码                                                                               |
| 2026-04-12 | Slice 1.10 完成：React ProjectRail + WelcomeScreen，739 行代码                                                                                          |
| 2026-04-12 | Slice 1.11 完成：SessionStatus 组件 + run_agent_turn session 持久化                                                                                     |
| 2026-04-12 | Slice 1.12 完成：Phase 1 集成测试，6 个测试函数，223 行代码                                                                                             |
| 2026-04-12 | **Phase 1 全部完成**：12/12 slices, 315 tests pass, 编译 0 errors                                                                                       |
| 2026-04-12 | Slice 4.8 完成：file_write/glob_search/content_search/file_edit 工具实现，170 tests pass                                                                |
| 2026-04-12 | **Phase 4 全部完成**：12/12 slices, 188 tests pass, 工具激活/workdir边界/PermissionMode/ToolSet/文件/Web/Memory/Cron工具全部实现                        |
| 2026-04-13 | **Phase 5A 全部完成**：4/4 slices, F1/N12/N5 修复，两条 Agent 路径工具定义和系统提示一致                                                                |
| 2026-04-13 | Slice 5b.3 完成：完整工具执行循环，max_iterations=10，权限检查+SSE事件+tool_result持久化，188 tests pass                                                |
| 2026-04-13 | **Phase 5B 全部完成**：5/5 slices, F2/F8/UI-5 流式工具循环, 188 tests pass                                                                              |
| 2026-04-13 | **Phase 5C 全部完成**：7/7 slices, N1/N3(阶段1+2)/F3-F6/F14/F17/N4/UI-1/Skill/SkillSearch, 198 tests pass                                               |
| 2026-04-13 | **Phase 5D 全部完成**：8/8 slices, 前端 UX 完善, 202 tests pass                                                                                         |
| 2026-04-13 | **Phase 5E 全部完成**：6/6 slices, 8 新工具 + 7 重叠工具替换 + lib.rs 删除(4493 行死代码), 215 tests pass                                               |
| 2026-04-13 | **Phase 5 系列全部完成** (5A-5E), 30 slices total, 215 tests pass, 0 clippy warnings                                                                    |
| 2026-04-13 | Phase 6A Slice 6a.1 完成：落地 SessionContextResolver + ToolExecutionBroker，agent/tools 命令层接入 broker                                              |
| 2026-04-13 | Phase 6A Slice 6a.2 完成：新增 BoundaryResolver 并统一 file/search 工具边界校验路径                                                                     |
| 2026-04-13 | Phase 6A Slice 6a.3 完成：高风险工具强制 dispatch_with_context + 会话强绑定 + context fingerprint 日志                                                  |
| 2026-04-13 | Phase 6A Slice 6a.4 完成：runtime 权限/沙箱语义对齐，bash 输出结构化 sandbox status，hooks 执行结果可追踪                                               |
| 2026-04-13 | Phase 6A Slice 6a.5 完成：AuditEmitter 审计事件模型落地，policy/start/finish/failed 统一 trace 链路与结构化失败诊断                                     |
| 2026-04-13 | Phase 6A Slice 6a.6 完成：前端工具卡片显示 policy/workdir/evidence，保留无证据写操作回复守卫并补充敏感信息脱敏                                          |
| 2026-04-13 | Phase 6A Slice 6a.7 完成：新增多会话并发隔离回归（file_write/grep_search/bash）与 phase6a-control-plane harness suite                                   |
| 2026-04-13 | Phase 6A Slice 6a.8 完成：新增控制平面灰度开关（controlPlaneV2Enabled/boundaryEnforceMode/sandboxStrictMode）与回滚矩阵                                 |
| 2026-04-13 | **Phase 6A 全部完成**：8/8 slices，控制平面加固闭环（隔离/边界/审计/灰度治理）                                                                          |
| 2026-04-13 | Phase 6C Slice 6c.1 完成：TaskOutcome 三真相聚合与终态透传（task_outcome/degraded_reason/resume_available）落地，范围审查通过                           |
| 2026-04-13 | Phase 6C Slice 6c.2 完成：request_id 贯穿 stream_diag_summary/stream_audit_link/audit 并规范化 diag_key，范围审查通过                                   |
| 2026-04-13 | Phase 6C Slice 6c.3 完成：前端接入 task_outcome 并区分 partial_success/failed，工具卡支持复制诊断串，范围审查通过                                       |
| 2026-04-13 | Phase 6C Slice 6c.4 完成：分层 timeout（connect/read/overall）与 retry/backoff 配置化接入，新增边界校验与溢出防护，范围审查通过                         |
| 2026-04-13 | Phase 6C Slice 6c.5 完成：ContextGovernor 三闸门统一 preflight（artifact/token/char）并复用 compact token 估算，范围审查通过                            |
| 2026-04-13 | Phase 6C Slice 6c.6 完成：degraded/resume_cursor 协议与一键继续入口落地，新增 is_resume_turn/inbound_resume_cursor 诊断字段                             |
| 2026-04-14 | Phase 6D Slice 6d.1 完成：Skills source precedence（workspace>user>builtin>remote-quarantine）、shadow 可解释决策与 quarantine 执行阻断通过 gate/review |
| 2026-04-14 | Phase 6D Slice 6d.2 完成：skill.json manifest 解析、review fail-closed 门禁与 skill_review/skill_install/skill_enable 审计事件落地并通过审查            |
| 2026-04-14 | Phase 6D Slice 6d.3 完成：bundled-skills 资源打包接入、builtin 默认只读与来源 metadata 记录通过 gate/review                                             |
| 2026-04-14 | Phase 6D Slice 6d.4 完成：前端技能治理面板支持 draft/quarantine/active/disabled 展示、enable/disable 操作与 shadow 说明，范围审查通过                   |
| 2026-04-14 | Phase 6D Slice 6d.5 完成：create/edit/review 前端流程与本地审查反馈落地，审查失败阻断启用并完成安全修复（路径 canonicalize）                            |
| 2026-04-14 | Phase 6D Slice 6d.6 完成：skills.sh 分发接入 stable/canary 通道，后端 artifact checksum+signature 校验 fail-closed，默认 quarantine                     |
| 2026-04-14 | Phase 6D Slice 6d.7 完成：agent skill_proposal->draft、approval/rollback 治理闭环与 adoption/rollback 事件路径落地通过审查                              |
| 2026-04-14 | Phase 6F Slice 6F.1 完成：SkillsGuard threat scanner (60+ patterns, 15 categories, invisible unicode, structural limits, Hermes trust policy)             |
| 2026-04-14 | Phase 6F Slice 6F.5 完成：Hub State Management (HubPaths/HubLock/HubState/AuditEvent, quarantine/lock/audit/unified_search)，26 hub tests pass      |
| 2026-04-14 | Phase 6F Slice 6F.6 完成：Skill Sync Manifest (manifest v1/v2, compute_dir_hash, sync with user mod detection)，10 sync tests pass      |
| 2026-04-14 | Phase 6F Slice 6F.7 完成：Skill Commands (normalize_command_key, scan/resolve/invocation builder, platform compat)，7 commands tests pass      |
| 2026-04-14 | Phase 6F Slice 6F.8 完成：Skill Config Variables (SkillConfigVar/Resolver, extract_config_vars, format_config_block)，8 tests pass      |
| 2026-04-14 | Phase 6F Slice 6F.9 完成：Frontend UI Extensions (SkillsHubView/SkillEditor/SkillSecurityReport)，1137 lines TSX      |
| 2026-04-14 | Phase 6F Slice 6F.10 完成：Integration Tests (122 skills tests pass: guard:38, manager:33, hub:26, sync:10, commands:7, config:8) |
| 2026-04-14 | Phase 6F Slice 6F.11 完成：External Skills Dirs & Remote Passthrough (ExternalSkillsDirs + RemotePassthroughManager, 16 tests pass) |
| 2026-04-14 | Phase 6F Slice 6F.12 完成：Snapshot Export/Import (SnapshotManager + SkillSnapshot types, 16 tests pass) |
| 2026-04-14 | **Phase 6F ALL COMPLETE** (12/12 slices, 154+ tests pass) |
| 2026-04-15 | Phase 6B Slice 6b.1 完成：SQLite P0 持久化 + 安全加固 (SqliteMemoryProvider, PathSanitizer, AtomicWrite, AccessControl) |
| 2026-04-15 | Phase 6B Slice 6b.2 完成：Token Budget + WorkingMemory (ContextBudget 4000/10-20-30-40%, FrozenSnapshot, WeibullDecay lambda=7d/k=1.2) |
| 2026-04-15 | Phase 6B Slice 6b.3 完成：Active Retrieval + Intent (QueryIntent 5 types, RRF Fusion k=60, ActiveRetrievalManager) |
| 2026-04-15 | Phase 6B Slice 6b.4 完成：FastEmbed + LanceDB (384d offline embeddings, IVF-PQ vector store, hybrid FTS5+Vector search) |
| 2026-04-15 | Phase 6B Slice 6b.5 完成：HRR Algebraic Reasoning (circular convolution bind/unbind, HolographicStore capacity O(√dim), HybridMemoryProvider) |
| 2026-04-15 | Phase 6B Slice 6b.6 完成：Self-Learning Modules (SelfModel capabilities/limitations, StandardReflectionEngine tool sequences, TrustTracker +0.05/-0.10) |
| 2026-04-15 | Phase 6B Slice 6b.7 完成：Trajectory Learning (ShareGPT JSONL export, TrajectoryManager with date rotation, privacy controls) |
| 2026-04-15 | Phase 6B Slice 6b.8 完成：Upstream Compatibility (claw-cli export strips extended fields, UPSTREAM-RELATIONSHIP.md, compact.rs provenance) |
| 2026-04-15 | Phase 6B Slice 6b.9 完成：Integration Test Suite (phase6b_integration.yaml 21 test cases, 573 workspace tests pass) |
| 2026-04-15 | **Phase 6B ALL COMPLETE** (9/9 slices, 573 tests pass, 0 clippy warnings) |
| 2026-04-17 | Phase 7B Slice 7B.5 完成：Tauri Event 桥接 + browser IPC commands (BrowserStatusEvent, emit_browser_status, get_browser_sessions, close_browser_session, get_chrome_status; 706 tests pass, REVIEW_PASS 10/10) |
| 2026-04-17 | Phase 7B Slice 7B.6 完成：BrowserCard 前端浮动卡片 + Store (useSyncExternalStore singleton store, Tauri event listener with active-flag race fix, Globe badge in header, 706 tests pass, REVIEW_PASS) |
| 2026-04-17 | Phase 7B Slice 7B.7 完成：Session 隔离 + 冷保存恢复 (ColdState JSON persist, incognito context, restore_cold_state; 712 tests pass, REVIEW_PASS) |
| 2026-04-17 | Phase 7B Slice 7B.8 完成：BrowserViewer 独立窗口 (open_browser_viewer_window Tauri cmd, BrowserViewerPage iframe+thumbnail fallback, Expand button in BrowserCard; 712 tests pass, REVIEW_PASS) |
| 2026-04-17 | **Phase 7B ALL COMPLETE** (8/8 slices, 712 tests pass, 0 clippy warnings; browser capability gap vs openhanako closed) |
| 2026-04-18 | 8A.1 | done | T-A1 PII Scrub 接入写入路径（ThreatScanner.scan_and_redact + 3 类新 pattern + sqlite/vector providers with_scanner 深度防御 + memory_pii_redacted audit + tauri.ts 扩展）| gate ✅ review ✅ (slice review_checklist 6/6 满足；harness review fmt/clippy/test/no-unwrap/no-secrets 5/6 PASS，剩余 doc-comment 失败均为 5e6f828 之前提交的 70+ 个 pre-existing pub fns，超出 8A.1 范围) |
| 2026-04-18 | 8A.2 | done | T-A2 JobRunner（jobs.db + tokio Semaphore 限流 + skip-after-N 重试预算 + memory_job_failed/skipped audit 事件 + 6 个新单测）| gate ✅ review ✅ |
| 2026-04-18 | 8A.3 | done | T-A3 LogicalDay 4AM 切日 + tz + locale helpers（runtime/logical_day.rs + runtime/locale.rs + chrono-tz 0.10 + MemoryFeatureConfig.timezone/logical_day_cutoff_hour，DST gap/ambiguous safe，13 logical_day + 5 locale + 2 config 新单测）| gate ✅ review ✅ |
| 2026-04-18 | 8A.4 | done | T-A4 Master + per-session 双开关 + disabledSince（SessionMeta/Session 增 memory_enabled/disabled_since/reenabled_at 全 Option + #[serde(default)] 向后兼容、is_session_memory_on 三态纯函数、SessionManager::set_session_memory_enabled、新 Tauri 命令 memory_session_set_enabled 三处注册、7 个新 session 单测）+ Bonus C1+C2：runtime::config::{current,set_current} OnceLock 双槽 + RuntimeConfig::language() accessor，解 8A.3 TODO，让 logical_day::get_today() 与 locale::is_zh() 真正生效；main.rs 启动时 set_current(cfg) | gate ✅ review ✅ 6/6 → **Phase A (8A.1-8A.4) 完成，触发 human_checkpoint #1** |
| 2026-04-18 | 8A.5 | done | T-B1 UtilityLlm shim + SessionSummaryStore SQLite/JSON 双写 + Provider Send+Sync bound（MockProvider 改 AtomicUsize） | gate ✅ review ✅ |
| 2026-04-18 | 8A.6 | done | T-B2 RollingSummaryPrompt + budget 线性缩放 + build_conversation_text（memory/summary/prompt.rs 新建 ~430 行 + 17 单测；compute_budget turn*40 clamp[40,400]/facts 30%/max_tokens*1.5 clamp[150,750]；中英双 system 含 "## 重要事实/Key facts" + "## 事情经过/What happened" 两节固定标题；has_prev → user 三段式；build_conversation_text 跳过 System/Tool role 与 ToolUse/ToolResult blocks，char-based 截断 assistant >300 字）| gate ✅ review ✅ 6/6 |
| 2026-04-18 | 8A.7 | done | T-B3 RollingSummarizer 端到端 9 步流水线（read existing → 增量切片 → conv_text → user-turn count → prompt+budget → JobRunner.run("rolling_summary",...) 包 UtilityLlm.complete → PII scrub → UPSERT SessionSummaryRecord(source=Rolling) → emit memory_summary_rolled）+ TurnHook trait（sync fn，§0.5 Δ-8）+ ConversationRuntime.turn_hook 字段 + with_turn_hook builder + run_turn 末尾调用（暂用 scope::global+session_id="-"，TODO 8B 接真实 scope）+ memory/audit.rs::memory_summary_rolled 关联函数（§0.5 Δ-3+Δ-4，extra 携 turn_count/chars_before/chars_after/latency_ms）+ tauri.ts MemoryEventName 追加 memory_summary_rolled + job_runner.rs cfg(test) pub(crate) open_in_memory_for_tests 让 sibling 模块复用；10 个新单测全 PASS（rolling 9 + conversation 1）| gate ✅ review ✅ 6/6 |
