# MIG-011 Desktop Host Thin Shell

## Status

- State: `draft`
- Owner: `@executor`
- Gap Module: [cc-haha benchmark](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- Last Updated: `2026-04-21`

---

## Goal

把 If2Ai 的 Tauri host 收口成真正的 desktop host thin shell：只负责窗口、tray、plugin、gateway lifecycle 与 native-only command，不再继续膨胀为业务主承载层。

## Depends On

- `MIG-010`

## Unlocks

- `MIG-015`

## Why Now

1. `main.rs` 现在同时是 native entrypoint 与业务 command registry，职责过宽。
2. 如果不先收壳层，后续引入 gateway 也会继续变成“多挂一个服务”而不是结构改造。
3. benchmark 的核心价值是把 Tauri 稳定地限制在 host 层。

## Allowed Files

- `src-tauri/src/main.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/commands/**`
- `src-tauri/src/modules/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `docs/exec-plans/**`

## Source Of Truth

- [cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告](../../staff-remediation/cc-haha-if2ai-full-architecture-benchmark-report.md)
- [Backend Application Control Plane Refactor Design](../../staff-remediation/backend-application-control-plane-refactor-design.md)

## UClaw / Benchmark References

- [desktop/src-tauri/src/lib.rs](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src-tauri/src/lib.rs>)

## Current Evidence

- [src-tauri/src/main.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/main.rs:9) 仍在直接汇总大批业务 command。
- [desktop/src-tauri/src/lib.rs](</Users/ryanliu/Documents/IfAI/cc-haha-main/desktop/src-tauri/src/lib.rs:126>) 的 host 主责任是启动和跟踪 sidecar/runtime。

## Required Changes

1. 明确 `main.rs` 的 host-only 职责边界。
2. 把 native-only command、window/tray/plugin wiring 与业务 command 分层。
3. 不再让 `main.rs` 继续成为增长中的业务命令清单。
4. 为后续 gateway / frontend facade 保留稳定 native host surface。

## Cutover

- Canonical host truth: native host 只拥有 host lifecycle 与 native-only capability。
- Legacy path to retire: `main.rs` 作为业务主 registry。
- Legacy path retired when新增业务能力默认不再直接扩展 `main.rs` command surface。

## Guardrails

- Do not merge if只是把命令清单搬到另一个 Rust god file。
- Do not merge if host 与业务分层只体现在注释里，不体现在入口组织上。
- Rollback plan: 保持当前入口，但撤回未完成的 host/gateway 分层，避免半重构状态。

## Workflow Truth Delta

- `app_boot: partial -> canonical_ready`

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条 host bootstrap / lifecycle 测试通过
- `main.rs` 的职责可清晰描述为 host composition root，而不是业务 command 总控

## Out Of Scope

- 不做前端 transport cutover
- 不做 streaming API 重构
- 不做 activation 商业逻辑

## Not This Pack

- 即使相关，也不在本 pack 内做 frontend AppShell 重构。
- 即使相关，也不在本 pack 内做 session/chat store 引入。

## Execution Notes

- 重点不是“减少几行代码”，而是让 host boundary 成立。
