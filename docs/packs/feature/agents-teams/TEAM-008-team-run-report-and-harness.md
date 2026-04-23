# TEAM-008: Team Run Report And Harness

## Status
- State: draft

## Goal
基于 canonical event log 生成 team run report，并让 harness 能评估 delegation、review gate、tool retry 与 final artifacts。

## Spec
- event log 可生成 TeamRunReport → `team_run_report_from_event_log`
- report 包含成员贡献、delegation graph、tool attempts、review decisions、artifacts → `team_report_contains_collaboration_sections`
- harness 可读取 TeamRunReport 执行 smoke evaluation → `harness_reads_team_run_report`

## Files
- `src-tauri/src/modules/team/**`
- `src-tauri/src/modules/harness/**`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9.6, §10
- `docs/packs/feature/migration-core/MIG-023-canonical-run-report-from-event-log.md`
- `docs/packs/feature/agents-teams/TEAM-007-team-tool-ledger-and-review-gates.md`

## Contract
- Team report 必须由 event log deterministic 派生。
- harness 不读取 frontend team projection 作为事实源。
- 不删除 single-agent run report。

## Verify
- `./scripts/pack run TEAM-008`
- `cargo test --manifest-path src-tauri/Cargo.toml team_report`
- `cargo test --manifest-path src-tauri/Cargo.toml harness`

