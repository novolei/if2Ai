# GAP-007: Harness Event Log Truth Cutover

## Status
- State: draft

## Goal
把 harness report、suite report、replay/eval 逐步切到 canonical run report/event log，避免 harness 自建并行 run truth。

## Spec
- harness report 优先从 event log 派生 → `harness_report_prefers_canonical_run_report`
- 旧 trace/report store 仅作为 fallback 或 migration input → `legacy_harness_store_is_fallback_only`
- report 包含 tool/permission/memory/recoverability summary → `canonical_report_contains_runtime_sections`

## Files
- `src-tauri/src/modules/harness/**`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §6-8, §11
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`
- `docs/packs/feature/migration-core/MIG-023-canonical-run-report-from-event-log.md`
- `docs/packs/feature/migration-core/MIG-008-harness-replay-and-eval-on-canonical-run-report.md`

## Contract
- 不删除旧 harness suite 命令。
- 不让 harness 直接解析 frontend projection state。
- report schema 必须可由 event log deterministic 重建。

## Verify
- `./scripts/pack run GAP-007`
- `cargo test --manifest-path src-tauri/Cargo.toml harness`
- `cargo test --manifest-path src-tauri/Cargo.toml run_report`

