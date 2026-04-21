# GFR-005f: agent.rs 目录化 + tests 抽出

## Status
- State: `done`

## Goal

GFR-005 收尾刀（preamble 之后）。`commands/agent.rs` (2718 LOC) 完成目录化 + tests block (~145 LOC) 抽到 sibling tests.rs。

**注意**：4 个 `#[tauri::command]` IPC fn (run_agent_turn / start_agent_stream / stop_agent_stream / respond_permission) 必须按 CHARTER 留 commands/，无法搬出。run_agent_turn (~571 LOC) + start_agent_stream (~1714 LOC) 的"函数体内重构"违反 CHARTER I6 (refactor 禁止改函数体)，需走 **CPD-001 FEAT pack** (turn-spine) 把 turn body 抽到 `application/turn_service`。

## Source / Destination
- `git mv commands/agent.rs → commands/agent/mod.rs`
- mod.rs lines 2453-2599 (`#[cfg(test)] mod tests { ... }`) → `commands/agent/tests.rs`

## Files (scope)
- src-tauri/src/commands/agent.rs (deleted via mv)
- src-tauri/src/commands/agent/mod.rs
- src-tauri/src/commands/agent/tests.rs

## Contract
- I1 / I3 / I6: 全部 byte-identical
- I2: agent_loop_executes_skill_tool_end_to_end test 跟随移动

## Verify
- cargo build PASS clean
- cargo fmt clean
- cargo clippy --workspace --all-targets -D warnings GREEN
- cargo test --no-run PASS

## Done
- mod.rs 2718 → 2574 LOC (-5%)
- tests.rs 144 LOC
- 005f 完成；agent.rs 真正瘦身留 CPD-001
