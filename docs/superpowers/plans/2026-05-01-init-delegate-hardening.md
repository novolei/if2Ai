# Plan: Init / delegate hardening (scope items 2–4)

**Date**: 2026-05-01  
**Scope**: Follow-up to codebase review — **excludes** AGENTS.md「非目标」items 1 & 5 (workspace `rust/crates`, harness gate semantics).

## Goal

- **Item 2**: Remove process-aborting `panic!` / `.expect` on `reqwest::Client` build in Claw provider and shared provider HTTP client; degrade with logged fallback where a safe default exists.
- **Item 3**: Document `StreamDelegate` / `tool_executor_slot` invariants so future refactors do not violate the contract assumed by `.expect`.
- **Item 4**: Stop workspace-level `dead_code = "allow"`; use `warn` so unused items surface in normal `cargo check` output.

## Tasks

- [x] `claw_provider`: `build_http_client` — no `panic`; log + `reqwest::Client::new()` fallback on builder failure.
- [x] `provider/client.rs`: `PROVIDER_HTTP_CLIENT` — same fallback pattern + `tracing`.
- [x] `stream_delegate.rs`: top-of-file `//!` invariant section for executor slot lifecycle.
- [x] `src-tauri/Cargo.toml`: `[lints.rust] dead_code = "warn"`.
- [x] `CLAUDE.md`: one-line pointer to `AGENTS.md`「非目标」for agents that only read CLAUDE.

## Verification

- `cargo check -p if2ai-backend`
- `cargo test -p if2ai-backend --lib` (or subset if slow) — at minimum no compile regressions.

## N/A

- **§5 systematic-debugging**: N/A — Skill `systematic-debugging` (`SKILL.md` §「When to Use」) applies to bug/root-cause work; this pass is preventive hardening from prior review, not a defect chase.
- **§8 TDD**: N/A — No new behavior branches requiring red-green; fallback path is logging + `Client::new()`; verification is `cargo check` / existing tests.
